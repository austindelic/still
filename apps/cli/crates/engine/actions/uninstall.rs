use crate::system::MacOS;
use std::path::PathBuf;

pub trait UninstallOps {
    fn install_root(&self) -> PathBuf {
        PathBuf::from("/opt/still")
    }
    fn bin_dir(&self) -> PathBuf;
    fn needs_admin(&self) -> bool;
    fn exe_suffix(&self) -> &'static str;
}

// MacOS implementation
impl UninstallOps for MacOS {
    fn install_root(&self) -> PathBuf {
        dirs::home_dir().unwrap().join(".still")
    }

    fn bin_dir(&self) -> PathBuf {
        dirs::home_dir().unwrap().join(".local/bin")
    }

    fn needs_admin(&self) -> bool {
        false
    }

    fn exe_suffix(&self) -> &'static str {
        ""
    }
}
