use crate::registries::specs::tool::ToolSpec;
use crate::system::{Linux, MacOS, System, Windows};
use std::path::PathBuf;

pub trait InstallOps {
    fn install_root(&self) -> PathBuf;
    fn bin_dir(&self) -> PathBuf;
    fn needs_admin(&self) -> bool;
    fn exe_suffix(&self) -> &'static str;
}

pub struct InstallRequest {
    pub tool: ToolSpec,
}

pub struct InstallResult {
    pub tool_name: String,
    pub version: String,
    pub install_path: PathBuf,
    pub binary_path: Option<PathBuf>,
}

pub async fn run(
    system: System,
    request: InstallRequest,
) -> Result<InstallResult, Box<dyn std::error::Error>> {
    // TODO: Implement install logic
    Err("Install not yet implemented".into())
}

impl InstallOps for MacOS {
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

impl InstallOps for Linux {
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

impl InstallOps for Windows {
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
