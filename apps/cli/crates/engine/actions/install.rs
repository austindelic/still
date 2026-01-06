use crate::registries::specs::tool::ToolSpec;
use crate::system::{MacOS, System};
use anyhow::Result;
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

pub async fn run(_system: System, _request: InstallRequest) -> Result<InstallResult> {
    // TODO: Implement install logic
    anyhow::bail!("Install not yet implemented")
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
