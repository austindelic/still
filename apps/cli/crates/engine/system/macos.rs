use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::error::{EngineError, Result};
use dirs::home_dir;

use crate::actions::install::{
    AppInstallCommand, InstallOps, PackageInstallCommand, backend_versioned_name,
    reject_pinned_app_version, unsupported_app_backend, unsupported_package_backend,
};
use crate::specs::brew::{BottleFileSpec, BottleSpec};
use crate::utils::link::SymlinkOps;
use crate::utils::paths::PathOps;

/// macOS host marker used to select platform-specific engine implementations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacOS;

impl PathOps for MacOS {
    fn root_dir() -> PathBuf {
        PathBuf::from("/opt").join("still")
    }

    fn cache_dir() -> PathBuf {
        dirs::cache_dir().unwrap()
    }

    fn bin_dir() -> PathBuf {
        Self::root_dir().join("bin")
    }

    fn config_dir() -> PathBuf {
        home_dir().unwrap().join(".config").join("still")
    }

    fn config_file() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    fn apps_dir() -> PathBuf {
        Self::root_dir().join("apps")
    }

    fn home_dir() -> PathBuf {
        dirs::home_dir().expect("error fetching home_dir with dirs::home_dir on macos")
    }

    fn tool_dir() -> PathBuf {
        Self::root_dir().join("tools")
    }
}

impl SymlinkOps for MacOS {
    fn create_symlink(target_path: &Path, link_path: &Path) -> io::Result<()> {
        std::os::unix::fs::symlink(target_path, link_path)?;
        Ok(())
    }
}

impl InstallOps for MacOS {
    fn select_bottle_file(bottle: &BottleSpec) -> Result<BottleFileSpec> {
        {
            #[cfg(target_arch = "aarch64")]
            {
                if let Some(file) = bottle.stable.files.get("arm64_sequoia") {
                    return Ok(file.clone());
                }
                if let Some(file) = bottle.stable.files.get("arm64_sonoma") {
                    return Ok(file.clone());
                }
                if let Some(file) = bottle.stable.files.get("arm64_tahoe") {
                    return Ok(file.clone());
                }
                if let Some(file) = bottle.stable.files.get("arm64_ventura") {
                    return Ok(file.clone());
                }
            }
            #[cfg(target_arch = "x86_64")]
            {
                if let Some(file) = bottle.stable.files.get("sonoma") {
                    return Ok(file.clone());
                }
                if let Some(file) = bottle.stable.files.get("tahoe") {
                    return Ok(file.clone());
                }
                if let Some(file) = bottle.stable.files.get("sequoia") {
                    return Ok(file.clone());
                }
            }
            if let Some(file) = bottle.stable.files.get("all") {
                return Ok(file.clone());
            }
        }

        bottle
            .stable
            .files
            .values()
            .next()
            .cloned()
            .ok_or_else(|| EngineError::message("No bottle files available for this system"))
    }

    async fn find_binary_recursive(
        install_path: &Path,
        formula_name: &str,
    ) -> Result<Option<PathBuf>> {
        let bin_dir = install_path.join("bin");
        if bin_dir.exists() {
            let potential_binary = bin_dir.join(formula_name);
            if potential_binary.exists() {
                return Ok(Some(potential_binary));
            }
            let mut entries = tokio::fs::read_dir(&bin_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                if path.is_file() {
                    let metadata = tokio::fs::metadata(&path).await?;
                    if is_executable(&metadata) {
                        return Ok(Some(path));
                    }
                }
            }
        }

        let mut dirs_to_check = vec![install_path.to_path_buf()];
        while let Some(dir) = dirs_to_check.pop() {
            let mut entries = tokio::fs::read_dir(&dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                if path.is_dir() {
                    if path.file_name().and_then(|n| n.to_str()) == Some("bin") {
                        let mut bin_entries = tokio::fs::read_dir(&path).await?;
                        while let Some(bin_entry) = bin_entries.next_entry().await? {
                            let bin_path = bin_entry.path();
                            if bin_path.is_file() {
                                let metadata = tokio::fs::metadata(&bin_path).await?;
                                if is_executable(&metadata) {
                                    return Ok(Some(bin_path));
                                }
                            }
                        }
                    } else {
                        dirs_to_check.push(path);
                    }
                }
            }
        }

        Ok(None)
    }

    fn package_install_command(
        name: &str,
        version: &str,
        backend: &str,
    ) -> Result<PackageInstallCommand> {
        match backend {
            "homebrew" | "brew" => Ok(PackageInstallCommand {
                backend: "homebrew".to_string(),
                program: "brew".to_string(),
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
            "homebrew-cask" | "brew-cask" | "cask" => {
                reject_pinned_app_version("homebrew-cask", version)?;
                Ok(AppInstallCommand {
                    backend: "homebrew-cask".to_string(),
                    program: "brew".to_string(),
                    args: vec![
                        "install".to_string(),
                        "--cask".to_string(),
                        name.to_string(),
                    ],
                })
            }
            "mas" => {
                reject_pinned_app_version(backend, version)?;
                Ok(AppInstallCommand {
                    backend: "mas".to_string(),
                    program: "mas".to_string(),
                    args: vec!["install".to_string(), name.to_string()],
                })
            }
            _ => unsupported_app_backend(backend),
        }
    }
}

fn is_executable(metadata: &std::fs::Metadata) -> bool {
    metadata.permissions().mode() & 0o111 != 0
}
