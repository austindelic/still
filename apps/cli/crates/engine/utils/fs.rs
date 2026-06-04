//! Filesystem helper functions that are not tied to one platform implementation.

use std::path::PathBuf;

/// Generic filesystem utilities.
///
/// Prefer platform-specific traits for production engine paths. Use this type
/// for small cross-platform helpers that do not need platform dispatch.
pub struct FsUtils;

impl FsUtils {
    /// Returns the default Still cache directory using the current `HOME` environment variable.
    ///
    /// The function reads `HOME`, appends `.cache/still`, and returns an error
    /// when `HOME` is missing. For platform-specific paths, prefer
    /// `system::SystemOps::cache_dir()`.
    pub fn still_cache_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
        let home = std::env::var("HOME")
            .map(PathBuf::from)
            .map_err(|_| "HOME env var not set")?;

        Ok(home.join(".cache").join("still"))
    }
}
