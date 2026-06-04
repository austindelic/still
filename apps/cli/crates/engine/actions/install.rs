//! Engine install action for resolving, downloading, verifying, extracting, and linking packages.

use crate::config::{ConfigScope, ConfigSelection, resolve_config_path};
use crate::config_edit::add_install_items_with_force;
use crate::error::EngineError;
use crate::error::{EngineContext, Result};
use crate::lockfile::lockfile_path;
use crate::platform::{PlatformId, current_platform};
use crate::registries::specs::tool::ToolSpec;
use crate::specs::backend::{
    default_backend, infer_item_kind_from_backend,
    normalize_auto_backend as normalize_backend_selection,
};
use crate::specs::brew::{BottleFileSpec, BottleSpec};
use crate::specs::item::{ItemKind, ItemSpec};
use crate::system::System;
use crate::utils::archive::ArchiveExtractor;
use crate::utils::hashing::Hashing;
use crate::utils::link::SymlinkOps;
use crate::utils::net::NetUtils;
use crate::utils::paths::PathOps;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Platform-specific install operations needed by the generic install flow.
///
/// Implementors provide the host-specific pieces of an otherwise shared install:
/// choosing the right Homebrew bottle file and discovering the executable after
/// extraction. Keep network, archive, and CLI formatting code out of this trait.
pub(crate) trait InstallOps {
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

    /// Builds the native package manager command for the current platform.
    fn package_install_command(
        name: &str,
        version: &str,
        backend: &str,
    ) -> Result<PackageInstallCommand>;

    /// Builds the native app manager command for the current platform.
    fn app_install_command(name: &str, version: &str, backend: &str) -> Result<AppInstallCommand>;
}

/// One item requested for install.
///
/// Build this after CLI/config parsing has classified the item as a tool,
/// package, or app. Resolution may still choose the final backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallItemRequest {
    pub kind: ItemKind,
    pub spec: ItemSpec,
    pub tool: ToolInstallOptions,
}

/// Tool-specific install extras from expanded tool config.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolInstallOptions {
    pub components: Vec<String>,
    pub targets: Vec<String>,
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

/// Builds install item requests from grouped and unclassified specs.
///
/// Explicit groups keep their caller-selected kind. Unclassified items are
/// accepted only when their backend implies one item kind unambiguously.
/// # Errors
/// Fails when an unclassified spec has no backend or a backend shared by
/// multiple item kinds.
pub fn items_from_specs(
    tools: Vec<ToolSpec>,
    packages: Vec<ToolSpec>,
    apps: Vec<ToolSpec>,
    unclassified: Vec<ToolSpec>,
) -> Result<Vec<InstallItemRequest>> {
    let mut items = Vec::new();
    push_spec_items(&mut items, ItemKind::Tool, tools);
    push_spec_items(&mut items, ItemKind::Package, packages);
    push_spec_items(&mut items, ItemKind::App, apps);
    for spec in unclassified {
        let kind = infer_spec_item_kind(&spec)?;
        push_spec_items(&mut items, kind, vec![spec]);
    }
    Ok(items)
}

fn push_spec_items(items: &mut Vec<InstallItemRequest>, kind: ItemKind, specs: Vec<ToolSpec>) {
    items.extend(specs.into_iter().map(|spec| InstallItemRequest {
        kind,
        spec: ItemSpec {
            name: spec.name,
            version: spec.version.parse().expect("validated ToolSpec version"),
            backend: spec.backend,
        },
        tool: Default::default(),
    }));
}

fn infer_spec_item_kind(spec: &ToolSpec) -> Result<ItemKind> {
    let Some(backend) = &spec.backend else {
        return Err(EngineError::message(format!(
            "cannot infer whether {} is a tool, package, or app; use --tool, --package, or --app",
            spec.name
        )));
    };
    infer_item_kind_from_backend(backend.as_str()).ok_or_else(|| {
        EngineError::message(format!(
            "cannot infer whether {}@{}@{} is a tool, package, or app; use --tool, --package, or --app",
            spec.name,
            spec.version,
            backend
        ))
    })
}

/// Request to install items and record the successful desired state.
#[derive(Debug, Clone)]
pub struct InstallAndRecordRequest {
    pub start_dir: PathBuf,
    pub home_dir: PathBuf,
    pub global: bool,
    pub force: bool,
    pub install: InstallRequest,
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

/// Installs requested items, records them in the selected config, and refreshes the lockfile.
/// # Errors
/// Fails when install execution, config editing, lockfile refresh, or rollback fails.
/// # Side Effects
/// Installs files, writes selected config, refreshes the adjacent lockfile, and
/// rolls back installed outputs if recording fails.
pub async fn run_and_record(request: InstallAndRecordRequest) -> Result<InstallResult> {
    let install_request = request.install.clone();
    let pending = prepare_install_config_write_at(
        &install_request,
        request.global,
        request.force,
        &request.start_dir,
        &request.home_dir,
    )?;
    let result = run(request.install).await?;
    if let Err(err) = record_install_items(pending).await {
        if let Err(rollback_err) = rollback_installed_items(&result.installed).await {
            return Err(err.context(format!(
                "failed to roll back installed artifacts after config recording failed: {rollback_err}"
            )));
        }
        return Err(
            err.context("failed to record installed items; rolled back installed artifacts")
        );
    }
    Ok(result)
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

#[derive(Debug, Clone)]
struct PendingInstallConfigWrite {
    path: PathBuf,
    home_dir: PathBuf,
    original: Option<String>,
    updated: String,
}

fn prepare_install_config_write_at(
    request: &InstallRequest,
    global: bool,
    force: bool,
    start_dir: &Path,
    home_dir: &Path,
) -> Result<PendingInstallConfigWrite> {
    let resolved = resolve_config_path(
        start_dir,
        home_dir,
        ConfigSelection {
            scope: if global {
                ConfigScope::Global
            } else {
                ConfigScope::Project
            },
            for_write: false,
        },
    )?;

    let original = match std::fs::read_to_string(&resolved.path) {
        Ok(content) => Some(content),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => return Err(err.into()),
    };
    let content = original.as_deref().unwrap_or("");
    let updated = add_install_items_with_force(content, &request.items, force)?;
    Ok(PendingInstallConfigWrite {
        path: resolved.path,
        home_dir: home_dir.to_path_buf(),
        original,
        updated,
    })
}

async fn record_install_items(pending: PendingInstallConfigWrite) -> Result<()> {
    if let Some(parent) = pending.path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let lockfile_path = lockfile_path(&pending.path);
    let original_lockfile = read_optional_file(&lockfile_path).await?;
    tokio::fs::write(&pending.path, pending.updated).await?;
    if let Err(err) =
        crate::actions::sync::refresh_active_lockfile(&pending.path, &pending.home_dir).await
    {
        restore_install_config(&pending.path, pending.original).await?;
        restore_install_lockfile(&lockfile_path, original_lockfile).await?;
        return Err(err);
    }
    Ok(())
}

async fn read_optional_file(path: &Path) -> Result<Option<String>> {
    match tokio::fs::read_to_string(path).await {
        Ok(content) => Ok(Some(content)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err.into()),
    }
}

async fn restore_install_config(path: &Path, original: Option<String>) -> Result<()> {
    match original {
        Some(original) => tokio::fs::write(path, original).await?,
        None if path.exists() => tokio::fs::remove_file(path).await?,
        None => {}
    }
    Ok(())
}

async fn restore_install_lockfile(path: &Path, original: Option<String>) -> Result<()> {
    match original {
        Some(original) => tokio::fs::write(path, original).await?,
        None if path.exists() => tokio::fs::remove_file(path).await?,
        None => {}
    }
    Ok(())
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
            package_root: native_receipt_root(ItemKind::Package),
            app_root: native_receipt_root(ItemKind::App),
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
    let install_path = tool_install_path(item);
    let staging_path = tool_staging_path(item);
    let result = install_tool_with_command_staged(item, &install_path, &staging_path).await;
    if result.is_err() {
        let _ = remove_path_if_exists(&staging_path).await;
    }
    result
}

async fn install_tool_with_command_staged(
    item: &InstallItemRequest,
    install_path: &Path,
    staging_path: &Path,
) -> Result<InstallResult> {
    let command = tool_install_command_at(item, staging_path)?;
    let backup_path = tool_backup_path(item);
    remove_path_if_exists(staging_path).await?;
    tokio::fs::create_dir_all(staging_path).await?;
    for step in command.steps() {
        let mut process = Command::new(&step.program);
        process.args(&step.args);
        process.envs(command.env.iter().map(|(key, value)| (key, value)));
        let output = process
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
    }

    let had_previous = promote_staged_install(staging_path, install_path, &backup_path).await?;
    let binary_path = match System::find_binary_recursive(install_path, &item.spec.name).await {
        Ok(binary_path) => binary_path,
        Err(err) => {
            rollback_promoted_install(install_path, &backup_path, had_previous).await?;
            return Err(err);
        }
    };
    let linked_executables = match link_binary(&binary_path).await {
        Ok(linked) => linked.into_iter().collect::<Vec<_>>(),
        Err(err) => {
            rollback_promoted_install(install_path, &backup_path, had_previous).await?;
            return Err(err);
        }
    };
    if let Err(err) = write_install_marker(
        install_path,
        item,
        &command.backend,
        &[install_path.to_path_buf()],
        &linked_executables,
    )
    .await
    {
        for linked in &linked_executables {
            let _ = remove_path_if_exists(linked).await;
        }
        rollback_promoted_install(install_path, &backup_path, had_previous).await?;
        return Err(err);
    }
    let _ = remove_path_if_exists(&backup_path).await;

    Ok(InstallResult {
        tool_name: item.spec.name.clone(),
        version: item.spec.version.to_string(),
        install_path: install_path.to_path_buf(),
        binary_path,
        outputs: vec![install_path.to_path_buf()],
        linked_executables,
        installed: Vec::new(),
    })
}

async fn promote_staged_install(
    staging_path: &Path,
    install_path: &Path,
    backup_path: &Path,
) -> Result<bool> {
    remove_path_if_exists(backup_path).await?;
    if let Some(parent) = install_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let had_previous = install_path.exists();
    if had_previous {
        tokio::fs::rename(install_path, backup_path)
            .await
            .with_context(|| format!("failed to back up {}", install_path.display()))?;
    }

    if let Err(err) = tokio::fs::rename(staging_path, install_path).await {
        if had_previous {
            let _ = tokio::fs::rename(backup_path, install_path).await;
        }
        return Err(err)
            .with_context(|| format!("failed to promote install to {}", install_path.display()));
    }

    Ok(had_previous)
}

async fn rollback_promoted_install(
    install_path: &Path,
    backup_path: &Path,
    had_previous: bool,
) -> Result<()> {
    remove_path_if_exists(install_path).await?;
    if had_previous {
        tokio::fs::rename(backup_path, install_path)
            .await
            .with_context(|| format!("failed to restore {}", install_path.display()))?;
    } else {
        remove_path_if_exists(backup_path).await?;
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ToolInstallCommand {
    backend: String,
    program: String,
    args: Vec<String>,
    env: Vec<(String, String)>,
    after: Vec<ToolInstallStep>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ToolInstallStep {
    program: String,
    args: Vec<String>,
}

impl ToolInstallCommand {
    fn steps(&self) -> Vec<ToolInstallStep> {
        let mut steps = vec![ToolInstallStep {
            program: self.program.clone(),
            args: self.args.clone(),
        }];
        steps.extend(self.after.clone());
        steps
    }
}

#[cfg(test)]
fn tool_install_command(item: &InstallItemRequest) -> Result<ToolInstallCommand> {
    tool_install_command_at(item, &tool_install_path(item))
}

fn tool_install_command_at(
    item: &InstallItemRequest,
    install_path: &Path,
) -> Result<ToolInstallCommand> {
    let backend = install_backend(
        ItemKind::Tool,
        item.spec.backend.as_ref().map(|backend| backend.as_str()),
        current_platform(),
    );
    tool_install_command_for_backend(
        &item.spec.name,
        item.spec.version.as_str(),
        &backend,
        &item.tool,
        install_path,
    )
}

fn tool_install_command_for_backend(
    name: &str,
    version: &str,
    backend: &str,
    options: &ToolInstallOptions,
    install_path: &Path,
) -> Result<ToolInstallCommand> {
    let normalized = install_backend(ItemKind::Tool, Some(backend), current_platform());
    let install_path = install_path.display().to_string();
    match normalized.as_str() {
        "rustup" => Ok(ToolInstallCommand {
            backend: "rustup".to_string(),
            program: "rustup".to_string(),
            args: vec![
                "toolchain".to_string(),
                "install".to_string(),
                version.to_string(),
            ],
            env: vec![
                ("RUSTUP_HOME".to_string(), format!("{install_path}/rustup")),
                ("CARGO_HOME".to_string(), format!("{install_path}/cargo")),
            ],
            after: rustup_extra_steps(version, options),
        }),
        "cargo" => {
            reject_tool_extras(&normalized, options)?;
            let mut args = vec![
                "install".to_string(),
                "--root".to_string(),
                install_path.clone(),
                name.to_string(),
            ];
            if version != "latest" {
                args.extend(["--version".to_string(), version.to_string()]);
            }
            Ok(ToolInstallCommand {
                backend: "cargo".to_string(),
                program: "cargo".to_string(),
                args,
                env: Vec::new(),
                after: Vec::new(),
            })
        }
        "npm" => {
            reject_tool_extras(&normalized, options)?;
            Ok(ToolInstallCommand {
                backend: "npm".to_string(),
                program: "npm".to_string(),
                args: vec![
                    "install".to_string(),
                    "--global".to_string(),
                    "--prefix".to_string(),
                    install_path.clone(),
                    package_with_version(name, version),
                ],
                env: Vec::new(),
                after: Vec::new(),
            })
        }
        "pnpm" => {
            reject_tool_extras(&normalized, options)?;
            Ok(ToolInstallCommand {
                backend: "pnpm".to_string(),
                program: "pnpm".to_string(),
                args: vec![
                    "add".to_string(),
                    "--global".to_string(),
                    package_with_version(name, version),
                ],
                env: vec![
                    ("PNPM_HOME".to_string(), format!("{install_path}/bin")),
                    ("NPM_CONFIG_PREFIX".to_string(), install_path.clone()),
                ],
                after: Vec::new(),
            })
        }
        "yarn" => {
            reject_tool_extras(&normalized, options)?;
            Ok(ToolInstallCommand {
                backend: "yarn".to_string(),
                program: "yarn".to_string(),
                args: vec![
                    "global".to_string(),
                    "add".to_string(),
                    "--prefix".to_string(),
                    install_path.clone(),
                    package_with_version(name, version),
                ],
                env: Vec::new(),
                after: Vec::new(),
            })
        }
        "pipx" => {
            reject_tool_extras(&normalized, options)?;
            Ok(ToolInstallCommand {
                backend: "pipx".to_string(),
                program: "pipx".to_string(),
                args: vec![
                    "install".to_string(),
                    "--install-dir".to_string(),
                    format!("{install_path}/venvs"),
                    "--bin-dir".to_string(),
                    format!("{install_path}/bin"),
                    python_package_with_version(name, version),
                ],
                env: Vec::new(),
                after: Vec::new(),
            })
        }
        "go" => {
            reject_tool_extras(&normalized, options)?;
            Ok(ToolInstallCommand {
                backend: "go".to_string(),
                program: "go".to_string(),
                args: vec![
                    "install".to_string(),
                    go_package_with_version(name, version),
                ],
                env: vec![
                    ("GOBIN".to_string(), format!("{install_path}/bin")),
                    ("GOPATH".to_string(), format!("{install_path}/go")),
                ],
                after: Vec::new(),
            })
        }
        "mise" => {
            reject_tool_extras(&normalized, options)?;
            Ok(ToolInstallCommand {
                backend: "mise".to_string(),
                program: "mise".to_string(),
                args: vec!["install".to_string(), format!("{name}@{version}")],
                env: vec![("MISE_DATA_DIR".to_string(), format!("{install_path}/mise"))],
                after: Vec::new(),
            })
        }
        "asdf" => {
            reject_tool_extras(&normalized, options)?;
            Ok(ToolInstallCommand {
                backend: "asdf".to_string(),
                program: "asdf".to_string(),
                args: vec!["install".to_string(), name.to_string(), version.to_string()],
                env: vec![("ASDF_DATA_DIR".to_string(), format!("{install_path}/asdf"))],
                after: Vec::new(),
            })
        }
        "aqua" => {
            reject_tool_extras(&normalized, options)?;
            Ok(ToolInstallCommand {
                backend: "aqua".to_string(),
                program: "aqua".to_string(),
                args: vec!["install".to_string(), package_with_version(name, version)],
                env: vec![("AQUA_ROOT_DIR".to_string(), format!("{install_path}/aqua"))],
                after: Vec::new(),
            })
        }
        "homebrew" | "brew" => Err(EngineError::Conflict {
            message: "homebrew tool installs use the Still-managed bottle installer".to_string(),
        }
        .into()),
        _ => unsupported_tool_backend(&normalized),
    }
}

fn tool_install_path(item: &InstallItemRequest) -> PathBuf {
    System::tool_dir()
        .join(&item.spec.name)
        .join(item.spec.version.as_str())
}

fn tool_staging_path(item: &InstallItemRequest) -> PathBuf {
    System::tool_dir()
        .join(&item.spec.name)
        .join(format!(".{}.staging", item.spec.version.as_str()))
}

fn tool_backup_path(item: &InstallItemRequest) -> PathBuf {
    System::tool_dir()
        .join(&item.spec.name)
        .join(format!(".{}.previous", item.spec.version.as_str()))
}

fn rustup_extra_steps(version: &str, options: &ToolInstallOptions) -> Vec<ToolInstallStep> {
    let mut steps = Vec::new();
    if !options.components.is_empty() {
        let mut args = vec!["component".to_string(), "add".to_string()];
        args.extend(options.components.clone());
        args.extend(["--toolchain".to_string(), version.to_string()]);
        steps.push(ToolInstallStep {
            program: "rustup".to_string(),
            args,
        });
    }
    if !options.targets.is_empty() {
        let mut args = vec!["target".to_string(), "add".to_string()];
        args.extend(options.targets.clone());
        args.extend(["--toolchain".to_string(), version.to_string()]);
        steps.push(ToolInstallStep {
            program: "rustup".to_string(),
            args,
        });
    }
    steps
}

fn reject_tool_extras(backend: &str, options: &ToolInstallOptions) -> Result<()> {
    if options.components.is_empty() && options.targets.is_empty() {
        return Ok(());
    }
    Err(EngineError::UnsupportedPlatform {
        feature: format!("tool backend {backend} components or targets"),
        platform: std::env::consts::OS.to_string(),
    }
    .into())
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

fn go_package_with_version(name: &str, version: &str) -> String {
    if version == "latest" {
        format!("{name}@latest")
    } else {
        format!("{name}@{version}")
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

    let install_path = native_receipt_path(item);
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
pub(crate) struct PackageInstallCommand {
    pub(crate) backend: String,
    pub(crate) program: String,
    pub(crate) args: Vec<String>,
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
    System::package_install_command(&spec.name, spec.version.as_str(), &normalized)
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
    native_receipt_path(item)
}

fn native_receipt_path(item: &InstallItemRequest) -> PathBuf {
    native_receipt_root(item.kind)
        .join(&item.spec.name)
        .join(item.spec.version.as_str())
}

fn native_receipt_root(kind: ItemKind) -> PathBuf {
    let folder = match kind {
        ItemKind::Tool => "tools",
        ItemKind::Package => "packages",
        ItemKind::App => "apps",
    };
    System::root_dir().join("receipts").join(folder)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AppInstallCommand {
    pub(crate) backend: String,
    pub(crate) program: String,
    pub(crate) args: Vec<String>,
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
    System::app_install_command(&spec.name, spec.version.as_str(), &normalized)
}

pub(crate) fn backend_versioned_name(name: &str, version: &str, separator: &str) -> String {
    if version.eq_ignore_ascii_case("latest") {
        name.to_string()
    } else {
        format!("{name}{separator}{version}")
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn append_version_args(mut args: Vec<String>, version: &str, flag: &str) -> Vec<String> {
    if !version.eq_ignore_ascii_case("latest") {
        args.extend([flag.to_string(), version.to_string()]);
    }
    args
}

#[cfg(target_os = "linux")]
pub(crate) fn reject_pinned_package_version(backend: &str, version: &str) -> Result<()> {
    reject_pinned_version(ItemKind::Package, backend, version)
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub(crate) fn reject_pinned_app_version(backend: &str, version: &str) -> Result<()> {
    reject_pinned_version(ItemKind::App, backend, version)
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
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

pub(crate) fn unsupported_app_backend(backend: &str) -> Result<AppInstallCommand> {
    Err(EngineError::UnsupportedPlatform {
        feature: format!("app backend {backend}"),
        platform: std::env::consts::OS.to_string(),
    }
    .into())
}

pub(crate) fn unsupported_package_backend(backend: &str) -> Result<PackageInstallCommand> {
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
        return Err(EngineError::message(format!(
            "formula.json not found at: {}",
            path.display()
        )));
    }
    Ok(())
}

async fn load_formula_json_array(path: &Path) -> Result<Vec<serde_json::Value>> {
    let content = tokio::fs::read_to_string(path)
        .await
        .with_context(|| format!("Failed to read formula.json at {}", path.display()))?;

    let json: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| EngineError::message(format!("Failed to parse JSON array: {e}")))?;

    let array = json
        .as_array()
        .ok_or_else(|| EngineError::message("Expected JSON array"))?;

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

    Err(EngineError::message("No matching formula"))
}

fn build_bottle_info(formula: &crate::specs::brew::FormulaSpec) -> Result<BottleInfo> {
    let bottle = formula.bottle.clone().ok_or_else(|| {
        EngineError::message(format!(
            "No bottle available for {}@{}",
            formula.name, formula.versions.stable
        ))
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
        .map_err(|e| EngineError::Archive(format!("Failed to extract bottle: {e}")))?;

    Ok(())
}

async fn link_binary(binary_path: &Option<PathBuf>) -> Result<Option<PathBuf>> {
    let Some(binary) = binary_path.as_ref() else {
        return Ok(None);
    };

    let system_bin_dir = System::bin_dir();
    tokio::fs::create_dir_all(&system_bin_dir).await?;
    let symlink_path = system_bin_dir.join(binary.file_name().ok_or_else(|| {
        EngineError::message(format!(
            "Binary path has no file name: {}",
            binary.display()
        ))
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
        .map_err(|e| EngineError::message(format!("Checksum verification failed: {e}")))?;

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
        return Err(EngineError::message(format!(
            "Failed to get GHCR token: HTTP {}",
            response.status()
        )));
    }

    let token_data: serde_json::Value = response
        .json()
        .await
        .context("Failed to parse token response")?;

    token_data
        .get("token")
        .and_then(|t| t.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| EngineError::message("Token not found in response"))
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
        return Err(EngineError::message(format!(
            "Failed to download bottle: HTTP {}",
            response.status()
        )));
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

#[cfg(test)]
mod tests {
    use crate::specs::item::{ItemKind, ItemSpec};

    use super::*;

    #[test]
    fn items_from_specs_preserves_groups_and_infers_unclassified_items() {
        let items = items_from_specs(
            vec!["rust@stable@rustup".parse().unwrap()],
            vec!["openssl@latest@apt-get".parse().unwrap()],
            vec!["firefox@latest@homebrew-cask".parse().unwrap()],
            vec![
                "stringer@latest@go".parse().unwrap(),
                "zed@latest@cask".parse().unwrap(),
            ],
        )
        .unwrap();

        let actual = items
            .into_iter()
            .map(|item| {
                (
                    item.kind,
                    item.spec.name,
                    item.spec.version.to_string(),
                    item.spec.backend.map(|backend| backend.to_string()),
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(
            actual,
            [
                (
                    ItemKind::Tool,
                    "rust".to_string(),
                    "stable".to_string(),
                    Some("rustup".to_string())
                ),
                (
                    ItemKind::Package,
                    "openssl".to_string(),
                    "latest".to_string(),
                    Some("apt-get".to_string())
                ),
                (
                    ItemKind::App,
                    "firefox".to_string(),
                    "latest".to_string(),
                    Some("homebrew-cask".to_string())
                ),
                (
                    ItemKind::Tool,
                    "stringer".to_string(),
                    "latest".to_string(),
                    Some("go".to_string())
                ),
                (
                    ItemKind::App,
                    "zed".to_string(),
                    "latest".to_string(),
                    Some("cask".to_string())
                )
            ]
        );
    }

    #[test]
    fn items_from_specs_rejects_ambiguous_unclassified_items() {
        let err = items_from_specs(
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec!["ripgrep@latest@brew".parse().unwrap()],
        )
        .unwrap_err();

        assert_eq!(
            err.to_string(),
            "cannot infer whether ripgrep@latest@brew is a tool, package, or app; use --tool, --package, or --app"
        );
    }

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
            tool: Default::default(),
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
            tool: Default::default(),
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
            tool: Default::default(),
        };

        let err = app_install_command(&item).unwrap_err();

        assert!(err.to_string().contains("app backend unknown-backend"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn app_install_command_plans_windows_scoop() {
        let item = InstallItemRequest {
            kind: ItemKind::App,
            spec: "firefox@latest@scoop".parse::<ItemSpec>().unwrap(),
            tool: Default::default(),
        };

        let command = app_install_command(&item).unwrap();

        assert_eq!(command.backend, "scoop");
        assert_eq!(command.program, "scoop");
        assert_eq!(command.args, ["install", "firefox"]);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn app_install_command_plans_macos_mas() {
        let item = InstallItemRequest {
            kind: ItemKind::App,
            spec: "497799835@latest@mas".parse::<ItemSpec>().unwrap(),
            tool: Default::default(),
        };

        let command = app_install_command(&item).unwrap();

        assert_eq!(command.backend, "mas");
        assert_eq!(command.program, "mas");
        assert_eq!(command.args, ["install", "497799835"]);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn app_install_command_rejects_pinned_cask_versions() {
        let item = InstallItemRequest {
            kind: ItemKind::App,
            spec: "firefox@121.0@homebrew-cask".parse::<ItemSpec>().unwrap(),
            tool: Default::default(),
        };

        let err = app_install_command(&item).unwrap_err();

        assert!(
            err.to_string()
                .contains("app backend homebrew-cask pinned version 121.0")
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn app_install_command_passes_pinned_versions_to_windows_backends() {
        let winget_item = InstallItemRequest {
            kind: ItemKind::App,
            spec: "Firefox@121.0@winget".parse::<ItemSpec>().unwrap(),
            tool: Default::default(),
        };
        let winget = app_install_command(&winget_item).unwrap();
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

        let scoop_item = InstallItemRequest {
            kind: ItemKind::App,
            spec: "firefox@121.0@scoop".parse::<ItemSpec>().unwrap(),
            tool: Default::default(),
        };
        let scoop = app_install_command(&scoop_item).unwrap();
        assert_eq!(scoop.args, ["install", "firefox@121.0"]);
    }

    #[test]
    fn app_install_path_uses_still_managed_receipt_storage() {
        let item = InstallItemRequest {
            kind: ItemKind::App,
            spec: "firefox@latest@flatpak".parse::<ItemSpec>().unwrap(),
            tool: Default::default(),
        };

        assert_eq!(
            app_install_path(&item),
            System::root_dir()
                .join("receipts/apps")
                .join("firefox")
                .join("latest")
        );
    }

    #[test]
    fn package_install_path_uses_still_managed_receipt_storage() {
        let item = InstallItemRequest {
            kind: ItemKind::Package,
            spec: "openssl@3@homebrew".parse::<ItemSpec>().unwrap(),
            tool: Default::default(),
        };

        assert_eq!(
            native_receipt_path(&item),
            System::root_dir()
                .join("receipts/packages")
                .join("openssl")
                .join("3")
        );
    }

    #[test]
    fn package_install_command_uses_platform_default_backend() {
        let item = InstallItemRequest {
            kind: ItemKind::Package,
            spec: "openssl".parse::<ItemSpec>().unwrap(),
            tool: Default::default(),
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
            tool: Default::default(),
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
            tool: Default::default(),
        };

        let err = package_install_command(&item).unwrap_err();

        assert!(err.to_string().contains("package backend unknown-backend"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn package_install_command_plans_windows_scoop() {
        let item = InstallItemRequest {
            kind: ItemKind::Package,
            spec: "openssl@latest@scoop".parse::<ItemSpec>().unwrap(),
            tool: Default::default(),
        };

        let command = package_install_command(&item).unwrap();

        assert_eq!(command.backend, "scoop");
        assert_eq!(command.program, "scoop");
        assert_eq!(command.args, ["install", "openssl"]);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn package_install_command_passes_pinned_versions_to_homebrew() {
        let item = InstallItemRequest {
            kind: ItemKind::Package,
            spec: "ripgrep@1@homebrew".parse::<ItemSpec>().unwrap(),
            tool: Default::default(),
        };

        let command = package_install_command(&item).unwrap();

        assert_eq!(command.args, ["install", "ripgrep@1"]);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn package_install_command_passes_pinned_versions_to_apt() {
        let item = InstallItemRequest {
            kind: ItemKind::Package,
            spec: "openssl@3@apt".parse::<ItemSpec>().unwrap(),
            tool: Default::default(),
        };

        let command = package_install_command(&item).unwrap();

        assert_eq!(command.args, ["install", "-y", "openssl=3"]);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn package_install_command_passes_pinned_versions_to_winget() {
        let item = InstallItemRequest {
            kind: ItemKind::Package,
            spec: "OpenSSL@3@winget".parse::<ItemSpec>().unwrap(),
            tool: Default::default(),
        };

        let command = package_install_command(&item).unwrap();
        assert_eq!(
            command.args,
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

    #[cfg(target_os = "linux")]
    #[test]
    fn package_install_command_rejects_pinned_versions_for_unsupported_backends() {
        let item = InstallItemRequest {
            kind: ItemKind::Package,
            spec: "openssl@3@dnf".parse::<ItemSpec>().unwrap(),
            tool: Default::default(),
        };

        let err = package_install_command(&item).unwrap_err();

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
            tool: Default::default(),
        };

        let command = tool_install_command(&item).unwrap();

        assert_eq!(command.program, "rustup");
        assert_eq!(command.args, ["toolchain", "install", "stable"]);
        assert!(command.env.iter().any(|(key, value)| {
            key == "RUSTUP_HOME" && value.ends_with("/tools/rust/stable/rustup")
        }));
        assert!(command.env.iter().any(|(key, value)| {
            key == "CARGO_HOME" && value.ends_with("/tools/rust/stable/cargo")
        }));
    }

    #[test]
    fn tool_install_command_plans_rustup_components_and_targets() {
        let item = InstallItemRequest {
            kind: ItemKind::Tool,
            spec: "rust@stable@rustup".parse::<ItemSpec>().unwrap(),
            tool: ToolInstallOptions {
                components: vec!["rustfmt".to_string(), "clippy".to_string()],
                targets: vec!["wasm32-unknown-unknown".to_string()],
            },
        };

        let command = tool_install_command(&item).unwrap();

        assert_eq!(command.args, ["toolchain", "install", "stable"]);
        assert!(command.env.iter().any(|(key, _)| key == "RUSTUP_HOME"));
        assert_eq!(
            command.after,
            [
                ToolInstallStep {
                    program: "rustup".to_string(),
                    args: vec![
                        "component".to_string(),
                        "add".to_string(),
                        "rustfmt".to_string(),
                        "clippy".to_string(),
                        "--toolchain".to_string(),
                        "stable".to_string()
                    ]
                },
                ToolInstallStep {
                    program: "rustup".to_string(),
                    args: vec![
                        "target".to_string(),
                        "add".to_string(),
                        "wasm32-unknown-unknown".to_string(),
                        "--toolchain".to_string(),
                        "stable".to_string()
                    ]
                }
            ]
        );
    }

    #[test]
    fn tool_install_command_rejects_tool_extras_for_non_rustup_backend() {
        let item = InstallItemRequest {
            kind: ItemKind::Tool,
            spec: "node@22@mise".parse::<ItemSpec>().unwrap(),
            tool: ToolInstallOptions {
                components: vec!["rustfmt".to_string()],
                targets: Vec::new(),
            },
        };

        let err = tool_install_command(&item).unwrap_err();

        assert!(
            err.to_string()
                .contains("tool backend mise components or targets")
        );
    }

    #[test]
    fn tool_install_command_plans_language_package_managers() {
        let cargo = tool_install_command(&InstallItemRequest {
            kind: ItemKind::Tool,
            spec: "cargo-nextest@0.9.99@cargo".parse::<ItemSpec>().unwrap(),
            tool: Default::default(),
        })
        .unwrap();
        let npm = tool_install_command(&InstallItemRequest {
            kind: ItemKind::Tool,
            spec: "typescript@5.8.0@npm".parse::<ItemSpec>().unwrap(),
            tool: Default::default(),
        })
        .unwrap();
        let pipx = tool_install_command(&InstallItemRequest {
            kind: ItemKind::Tool,
            spec: "ruff@0.11.0@pipx".parse::<ItemSpec>().unwrap(),
            tool: Default::default(),
        })
        .unwrap();
        let go = tool_install_command(&InstallItemRequest {
            kind: ItemKind::Tool,
            spec: "stringer@latest@go".parse::<ItemSpec>().unwrap(),
            tool: Default::default(),
        })
        .unwrap();

        assert_eq!(
            cargo.args,
            [
                "install",
                "--root",
                System::tool_dir()
                    .join("cargo-nextest")
                    .join("0.9.99")
                    .to_str()
                    .unwrap(),
                "cargo-nextest",
                "--version",
                "0.9.99"
            ]
        );
        assert_eq!(
            npm.args,
            [
                "install",
                "--global",
                "--prefix",
                System::tool_dir()
                    .join("typescript")
                    .join("5.8.0")
                    .to_str()
                    .unwrap(),
                "typescript@5.8.0"
            ]
        );
        assert_eq!(
            pipx.args,
            [
                "install",
                "--install-dir",
                System::tool_dir()
                    .join("ruff")
                    .join("0.11.0")
                    .join("venvs")
                    .to_str()
                    .unwrap(),
                "--bin-dir",
                System::tool_dir()
                    .join("ruff")
                    .join("0.11.0")
                    .join("bin")
                    .to_str()
                    .unwrap(),
                "ruff==0.11.0"
            ]
        );
        assert_eq!(go.args, ["install", "stringer@latest"]);
        assert!(go.env.iter().any(|(key, value)| {
            key == "GOBIN" && value.ends_with("/tools/stringer/latest/bin")
        }));
    }

    #[test]
    fn tool_install_command_plans_version_managers() {
        let mise = tool_install_command(&InstallItemRequest {
            kind: ItemKind::Tool,
            spec: "node@22@mise".parse::<ItemSpec>().unwrap(),
            tool: Default::default(),
        })
        .unwrap();
        let asdf = tool_install_command(&InstallItemRequest {
            kind: ItemKind::Tool,
            spec: "node@22@asdf".parse::<ItemSpec>().unwrap(),
            tool: Default::default(),
        })
        .unwrap();
        let aqua = tool_install_command(&InstallItemRequest {
            kind: ItemKind::Tool,
            spec: "ripgrep@14.1.1@aqua".parse::<ItemSpec>().unwrap(),
            tool: Default::default(),
        })
        .unwrap();

        assert_eq!(mise.args, ["install", "node@22"]);
        assert_eq!(asdf.args, ["install", "node", "22"]);
        assert_eq!(aqua.args, ["install", "ripgrep@14.1.1"]);
        assert!(mise.env.iter().any(|(key, _)| key == "MISE_DATA_DIR"));
        assert!(asdf.env.iter().any(|(key, _)| key == "ASDF_DATA_DIR"));
        assert!(aqua.env.iter().any(|(key, _)| key == "AQUA_ROOT_DIR"));
    }

    #[test]
    fn command_backed_tool_install_can_scope_backend_to_staging_path() {
        let item = InstallItemRequest {
            kind: ItemKind::Tool,
            spec: "typescript@5.8.0@npm".parse::<ItemSpec>().unwrap(),
            tool: Default::default(),
        };
        let staging_path = tool_staging_path(&item);

        let command = tool_install_command_at(&item, &staging_path).unwrap();

        assert_eq!(
            command.args,
            [
                "install",
                "--global",
                "--prefix",
                staging_path.to_str().unwrap(),
                "typescript@5.8.0"
            ]
        );
        assert!(staging_path.ends_with("typescript/.5.8.0.staging"));
    }

    #[tokio::test]
    async fn promote_staged_install_keeps_backup_until_finalized() {
        let temp = tempfile::tempdir().unwrap();
        let staging_path = temp.path().join(".latest.staging");
        let install_path = temp.path().join("latest");
        let backup_path = temp.path().join(".latest.previous");
        tokio::fs::create_dir_all(&staging_path).await.unwrap();
        tokio::fs::write(staging_path.join("new.txt"), "new")
            .await
            .unwrap();
        tokio::fs::create_dir_all(&install_path).await.unwrap();
        tokio::fs::write(install_path.join("old.txt"), "old")
            .await
            .unwrap();

        let had_previous = promote_staged_install(&staging_path, &install_path, &backup_path)
            .await
            .unwrap();

        assert!(had_previous);
        assert!(install_path.join("new.txt").is_file());
        assert!(!install_path.join("old.txt").exists());
        assert!(!staging_path.exists());
        assert!(backup_path.join("old.txt").is_file());
    }

    #[tokio::test]
    async fn rollback_promoted_install_restores_previous_install() {
        let temp = tempfile::tempdir().unwrap();
        let install_path = temp.path().join("latest");
        let backup_path = temp.path().join(".latest.previous");
        tokio::fs::create_dir_all(&install_path).await.unwrap();
        tokio::fs::write(install_path.join("new.txt"), "new")
            .await
            .unwrap();
        tokio::fs::create_dir_all(&backup_path).await.unwrap();
        tokio::fs::write(backup_path.join("old.txt"), "old")
            .await
            .unwrap();

        rollback_promoted_install(&install_path, &backup_path, true)
            .await
            .unwrap();

        assert!(install_path.join("old.txt").is_file());
        assert!(!install_path.join("new.txt").exists());
        assert!(!backup_path.exists());
    }

    #[tokio::test]
    async fn rollback_promoted_install_removes_new_install_without_previous() {
        let temp = tempfile::tempdir().unwrap();
        let install_path = temp.path().join("latest");
        let backup_path = temp.path().join(".latest.previous");
        tokio::fs::create_dir_all(&install_path).await.unwrap();
        tokio::fs::write(install_path.join("new.txt"), "new")
            .await
            .unwrap();

        rollback_promoted_install(&install_path, &backup_path, false)
            .await
            .unwrap();

        assert!(!install_path.exists());
        assert!(!backup_path.exists());
    }

    #[test]
    fn tool_install_command_rejects_unknown_backend() {
        let item = InstallItemRequest {
            kind: ItemKind::Tool,
            spec: "ripgrep@latest@unknown-backend"
                .parse::<ItemSpec>()
                .unwrap(),
            tool: Default::default(),
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
            tool: Default::default(),
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

    #[test]
    fn install_config_preflight_rejects_changed_existing_entries_before_write() {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("still.toml");
        std::fs::write(
            &config_path,
            r#"
            [tools]
            rust = "stable"
            "#,
        )
        .unwrap();
        let request = InstallRequest {
            items: vec![InstallItemRequest {
                kind: ItemKind::Tool,
                spec: "rust@1.76.0@rustup".parse::<ItemSpec>().unwrap(),
                tool: Default::default(),
            }],
        };

        let err = prepare_install_config_write_at(&request, false, false, temp.path(), temp.path())
            .unwrap_err();

        assert!(err.to_string().contains("--force"));
        assert_eq!(
            std::fs::read_to_string(config_path).unwrap(),
            r#"
            [tools]
            rust = "stable"
            "#
        );
    }

    #[test]
    fn install_config_preflight_uses_global_when_no_project_config_exists() {
        let temp = tempfile::tempdir().unwrap();
        let request = InstallRequest {
            items: vec![install_item(ItemKind::Package, "openssl")],
        };

        let pending =
            prepare_install_config_write_at(&request, false, false, temp.path(), temp.path())
                .unwrap();

        assert_eq!(pending.path, temp.path().join(".config/still/config.toml"));
        assert!(pending.updated.contains("latest = [\"openssl\"]"));
    }

    #[tokio::test]
    async fn install_recording_does_not_mutate_config_when_lockfile_backup_fails() {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("still.toml");
        let original = "[tools]\nrust = \"stable\"\n";
        std::fs::write(&config_path, original).unwrap();
        let lockfile_path = temp.path().join("still.lock.toml");
        std::fs::create_dir(&lockfile_path).unwrap();

        let err = record_install_items(PendingInstallConfigWrite {
            path: config_path.clone(),
            home_dir: temp.path().to_path_buf(),
            original: Some(original.to_string()),
            updated: "[tools]\nrust = \"stable\"\nnode = \"latest\"\n".to_string(),
        })
        .await
        .unwrap_err();

        assert!(!err.to_string().is_empty());
        assert_eq!(std::fs::read_to_string(config_path).unwrap(), original);
        assert!(lockfile_path.is_dir());
    }

    #[tokio::test]
    async fn install_recording_restores_lockfile_when_refresh_fails() {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("still.toml");
        let lockfile_path = temp.path().join("still.lock.toml");
        let original_config = "[tools]\nrust = \"stable\"\n";
        let original_lockfile = "# original lockfile\n";
        std::fs::write(&config_path, original_config).unwrap();
        std::fs::write(&lockfile_path, original_lockfile).unwrap();

        let err = record_install_items(PendingInstallConfigWrite {
            path: config_path.clone(),
            home_dir: temp.path().to_path_buf(),
            original: Some(original_config.to_string()),
            updated: "[tools]\nrust = {}\n".to_string(),
        })
        .await
        .unwrap_err();

        assert!(
            err.to_string()
                .contains("tool \"rust\" must define version")
        );
        assert_eq!(
            std::fs::read_to_string(config_path).unwrap(),
            original_config
        );
        assert_eq!(
            std::fs::read_to_string(lockfile_path).unwrap(),
            original_lockfile
        );
    }

    #[tokio::test]
    async fn install_restore_lockfile_puts_original_content_back() {
        let temp = tempfile::tempdir().unwrap();
        let lockfile_path = temp.path().join("still.lock.toml");
        std::fs::write(&lockfile_path, "# updated\n").unwrap();

        restore_install_lockfile(&lockfile_path, Some("# original\n".to_string()))
            .await
            .unwrap();

        assert_eq!(
            std::fs::read_to_string(lockfile_path).unwrap(),
            "# original\n"
        );
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
                return Err(EngineError::message("planned failure"));
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
            tool: Default::default(),
        }
    }
}
