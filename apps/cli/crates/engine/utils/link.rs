//! Symlink helpers for host platform implementations.

use crate::system::MacOS;
use std::io;
use std::path::Path;

/// Platform-specific symlink creation behavior.
///
/// Implementors adapt Still's link operation to the host filesystem APIs.
pub trait SymlinkOps {
    /// Creates a symlink from `link_path` to `target_path`.
    ///
    /// `target_path` is the existing binary or file to point at. `link_path` is
    /// the filesystem path to create. Implementations return the underlying I/O
    /// error when the target cannot be linked, the parent directory is missing,
    /// or the link path already exists.
    fn create_symlink(target_path: &Path, link_path: &Path) -> io::Result<()>;
}

impl SymlinkOps for MacOS {
    fn create_symlink(target_path: &Path, link_path: &Path) -> io::Result<()> {
        std::os::unix::fs::symlink(target_path, link_path)?;
        Ok(())
    }
}
