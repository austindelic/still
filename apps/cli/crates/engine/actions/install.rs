//! Engine install action for resolving, downloading, verifying, extracting, and linking packages.

use crate::error::EngineError;
use crate::platform::{PlatformId, current_platform};
use crate::registries::specs::tool::ToolSpec;
use crate::specs::backend::{
    default_backend, normalize_auto_backend as normalize_backend_selection,
};
use crate::specs::brew::{BottleFileSpec, BottleSpec};
use crate::specs::item::{ItemKind, ItemSpec};
use crate::system::{Linux, MacOS, System, Windows};
use crate::utils::archive::ArchiveExtractor;
use crate::utils::hashing::Hashing;
use crate::utils::link::SymlinkOps;
use crate::utils::net::NetUtils;
use crate::utils::paths::PathOps;
use anyhow::{Context, Result};
use serde::Serialize;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Platform-specific install operations needed by the generic install flow.
///
/// Implementors provide the host-specific pieces of an otherwise shared install:
/// choosing the right Homebrew bottle file and discovering the executable after
/// extraction. Keep network, archive, and CLI formatting code out of this trait.
pub trait InstallOps {
    /// Selects the best bottle file for the current platform.
    ///
    /// `bottle` is the parsed Homebrew bottle metadata for one formula version.
    /// Implementations return the file matching the current OS/architecture or an
    /// error explaining why no compatible bottle exists.
    fn select_bottle_file(bottle: &BottleSpec) -> Result<BottleFileSpec>;

    /// Locates an executable inside an extracted install directory.
    ///
    /// `install_path` is the directory that just received the extracted archive.
    /// `formula_name` is the expected executable name and should be preferred
    /// over unrelated files. Returns `Ok(None)` when extraction succeeded but no
    /// executable could be identified.
    async fn find_binary_recursive(
        install_path: &Path,
        formula_name: &str,
    ) -> Result<Option<PathBuf>>;
}

/// One item requested for install.
///
/// Build this after CLI/config parsing has classified the item as a tool,
/// package, or app. Resolution may still choose the final backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallItemRequest {
    pub kind: ItemKind,
    pub spec: ItemSpec,
}

/// Request to install parsed tool/package/app specs.
///
/// Build this at the boundary where user input or config has already been
/// validated. The install action owns resolution, download, verification,
/// extraction, linking, and later config writes from this point forward.
#[derive(Debug, Clone)]
pub struct InstallRequest {
    /// Parsed and classified items requested by the caller.
    pub items: Vec<InstallItemRequest>,
}

/// Result of a completed install operation.
///
/// The result is intentionally small and user-facing: it contains the canonical
/// installed identity, the final install directory, and the linked executable
/// when one was found.
#[derive(Debug)]
pub struct InstallResult {
    /// Canonical installed tool name.
    pub tool_name: String,
    /// Installed version.
    pub version: String,
    /// Directory where the item was installed.
    pub install_path: PathBuf,
    /// Linked executable discovered during install, if any.
    pub binary_path: Option<PathBuf>,
    /// Filesystem outputs created or tracked for this install.
    pub outputs: Vec<PathBuf>,
    /// Executable links created in Still's bin directory.
    pub linked_executables: Vec<PathBuf>,
    /// Every item installed during this request, in request order.
    pub installed: Vec<InstalledItemResult>,
}

/// One installed item result inside a possibly multi-item install request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledItemResult {
    pub kind: ItemKind,
    pub name: String,
    pub version: String,
    pub install_path: PathBuf,
    pub binary_path: Option<PathBuf>,
    pub outputs: Vec<PathBuf>,
    pub linked_executables: Vec<PathBuf>,
}

/// Installs individual items for the multi-item install coordinator.
pub trait ItemInstaller {
    /// Installs one item and returns its filesystem outputs.
    /// # Errors
    /// Fails when backend resolution or installation fails.
    async fn install(&mut self, item: &InstallItemRequest) -> Result<InstallResult>;
}

#[derive(Debug, Default)]
struct RealItemInstaller;

impl ItemInstaller for RealItemInstaller {
    async fn install(&mut self, item: &InstallItemRequest) -> Result<InstallResult> {
        install_one(item).await
    }
}

/// Installs one requested item into Still-managed storage and links its executable when found.
/// # Arguments
/// * `request` - Parsed package name and version request.
/// # Returns
/// The installed package identity, install path, and discovered executable path.
/// # Errors
/// Fails if registry lookup, bottle selection, download, checksum verification,
/// extraction, or linking fails.
/// # Side Effects
/// Downloads an archive, replaces the install directory, and may create a symlink.
pub async fn run(request: InstallRequest) -> Result<InstallResult> {
    let mut installer = RealItemInstaller;
    run_with_installer(request, &mut installer).await
}

/// Installs requested items with an injected installer.
/// # Errors
/// Fails if any item fails. Already-installed items from the same request are
/// rolled back before returning the install error.
pub async fn run_with_installer(
    request: InstallRequest,
    installer: &mut impl ItemInstaller,
) -> Result<InstallResult> {
    run_with_installer_and_rollback_roots(request, installer, &RollbackRoots::system()).await
}

/// Removes installed outputs returned by a completed install request.
/// # Errors
/// Fails when a Still-managed output or linked executable cannot be removed.
/// # Side Effects
/// Deletes files, directories, and links recorded in install results when they
/// are under Still-managed install roots.
pub async fn rollback_installed_items(installed: &[InstalledItemResult]) -> Result<()> {
    rollback_installed_items_at(installed, &RollbackRoots::system()).await
}

async fn run_with_installer_and_rollback_roots(
    request: InstallRequest,
    installer: &mut impl ItemInstaller,
    rollback_roots: &RollbackRoots,
) -> Result<InstallResult> {
    if request.items.is_empty() {
        return Err(EngineError::EmptyInstallRequest.into());
    }

    let mut last = None;
    let mut installed = Vec::new();
    for item in &request.items {
        let result = match installer.install(item).await.with_context(|| {
            format!(
                "failed to install {} {}@{}",
                item.kind, item.spec.name, item.spec.version
            )
        }) {
            Ok(result) => result,
            Err(err) => {
                if let Err(rollback_err) =
                    rollback_installed_items_at(&installed, rollback_roots).await
                {
                    return Err(err.context(format!(
                        "failed to roll back partial installs: {rollback_err}"
                    )));
                }
                return Err(err);
            }
        };
        installed.push(InstalledItemResult {
            kind: item.kind,
            name: result.tool_name.clone(),
            version: result.version.clone(),
            install_path: result.install_path.clone(),
            binary_path: result.binary_path.clone(),
            outputs: result.outputs.clone(),
            linked_executables: result.linked_executables.clone(),
        });
        last = Some(result);
    }

    let mut result: InstallResult = last.ok_or_else(|| EngineError::EmptyInstallRequest)?;
    result.installed = installed;
    Ok(result)
}

#[derive(Debug, Clone)]
struct RollbackRoots {
    tool_root: PathBuf,
    package_root: PathBuf,
    app_root: PathBuf,
    bin_dir: PathBuf,
}

impl RollbackRoots {
    fn system() -> Self {
        Self {
            tool_root: System::tool_dir(),
            package_root: System::root_dir().join("packages"),
            app_root: System::apps_dir(),
            bin_dir: System::bin_dir(),
        }
    }

    fn install_root(&self, kind: ItemKind) -> &Path {
        match kind {
            ItemKind::Tool => &self.tool_root,
            ItemKind::Package => &self.package_root,
            ItemKind::App => &self.app_root,
        }
    }
}

async fn rollback_installed_items_at(
    installed: &[InstalledItemResult],
    roots: &RollbackRoots,
) -> Result<()> {
    for item in installed.iter().rev() {
        for linked in item.linked_executables.iter().rev() {
            if linked.starts_with(&roots.bin_dir) {
                remove_path_if_exists(linked).await?;
            }
        }

        let install_root = roots.install_root(item.kind);
        let item_root = install_root.join(&item.name);
        let mut outputs = item.outputs.clone();
        outputs.push(item.install_path.clone());
        outputs.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
        outputs.dedup();
        for output in outputs {
            if output.starts_with(&item_root) {
                remove_path_if_exists(&output).await?;
            }
        }
    }
    Ok(())
}

async fn remove_path_if_exists(path: &Path) -> Result<()> {
    let metadata = match tokio::fs::symlink_metadata(path).await {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err.into()),
    };
    if metadata.is_dir() {
        tokio::fs::remove_dir_all(path).await?;
    } else {
        tokio::fs::remove_file(path).await?;
    }
    Ok(())
}

async fn install_one(item: &InstallItemRequest) -> Result<InstallResult> {
    if item.kind == ItemKind::App {
        return install_app(item).await;
    }
    if item.kind == ItemKind::Package {
        return install_package(item).await;
    }
    if should_use_tool_command_backend(item) {
        return install_tool_with_command(item).await;
    }

    let tool = ToolSpec {
        name: item.spec.name.clone(),
        version: item.spec.version.to_string(),
        backend: item.spec.backend.clone(),
    };

    let formula_path = formula_json_path();
    ensure_formula_json_exists(&formula_path)?;

    let formulas = load_formula_json_array(&formula_path).await?;
    let formula = find_matching_formula(&formulas, &tool.name)
        .with_context(|| format!("Formula '{}' not found in formula.json", tool.name))?;

    let bottle_info = build_bottle_info(&formula)?;
    let bottle_file = System::select_bottle_file(&bottle_info.bottle)?;
    let bottle_data = fetch_and_verify_bottle(&formula.name, &bottle_file).await?;

    let install_path = compute_install_path(&formula.name, &formula.versions.stable);
    reinstall_to_path(&bottle_data, &install_path).await?;

    let binary_path = System::find_binary_recursive(&install_path, &formula.name).await?;
    let linked_executables = link_binary(&binary_path)
        .await?
        .into_iter()
        .collect::<Vec<_>>();
    write_install_marker(
        &install_path,
        item,
        "homebrew",
        &[install_path.clone()],
        &linked_executables,
    )
    .await?;

    Ok(InstallResult {
        tool_name: formula.name.clone(),
        version: formula.versions.stable.clone(),
        install_path: install_path.clone(),
        binary_path,
        outputs: vec![install_path],
        linked_executables,
        installed: Vec::new(),
    })
}

fn should_use_tool_command_backend(item: &InstallItemRequest) -> bool {
    item.spec
        .backend
        .as_ref()
        .is_some_and(|backend| !matches!(backend.as_str(), "auto" | "homebrew" | "brew"))
}

async fn install_tool_with_command(item: &InstallItemRequest) -> Result<InstallResult> {
    let command = tool_install_command(item)?;
    let output = Command::new(&command.program)
        .args(&command.args)
        .output()
        .with_context(|| format!("failed to run tool backend {}", command.backend))?;
    if !output.status.success() {
        return Err(EngineError::Conflict {
            message: format!(
                "tool backend {} failed: {}",
                command.backend,
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        }
        .into());
    }

    let install_path = System::tool_dir()
        .join(&item.spec.name)
        .join(item.spec.version.as_str());
    write_install_marker(
        &install_path,
        item,
        &command.backend,
        &[install_path.clone()],
        &[],
    )
    .await?;

    Ok(InstallResult {
        tool_name: item.spec.name.clone(),
        version: item.spec.version.to_string(),
        install_path: install_path.clone(),
        binary_path: None,
        outputs: vec![install_path],
        linked_executables: Vec::new(),
        installed: Vec::new(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ToolInstallCommand {
    backend: String,
    program: String,
    args: Vec<String>,
}

fn tool_install_command(item: &InstallItemRequest) -> Result<ToolInstallCommand> {
    let backend = install_backend(
        ItemKind::Tool,
        item.spec.backend.as_ref().map(|backend| backend.as_str()),
        current_platform(),
    );
    tool_install_command_for_backend(&item.spec.name, item.spec.version.as_str(), &backend)
}

fn tool_install_command_for_backend(
    name: &str,
    version: &str,
    backend: &str,
) -> Result<ToolInstallCommand> {
    let normalized = install_backend(ItemKind::Tool, Some(backend), current_platform());
    match normalized.as_str() {
        "rustup" => Ok(ToolInstallCommand {
            backend: "rustup".to_string(),
            program: "rustup".to_string(),
            args: vec![
                "toolchain".to_string(),
                "install".to_string(),
                version.to_string(),
            ],
        }),
        "cargo" => {
            let mut args = vec!["install".to_string(), name.to_string()];
            if version != "latest" {
                args.extend(["--version".to_string(), version.to_string()]);
            }
            Ok(ToolInstallCommand {
                backend: "cargo".to_string(),
                program: "cargo".to_string(),
                args,
            })
        }
        "npm" => Ok(ToolInstallCommand {
            backend: "npm".to_string(),
            program: "npm".to_string(),
            args: vec![
                "install".to_string(),
                "--global".to_string(),
                package_with_version(name, version),
            ],
        }),
        "pnpm" => Ok(ToolInstallCommand {
            backend: "pnpm".to_string(),
            program: "pnpm".to_string(),
            args: vec![
                "add".to_string(),
                "--global".to_string(),
                package_with_version(name, version),
            ],
        }),
        "yarn" => Ok(ToolInstallCommand {
            backend: "yarn".to_string(),
            program: "yarn".to_string(),
            args: vec![
                "global".to_string(),
                "add".to_string(),
                package_with_version(name, version),
            ],
        }),
        "pipx" => Ok(ToolInstallCommand {
            backend: "pipx".to_string(),
            program: "pipx".to_string(),
            args: vec![
                "install".to_string(),
                python_package_with_version(name, version),
            ],
        }),
        "mise" => Ok(ToolInstallCommand {
            backend: "mise".to_string(),
            program: "mise".to_string(),
            args: vec!["install".to_string(), format!("{name}@{version}")],
        }),
        "asdf" => Ok(ToolInstallCommand {
            backend: "asdf".to_string(),
            program: "asdf".to_string(),
            args: vec!["install".to_string(), name.to_string(), version.to_string()],
        }),
        "aqua" => Ok(ToolInstallCommand {
            backend: "aqua".to_string(),
            program: "aqua".to_string(),
            args: vec!["install".to_string(), package_with_version(name, version)],
        }),
        "homebrew" | "brew" => Err(EngineError::Conflict {
            message: "homebrew tool installs use the Still-managed bottle installer".to_string(),
        }
        .into()),
        _ => unsupported_tool_backend(&normalized),
    }
}

fn package_with_version(name: &str, version: &str) -> String {
    if version == "latest" {
        name.to_string()
    } else {
        format!("{name}@{version}")
    }
}

fn python_package_with_version(name: &str, version: &str) -> String {
    if version == "latest" {
        name.to_string()
    } else {
        format!("{name}=={version}")
    }
}

fn unsupported_tool_backend(backend: &str) -> Result<ToolInstallCommand> {
    Err(EngineError::UnsupportedPlatform {
        feature: format!("tool backend {backend}"),
        platform: std::env::consts::OS.to_string(),
    }
    .into())
}

async fn install_package(item: &InstallItemRequest) -> Result<InstallResult> {
    let command = package_install_command(item)?;
    let output = Command::new(&command.program)
        .args(&command.args)
        .output()
        .with_context(|| format!("failed to run package backend {}", command.backend))?;
    if !output.status.success() {
        return Err(EngineError::Conflict {
            message: format!(
                "package backend {} failed: {}",
                command.backend,
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        }
        .into());
    }

    let install_path = System::root_dir()
        .join("packages")
        .join(&item.spec.name)
        .join(item.spec.version.as_str());
    write_install_marker(
        &install_path,
        item,
        &command.backend,
        &[install_path.clone()],
        &[],
    )
    .await?;

    Ok(InstallResult {
        tool_name: item.spec.name.clone(),
        version: item.spec.version.to_string(),
        install_path: install_path.clone(),
        binary_path: None,
        outputs: vec![install_path],
        linked_executables: Vec::new(),
        installed: Vec::new(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PackageInstallCommand {
    backend: String,
    program: String,
    args: Vec<String>,
}

fn package_install_command(item: &InstallItemRequest) -> Result<PackageInstallCommand> {
    let backend = install_backend(
        ItemKind::Package,
        item.spec.backend.as_ref().map(|backend| backend.as_str()),
        current_platform(),
    );
    package_install_command_for_backend(&item.spec, &backend)
}

fn package_install_command_for_backend(
    spec: &ItemSpec,
    backend: &str,
) -> Result<PackageInstallCommand> {
    let normalized = install_backend(ItemKind::Package, Some(backend), current_platform());
    package_install_command_for_platform(
        &spec.name,
        spec.version.as_str(),
        &normalized,
        current_platform(),
    )
}

fn package_install_command_for_platform(
    name: &str,
    version: &str,
    backend: &str,
    platform: PlatformId,
) -> Result<PackageInstallCommand> {
    match platform {
        PlatformId::Macos => match backend {
            "homebrew" | "brew" => Ok(PackageInstallCommand {
                backend: "homebrew".to_string(),
                program: "brew".to_string(),
                args: vec![
                    "install".to_string(),
                    backend_versioned_name(name, version, "@"),
                ],
            }),
            _ => unsupported_package_backend(backend),
        },
        PlatformId::Linux => match backend {
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
        },
        PlatformId::Windows => match backend {
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
        },
    }
}

async fn install_app(item: &InstallItemRequest) -> Result<InstallResult> {
    let command = app_install_command(item)?;
    let output = Command::new(&command.program)
        .args(&command.args)
        .output()
        .with_context(|| format!("failed to run app backend {}", command.backend))?;
    if !output.status.success() {
        return Err(EngineError::Conflict {
            message: format!(
                "app backend {} failed: {}",
                command.backend,
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        }
        .into());
    }

    let install_path = app_install_path(item);
    write_install_marker(
        &install_path,
        item,
        &command.backend,
        &[install_path.clone()],
        &[],
    )
    .await?;

    Ok(InstallResult {
        tool_name: item.spec.name.clone(),
        version: item.spec.version.to_string(),
        install_path: install_path.clone(),
        binary_path: None,
        outputs: vec![install_path],
        linked_executables: Vec::new(),
        installed: Vec::new(),
    })
}

fn app_install_path(item: &InstallItemRequest) -> PathBuf {
    System::apps_dir()
        .join(&item.spec.name)
        .join(item.spec.version.as_str())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AppInstallCommand {
    backend: String,
    program: String,
    args: Vec<String>,
}

fn app_install_command(item: &InstallItemRequest) -> Result<AppInstallCommand> {
    let backend = install_backend(
        ItemKind::App,
        item.spec.backend.as_ref().map(|backend| backend.as_str()),
        current_platform(),
    );
    app_install_command_for_backend(&item.spec, &backend)
}

fn app_install_command_for_backend(spec: &ItemSpec, backend: &str) -> Result<AppInstallCommand> {
    let normalized = install_backend(ItemKind::App, Some(backend), current_platform());
    app_install_command_for_platform(
        &spec.name,
        spec.version.as_str(),
        &normalized,
        current_platform(),
    )
}

fn app_install_command_for_platform(
    name: &str,
    version: &str,
    backend: &str,
    platform: PlatformId,
) -> Result<AppInstallCommand> {
    match platform {
        PlatformId::Macos => match backend {
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
        },
        PlatformId::Linux => match backend {
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
        },
        PlatformId::Windows => match backend {
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
        },
    }
}

fn backend_versioned_name(name: &str, version: &str, separator: &str) -> String {
    if version.eq_ignore_ascii_case("latest") {
        name.to_string()
    } else {
        format!("{name}{separator}{version}")
    }
}

fn append_version_args(mut args: Vec<String>, version: &str, flag: &str) -> Vec<String> {
    if !version.eq_ignore_ascii_case("latest") {
        args.extend([flag.to_string(), version.to_string()]);
    }
    args
}

fn reject_pinned_package_version(backend: &str, version: &str) -> Result<()> {
    reject_pinned_version(ItemKind::Package, backend, version)
}

fn reject_pinned_app_version(backend: &str, version: &str) -> Result<()> {
    reject_pinned_version(ItemKind::App, backend, version)
}

fn reject_pinned_version(kind: ItemKind, backend: &str, version: &str) -> Result<()> {
    if version.eq_ignore_ascii_case("latest") {
        return Ok(());
    }
    Err(EngineError::UnsupportedPlatform {
        feature: format!("{kind} backend {backend} pinned version {version}"),
        platform: std::env::consts::OS.to_string(),
    }
    .into())
}

fn unsupported_app_backend(backend: &str) -> Result<AppInstallCommand> {
    Err(EngineError::UnsupportedPlatform {
        feature: format!("app backend {backend}"),
        platform: std::env::consts::OS.to_string(),
    }
    .into())
}

fn unsupported_package_backend(backend: &str) -> Result<PackageInstallCommand> {
    Err(EngineError::UnsupportedPlatform {
        feature: format!("package backend {backend}"),
        platform: std::env::consts::OS.to_string(),
    }
    .into())
}

fn install_backend(kind: ItemKind, backend: Option<&str>, platform: PlatformId) -> String {
    let backend = backend.and_then(|backend| {
        let trimmed = backend.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    });
    normalize_backend_selection(kind, backend, platform)
        .unwrap_or_else(|| default_backend(kind, platform).to_string())
}

async fn write_install_marker(
    install_path: &Path,
    item: &InstallItemRequest,
    backend: &str,
    outputs: &[PathBuf],
    linked_executables: &[PathBuf],
) -> Result<()> {
    tokio::fs::create_dir_all(install_path).await?;
    let marker_path = install_path.join("install.toml");
    let output_paths = paths_to_strings(outputs);
    let linked_paths = paths_to_strings(linked_executables);
    let install_path = install_path.display().to_string();
    let marker = InstallMarker {
        kind: item.kind.to_string(),
        name: item.spec.name.as_str(),
        version: item.spec.version.as_str(),
        backend,
        install_path: &install_path,
        outputs: &output_paths,
        linked_executables: &linked_paths,
    };
    let content = toml_edit::ser::to_string(&marker)?;
    tokio::fs::write(marker_path, content).await?;
    Ok(())
}

#[derive(Debug, Serialize)]
struct InstallMarker<'a> {
    kind: String,
    name: &'a str,
    version: &'a str,
    backend: &'a str,
    install_path: &'a str,
    outputs: &'a [String],
    linked_executables: &'a [String],
}

fn paths_to_strings(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .map(|path| path.display().to_string())
        .collect()
}

/* ----------------------------- small helpers ----------------------------- */

fn formula_json_path() -> PathBuf {
    System::cache_dir().join("still").join("formula.json")
}

fn ensure_formula_json_exists(path: &Path) -> Result<()> {
    if !path.exists() {
        anyhow::bail!("formula.json not found at: {}", path.display());
    }
    Ok(())
}

async fn load_formula_json_array(path: &Path) -> Result<Vec<serde_json::Value>> {
    let content = tokio::fs::read_to_string(path)
        .await
        .with_context(|| format!("Failed to read formula.json at {}", path.display()))?;

    let json: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Failed to parse JSON array: {e}"))?;

    let array = json
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("Expected JSON array"))?;

    Ok(array.clone())
}

fn find_matching_formula(
    formulas: &[serde_json::Value],
    tool_name: &str,
) -> Result<crate::specs::brew::FormulaSpec> {
    for v in formulas {
        let Ok(f) = serde_json::from_value::<crate::specs::brew::FormulaSpec>(v.clone()) else {
            continue; // skip malformed formulas
        };

        if f.name == tool_name
            || f.aliases.contains(&tool_name.to_string())
            || f.oldnames.contains(&tool_name.to_string())
        {
            return Ok(f);
        }
    }

    anyhow::bail!("No matching formula")
}

fn build_bottle_info(formula: &crate::specs::brew::FormulaSpec) -> Result<BottleInfo> {
    let bottle = formula.bottle.clone().ok_or_else(|| {
        anyhow::anyhow!(
            "No bottle available for {}@{}",
            formula.name,
            formula.versions.stable
        )
    })?;

    Ok(BottleInfo {
        formula_name: formula.name.clone(),
        version: formula.versions.stable.clone(),
        bottle,
    })
}

fn compute_install_path(formula_name: &str, version: &str) -> PathBuf {
    System::tool_dir().join(formula_name).join(version)
}

async fn reinstall_to_path(bottle_data: &[u8], install_path: &Path) -> Result<()> {
    if install_path.exists() {
        tokio::fs::remove_dir_all(install_path)
            .await
            .with_context(|| {
                format!(
                    "Failed to remove existing installation at {}",
                    install_path.display()
                )
            })?;
    }

    ArchiveExtractor::extract_tar_gz(bottle_data, install_path)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to extract bottle: {e}"))?;

    Ok(())
}

async fn link_binary(binary_path: &Option<PathBuf>) -> Result<Option<PathBuf>> {
    let Some(binary) = binary_path.as_ref() else {
        return Ok(None);
    };

    let system_bin_dir = System::bin_dir();
    tokio::fs::create_dir_all(&system_bin_dir).await?;
    let symlink_path =
        system_bin_dir.join(binary.file_name().ok_or_else(|| {
            anyhow::anyhow!("Binary path has no file name: {}", binary.display())
        })?);

    if symlink_path.exists() || symlink_path.is_symlink() {
        let _ = tokio::fs::remove_file(&symlink_path).await;
    }

    System::create_symlink(binary, &symlink_path).context("failed to create symlink to bin")?;

    Ok(Some(symlink_path))
}

/* -------------------------- network + verification -------------------------- */

async fn fetch_and_verify_bottle(
    formula_name: &str,
    bottle_file: &BottleFileSpec,
) -> Result<Vec<u8>> {
    let token = get_ghcr_token(formula_name).await?;
    let bottle_data = download_bottle(&bottle_file.url, &token)
        .await
        .context("Failed to download bottle")?;

    Hashing::verify_sha256(&bottle_data, &bottle_file.sha256)
        .map_err(|e| anyhow::anyhow!("Checksum verification failed: {e}"))?;

    Ok(bottle_data)
}

/// Get a bearer token from GitHub Container Registry
async fn get_ghcr_token(formula_name: &str) -> Result<String> {
    let token_url = format!(
        "https://ghcr.io/token?scope=repository:homebrew/core/{}:pull",
        formula_name
    );

    let client = NetUtils::client();
    let response = client
        .get(&token_url)
        .send()
        .await
        .context("Failed to request GHCR token")?;

    if !response.status().is_success() {
        anyhow::bail!("Failed to get GHCR token: HTTP {}", response.status());
    }

    let token_data: serde_json::Value = response
        .json()
        .await
        .context("Failed to parse token response")?;

    token_data
        .get("token")
        .and_then(|t| t.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("Token not found in response"))
}

/// Download a bottle file with authentication
async fn download_bottle(url: &str, token: &str) -> Result<Vec<u8>> {
    let client = NetUtils::client();
    let response = client
        .get(url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .context("Failed to download bottle")?;

    if !response.status().is_success() {
        anyhow::bail!("Failed to download bottle: HTTP {}", response.status());
    }

    let bytes = response
        .bytes()
        .await
        .context("Failed to read bottle data")?;

    Ok(bytes.to_vec())
}

/// Information about a Homebrew bottle selected during install planning.
///
/// This groups the formula identity and bottle metadata so helper functions do
/// not need to pass loosely related values separately.
#[derive(Debug, Clone)]
pub struct BottleInfo {
    /// Formula name that owns this bottle.
    pub formula_name: String,
    /// Formula version represented by this bottle.
    pub version: String,
    /// Bottle metadata including per-platform downloadable files.
    pub bottle: BottleSpec,
}

/* --------------------------- macOS impl unchanged --------------------------- */

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
            .ok_or_else(|| anyhow::anyhow!("No bottle files available for this system"))
    }

    async fn find_binary_recursive(
        install_path: &Path,
        formula_name: &str,
    ) -> Result<Option<PathBuf>> {
        // First, try the direct bin directory
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

        // Recursively search for bin directories
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
}

#[cfg(unix)]
fn is_executable(metadata: &std::fs::Metadata) -> bool {
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(windows)]
fn is_executable(_metadata: &std::fs::Metadata) -> bool {
    true
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
}

#[cfg(test)]
mod tests {
    use crate::specs::item::{ItemKind, ItemSpec};
    use anyhow::anyhow;

    use super::*;

    #[tokio::test]
    async fn install_rejects_empty_requests() {
        let err = run(InstallRequest { items: Vec::new() }).await.unwrap_err();

        assert!(err.to_string().contains("at least one item"));
    }

    #[tokio::test]
    async fn multi_item_install_rolls_back_previous_outputs_on_failure() {
        let temp = tempfile::tempdir().unwrap();
        let roots = RollbackRoots {
            tool_root: temp.path().join("tools"),
            package_root: temp.path().join("packages"),
            app_root: temp.path().join("apps"),
            bin_dir: temp.path().join("bin"),
        };
        let mut installer = FakeItemInstaller {
            roots: roots.clone(),
            calls: 0,
            fail_on_call: Some(1),
        };

        let err = run_with_installer_and_rollback_roots(
            InstallRequest {
                items: vec![
                    install_item(ItemKind::Tool, "ripgrep"),
                    install_item(ItemKind::Package, "openssl"),
                ],
            },
            &mut installer,
            &roots,
        )
        .await
        .unwrap_err();

        assert!(
            err.to_string()
                .contains("failed to install package openssl@latest")
        );
        assert!(!roots.tool_root.join("ripgrep/latest").exists());
        assert!(!roots.bin_dir.join("ripgrep").exists());
    }

    #[tokio::test]
    async fn multi_item_install_keeps_outputs_when_all_items_succeed() {
        let temp = tempfile::tempdir().unwrap();
        let roots = RollbackRoots {
            tool_root: temp.path().join("tools"),
            package_root: temp.path().join("packages"),
            app_root: temp.path().join("apps"),
            bin_dir: temp.path().join("bin"),
        };
        let mut installer = FakeItemInstaller {
            roots: roots.clone(),
            calls: 0,
            fail_on_call: None,
        };

        let result = run_with_installer_and_rollback_roots(
            InstallRequest {
                items: vec![
                    install_item(ItemKind::Tool, "ripgrep"),
                    install_item(ItemKind::Package, "openssl"),
                ],
            },
            &mut installer,
            &roots,
        )
        .await
        .unwrap();

        assert_eq!(result.installed.len(), 2);
        assert!(roots.tool_root.join("ripgrep/latest").exists());
        assert!(roots.package_root.join("openssl/latest").exists());
    }

    #[tokio::test]
    async fn rollback_removes_completed_install_outputs() {
        let temp = tempfile::tempdir().unwrap();
        let roots = RollbackRoots {
            tool_root: temp.path().join("tools"),
            package_root: temp.path().join("packages"),
            app_root: temp.path().join("apps"),
            bin_dir: temp.path().join("bin"),
        };
        let mut installer = FakeItemInstaller {
            roots: roots.clone(),
            calls: 0,
            fail_on_call: None,
        };

        let result = run_with_installer_and_rollback_roots(
            InstallRequest {
                items: vec![install_item(ItemKind::Tool, "ripgrep")],
            },
            &mut installer,
            &roots,
        )
        .await
        .unwrap();

        assert!(roots.tool_root.join("ripgrep/latest").exists());
        assert!(roots.bin_dir.join("ripgrep").exists());

        rollback_installed_items_at(&result.installed, &roots)
            .await
            .unwrap();

        assert!(!roots.tool_root.join("ripgrep/latest").exists());
        assert!(!roots.bin_dir.join("ripgrep").exists());
    }

    #[test]
    fn app_install_command_uses_platform_default_backend() {
        let item = InstallItemRequest {
            kind: ItemKind::App,
            spec: "firefox".parse::<ItemSpec>().unwrap(),
        };

        let command = app_install_command(&item).unwrap();

        #[cfg(target_os = "macos")]
        assert_eq!(command.program, "brew");
        #[cfg(target_os = "linux")]
        assert_eq!(command.program, "flatpak");
        #[cfg(target_os = "windows")]
        assert_eq!(command.program, "winget");
    }

    #[test]
    fn app_install_command_treats_auto_as_platform_default_backend() {
        let item = InstallItemRequest {
            kind: ItemKind::App,
            spec: "firefox@latest@auto".parse::<ItemSpec>().unwrap(),
        };

        let command = app_install_command(&item).unwrap();

        #[cfg(target_os = "macos")]
        assert_eq!(command.backend, "homebrew-cask");
        #[cfg(target_os = "linux")]
        assert_eq!(command.backend, "flatpak");
        #[cfg(target_os = "windows")]
        assert_eq!(command.backend, "winget");
    }

    #[test]
    fn install_backend_treats_empty_backend_as_platform_default() {
        assert_eq!(
            install_backend(ItemKind::App, Some(" "), PlatformId::Linux),
            "flatpak"
        );
        assert_eq!(
            install_backend(ItemKind::Package, None, PlatformId::Windows),
            "winget"
        );
    }

    #[test]
    fn app_install_command_rejects_unknown_backend() {
        let item = InstallItemRequest {
            kind: ItemKind::App,
            spec: "firefox@latest@unknown-backend"
                .parse::<ItemSpec>()
                .unwrap(),
        };

        let err = app_install_command(&item).unwrap_err();

        assert!(err.to_string().contains("app backend unknown-backend"));
    }

    #[test]
    fn app_install_command_plans_windows_scoop() {
        let command =
            app_install_command_for_platform("firefox", "latest", "scoop", PlatformId::Windows)
                .unwrap();

        assert_eq!(command.backend, "scoop");
        assert_eq!(command.program, "scoop");
        assert_eq!(command.args, ["install", "firefox"]);
    }

    #[test]
    fn app_install_command_plans_macos_mas() {
        let command =
            app_install_command_for_platform("497799835", "latest", "mas", PlatformId::Macos)
                .unwrap();

        assert_eq!(command.backend, "mas");
        assert_eq!(command.program, "mas");
        assert_eq!(command.args, ["install", "497799835"]);
    }

    #[test]
    fn app_install_command_rejects_pinned_cask_versions() {
        let err = app_install_command_for_platform(
            "firefox",
            "121.0",
            "homebrew-cask",
            PlatformId::Macos,
        )
        .unwrap_err();

        assert!(
            err.to_string()
                .contains("app backend homebrew-cask pinned version 121.0")
        );
    }

    #[test]
    fn app_install_command_passes_pinned_versions_to_windows_backends() {
        let winget =
            app_install_command_for_platform("Firefox", "121.0", "winget", PlatformId::Windows)
                .unwrap();
        assert_eq!(
            winget.args,
            [
                "install",
                "--id",
                "Firefox",
                "--version",
                "121.0",
                "--accept-package-agreements",
                "--accept-source-agreements"
            ]
        );

        let scoop =
            app_install_command_for_platform("firefox", "121.0", "scoop", PlatformId::Windows)
                .unwrap();
        assert_eq!(scoop.args, ["install", "firefox@121.0"]);
    }

    #[test]
    fn app_install_path_uses_still_managed_app_storage() {
        let item = InstallItemRequest {
            kind: ItemKind::App,
            spec: "firefox@latest@flatpak".parse::<ItemSpec>().unwrap(),
        };

        assert_eq!(
            app_install_path(&item),
            System::apps_dir().join("firefox").join("latest")
        );
    }

    #[test]
    fn package_install_command_uses_platform_default_backend() {
        let item = InstallItemRequest {
            kind: ItemKind::Package,
            spec: "openssl".parse::<ItemSpec>().unwrap(),
        };

        let command = package_install_command(&item).unwrap();

        #[cfg(target_os = "macos")]
        assert_eq!(command.program, "brew");
        #[cfg(target_os = "linux")]
        assert_eq!(command.program, "apt-get");
        #[cfg(target_os = "windows")]
        assert_eq!(command.program, "winget");
    }

    #[test]
    fn package_install_command_treats_auto_as_platform_default_backend() {
        let item = InstallItemRequest {
            kind: ItemKind::Package,
            spec: "openssl@latest@auto".parse::<ItemSpec>().unwrap(),
        };

        let command = package_install_command(&item).unwrap();

        #[cfg(target_os = "macos")]
        assert_eq!(command.backend, "homebrew");
        #[cfg(target_os = "linux")]
        assert_eq!(command.backend, "apt");
        #[cfg(target_os = "windows")]
        assert_eq!(command.backend, "winget");
    }

    #[test]
    fn package_install_command_rejects_unknown_backend() {
        let item = InstallItemRequest {
            kind: ItemKind::Package,
            spec: "openssl@latest@unknown-backend"
                .parse::<ItemSpec>()
                .unwrap(),
        };

        let err = package_install_command(&item).unwrap_err();

        assert!(err.to_string().contains("package backend unknown-backend"));
    }

    #[test]
    fn package_install_command_plans_windows_scoop() {
        let command =
            package_install_command_for_platform("openssl", "latest", "scoop", PlatformId::Windows)
                .unwrap();

        assert_eq!(command.backend, "scoop");
        assert_eq!(command.program, "scoop");
        assert_eq!(command.args, ["install", "openssl"]);
    }

    #[test]
    fn package_install_command_passes_pinned_versions_to_supported_backends() {
        let brew =
            package_install_command_for_platform("ripgrep", "1", "homebrew", PlatformId::Macos)
                .unwrap();
        assert_eq!(brew.args, ["install", "ripgrep@1"]);

        let apt =
            package_install_command_for_platform("openssl", "3", "apt", PlatformId::Linux).unwrap();
        assert_eq!(apt.args, ["install", "-y", "openssl=3"]);

        let winget =
            package_install_command_for_platform("OpenSSL", "3", "winget", PlatformId::Windows)
                .unwrap();
        assert_eq!(
            winget.args,
            [
                "install",
                "--id",
                "OpenSSL",
                "--version",
                "3",
                "--accept-package-agreements",
                "--accept-source-agreements"
            ]
        );
    }

    #[test]
    fn package_install_command_rejects_pinned_versions_for_unsupported_backends() {
        let err = package_install_command_for_platform("openssl", "3", "dnf", PlatformId::Linux)
            .unwrap_err();

        assert!(
            err.to_string()
                .contains("package backend dnf pinned version 3")
        );
    }

    #[test]
    fn tool_install_command_plans_rustup() {
        let item = InstallItemRequest {
            kind: ItemKind::Tool,
            spec: "rust@stable@rustup".parse::<ItemSpec>().unwrap(),
        };

        let command = tool_install_command(&item).unwrap();

        assert_eq!(command.program, "rustup");
        assert_eq!(command.args, ["toolchain", "install", "stable"]);
    }

    #[test]
    fn tool_install_command_plans_language_package_managers() {
        let cargo = tool_install_command(&InstallItemRequest {
            kind: ItemKind::Tool,
            spec: "cargo-nextest@0.9.99@cargo".parse::<ItemSpec>().unwrap(),
        })
        .unwrap();
        let npm = tool_install_command(&InstallItemRequest {
            kind: ItemKind::Tool,
            spec: "typescript@5.8.0@npm".parse::<ItemSpec>().unwrap(),
        })
        .unwrap();
        let pipx = tool_install_command(&InstallItemRequest {
            kind: ItemKind::Tool,
            spec: "ruff@0.11.0@pipx".parse::<ItemSpec>().unwrap(),
        })
        .unwrap();

        assert_eq!(
            cargo.args,
            ["install", "cargo-nextest", "--version", "0.9.99"]
        );
        assert_eq!(npm.args, ["install", "--global", "typescript@5.8.0"]);
        assert_eq!(pipx.args, ["install", "ruff==0.11.0"]);
    }

    #[test]
    fn tool_install_command_plans_version_managers() {
        let mise = tool_install_command(&InstallItemRequest {
            kind: ItemKind::Tool,
            spec: "node@22@mise".parse::<ItemSpec>().unwrap(),
        })
        .unwrap();
        let asdf = tool_install_command(&InstallItemRequest {
            kind: ItemKind::Tool,
            spec: "node@22@asdf".parse::<ItemSpec>().unwrap(),
        })
        .unwrap();
        let aqua = tool_install_command(&InstallItemRequest {
            kind: ItemKind::Tool,
            spec: "ripgrep@14.1.1@aqua".parse::<ItemSpec>().unwrap(),
        })
        .unwrap();

        assert_eq!(mise.args, ["install", "node@22"]);
        assert_eq!(asdf.args, ["install", "node", "22"]);
        assert_eq!(aqua.args, ["install", "ripgrep@14.1.1"]);
    }

    #[test]
    fn tool_install_command_rejects_unknown_backend() {
        let item = InstallItemRequest {
            kind: ItemKind::Tool,
            spec: "ripgrep@latest@unknown-backend"
                .parse::<ItemSpec>()
                .unwrap(),
        };

        let err = tool_install_command(&item).unwrap_err();

        assert!(err.to_string().contains("tool backend unknown-backend"));
    }

    #[tokio::test]
    async fn install_marker_records_outputs_and_links() {
        let temp = tempfile::tempdir().unwrap();
        let install_path = temp.path().join("tools/ripgrep/14.1.1");
        let item = InstallItemRequest {
            kind: ItemKind::Tool,
            spec: "ripgrep@14.1.1@homebrew".parse::<ItemSpec>().unwrap(),
        };

        write_install_marker(
            &install_path,
            &item,
            "homebrew",
            std::slice::from_ref(&install_path),
            &[temp.path().join("bin/rg")],
        )
        .await
        .unwrap();

        let marker = tokio::fs::read_to_string(install_path.join("install.toml"))
            .await
            .unwrap();

        assert!(marker.contains("kind = \"tool\""));
        assert!(marker.contains("outputs = ["));
        assert!(marker.contains("linked_executables = ["));
        assert!(marker.contains("bin/rg"));
    }

    struct FakeItemInstaller {
        roots: RollbackRoots,
        calls: usize,
        fail_on_call: Option<usize>,
    }

    impl ItemInstaller for FakeItemInstaller {
        async fn install(&mut self, item: &InstallItemRequest) -> Result<InstallResult> {
            if self.fail_on_call == Some(self.calls) {
                self.calls += 1;
                return Err(anyhow!("planned failure"));
            }
            self.calls += 1;
            let install_root = self.roots.install_root(item.kind);
            let install_path = install_root
                .join(&item.spec.name)
                .join(item.spec.version.as_str());
            let link_path = self.roots.bin_dir.join(&item.spec.name);
            tokio::fs::create_dir_all(&install_path).await?;
            tokio::fs::create_dir_all(&self.roots.bin_dir).await?;
            tokio::fs::write(&link_path, "link").await?;

            Ok(InstallResult {
                tool_name: item.spec.name.clone(),
                version: item.spec.version.to_string(),
                install_path: install_path.clone(),
                binary_path: Some(link_path.clone()),
                outputs: vec![install_path],
                linked_executables: vec![link_path],
                installed: Vec::new(),
            })
        }
    }

    fn install_item(kind: ItemKind, spec: &str) -> InstallItemRequest {
        InstallItemRequest {
            kind,
            spec: spec.parse::<ItemSpec>().unwrap(),
        }
    }
}
