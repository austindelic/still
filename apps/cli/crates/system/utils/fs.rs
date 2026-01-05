use std::path::PathBuf;

/// Filesystem utilities
pub struct FsUtils;

impl FsUtils {
    /// Get the still cache directory
    /// Note: For platform-specific paths, use system::SystemUtil::cache_dir()
    pub fn still_cache_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
        let home = std::env::var("HOME")
            .map(PathBuf::from)
            .map_err(|_| "HOME env var not set")?;

        Ok(home.join(".cache").join("still"))
    }
}

