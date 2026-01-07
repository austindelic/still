use crate::system::MacOS;
use std::io;
use std::path::Path;
pub trait SymlinkOps {
    fn create_symlink(target_path: &Path, link_path: &Path) -> io::Result<()>;
}

impl SymlinkOps for MacOS {
    fn create_symlink(target_path: &Path, link_path: &Path) -> io::Result<()> {
        std::os::unix::fs::symlink(target_path, link_path)?;
        Ok(())
    }
}

