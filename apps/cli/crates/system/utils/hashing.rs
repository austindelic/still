use sha2::{Digest, Sha256};

/// Hashing utilities
pub struct Hashing;

impl Hashing {
    /// Compute SHA-256 hash of data
    pub fn sha256(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }

    /// Verify SHA-256 hash
    pub fn verify_sha256(data: &[u8], expected_hash: &str) -> Result<(), Box<dyn std::error::Error>> {
        let computed_hash = Self::sha256(data);

        if computed_hash != expected_hash {
            return Err(format!(
                "SHA256 verification failed: expected {}, got {}",
                expected_hash, computed_hash
            ).into());
        }

        Ok(())
    }
}

