use sha2::{Sha256, Digest};
use blake3;
use std::fs::File;
use std::io;
use std::path::Path;

pub struct ChecksumVerifier;

#[allow(dead_code)]
impl ChecksumVerifier {
    /// Verify the checksum of a file. Supports BLAKE3 (default) and SHA256.
    #[allow(unused_imports)]
    pub fn verify_file(path: &Path, expected_hex: &str) -> Result<(), String> {
        // bring io::Read into scope just for this function if needed
        use std::io::Read;
        let mut file = File::open(path).map_err(|e| format!("Failed to open file for checksum: {}", e))?;
        
        let actual_hex = if expected_hex.starts_with("blake3:") {
            let mut hasher = blake3::Hasher::new();
            io::copy(&mut file, &mut hasher).map_err(|e| format!("Failed to compute BLAKE3 hash: {}", e))?;
            hasher.finalize().to_hex().to_string()
        } else {
            // Default to SHA256 or explicit sha256:
            let mut hasher = Sha256::new();
            io::copy(&mut file, &mut hasher).map_err(|e| format!("Failed to compute SHA256 hash: {}", e))?;
            hex::encode(hasher.finalize())
        };

        let target = if expected_hex.contains(':') {
            expected_hex.split(':').last().unwrap()
        } else {
            expected_hex
        };

        if actual_hex != target {
            return Err(format!("Checksum verification failed!\nExpected: {}\nActual:   {}", target, actual_hex));
        }

        Ok(())
    }

    /// Compute BLAKE3 checksum of a byte slice.
    pub fn compute_blake3(data: &[u8]) -> String {
        blake3::hash(data).to_hex().to_string()
    }

    /// Compute SHA256 checksum of a byte slice.
    pub fn compute_sha256(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_blake3() {
        let data = b"hello nyx";
        let expected = blake3::hash(data).to_hex().to_string();
        let actual = ChecksumVerifier::compute_blake3(data);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_compute_sha256() {
        let data = b"hello nyx";
        let mut hasher = Sha256::new();
        hasher.update(data);
        let expected = hex::encode(hasher.finalize());
        let actual = ChecksumVerifier::compute_sha256(data);
        assert_eq!(actual, expected);
    }
}
