use std::io;
use std::path::{Path, PathBuf};

use crate::error::Result;
use dirs::home_dir;

use crate::actions::install::{
    AppInstallCommand, InstallOps, PackageInstallCommand, backend_versioned_name,
    reject_pinned_app_version, reject_pinned_package_version, unsupported_app_backend,
    unsupported_package_backend,
};
use crate::error::EngineError;
use crate::infra::link::SymlinkOps;
use crate::infra::paths::PathOps;
use crate::specs::brew::{BottleFileSpec, BottleSpec};

/// Linux host marker used to select platform-specific engine implementations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Linux;

impl PathOps for Linux {
    fn root_dir() -> PathBuf {
        home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".local")
            .join("share")
            .join("still")
    }

    fn cache_dir() -> PathBuf {
        dirs::cache_dir().unwrap_or_else(Self::root_dir)
    }

    fn bin_dir() -> PathBuf {
        home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".local")
            .join("bin")
    }

    fn config_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| Self::home_dir().join(".config"))
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

impl SymlinkOps for Linux {
    fn create_symlink(target_path: &Path, link_path: &Path) -> io::Result<()> {
        std::os::unix::fs::symlink(target_path, link_path)?;
        Ok(())
    }
}

impl InstallOps for Linux {
    fn select_bottle_file(_bottle: &BottleSpec) -> Result<BottleFileSpec> {
        Err(EngineError::UnsupportedPlatform {
            feature: "homebrew bottle installs".to_string(),
            platform: "linux".to_string(),
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
            "apt" | "apt-get" => Ok(PackageInstallCommand {
                backend: "apt".to_string(),
                program: "apt-get".to_string(),
                args: vec![
                    "install".to_string(),
                    "-y".to_string(),
                    backend_versioned_name(name, version, "="),
                ],
            }),
            "dnf" => {
                reject_pinned_package_version(backend, version)?;
                Ok(PackageInstallCommand {
                    backend: "dnf".to_string(),
                    program: "dnf".to_string(),
                    args: vec!["install".to_string(), "-y".to_string(), name.to_string()],
                })
            }
            "pacman" => {
                reject_pinned_package_version(backend, version)?;
                Ok(PackageInstallCommand {
                    backend: "pacman".to_string(),
                    program: "pacman".to_string(),
                    args: vec![
                        "-S".to_string(),
                        "--noconfirm".to_string(),
                        name.to_string(),
                    ],
                })
            }
            "nix" => {
                reject_pinned_package_version(backend, version)?;
                Ok(PackageInstallCommand {
                    backend: "nix".to_string(),
                    program: "nix".to_string(),
                    args: vec![
                        "profile".to_string(),
                        "install".to_string(),
                        format!("nixpkgs#{name}"),
                    ],
                })
            }
            _ => unsupported_package_backend(backend),
        }
    }

    fn app_install_command(name: &str, version: &str, backend: &str) -> Result<AppInstallCommand> {
        match backend {
            "flatpak" => {
                reject_pinned_app_version(backend, version)?;
                Ok(AppInstallCommand {
                    backend: "flatpak".to_string(),
                    program: "flatpak".to_string(),
                    args: vec![
                        "install".to_string(),
                        "-y".to_string(),
                        "flathub".to_string(),
                        name.to_string(),
                    ],
                })
            }
            "snap" => {
                reject_pinned_app_version(backend, version)?;
                Ok(AppInstallCommand {
                    backend: "snap".to_string(),
                    program: "snap".to_string(),
                    args: vec!["install".to_string(), name.to_string()],
                })
            }
            _ => unsupported_app_backend(backend),
        }
    }
}
