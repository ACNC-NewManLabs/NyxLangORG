# Nyx Protocol Security Specifications (v1.0)

## Security Model
The Nyx Protocol DSL and cryptographic standard library achieve **IND-CCA2** security (Indistinguishability under Chosen Ciphertext Attack) through the following design principles:

### 1. Authenticated Encryption (AEAD)
All data at rest and in transit is protected using **ChaCha20-Poly1305**.
- **Integrity**: Poly1305 MAC ensures any modification to the ciphertext or nonce results in a decryption failure.
- **Confidentiality**: ChaCha20 stream cipher prevents unauthorized access to message content.
- **Hardening**: Ephemeral seals use a forward-secure 12-byte random nonce generated via `OsRng`.

### 2. Forward Secrecy
The `seal_ephemeral` and `open_ephemeral` primitives utilize **X25519 Diffie-Hellman** with transient keypairs.
- A new ephemeral keypair is generated for every `seal_ephemeral` call.
- The ephemeral public key is prepended to the ciphertext.
- Compromise of the long-term identity key does not compromise past ephemeral messages.

### 3. Side-Channel Resistance
Sensitive data is protected against memory-based side-channels:
- **Zeroization**: All session keys, shared secrets, and intermediate cryptographic buffers implement the `Zeroize` trait.
- **Explicit Clearing**: Ephemeral keys are explicitly zeroized immediately after use in `stdlib/src/crypto.rs`.
- **Constant-Time Operations**: Key comparisons use the `subtle` crate for constant-time equality checks.

### 4. Password Hardening (Argon2id)
Nyx uses **Argon2id** for password hashing to resist both GPU cracking and side-channel attacks.
- **m_cost**: 65536 (64MB)
- **t_cost**: 3 iterations
- **p_cost**: 4 parallel threads
- **Salt**: 128-bit cryptographically secure random salt via `OsRng`.

## Adversarial Verification
The robustness of this stack has been verified via `tests/security/malicious_cipher_test.nyx`, which performs:
1. **Integrity Check**: Verifies that flipping a single bit in the ciphertext causes immediate decryption failure.
2. **Truncation Recovery**: Ensures truncated payloads are rejected before decryption logic is exercised.
3. **Memory Safety**: Confirms that corrupted inputs do not lead to VM crashes or memory leaks.

## Compliance
This implementation adheres to the recommendations in **RFC 8439** (ChaCha20-Poly1305) and **RFC 7748** (X25519).
