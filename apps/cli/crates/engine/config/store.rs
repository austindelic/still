//! Config and lockfile storage helpers.

use std::path::{Path, PathBuf};

use crate::lockfile::lockfile_path;

/// Returns the lockfile path colocated with a selected config file.
pub fn colocated_lockfile_path(config_path: &Path) -> PathBuf {
    lockfile_path(config_path)
}
