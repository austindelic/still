use std::io;
use std::path::{Path, PathBuf};

use dirs::home_dir;

use crate::infra::link::SymlinkOps;
use crate::infra::paths::PathOps;

/// macOS host marker used to select platform-specific engine implementations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacOS;

impl PathOps for MacOS {
    fn root_dir() -> PathBuf {
        PathBuf::from("/opt").join("still")
    }

    fn cache_dir() -> PathBuf {
        dirs::cache_dir().unwrap()
    }

    fn bin_dir() -> PathBuf {
        Self::root_dir().join("bin")
    }

    fn config_dir() -> PathBuf {
        home_dir().unwrap().join(".config").join("still")
    }

    fn config_file() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    fn apps_dir() -> PathBuf {
        Self::root_dir().join("apps")
    }

    fn home_dir() -> PathBuf {
        dirs::home_dir().expect("error fetching home_dir with dirs::home_dir on macos")
    }

    fn tool_dir() -> PathBuf {
        Self::root_dir().join("tools")
    }
}

impl SymlinkOps for MacOS {
    fn create_symlink(target_path: &Path, link_path: &Path) -> io::Result<()> {
        std::os::unix::fs::symlink(target_path, link_path)?;
        Ok(())
    }
}
