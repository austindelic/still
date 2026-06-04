use std::io;
use std::path::{Path, PathBuf};

use dirs::home_dir;

use crate::infra::link::SymlinkOps;
use crate::infra::paths::PathOps;

/// Windows host marker used to select platform-specific engine implementations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Windows;

impl PathOps for Windows {
    fn root_dir() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(Self::home_dir)
            .join("still")
    }

    fn cache_dir() -> PathBuf {
        dirs::cache_dir().unwrap_or_else(|| Self::root_dir().join("cache"))
    }

    fn bin_dir() -> PathBuf {
        Self::root_dir().join("bin")
    }

    fn config_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(Self::home_dir)
            .join("still")
    }

    fn config_file() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    fn apps_dir() -> PathBuf {
        Self::root_dir().join("apps")
    }

    fn home_dir() -> PathBuf {
        home_dir().unwrap_or_else(|| PathBuf::from("."))
    }

    fn tool_dir() -> PathBuf {
        Self::root_dir().join("tools")
    }
}

impl SymlinkOps for Windows {
    fn create_symlink(target_path: &Path, link_path: &Path) -> io::Result<()> {
        std::os::windows::fs::symlink_file(target_path, link_path)?;
        Ok(())
    }
}
