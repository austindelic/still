//! Engine action for planning config synchronization.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::actions::install::{InstallItemRequest, InstallRequest};
use crate::config::{ConfigScope, ConfigSelection, global_config_path, resolve_config_path};
use crate::lockfile::{lockfile_path, render_merged_lockfile};
use crate::platform::{PlatformFilter, PlatformId, current_platform};
use crate::specs::backend::normalize_auto_backend;
use crate::specs::item::{ItemKind, ItemSpec};
use crate::specs::toml::{PackageEntry, PackageMap, StillConfig, ToolEntry, parse_still_toml};
use crate::system::System;
use crate::utils::paths::PathOps;

/// Request to synchronize installed state with config.
#[derive(Debug, Clone)]
pub struct SyncRequest {
    pub start_dir: PathBuf,
    pub home_dir: PathBuf,
    pub global: bool,
}

/// Sync report for desired install state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncResult {
    pub path: PathBuf,
    pub lockfile_path: PathBuf,
    pub items: Vec<SyncItem>,
    pub drift: Vec<SyncDrift>,
    pub missing: Vec<SyncItem>,
    pub installed: Vec<SyncItem>,
}

/// One desired item that sync should reconcile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncItem {
    pub kind: ItemKind,
    pub logical_name: String,
    pub spec: ItemSpec,
    pub desired_state: String,
}

/// Drift detected before writing the new lockfile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncDrift {
    LockfileMissing,
    LockfileOutdated,
}

/// Installs missing items selected by sync.
pub trait SyncInstaller {
    /// Installs the supplied missing items.
    /// # Errors
    /// Fails when backend resolution, download, extraction, linking, or app
    /// registration fails.
    async fn install(&mut self, items: Vec<InstallItemRequest>) -> Result<()>;
}

/// Production sync installer backed by the engine install action.
#[derive(Debug, Default)]
pub struct RealSyncInstaller;

impl SyncInstaller for RealSyncInstaller {
    async fn install(&mut self, items: Vec<InstallItemRequest>) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }
        super::install::run(InstallRequest { items }).await?;
        Ok(())
    }
}

/// Reconciles config, lockfile, and missing installs with the production installer.
/// # Errors
/// Fails when planning, lockfile writing, missing detection, or install execution fails.
pub async fn run(request: SyncRequest) -> Result<SyncResult> {
    let mut installer = RealSyncInstaller;
    run_with_installer(request, &mut installer).await
}

/// Reconciles config with an injected installer for tests and alternate frontends.
/// # Errors
/// Fails when planning, lockfile writing, missing detection, or install execution fails.
pub async fn run_with_installer(
    request: SyncRequest,
    installer: &mut impl SyncInstaller,
) -> Result<SyncResult> {
    let mut result = plan(request).await?;
    let to_install = install_requests(&result.missing);
    installer.install(to_install).await?;
    result.installed = result.missing.clone();
    result.missing = missing_items(&result.items).await?;
    Ok(result)
}

/// Rewrites the lockfile next to `config_path` from the current desired state.
/// # Errors
/// Fails when the config cannot be read, parsed, normalized, or written.
pub async fn refresh_lockfile(config_path: &Path) -> Result<PathBuf> {
    let items = sync_items_for_path(config_path).await?;
    let path = lockfile_path(config_path);
    let existing = read_optional_lockfile(&path).await?;
    let rendered = render_merged_lockfile(existing.as_deref(), &items);
    tokio::fs::write(&path, rendered)
        .await
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(path)
}

/// Reads config, writes a lockfile, and returns desired install state.
/// # Errors
/// Fails when config cannot be found, read, parsed, or normalized.
pub async fn plan(request: SyncRequest) -> Result<SyncResult> {
    let resolved = resolve_config_path(
        &request.start_dir,
        &request.home_dir,
        ConfigSelection {
            scope: if request.global {
                ConfigScope::Global
            } else {
                ConfigScope::Project
            },
            for_write: false,
        },
    )?;
    let items = if !request.global && resolved.scope == ConfigScope::Project {
        sync_items_for_active_project_path(&resolved.path, &request.home_dir).await?
    } else {
        sync_items_for_path(&resolved.path).await?
    };
    let lockfile_path = lockfile_path(&resolved.path);
    let existing_lockfile = read_optional_lockfile(&lockfile_path).await?;
    let rendered_lockfile = render_merged_lockfile(existing_lockfile.as_deref(), &items);
    let drift = lockfile_drift(existing_lockfile.as_deref(), &rendered_lockfile);
    let missing = missing_items(&items).await?;
    tokio::fs::write(&lockfile_path, rendered_lockfile)
        .await
        .with_context(|| format!("failed to write {}", lockfile_path.display()))?;

    Ok(SyncResult {
        path: resolved.path,
        lockfile_path,
        items,
        drift,
        missing,
        installed: Vec::new(),
    })
}

async fn sync_items_for_path(config_path: &Path) -> Result<Vec<SyncItem>> {
    let content = tokio::fs::read_to_string(config_path)
        .await
        .with_context(|| format!("failed to read {}", config_path.display()))?;
    let config = parse_still_toml(&content)?;
    sync_items(config)
}

async fn sync_items_for_active_project_path(
    project_path: &Path,
    home_dir: &Path,
) -> Result<Vec<SyncItem>> {
    let mut items = sync_items_for_path(project_path).await?;
    let global_path = global_config_path(home_dir);
    let global_items = match sync_items_for_path(&global_path).await {
        Ok(items) => items,
        Err(err)
            if err
                .downcast_ref::<std::io::Error>()
                .is_some_and(|io| io.kind() == std::io::ErrorKind::NotFound) =>
        {
            Vec::new()
        }
        Err(err) => return Err(err),
    };
    merge_global_only_items(&mut items, global_items);
    Ok(items)
}

fn merge_global_only_items(items: &mut Vec<SyncItem>, global_items: Vec<SyncItem>) {
    for global_item in global_items {
        if items.iter().any(|item| {
            item.kind == global_item.kind && item.logical_name == global_item.logical_name
        }) {
            continue;
        }
        items.push(global_item);
    }
}

fn install_requests(items: &[SyncItem]) -> Vec<InstallItemRequest> {
    items
        .iter()
        .map(|item| InstallItemRequest {
            kind: item.kind,
            spec: item.spec.clone(),
        })
        .collect()
}

async fn read_optional_lockfile(path: &std::path::Path) -> Result<Option<String>> {
    match tokio::fs::read_to_string(path).await {
        Ok(content) => Ok(Some(content)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).with_context(|| format!("failed to read {}", path.display())),
    }
}

fn lockfile_drift(existing: Option<&str>, desired: &str) -> Vec<SyncDrift> {
    match existing {
        Some(existing) if existing == desired => Vec::new(),
        Some(_) => vec![SyncDrift::LockfileOutdated],
        None => vec![SyncDrift::LockfileMissing],
    }
}

async fn missing_items(items: &[SyncItem]) -> Result<Vec<SyncItem>> {
    let mut missing = Vec::new();
    for item in items {
        if tokio::fs::metadata(installed_path(item)).await.is_err() {
            missing.push(item.clone());
        }
    }
    Ok(missing)
}

fn installed_path(item: &SyncItem) -> PathBuf {
    let root = match item.kind {
        ItemKind::Tool => System::tool_dir(),
        ItemKind::Package => System::root_dir().join("packages"),
        ItemKind::App => System::apps_dir(),
    };
    root.join(&item.spec.name).join(item.spec.version.as_str())
}

fn sync_items(config: StillConfig) -> Result<Vec<SyncItem>> {
    let platform = current_platform();
    let mut items = Vec::new();
    for (name, entry) in config.tools {
        let desired_state = format!("tool:{name}:{entry:?}");
        items.push(SyncItem {
            kind: ItemKind::Tool,
            logical_name: name.clone(),
            spec: tool_spec(name, entry, platform)?,
            desired_state,
        });
    }
    items.extend(package_items(ItemKind::Package, config.packages, platform)?);
    items.extend(package_items(ItemKind::App, config.apps, platform)?);
    Ok(items)
}

fn tool_spec(name: String, entry: ToolEntry, platform: PlatformId) -> Result<ItemSpec> {
    match entry {
        ToolEntry::Version(version) => item_spec(name, version, None),
        ToolEntry::Expanded(tool) => {
            let version = if tool.version.is_empty() {
                "latest".to_string()
            } else {
                tool.version
            };
            item_spec(
                name,
                version,
                backend_for_platform(ItemKind::Tool, tool.backend, tool.backends, platform)?,
            )
        }
    }
}

fn package_items(kind: ItemKind, map: PackageMap, platform: PlatformId) -> Result<Vec<SyncItem>> {
    let mut items = Vec::new();
    for name in map.latest {
        let desired_state = format!("{kind}:{name}:latest");
        items.push(SyncItem {
            kind,
            logical_name: name.clone(),
            spec: item_spec(name, "latest".to_string(), None)?,
            desired_state,
        });
    }

    for (name, entry) in map.entries {
        let desired_state = format!("{kind}:{name}:{entry:?}");
        let PackageEntry::Expanded(package) = entry;
        let filter = PlatformFilter::from_config(
            package.platforms.as_deref().unwrap_or(&[]),
            package.ignore.as_deref(),
            package.only.as_deref(),
        )?;
        if !filter.matches(platform) {
            continue;
        }

        let logical_name = name.clone();
        let resolved_name = name_for_platform(name, package.names, platform)?;
        items.push(SyncItem {
            kind,
            logical_name: logical_name.clone(),
            spec: item_spec(
                resolved_name,
                package.version.unwrap_or_else(|| "latest".to_string()),
                backend_for_platform(kind, package.backend, package.backends, platform)?,
            )?,
            desired_state,
        });
    }
    Ok(items)
}

fn name_for_platform(
    name: String,
    names: std::collections::BTreeMap<String, String>,
    platform: PlatformId,
) -> Result<String> {
    for (key, value) in names {
        let key_platform: PlatformId = key.parse()?;
        if key_platform == platform {
            return Ok(value);
        }
    }
    Ok(name)
}

fn backend_for_platform(
    kind: ItemKind,
    backend: Option<String>,
    backends: std::collections::BTreeMap<String, String>,
    platform: PlatformId,
) -> Result<Option<String>> {
    for (key, value) in backends {
        let key_platform: PlatformId = key.parse()?;
        if key_platform == platform {
            return Ok(normalize_auto_backend(kind, Some(value), platform));
        }
    }
    Ok(normalize_auto_backend(kind, backend, platform))
}

fn item_spec(name: String, version: String, backend: Option<String>) -> Result<ItemSpec> {
    match backend {
        Some(backend) => format!("{name}@{version}@{backend}").parse(),
        None => format!("{name}@{version}").parse(),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[derive(Default)]
    struct FakeInstaller {
        installed: Vec<InstallItemRequest>,
    }

    impl SyncInstaller for FakeInstaller {
        async fn install(&mut self, items: Vec<InstallItemRequest>) -> Result<()> {
            self.installed.extend(items);
            Ok(())
        }
    }

    #[tokio::test]
    async fn sync_plans_tools_packages_and_apps() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("still.toml"),
            r#"
            [tools]
            jq = "latest"
            rust = { version = "stable", backend = "rustup" }

            [packages]
            latest = ["openssl"]
            llvm = { version = "18", backend = "homebrew" }

            [apps]
            latest = ["firefox"]
            "#,
        )
        .unwrap();

        let result = plan(SyncRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
        })
        .await
        .unwrap();

        assert_eq!(result.path, temp.path().join("still.toml"));
        assert_eq!(result.lockfile_path, temp.path().join("still.lock.toml"));
        assert_eq!(result.drift, [SyncDrift::LockfileMissing]);
        assert!(temp.path().join("still.lock.toml").is_file());
        assert!(
            result
                .items
                .iter()
                .any(|item| { item.kind == ItemKind::Tool && item.spec.name == "rust" })
        );
        assert!(
            result
                .items
                .iter()
                .any(|item| { item.kind == ItemKind::Package && item.spec.name == "openssl" })
        );
        assert!(
            result
                .items
                .iter()
                .any(|item| { item.kind == ItemKind::App && item.spec.name == "firefox" })
        );
    }

    #[tokio::test]
    async fn sync_skips_items_for_other_platforms() {
        let temp = tempfile::tempdir().unwrap();
        let other_platform = if cfg!(target_os = "windows") {
            "linux"
        } else {
            "windows"
        };
        fs::write(
            temp.path().join("still.toml"),
            format!(
                r#"
                [packages.skip-me]
                version = "latest"
                only = "{other_platform}"
                "#
            ),
        )
        .unwrap();

        let result = plan(SyncRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
        })
        .await
        .unwrap();

        assert!(result.items.is_empty());
        assert_eq!(result.drift, [SyncDrift::LockfileMissing]);
    }

    #[tokio::test]
    async fn sync_only_prepares_services_without_running_them() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("still.toml"),
            r#"
            [services]
            web = "exit 99"
            "#,
        )
        .unwrap();

        let result = plan(SyncRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
        })
        .await
        .unwrap();

        assert!(result.items.is_empty());
        assert_eq!(result.drift, [SyncDrift::LockfileMissing]);
    }

    #[tokio::test]
    async fn sync_uses_platform_specific_backend_overrides() {
        let temp = tempfile::tempdir().unwrap();
        let platform = current_platform().to_string();
        fs::write(
            temp.path().join("still.toml"),
            format!(
                r#"
                [tools.rust]
                version = "stable"
                backend = "rustup"
                backends = {{ {platform} = "mise" }}

                [packages.openssl]
                version = "3"
                backend = "homebrew"
                backends = {{ {platform} = "apt" }}
                "#
            ),
        )
        .unwrap();

        let result = plan(SyncRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
        })
        .await
        .unwrap();

        assert!(result.items.iter().any(|item| {
            item.kind == ItemKind::Tool
                && item.spec.name == "rust"
                && item.spec.backend.as_ref().unwrap().as_str() == "mise"
        }));
        assert!(result.items.iter().any(|item| {
            item.kind == ItemKind::Package
                && item.spec.name == "openssl"
                && item.spec.backend.as_ref().unwrap().as_str() == "apt"
        }));
    }

    #[tokio::test]
    async fn sync_resolves_literal_auto_backends() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("still.toml"),
            r#"
            [tools.rust]
            version = "stable"
            backend = "auto"

            [packages.openssl]
            version = "3"
            backend = "auto"

            [apps.firefox]
            version = "latest"
            backend = "auto"
            "#,
        )
        .unwrap();

        let result = plan(SyncRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
        })
        .await
        .unwrap();

        assert!(result.items.iter().any(|item| {
            item.kind == ItemKind::Tool
                && item.spec.name == "rust"
                && item.spec.backend.as_ref().unwrap().as_str() == "homebrew"
        }));
        assert!(result.items.iter().any(|item| {
            item.kind == ItemKind::Package
                && item.spec.name == "openssl"
                && item.spec.backend.as_ref().unwrap().as_str()
                    == crate::specs::backend::default_backend(ItemKind::Package, current_platform())
        }));
        assert!(result.items.iter().any(|item| {
            item.kind == ItemKind::App
                && item.spec.name == "firefox"
                && item.spec.backend.as_ref().unwrap().as_str()
                    == crate::specs::backend::default_backend(ItemKind::App, current_platform())
        }));
    }

    #[tokio::test]
    async fn sync_items_preserve_tool_metadata_for_lockfile_checksums() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("still.toml"),
            r#"
            [tools.rust]
            version = "stable"
            backend = "rustup"
            components = ["rustfmt", "clippy"]
            targets = ["wasm32-unknown-unknown"]
            "#,
        )
        .unwrap();

        let result = plan(SyncRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
        })
        .await
        .unwrap();

        let rust = result
            .items
            .iter()
            .find(|item| item.kind == ItemKind::Tool && item.spec.name == "rust")
            .unwrap();
        assert!(rust.desired_state.contains("rustfmt"));
        assert!(rust.desired_state.contains("wasm32-unknown-unknown"));
    }

    #[tokio::test]
    async fn sync_uses_platform_specific_package_names() {
        let temp = tempfile::tempdir().unwrap();
        let platform = current_platform().to_string();
        fs::write(
            temp.path().join("still.toml"),
            format!(
                r#"
                [packages.fd]
                version = "latest"
                names = {{ {platform} = "fd-find" }}

                [apps.browser]
                version = "latest"
                names = {{ {platform} = "firefox" }}
                "#
            ),
        )
        .unwrap();

        let result = plan(SyncRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
        })
        .await
        .unwrap();

        assert!(
            result
                .items
                .iter()
                .any(|item| { item.kind == ItemKind::Package && item.spec.name == "fd-find" })
        );
        assert!(
            result
                .items
                .iter()
                .any(|item| { item.kind == ItemKind::App && item.spec.name == "firefox" })
        );
        assert!(!result.items.iter().any(|item| item.spec.name == "browser"));
    }

    #[tokio::test]
    async fn sync_writes_lockfile() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("still.toml"),
            "[tools]\nrust = \"stable\"\n",
        )
        .unwrap();

        let result = plan(SyncRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
        })
        .await
        .unwrap();

        let lockfile = fs::read_to_string(result.lockfile_path).unwrap();
        assert!(lockfile.contains("name = \"rust\""));
        assert!(lockfile.contains("version = \"stable\""));
    }

    #[tokio::test]
    async fn sync_reports_missing_desired_items() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("still.toml"),
            "[tools]\nstill-test-definitely-missing = \"0.0.1\"\n",
        )
        .unwrap();

        let result = plan(SyncRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
        })
        .await
        .unwrap();

        assert_eq!(result.missing.len(), 1);
        assert_eq!(result.missing[0].spec.name, "still-test-definitely-missing");
    }

    #[tokio::test]
    async fn sync_global_forces_global_config() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("still.toml"),
            "[tools]\nrust = \"stable\"\n",
        )
        .unwrap();
        let global = temp.path().join(".config/still/config.toml");
        fs::create_dir_all(global.parent().unwrap()).unwrap();
        fs::write(&global, "[tools]\nnode = \"22\"\n").unwrap();

        let result = plan(SyncRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: true,
        })
        .await
        .unwrap();

        assert_eq!(result.path, global);
        assert!(
            result
                .items
                .iter()
                .any(|item| item.kind == ItemKind::Tool && item.spec.name == "node")
        );
        assert!(!result.items.iter().any(|item| item.spec.name == "rust"));
    }

    #[tokio::test]
    async fn sync_project_state_includes_global_only_items() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("repo");
        let global = temp.path().join(".config/still/config.toml");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(global.parent().unwrap()).unwrap();
        fs::write(project.join("still.toml"), "[tools]\nrust = \"stable\"\n").unwrap();
        fs::write(&global, "[tools]\nnode = \"22\"\n").unwrap();

        let result = plan(SyncRequest {
            start_dir: project.clone(),
            home_dir: temp.path().to_path_buf(),
            global: false,
        })
        .await
        .unwrap();

        assert_eq!(result.path, project.join("still.toml"));
        assert!(
            result
                .items
                .iter()
                .any(|item| item.kind == ItemKind::Tool && item.spec.name == "rust")
        );
        assert!(
            result
                .items
                .iter()
                .any(|item| item.kind == ItemKind::Tool && item.spec.name == "node")
        );
    }

    #[tokio::test]
    async fn sync_project_items_override_global_items() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("repo");
        let global = temp.path().join(".config/still/config.toml");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(global.parent().unwrap()).unwrap();
        fs::write(project.join("still.toml"), "[tools]\nrust = \"stable\"\n").unwrap();
        fs::write(&global, "[tools]\nrust = \"1.80.0\"\nnode = \"22\"\n").unwrap();

        let result = plan(SyncRequest {
            start_dir: project,
            home_dir: temp.path().to_path_buf(),
            global: false,
        })
        .await
        .unwrap();

        let rust_items = result
            .items
            .iter()
            .filter(|item| item.kind == ItemKind::Tool && item.spec.name == "rust")
            .collect::<Vec<_>>();
        assert_eq!(rust_items.len(), 1);
        assert_eq!(rust_items[0].spec.version.as_str(), "stable");
        assert!(
            result
                .items
                .iter()
                .any(|item| item.kind == ItemKind::Tool && item.spec.name == "node")
        );
    }

    #[tokio::test]
    async fn sync_project_items_override_global_items_by_logical_name() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("repo");
        let global = temp.path().join(".config/still/config.toml");
        let platform = current_platform().to_string();
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(global.parent().unwrap()).unwrap();
        fs::write(
            project.join("still.toml"),
            format!(
                r#"
                [packages.fd]
                version = "latest"
                names = {{ {platform} = "fd-find" }}
                "#
            ),
        )
        .unwrap();
        fs::write(&global, "[packages]\nlatest = [\"fd\"]\n").unwrap();

        let result = plan(SyncRequest {
            start_dir: project,
            home_dir: temp.path().to_path_buf(),
            global: false,
        })
        .await
        .unwrap();

        let fd_items = result
            .items
            .iter()
            .filter(|item| item.kind == ItemKind::Package && item.logical_name == "fd")
            .collect::<Vec<_>>();
        assert_eq!(fd_items.len(), 1);
        assert_eq!(fd_items[0].spec.name, "fd-find");
    }

    #[tokio::test]
    async fn sync_installs_missing_items_through_installer() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("still.toml"),
            "[tools]\nstill-test-definitely-missing = \"0.0.1\"\n",
        )
        .unwrap();
        let mut installer = FakeInstaller::default();

        let result = run_with_installer(
            SyncRequest {
                start_dir: temp.path().to_path_buf(),
                home_dir: temp.path().to_path_buf(),
                global: false,
            },
            &mut installer,
        )
        .await
        .unwrap();

        assert_eq!(installer.installed.len(), 1);
        assert_eq!(
            installer.installed[0].spec.name,
            "still-test-definitely-missing"
        );
        assert_eq!(result.installed.len(), 1);
    }

    #[tokio::test]
    async fn sync_reports_no_lockfile_drift_after_repeated_run() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("still.toml"),
            "[tools]\nrust = \"stable\"\n",
        )
        .unwrap();

        plan(SyncRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
        })
        .await
        .unwrap();
        let result = plan(SyncRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
        })
        .await
        .unwrap();

        assert!(result.drift.is_empty());
    }

    #[tokio::test]
    async fn sync_reports_outdated_lockfile_drift() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("still.toml"),
            "[tools]\nrust = \"stable\"\n",
        )
        .unwrap();
        fs::write(temp.path().join("still.lock.toml"), "old\n").unwrap();

        let result = plan(SyncRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
        })
        .await
        .unwrap();

        assert_eq!(result.drift, [SyncDrift::LockfileOutdated]);
    }
}
