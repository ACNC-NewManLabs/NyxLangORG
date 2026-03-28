//! NYX Crypto Layer [Layer 13]
//! Industrial-grade hashing, encryption, and signatures.

pub mod crypto {
    use crate::error::{NyxError, ErrorCategory};
    use crate::collections::vec::Vec as NyxVec;
    use zeroize::{Zeroize, ZeroizeOnDrop};
    use std::vec::Vec as StdVec;

    #[derive(Zeroize, ZeroizeOnDrop)]
    pub struct SecretBytes(StdVec<u8>);

    impl SecretBytes {
        pub fn new(data: StdVec<u8>) -> Self {
            Self(data)
        }
        pub fn as_slice(&self) -> &[u8] {
            &self.0
        }
    }

    #[derive(Zeroize, ZeroizeOnDrop)]
    pub struct SecretKey(StdVec<u8>);

    impl SecretKey {
        pub fn new(data: StdVec<u8>) -> Self {
            Self(data)
        }
        pub fn as_slice(&self) -> &[u8] {
            &self.0
        }
    }

    pub mod hash {
        use super::*;
        use sha3::Sha3_256;
        use sha2::Sha256;
        use sha3::Digest as _;

        pub fn sha3(data: &[u8]) -> NyxVec<u8> {
            let mut hasher = Sha3_256::new();
            hasher.update(data);
            let res = hasher.finalize();
            let mut v = NyxVec::with_capacity(res.len());
            for b in res { v.push(b); }
            v
        }

        pub fn sha256(data: &[u8]) -> NyxVec<u8> {
            let mut hasher = Sha256::new();
            hasher.update(data);
            let res = hasher.finalize();
            let mut v = NyxVec::with_capacity(res.len());
            for b in res { v.push(b); }
            v
        }

        pub fn blake3(data: &[u8]) -> NyxVec<u8> {
            let res = blake3::hash(data);
            let mut v = NyxVec::with_capacity(32);
            for b in res.as_bytes() { v.push(*b); }
            v
        }
    }

    pub mod cipher {
        use super::*;
        use aes_gcm::aead::{Aead, KeyInit, OsRng};
        use aes_gcm::{Aes128Gcm, Aes256Gcm, Nonce};
        use chacha20poly1305::ChaCha20Poly1305;
        use rand::RngCore;

        const AES128_KEY_LEN: usize = 16;
        const AES256_KEY_LEN: usize = 32;
        const CHACHA_KEY_LEN: usize = 32;
        const AEAD_NONCE_LEN: usize = 12;

        fn build_nonce() -> [u8; AEAD_NONCE_LEN] {
            let mut nonce = [0u8; AEAD_NONCE_LEN];
            OsRng.fill_bytes(&mut nonce);
            nonce
        }

        pub fn aes_encrypt(data: &[u8], key: &[u8]) -> Result<NyxVec<u8>, NyxError> {
            let nonce = build_nonce();
            let ciphertext = match key.len() {
                AES128_KEY_LEN => {
                    let cipher = Aes128Gcm::new_from_slice(key).map_err(|_| NyxError::new("CRYP001", "Invalid AES-128 key", ErrorCategory::Security))?;
                    cipher.encrypt(Nonce::from_slice(&nonce), data).map_err(|_| NyxError::new("CRYP002", "AES-GCM encryption failure", ErrorCategory::Security))?
                }
                AES256_KEY_LEN => {
                    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| NyxError::new("CRYP003", "Invalid AES-256 key", ErrorCategory::Security))?;
                    cipher.encrypt(Nonce::from_slice(&nonce), data).map_err(|_| NyxError::new("CRYP004", "AES-GCM encryption failure", ErrorCategory::Security))?
                }
                _ => return Err(NyxError::new("CRYP005", format!("Invalid key length: {}", key.len()), ErrorCategory::Security)),
            };
            
            let mut out = NyxVec::with_capacity(AEAD_NONCE_LEN + ciphertext.len());
            for b in &nonce { out.push(*b); }
            for b in ciphertext { out.push(b); }
            Ok(out)
        }

        pub fn aes_decrypt(data: &[u8], key: &[u8]) -> Result<NyxVec<u8>, NyxError> {
            if data.len() < AEAD_NONCE_LEN {
                return Err(NyxError::new("CRYP014", "Ciphertext too short", ErrorCategory::Security));
            }
            let (nonce, ciphertext) = data.split_at(AEAD_NONCE_LEN);
            let plaintext = match key.len() {
                AES128_KEY_LEN => {
                    let cipher = Aes128Gcm::new_from_slice(key).map_err(|_| NyxError::new("CRYP001", "Invalid AES-128 key", ErrorCategory::Security))?;
                    cipher.decrypt(Nonce::from_slice(nonce), ciphertext).map_err(|_| NyxError::new("CRYP015", "AES-GCM decryption failure", ErrorCategory::Security))?
                }
                AES256_KEY_LEN => {
                    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| NyxError::new("CRYP003", "Invalid AES-256 key", ErrorCategory::Security))?;
                    cipher.decrypt(Nonce::from_slice(nonce), ciphertext).map_err(|_| NyxError::new("CRYP016", "AES-GCM decryption failure", ErrorCategory::Security))?
                }
                _ => return Err(NyxError::new("CRYP005", format!("Invalid key length: {}", key.len()), ErrorCategory::Security)),
            };
            let mut v = NyxVec::with_capacity(plaintext.len());
            for b in plaintext { v.push(b); }
            Ok(v)
        }

        pub fn chacha_encrypt(data: &[u8], key: &[u8]) -> Result<NyxVec<u8>, NyxError> {
            if key.len() != CHACHA_KEY_LEN {
                return Err(NyxError::new("CRYP017", "Invalid ChaCha20 key length", ErrorCategory::Security));
            }
            let nonce = build_nonce();
            let cipher = ChaCha20Poly1305::new_from_slice(key).map_err(|_| NyxError::new("CRYP018", "Invalid ChaCha20 key", ErrorCategory::Security))?;
            let ciphertext = cipher.encrypt(Nonce::from_slice(&nonce), data).map_err(|_| NyxError::new("CRYP019", "ChaCha20 encryption failure", ErrorCategory::Security))?;
            
            let mut out = NyxVec::with_capacity(AEAD_NONCE_LEN + ciphertext.len());
            for b in &nonce { out.push(*b); }
            for b in ciphertext { out.push(b); }
            Ok(out)
        }

        pub fn chacha_decrypt(data: &[u8], key: &[u8]) -> Result<NyxVec<u8>, NyxError> {
            use zeroize::Zeroize;
            if key.len() != CHACHA_KEY_LEN {
                return Err(NyxError::new("CRYP017", "Invalid ChaCha20 key length", ErrorCategory::Security));
            }
            if data.len() < AEAD_NONCE_LEN {
                return Err(NyxError::new("CRYP014", "Ciphertext too short", ErrorCategory::Security));
            }
            let (nonce, ciphertext) = data.split_at(AEAD_NONCE_LEN);
            let mut k = key.to_vec();
            let cipher = ChaCha20Poly1305::new_from_slice(&k).map_err(|_| NyxError::new("CRYP018", "Invalid ChaCha20 key", ErrorCategory::Security))?;
            let res = cipher.decrypt(Nonce::from_slice(nonce), ciphertext).map_err(|_| NyxError::new("CRYP020", "ChaCha20 decryption failure", ErrorCategory::Security));
            k.zeroize();
            
            match res {
                Ok(plaintext) => {
                    let mut v = NyxVec::with_capacity(plaintext.len());
                    for b in plaintext { v.push(b); }
                    Ok(v)
                }
                Err(e) => Err(e),
            }
        }

        pub fn chacha_encrypt_with_nonce(data: &[u8], key: &[u8], nonce: &[u8]) -> Result<NyxVec<u8>, NyxError> {
            if key.len() != CHACHA_KEY_LEN {
                return Err(NyxError::new("CRYP017", "Invalid ChaCha20 key length", ErrorCategory::Security));
            }
            if nonce.len() != AEAD_NONCE_LEN {
                return Err(NyxError::new("CRYP029", "Invalid ChaCha20 nonce length", ErrorCategory::Security));
            }
            let cipher = ChaCha20Poly1305::new_from_slice(key).map_err(|_| NyxError::new("CRYP018", "Invalid ChaCha20 key", ErrorCategory::Security))?;
            let ciphertext = cipher.encrypt(Nonce::from_slice(nonce), data).map_err(|_| NyxError::new("CRYP019", "ChaCha20 encryption failure", ErrorCategory::Security))?;
            
            let mut out = NyxVec::with_capacity(AEAD_NONCE_LEN + ciphertext.len());
            for b in nonce { out.push(*b); }
            for b in ciphertext { out.push(b); }
            Ok(out)
        }

        pub fn chacha_decrypt_with_nonce(data: &[u8], key: &[u8], nonce: &[u8]) -> Result<NyxVec<u8>, NyxError> {
            if key.len() != CHACHA_KEY_LEN {
                return Err(NyxError::new("CRYP017", "Invalid ChaCha20 key length", ErrorCategory::Security));
            }
            if nonce.len() != AEAD_NONCE_LEN {
                return Err(NyxError::new("CRYP029", "Invalid ChaCha20 nonce length", ErrorCategory::Security));
            }
            let cipher = ChaCha20Poly1305::new_from_slice(key).map_err(|_| NyxError::new("CRYP018", "Invalid ChaCha20 key", ErrorCategory::Security))?;
            let plaintext = cipher.decrypt(Nonce::from_slice(nonce), data).map_err(|_| NyxError::new("CRYP020", "ChaCha20 decryption failure", ErrorCategory::Security))?;
            
            let mut v = NyxVec::with_capacity(plaintext.len());
            for b in plaintext { v.push(b); }
            Ok(v)
        }
    }

    pub mod random {
        use super::*;
        use rand::rngs::OsRng;
        use rand::RngCore;
        
        pub fn random_bytes(len: usize) -> NyxVec<u8> {
            let mut data = vec![0u8; len];
            OsRng.fill_bytes(&mut data);
            let mut v = NyxVec::with_capacity(len);
            for b in data { v.push(b); }
            v
        }
    }

    pub mod signature {
        use super::*;
        use ed25519_dalek::{Signature, Signer, Verifier, SigningKey, VerifyingKey};

        pub fn sign(data: &[u8], key: &[u8]) -> Result<NyxVec<u8>, NyxError> {
            let signing_key = match key.len() {
                32 => SigningKey::from_bytes(key.try_into().map_err(|_| NyxError::new("CRYP006", "Invalid secret key length", ErrorCategory::Security))?),
                64 => SigningKey::from_keypair_bytes(key.try_into().map_err(|_| NyxError::new("CRYP007", "Invalid keypair length", ErrorCategory::Security))?)
                    .map_err(|_| NyxError::new("CRYP008", "Invalid keypair bytes", ErrorCategory::Security))?,
                _ => return Err(NyxError::new("CRYP009", "Signature key must be 32 or 64 bytes", ErrorCategory::Security)),
            };
            let sig = signing_key.sign(data).to_bytes();
            let mut v = NyxVec::with_capacity(sig.len());
            for b in sig { v.push(b); }
            Ok(v)
        }

        pub fn verify(data: &[u8], sig: &[u8], key: &[u8]) -> Result<bool, NyxError> {
            let verifying_key = match key.len() {
                32 => VerifyingKey::from_bytes(key.try_into().map_err(|_| NyxError::new("CRYP010", "Invalid public key length", ErrorCategory::Security))?)
                    .map_err(|_| NyxError::new("CRYP011", "Invalid public key bytes", ErrorCategory::Security))?,
                _ => return Err(NyxError::new("CRYP012", "Verifying key must be 32 bytes", ErrorCategory::Security)),
            };
            let signature = Signature::from_slice(sig).map_err(|_| NyxError::new("CRYP013", "Invalid signature format", ErrorCategory::Security))?;
            Ok(verifying_key.verify(data, &signature).is_ok())
        }
    }

    pub mod kdf {
        use super::*;
        use argon2::{
            password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
            Argon2,
        };
        use hkdf::Hkdf;
        use sha2::Sha256;

        pub fn argon2_hash(password: &[u8], salt: &[u8]) -> Result<String, NyxError> {
            let salt = SaltString::encode_b64(salt).map_err(|_| NyxError::new("CRYP021", "Invalid salt for Argon2", ErrorCategory::Security))?;
            let argon2 = Argon2::default();
            let password_hash = argon2.hash_password(password, &salt)
                .map_err(|_| NyxError::new("CRYP022", "Argon2 hashing failure", ErrorCategory::Security))?
                .to_string();
            Ok(password_hash)
        }

        pub fn argon2_verify(password: &[u8], hash: &str) -> bool {
            match PasswordHash::new(hash) {
                Ok(parsed) => Argon2::default().verify_password(password, &parsed).is_ok(),
                Err(_) => false,
            }
        }

        pub fn hkdf_expand(ikm: &[u8], salt: Option<&[u8]>, info: &[u8], len: usize) -> Result<StdVec<u8>, NyxError> {
            let hk = Hkdf::<Sha256>::new(salt, ikm);
            let mut okm = vec![0u8; len];
            hk.expand(info, &mut okm).map_err(|_| NyxError::new("CRYP023", "HKDF expansion failure", ErrorCategory::Security))?;
            Ok(okm)
        }

        pub fn pbkdf2_hmac(password: &[u8], salt: &[u8], rounds: u32, len: usize) -> NyxVec<u8> {
            let mut okm = vec![0u8; len];
            pbkdf2::pbkdf2_hmac::<Sha256>(password, salt, rounds, &mut okm);
            let mut v = NyxVec::with_capacity(len);
            for b in okm { v.push(b); }
            v
        }
    }

    pub mod key_exchange {
        use super::*;
        use x25519_dalek::{PublicKey, StaticSecret};
        use rand::RngCore;

        pub fn generate_x25519_keypair() -> (SecretKey, SecretBytes) {
            use rand::rngs::OsRng;
            let mut seed = [0u8; 32];
            OsRng.fill_bytes(&mut seed);
            let secret = StaticSecret::from(seed);
            let public = PublicKey::from(&secret);
            
            (SecretKey(secret.to_bytes().to_vec()), SecretBytes(public.as_bytes().to_vec()))
        }

        pub fn diffie_hellman(secret: &[u8], public: &[u8]) -> Result<SecretBytes, NyxError> {
            let s_bytes: [u8; 32] = secret.try_into().map_err(|_| NyxError::new("CRYP024", "Invalid X25519 secret key length", ErrorCategory::Security))?;
            let p_bytes: [u8; 32] = public.try_into().map_err(|_| NyxError::new("CRYP025", "Invalid X25519 public key length", ErrorCategory::Security))?;
            
            let secret = StaticSecret::from(s_bytes);
            let public = PublicKey::from(p_bytes);
            
            let shared = secret.diffie_hellman(&public);
            Ok(SecretBytes(shared.as_bytes().to_vec()))
        }
    }

    pub mod auth {
        use super::*;
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        type HmacSha256 = Hmac<Sha256>;

        pub fn hmac_sha256(key: &[u8], data: &[u8]) -> Result<NyxVec<u8>, NyxError> {
            let mut mac = HmacSha256::new_from_slice(key).map_err(|_| NyxError::new("CRYP026", "Invalid HMAC key length", ErrorCategory::Security))?;
            mac.update(data);
            let result = mac.finalize();
            let code = result.into_bytes();
            let mut v = NyxVec::with_capacity(code.len());
            for b in code { v.push(b); }
            Ok(v)
        }
    }

    pub mod encoding {
        use super::*;
        use base64::{engine::general_purpose, Engine as _};

        pub fn to_base64(data: &[u8]) -> String {
            general_purpose::STANDARD.encode(data)
        }

        pub fn from_base64(encoded: &str) -> Result<NyxVec<u8>, NyxError> {
            let data = general_purpose::STANDARD.decode(encoded).map_err(|_| NyxError::new("CRYP027", "Invalid Base64 string", ErrorCategory::Security))?;
            let mut v = NyxVec::with_capacity(data.len());
            for b in data { v.push(b); }
            Ok(v)
        }

        pub fn to_hex(data: &[u8]) -> String {
            hex::encode(data)
        }

        pub fn from_hex(encoded: &str) -> Result<NyxVec<u8>, NyxError> {
            let data = hex::decode(encoded).map_err(|_| NyxError::new("CRYP028", "Invalid Hex string", ErrorCategory::Security))?;
            let mut v = NyxVec::with_capacity(data.len());
            for b in data { v.push(b); }
            Ok(v)
        }
    }

    pub mod seal {
        use super::*;
        use super::cipher;
        use super::random;
        use argon2::{Argon2, password_hash::SaltString};
        use rand::rngs::OsRng;
        use rand::RngCore;

        const MAGIC: &[u8; 4] = b"NYX!";
        const VERSION: u8 = 0x01;
        const SALT_LEN: usize = 16;
        const NONCE_LEN: usize = 12;

        pub fn seal(message: &[u8], password: &[u8]) -> Result<NyxVec<u8>, NyxError> {
            let mut salt_bytes = [0u8; SALT_LEN];
            OsRng.fill_bytes(&mut salt_bytes);
            let salt_str = SaltString::encode_b64(&salt_bytes).map_err(|_| NyxError::new("SEAL001", "Salt encoding failed", ErrorCategory::Security))?;

            let argon2 = Argon2::new(
                argon2::Algorithm::Argon2id,
                argon2::Version::V0x13,
                argon2::Params::new(65536, 3, 4, None).unwrap(),
            );
            let mut key = [0u8; 32];
            argon2.hash_password_into(password, salt_str.as_salt().as_ref().as_bytes(), &mut key)
                .map_err(|_| NyxError::new("SEAL002", "Key derivation failed", ErrorCategory::Security))?;

            let nonce = random::random_bytes(NONCE_LEN);
            let encrypted = cipher::chacha_encrypt_with_nonce(message, &key, nonce.as_slice())?;
            let ciphertext = &encrypted.as_slice()[NONCE_LEN..];

            let mut out = NyxVec::with_capacity(MAGIC.len() + 1 + 1 + SALT_LEN + NONCE_LEN + ciphertext.len());
            for b in MAGIC { out.push(*b); }
            out.push(VERSION);
            out.push(0x00);
            for b in &salt_bytes { out.push(*b); }
            for b in nonce.as_slice() { out.push(*b); }
            for b in ciphertext { out.push(*b); }

            key.zeroize();
            Ok(out)
        }

        pub fn open(sealed: &[u8], password: &[u8]) -> Result<NyxVec<u8>, NyxError> {
            if sealed.len() < MAGIC.len() + 2 + SALT_LEN + NONCE_LEN {
                return Err(NyxError::new("SEAL003", "Sealed data too short", ErrorCategory::Security));
            }
            if &sealed[0..4] != MAGIC {
                return Err(NyxError::new("SEAL004", "Invalid magic bytes", ErrorCategory::Security));
            }
            if sealed[4] != VERSION {
                return Err(NyxError::new("SEAL005", "Unsupported seal version", ErrorCategory::Security));
            }

            let salt_bytes = &sealed[6..6+SALT_LEN];
            let nonce = &sealed[6+SALT_LEN..6+SALT_LEN+NONCE_LEN];
            let ciphertext = &sealed[6+SALT_LEN+NONCE_LEN..];

            let argon2 = Argon2::new(
                argon2::Algorithm::Argon2id,
                argon2::Version::V0x13,
                argon2::Params::new(65536, 3, 4, None).unwrap(),
            );
            let salt_str = SaltString::encode_b64(salt_bytes).map_err(|_| NyxError::new("SEAL001", "Salt encoding failed", ErrorCategory::Security))?;
            let mut key = [0u8; 32];
            argon2.hash_password_into(password, salt_str.as_salt().as_ref().as_bytes(), &mut key)
                .map_err(|_| NyxError::new("SEAL002", "Key derivation failed", ErrorCategory::Security))?;

            let res = cipher::chacha_decrypt_with_nonce(ciphertext, &key, nonce);
            key.zeroize();
            res
        }

        pub fn seal_ephemeral(message: &[u8], peer_public_key: &[u8]) -> Result<NyxVec<u8>, NyxError> {
            use super::key_exchange;
            use super::kdf;
            use super::random;
            use super::cipher;
            use zeroize::Zeroize;

            let (e_secret, e_public) = key_exchange::generate_x25519_keypair();
            let shared_secret = key_exchange::diffie_hellman(e_secret.as_slice(), peer_public_key)?;

            let mut session_key = kdf::hkdf_expand(shared_secret.as_slice(), None, b"nyx_ephemeral_session", 32)?;
            let nonce = random::random_bytes(12);
            let encrypted = cipher::chacha_encrypt_with_nonce(message, session_key.as_slice(), nonce.as_slice())?;
            let ciphertext = &encrypted.as_slice()[12..];

            // Explicit Zeroization
            session_key.zeroize();
            let mut ss = shared_secret;
            ss.zeroize();

            let mut out = NyxVec::with_capacity(MAGIC.len() + 1 + 32 + 12 + ciphertext.len());
            for b in MAGIC { out.push(*b); }
            out.push(0x02);
            for b in e_public.as_slice() { out.push(*b); }
            for b in nonce.as_slice() { out.push(*b); }
            for b in ciphertext { out.push(*b); }

            Ok(out)
        }

        pub fn open_ephemeral(sealed: &[u8], my_secret_key: &[u8]) -> Result<NyxVec<u8>, NyxError> {
            use super::key_exchange;
            use super::kdf;
            use super::cipher;
            use zeroize::Zeroize;

            if sealed.len() < MAGIC.len() + 1 + 32 + 12 {
                return Err(NyxError::new("SEAL003", "Sealed data too short", ErrorCategory::Security));
            }
            if &sealed[0..4] != MAGIC {
                return Err(NyxError::new("SEAL004", "Invalid magic bytes", ErrorCategory::Security));
            }
            if sealed[4] != 0x02 {
                return Err(NyxError::new("SEAL006", "Not an ephemeral seal", ErrorCategory::Security));
            }

            let e_public = &sealed[5..37];
            let nonce = &sealed[37..49];
            let ciphertext = &sealed[49..];

            let shared_secret = key_exchange::diffie_hellman(my_secret_key, e_public)?;
            let mut session_key = kdf::hkdf_expand(shared_secret.as_slice(), None, b"nyx_ephemeral_session", 32)?;
            let res = cipher::chacha_decrypt_with_nonce(ciphertext, session_key.as_slice(), nonce);
            
            // Explicit Zeroization
            session_key.zeroize();
            let mut ss = shared_secret;
            ss.zeroize();
            
            res
        }
    }
}

pub use crypto::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hashing() {
        let data = b"Nyx Industrial Crypto Test";
        let h1 = hash::sha3(data);
        let h2 = hash::blake3(data);
        assert_eq!(h1.len(), 32);
        assert_eq!(h2.len(), 32);
    }
}
