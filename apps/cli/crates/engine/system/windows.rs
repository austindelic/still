use std::io;
use std::path::{Path, PathBuf};

use crate::error::Result;
use dirs::home_dir;

use crate::actions::install::{
    AppInstallCommand, InstallOps, PackageInstallCommand, append_version_args,
    backend_versioned_name, unsupported_app_backend, unsupported_package_backend,
};
use crate::error::EngineError;
use crate::specs::brew::{BottleFileSpec, BottleSpec};
use crate::utils::link::SymlinkOps;
use crate::utils::paths::PathOps;

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

impl InstallOps for Windows {
    fn select_bottle_file(_bottle: &BottleSpec) -> Result<BottleFileSpec> {
        Err(EngineError::UnsupportedPlatform {
            feature: "homebrew bottle installs".to_string(),
            platform: "windows".to_string(),
        }
        .into())
    }

    async fn find_binary_recursive(
        _install_path: &Path,
        _formula_name: &str,
    ) -> Result<Option<PathBuf>> {
        Ok(None)
    }

    fn package_install_command(
        name: &str,
        version: &str,
        backend: &str,
    ) -> Result<PackageInstallCommand> {
        match backend {
            "winget" => Ok(PackageInstallCommand {
                backend: "winget".to_string(),
                program: "winget".to_string(),
                args: append_version_args(
                    vec!["install".to_string(), "--id".to_string(), name.to_string()],
                    version,
                    "--version",
                )
                .into_iter()
                .chain([
                    "--accept-package-agreements".to_string(),
                    "--accept-source-agreements".to_string(),
                ])
                .collect(),
            }),
            "chocolatey" | "choco" => Ok(PackageInstallCommand {
                backend: "chocolatey".to_string(),
                program: "choco".to_string(),
                args: append_version_args(
                    vec!["install".to_string(), "-y".to_string(), name.to_string()],
                    version,
                    "--version",
                ),
            }),
            "scoop" => Ok(PackageInstallCommand {
                backend: "scoop".to_string(),
                program: "scoop".to_string(),
                args: vec![
                    "install".to_string(),
                    backend_versioned_name(name, version, "@"),
                ],
            }),
            _ => unsupported_package_backend(backend),
        }
    }

    fn app_install_command(name: &str, version: &str, backend: &str) -> Result<AppInstallCommand> {
        match backend {
            "winget" => Ok(AppInstallCommand {
                backend: "winget".to_string(),
                program: "winget".to_string(),
                args: append_version_args(
                    vec!["install".to_string(), "--id".to_string(), name.to_string()],
                    version,
                    "--version",
                )
                .into_iter()
                .chain([
                    "--accept-package-agreements".to_string(),
                    "--accept-source-agreements".to_string(),
                ])
                .collect(),
            }),
            "chocolatey" | "choco" => Ok(AppInstallCommand {
                backend: "chocolatey".to_string(),
                program: "choco".to_string(),
                args: append_version_args(
                    vec!["install".to_string(), "-y".to_string(), name.to_string()],
                    version,
                    "--version",
                ),
            }),
            "scoop" => Ok(AppInstallCommand {
                backend: "scoop".to_string(),
                program: "scoop".to_string(),
                args: vec![
                    "install".to_string(),
                    backend_versioned_name(name, version, "@"),
                ],
            }),
            _ => unsupported_app_backend(backend),
        }
    }
}
