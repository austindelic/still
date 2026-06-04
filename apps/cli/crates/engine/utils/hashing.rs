//! Hashing and checksum verification helpers.

use sha2::{Digest, Sha256};

/// SHA-256 hashing utilities.
///
/// These helpers are deterministic and side-effect free; use them before trusting
/// downloaded archives or registry payloads.
pub struct Hashing;

impl Hashing {
    /// Computes the lowercase hex SHA-256 hash of `data`.
    ///
    /// `data` is hashed exactly as provided. The returned string is lowercase
    /// hexadecimal and suitable for comparison with Homebrew checksum metadata.
    pub fn sha256(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }

    /// Verifies that `data` matches an expected lowercase hex SHA-256 hash.
    ///
    /// `expected_hash` must be the canonical hex digest from the registry. The
    /// method returns `Ok(())` on a match and an error containing both expected
    /// and computed hashes on mismatch.
    pub fn verify_sha256(
        data: &[u8],
        expected_hash: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let computed_hash = Self::sha256(data);

        if computed_hash != expected_hash {
            return Err(format!(
                "SHA256 verification failed: expected {}, got {}",
                expected_hash, computed_hash
            )
            .into());
        }

        Ok(())
    }
}
