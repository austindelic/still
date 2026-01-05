use system::{Linux, MacOS, Windows};
use std::path::PathBuf;

pub trait InstallPolicy {
    fn install_root(&self) -> PathBuf;
    fn bin_dir(&self) -> PathBuf;
    fn needs_admin(&self) -> bool;
    fn exe_suffix(&self) -> &'static str;
}

// MacOS implementation
impl InstallPolicy for MacOS {
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

// Linux implementation
impl InstallPolicy for Linux {
    fn install_root(&self) -> PathBuf {
        dirs::home_dir().unwrap().join(".still")
    }

    fn bin_dir(&self) -> PathBuf {
        dirs::home_dir().unwrap().join(".local/bin")
    }

    fn needs_admin(&self) -> bool {
        true
    }

    fn exe_suffix(&self) -> &'static str {
        ""
    }
}

// Windows implementation
impl InstallPolicy for Windows {
    fn install_root(&self) -> PathBuf {
        PathBuf::from(r"C:\still")
    }

    fn bin_dir(&self) -> PathBuf {
        PathBuf::from(r"C:\still\bin")
    }

    fn needs_admin(&self) -> bool {
        false
    }

    fn exe_suffix(&self) -> &'static str {
        ".exe"
    }
}
