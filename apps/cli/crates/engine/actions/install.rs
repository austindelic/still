//! Engine install action for classifying requests and delegating to source plans.

use std::path::{Path, PathBuf};

use crate::config::edit::add_install_items_with_force;
use crate::config::{ConfigScope, ConfigSelection, resolve_config_path};
use crate::error::{EngineContext, EngineError, Result};
use crate::install::layout::native_receipt_root;
use crate::lockfile::lockfile_path;
use crate::platform::HostPlatform;
use crate::resolve::{SourceIntent, SourceRegistry, infer_item_kind_from_backend};
use crate::specs::item::{ItemKind, ItemSpec};
use crate::specs::tool::ToolSpec;

/// One item requested for install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallItemRequest {
    pub kind: Option<ItemKind>,
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
#[derive(Debug, Clone)]
pub struct InstallRequest {
    pub items: Vec<InstallItemRequest>,
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
#[derive(Debug)]
pub struct InstallResult {
    pub tool_name: String,
    pub version: String,
    pub install_path: PathBuf,
    pub binary_path: Option<PathBuf>,
    pub outputs: Vec<PathBuf>,
    pub linked_executables: Vec<PathBuf>,
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

/// Request after engine-owned kind inference has completed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassifiedInstallItemRequest {
    pub kind: ItemKind,
    pub spec: ItemSpec,
    pub tool: ToolInstallOptions,
}

/// Installs individual items for the multi-item install coordinator.
pub trait ItemInstaller {
    async fn install(&mut self, item: &ClassifiedInstallItemRequest) -> Result<InstallResult>;
}

#[derive(Debug, Default)]
struct RealItemInstaller;

impl ItemInstaller for RealItemInstaller {
    async fn install(&mut self, item: &ClassifiedInstallItemRequest) -> Result<InstallResult> {
        let source = selected_source_for_item(item)?;
        Err(EngineError::SourceInstallNotImplemented {
            source_id: source,
            kind: item.kind.to_string(),
            name: item.spec.name.clone(),
            version: item.spec.version.to_string(),
        })
    }
}

/// Builds install item requests from grouped and unclassified specs.
pub fn items_from_specs(
    tools: Vec<ToolSpec>,
    packages: Vec<ToolSpec>,
    apps: Vec<ToolSpec>,
    unclassified: Vec<ToolSpec>,
) -> Result<Vec<InstallItemRequest>> {
    let mut items = Vec::new();
    push_spec_items(&mut items, Some(ItemKind::Tool), tools);
    push_spec_items(&mut items, Some(ItemKind::Package), packages);
    push_spec_items(&mut items, Some(ItemKind::App), apps);
    push_spec_items(&mut items, None, unclassified);
    Ok(items)
}

fn push_spec_items(
    items: &mut Vec<InstallItemRequest>,
    kind: Option<ItemKind>,
    specs: Vec<ToolSpec>,
) {
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

pub(crate) fn classify_install_items(
    items: &[InstallItemRequest],
) -> Result<Vec<ClassifiedInstallItemRequest>> {
    items
        .iter()
        .map(|item| {
            Ok(ClassifiedInstallItemRequest {
                kind: classify_install_item(item)?,
                spec: item.spec.clone(),
                tool: item.tool.clone(),
            })
        })
        .collect()
}

fn classify_install_item(item: &InstallItemRequest) -> Result<ItemKind> {
    if let Some(kind) = item.kind {
        return Ok(kind);
    }
    infer_spec_item_kind(&item.spec)
}

fn infer_spec_item_kind(spec: &ItemSpec) -> Result<ItemKind> {
    let Some(source) = &spec.backend else {
        return Err(EngineError::message(format!(
            "cannot infer whether {} is a tool, package, or app; use --tool, --package, or --app",
            spec.name
        )));
    };
    infer_item_kind_from_backend(source.as_str()).ok_or_else(|| {
        EngineError::message(format!(
            "cannot infer whether {}@{} from {} is a tool, package, or app; use --tool, --package, or --app",
            spec.name, spec.version, source
        ))
    })
}

/// Installs requested items.
pub async fn run(request: InstallRequest) -> Result<InstallResult> {
    let mut installer = RealItemInstaller;
    run_with_installer(request, &mut installer).await
}

/// Installs requested items, records them in the selected config, and refreshes the lockfile.
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
pub async fn run_with_installer(
    request: InstallRequest,
    installer: &mut impl ItemInstaller,
) -> Result<InstallResult> {
    run_with_installer_and_rollback_roots(request, installer, &RollbackRoots::system()).await
}

/// Removes installed outputs returned by a completed install request.
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
        return Err(EngineError::EmptyInstallRequest);
    }

    let items = classify_install_items(&request.items)?;
    let mut last = None;
    let mut installed = Vec::new();
    for item in &items {
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

fn selected_source_for_item(item: &ClassifiedInstallItemRequest) -> Result<String> {
    let intent =
        SourceIntent::from_optional(item.spec.backend.as_ref().map(|source| source.as_str()))?;
    let selection =
        SourceRegistry::bundled()
            .resolver()
            .resolve(item.kind, intent, HostPlatform::detect())?;
    selection
        .primary()
        .map(|source| source.to_string())
        .ok_or_else(|| EngineError::NoSourceCandidates {
            kind: item.kind.to_string(),
            platform: HostPlatform::detect().id().to_string(),
        })
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
            tool_root: native_receipt_root(ItemKind::Tool),
            package_root: native_receipt_root(ItemKind::Package),
            app_root: native_receipt_root(ItemKind::App),
            bin_dir: crate::install::layout::bin_dir(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items_from_specs_preserves_groups_and_defers_unclassified_items() {
        let items = items_from_specs(
            vec!["rust@stable@rustup".parse().unwrap()],
            vec!["openssl@latest@apt".parse().unwrap()],
            vec!["firefox@latest@flatpak".parse().unwrap()],
            vec!["stringer@latest@go".parse().unwrap()],
        )
        .unwrap();

        assert_eq!(items[0].kind, Some(ItemKind::Tool));
        assert_eq!(items[1].kind, Some(ItemKind::Package));
        assert_eq!(items[2].kind, Some(ItemKind::App));
        assert_eq!(items[3].kind, None);
    }

    #[test]
    fn engine_classifies_unclassified_items_from_obvious_sources() {
        let items = vec![
            install_item(None, "ripgrep@latest@cargo"),
            install_item(None, "openssl@latest@apt"),
            install_item(None, "firefox@latest@flatpak"),
        ];

        let classified = classify_install_items(&items).unwrap();

        assert_eq!(classified[0].kind, ItemKind::Tool);
        assert_eq!(classified[1].kind, ItemKind::Package);
        assert_eq!(classified[2].kind, ItemKind::App);
    }

    #[test]
    fn engine_rejects_ambiguous_unclassified_items() {
        let err =
            classify_install_items(&[install_item(None, "ripgrep@latest@homebrew")]).unwrap_err();

        assert!(err.to_string().contains("cannot infer whether"));
    }

    #[tokio::test]
    async fn run_rejects_empty_requests() {
        let err = run(InstallRequest { items: Vec::new() }).await.unwrap_err();

        assert!(matches!(err, EngineError::EmptyInstallRequest));
    }

    #[tokio::test]
    async fn run_with_installer_receives_classified_items() {
        let mut installer = FakeItemInstaller::default();
        let result = run_with_installer(
            InstallRequest {
                items: vec![install_item(None, "ripgrep@latest@cargo")],
            },
            &mut installer,
        )
        .await
        .unwrap();

        assert_eq!(installer.seen, vec![ItemKind::Tool]);
        assert_eq!(result.installed[0].kind, ItemKind::Tool);
    }

    #[cfg(feature = "sources")]
    #[tokio::test]
    async fn real_installer_reports_source_install_not_implemented() {
        let err = run(InstallRequest {
            items: vec![install_item(Some(ItemKind::Tool), "ripgrep@latest@cargo")],
        })
        .await
        .unwrap_err();

        assert!(matches!(
            err,
            EngineError::Context { source, .. }
                if matches!(
                    *source,
                    EngineError::SourceInstallNotImplemented { ref source_id, .. } if source_id == "cargo"
                )
        ));
    }

    #[derive(Default)]
    struct FakeItemInstaller {
        seen: Vec<ItemKind>,
    }

    impl ItemInstaller for FakeItemInstaller {
        async fn install(&mut self, item: &ClassifiedInstallItemRequest) -> Result<InstallResult> {
            self.seen.push(item.kind);
            let install_path = PathBuf::from("/tmp")
                .join(item.kind.to_string())
                .join(&item.spec.name)
                .join(item.spec.version.as_str());
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
    }

    fn install_item(kind: Option<ItemKind>, spec: &str) -> InstallItemRequest {
        InstallItemRequest {
            kind,
            spec: spec.parse().unwrap(),
            tool: Default::default(),
        }
    }
}
