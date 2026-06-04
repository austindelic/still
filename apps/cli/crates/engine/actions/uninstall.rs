//! Engine action for removing desired install state.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::error::{EngineContext, Result};
use serde::Deserialize;

use crate::actions::sync::refresh_active_lockfile;
use crate::config::{ConfigScope, ConfigSelection, resolve_config_path};
use crate::config_edit::{RemoveItemTarget, remove_item, remove_item_target};
use crate::error::EngineError;
use crate::lockfile::lockfile_path;
use crate::platform::{PlatformId, current_platform};
use crate::specs::item::{BackendId, ItemKind};
use crate::specs::toml::{PackageEntry, PackageMap, parse_still_toml};
use crate::system::System;
use crate::utils::paths::PathOps;

/// Request to remove one configured item.
#[derive(Debug, Clone)]
pub struct UninstallRequest {
    pub start_dir: PathBuf,
    pub home_dir: PathBuf,
    pub global: bool,
    pub target: UninstallTarget,
}

/// Item target supplied by uninstall.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UninstallTarget {
    pub kind: Option<ItemKind>,
    pub name: String,
    pub version: String,
    pub backend: Option<BackendId>,
    pub exact: bool,
}

/// Desired-state removal result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UninstallResult {
    pub path: PathBuf,
    pub kind: ItemKind,
    pub name: String,
    pub removed_paths: Vec<PathBuf>,
}

/// Removes an item from `still.toml`.
/// # Errors
/// Fails when config cannot be read/written or the item is not configured.
pub async fn run(request: UninstallRequest) -> Result<UninstallResult> {
    let resolved = resolve_config_path(
        &request.start_dir,
        &request.home_dir,
        ConfigSelection {
            scope: if request.global {
                ConfigScope::Global
            } else {
                ConfigScope::Project
            },
            for_write: request.global,
        },
    )?;
    let content = tokio::fs::read_to_string(&resolved.path)
        .await
        .with_context(|| format!("failed to read {}", resolved.path.display()))?;
    let (updated, removed) = if request.target.exact {
        remove_item_target(
            &content,
            &RemoveItemTarget {
                kind: request.target.kind,
                name: request.target.name.clone(),
                version: request.target.version.clone(),
                backend: request.target.backend.clone(),
            },
        )?
    } else {
        remove_item(&content, request.target.kind, &request.target.name)?
    };
    let Some(kind) = removed else {
        return Err(EngineError::Conflict {
            message: format!("{} is not configured", request.target.name),
        }
        .into());
    };
    let artifact_targets = artifact_targets_for_config(&content, kind, &request.target)?;
    let lockfile_path = lockfile_path(&resolved.path);
    let original_lockfile = read_optional_file(&lockfile_path).await?;

    tokio::fs::write(&resolved.path, updated)
        .await
        .with_context(|| format!("failed to write {}", resolved.path.display()))?;
    if let Err(err) = refresh_active_lockfile(&resolved.path, &request.home_dir).await {
        restore_desired_state(&resolved.path, &content, &lockfile_path, original_lockfile).await?;
        return Err(err);
    }
    let removed_paths = remove_installed_artifacts(kind, &artifact_targets).await?;

    Ok(UninstallResult {
        path: resolved.path,
        kind,
        name: request.target.name,
        removed_paths,
    })
}

async fn remove_installed_artifacts(
    kind: ItemKind,
    targets: &[UninstallTarget],
) -> Result<Vec<PathBuf>> {
    let mut removed = Vec::new();
    let mut seen = BTreeSet::new();
    for target in targets {
        for path in managed_artifact_paths(kind, target).await? {
            if !seen.insert(path.clone()) || tokio::fs::symlink_metadata(&path).await.is_err() {
                continue;
            }
            remove_path(&path).await?;
            removed.push(path);
        }
    }
    Ok(removed)
}

async fn read_optional_file(path: &Path) -> Result<Option<String>> {
    match tokio::fs::read_to_string(path).await {
        Ok(content) => Ok(Some(content)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).with_context(|| format!("failed to read {}", path.display())),
    }
}

async fn restore_desired_state(
    config_path: &Path,
    original_config: &str,
    lockfile_path: &Path,
    original_lockfile: Option<String>,
) -> Result<()> {
    tokio::fs::write(config_path, original_config)
        .await
        .with_context(|| format!("failed to restore {}", config_path.display()))?;
    match original_lockfile {
        Some(content) => {
            tokio::fs::write(lockfile_path, content)
                .await
                .with_context(|| format!("failed to restore {}", lockfile_path.display()))?;
        }
        None => match tokio::fs::remove_file(lockfile_path).await {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("failed to remove {}", lockfile_path.display()));
            }
        },
    }
    Ok(())
}

fn artifact_targets_for_config(
    content: &str,
    kind: ItemKind,
    target: &UninstallTarget,
) -> Result<Vec<UninstallTarget>> {
    let mut targets = vec![target.clone()];
    let Some(resolved_name) = resolved_artifact_name(content, kind, &target.name)? else {
        return Ok(targets);
    };
    if resolved_name != target.name {
        let mut resolved = target.clone();
        resolved.name = resolved_name;
        targets.push(resolved);
    }
    Ok(targets)
}

fn resolved_artifact_name(content: &str, kind: ItemKind, name: &str) -> Result<Option<String>> {
    let config = parse_still_toml(content)?;
    let map = match kind {
        ItemKind::Tool => return Ok(None),
        ItemKind::Package => config.packages,
        ItemKind::App => config.apps,
    };
    Ok(package_name_for_current_platform(&map, name))
}

fn package_name_for_current_platform(map: &PackageMap, name: &str) -> Option<String> {
    let PackageEntry::Expanded(package) = map.entries.get(name)?;
    let platform = current_platform();
    package.names.iter().find_map(|(key, value)| {
        let key_platform: PlatformId = key.parse().ok()?;
        (key_platform == platform).then(|| value.clone())
    })
}

async fn managed_artifact_paths(kind: ItemKind, target: &UninstallTarget) -> Result<Vec<PathBuf>> {
    managed_artifact_paths_at(kind, target, &install_root(kind), &System::bin_dir()).await
}

async fn managed_artifact_paths_at(
    kind: ItemKind,
    target: &UninstallTarget,
    install_root: &Path,
    bin_dir: &Path,
) -> Result<Vec<PathBuf>> {
    let name = target.name.as_str();
    let item_root = install_root.join(name);
    let mut paths = BTreeSet::new();
    let mut entries = match tokio::fs::read_dir(&item_root).await {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err.into()),
    };

    while let Some(entry) = entries.next_entry().await? {
        let install_path = entry.path();
        if !entry.metadata().await?.is_dir() {
            continue;
        }
        let marker = match read_install_marker(&install_path).await? {
            Some(marker) if marker.matches(kind, target) => marker,
            Some(_) | None => continue,
        };
        paths.insert(install_path.clone());
        for output in marker.outputs {
            let path = PathBuf::from(output);
            if is_safe_install_output(install_root, name, &path) {
                paths.insert(path);
            }
        }
        for linked in marker.linked_executables {
            let path = PathBuf::from(linked);
            if is_safe_link_path(bin_dir, &path) {
                paths.insert(path);
            }
        }
    }

    let mut paths = paths.into_iter().collect::<Vec<_>>();
    paths.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    Ok(paths)
}

async fn read_install_marker(install_path: &Path) -> Result<Option<InstallMarker>> {
    let marker_path = install_path.join("install.toml");
    let content = match tokio::fs::read_to_string(&marker_path).await {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err.into()),
    };
    let marker = toml_edit::de::from_str(&content).with_context(|| {
        format!(
            "failed to parse Still install marker {}",
            marker_path.display()
        )
    })?;
    Ok(Some(marker))
}

#[derive(Debug, Deserialize)]
struct InstallMarker {
    kind: String,
    name: String,
    #[serde(default = "latest_version")]
    version: String,
    #[serde(default)]
    backend: Option<String>,
    #[serde(default)]
    outputs: Vec<String>,
    #[serde(default)]
    linked_executables: Vec<String>,
}

fn latest_version() -> String {
    "latest".to_string()
}

impl InstallMarker {
    fn matches(&self, kind: ItemKind, target: &UninstallTarget) -> bool {
        self.kind == kind.to_string()
            && self.name == target.name
            && (!target.exact
                || (self.version == target.version
                    && target
                        .backend
                        .as_ref()
                        .is_none_or(|backend| self.backend.as_deref() == Some(backend.as_str()))))
    }
}

fn is_safe_install_output(install_root: &Path, name: &str, path: &Path) -> bool {
    path.starts_with(install_root.join(name))
}

fn is_safe_link_path(bin_dir: &Path, path: &Path) -> bool {
    path.starts_with(bin_dir)
}

fn install_root(kind: ItemKind) -> PathBuf {
    let install_root = match kind {
        ItemKind::Tool => System::tool_dir(),
        ItemKind::Package => System::root_dir().join("receipts").join("packages"),
        ItemKind::App => System::root_dir().join("receipts").join("apps"),
    };
    install_root
}

async fn remove_path(path: &PathBuf) -> Result<()> {
    let metadata = tokio::fs::symlink_metadata(path).await?;
    if metadata.is_dir() {
        tokio::fs::remove_dir_all(path).await?;
    } else {
        tokio::fs::remove_file(path).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::specs::toml::parse_still_toml;

    use super::*;

    #[tokio::test]
    async fn uninstall_removes_configured_package() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("still.toml");
        fs::write(&path, "[packages]\nlatest = [\"openssl\", \"llvm\"]\n").unwrap();

        let result = run(UninstallRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
            target: target("openssl"),
        })
        .await
        .unwrap();

        assert_eq!(result.kind, ItemKind::Package);
        let config = parse_still_toml(&fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(config.packages.latest, ["llvm"]);
        let lockfile = fs::read_to_string(temp.path().join("still.lock.toml")).unwrap();
        assert!(lockfile.contains("name = \"llvm\""));
        assert!(!lockfile.contains("name = \"openssl\""));
    }

    #[tokio::test]
    async fn uninstall_does_not_change_config_when_lockfile_cannot_be_backed_up() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("still.toml");
        let original = "[packages]\nlatest = [\"openssl\", \"llvm\"]\n";
        fs::write(&path, original).unwrap();
        let lockfile_path = temp.path().join("still.lock.toml");
        fs::create_dir(&lockfile_path).unwrap();

        let err = run(UninstallRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
            target: target("openssl"),
        })
        .await
        .unwrap_err();

        assert!(err.to_string().contains("still.lock.toml"));
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
        assert!(lockfile_path.is_dir());
    }

    #[tokio::test]
    async fn restore_desired_state_restores_config_and_lockfile() {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("still.toml");
        let lockfile_path = temp.path().join("still.lock.toml");
        fs::write(&config_path, "[packages]\nlatest = []\n").unwrap();
        fs::write(&lockfile_path, "# updated\n").unwrap();

        restore_desired_state(
            &config_path,
            "[packages]\nlatest = [\"openssl\"]\n",
            &lockfile_path,
            Some("# original\n".to_string()),
        )
        .await
        .unwrap();

        assert_eq!(
            fs::read_to_string(&config_path).unwrap(),
            "[packages]\nlatest = [\"openssl\"]\n"
        );
        assert_eq!(fs::read_to_string(&lockfile_path).unwrap(), "# original\n");
    }

    #[tokio::test]
    async fn uninstall_exact_target_removes_matching_version_only() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("still.toml");
        fs::write(
            &path,
            r#"
            [packages]
            latest = ["llvm"]
            openssl = { version = "3", backend = "homebrew" }
            "#,
        )
        .unwrap();

        let result = run(UninstallRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
            target: exact_target("openssl", "3", "homebrew"),
        })
        .await
        .unwrap();

        assert_eq!(result.kind, ItemKind::Package);
        let config = parse_still_toml(&fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(config.packages.latest, ["llvm"]);
        assert!(!config.packages.entries.contains_key("openssl"));
    }

    #[tokio::test]
    async fn uninstall_exact_latest_removes_keyed_app_with_backend() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("still.toml");
        fs::write(
            &path,
            r#"
            [apps]
            firefox = { backend = "homebrew-cask" }
            "#,
        )
        .unwrap();
        let mut target = target("firefox");
        target.kind = Some(ItemKind::App);
        target.exact = true;

        let result = run(UninstallRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
            target,
        })
        .await
        .unwrap();

        assert_eq!(result.kind, ItemKind::App);
        let config = parse_still_toml(&fs::read_to_string(path).unwrap()).unwrap();
        assert!(!config.apps.entries.contains_key("firefox"));
    }

    #[test]
    fn uninstall_artifact_targets_include_platform_specific_package_name() {
        let platform = current_platform().to_string();
        let config = format!(
            r#"
            [packages.fd]
            version = "latest"
            names = {{ {platform} = "fd-find" }}
            "#
        );

        let targets =
            artifact_targets_for_config(&config, ItemKind::Package, &target("fd")).unwrap();

        assert_eq!(targets.len(), 2);
        assert!(targets.iter().any(|target| target.name == "fd"));
        assert!(targets.iter().any(|target| target.name == "fd-find"));
    }

    #[tokio::test]
    async fn uninstall_explicit_kind_only_removes_that_kind() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("still.toml");
        fs::write(
            &path,
            r#"
            [tools]
            zed = "latest"

            [apps]
            latest = ["zed"]
            "#,
        )
        .unwrap();

        let mut target = target("zed");
        target.kind = Some(ItemKind::App);
        let result = run(UninstallRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
            target,
        })
        .await
        .unwrap();

        assert_eq!(result.kind, ItemKind::App);
        let config = parse_still_toml(&fs::read_to_string(path).unwrap()).unwrap();
        assert!(config.tools.contains_key("zed"));
        assert!(config.apps.latest.is_empty());
    }

    #[tokio::test]
    async fn uninstall_errors_for_unknown_item() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("still.toml"),
            "[tools]\nrust = \"stable\"\n",
        )
        .unwrap();

        let err = run(UninstallRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
            target: target("node"),
        })
        .await
        .unwrap_err();

        assert!(err.to_string().contains("not configured"));
    }

    #[tokio::test]
    async fn managed_artifact_paths_reads_marker_owned_paths() {
        let temp = tempfile::tempdir().unwrap();
        let install_root = temp.path().join("packages");
        let bin_dir = temp.path().join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        let install_path = install_root.join("openssl").join("latest");
        let link_path = bin_dir.join("openssl");
        fs::create_dir_all(&install_path).unwrap();
        fs::write(&link_path, "link").unwrap();
        fs::write(
            install_path.join("install.toml"),
            format!(
                r#"
                kind = "package"
                name = "openssl"
                version = "latest"
                backend = "test"
                install_path = "{}"
                outputs = ["{}"]
                linked_executables = ["{}"]
                "#,
                install_path.display(),
                install_path.display(),
                link_path.display()
            ),
        )
        .unwrap();

        let paths = managed_artifact_paths_at(
            ItemKind::Package,
            &target("openssl"),
            &install_root,
            &bin_dir,
        )
        .await
        .unwrap();

        assert!(paths.contains(&install_path));
        assert!(paths.contains(&link_path));
    }

    #[tokio::test]
    async fn managed_artifact_paths_ignores_unmarked_artifacts() {
        let temp = tempfile::tempdir().unwrap();
        let install_root = temp.path().join("packages");
        let bin_dir = temp.path().join("bin");
        let custom_path = install_root.join("openssl").join("latest");
        fs::create_dir_all(&custom_path).unwrap();
        fs::write(custom_path.join("custom.txt"), "keep").unwrap();

        let paths = managed_artifact_paths_at(
            ItemKind::Package,
            &target("openssl"),
            &install_root,
            &bin_dir,
        )
        .await
        .unwrap();

        assert!(paths.is_empty());
    }

    #[tokio::test]
    async fn managed_artifact_paths_rejects_marker_paths_outside_managed_roots() {
        let temp = tempfile::tempdir().unwrap();
        let install_root = temp.path().join("packages");
        let bin_dir = temp.path().join("bin");
        let install_path = install_root.join("openssl").join("latest");
        let outside = temp.path().join("outside");
        fs::create_dir_all(&install_path).unwrap();
        fs::write(
            install_path.join("install.toml"),
            format!(
                r#"
                kind = "package"
                name = "openssl"
                version = "latest"
                backend = "test"
                install_path = "{}"
                outputs = ["{}"]
                linked_executables = ["{}"]
                "#,
                install_path.display(),
                outside.display(),
                outside.display()
            ),
        )
        .unwrap();

        let paths = managed_artifact_paths_at(
            ItemKind::Package,
            &target("openssl"),
            &install_root,
            &bin_dir,
        )
        .await
        .unwrap();

        assert!(paths.contains(&install_path));
        assert!(!paths.contains(&outside));
    }

    #[tokio::test]
    async fn managed_artifact_paths_honors_exact_version_targets() {
        let temp = tempfile::tempdir().unwrap();
        let install_root = temp.path().join("packages");
        let bin_dir = temp.path().join("bin");
        let latest_path = install_root.join("openssl").join("latest");
        let version_path = install_root.join("openssl").join("3");
        fs::create_dir_all(&latest_path).unwrap();
        fs::create_dir_all(&version_path).unwrap();
        fs::write(
            latest_path.join("install.toml"),
            r#"
            kind = "package"
            name = "openssl"
            version = "latest"
            backend = "test"
            outputs = []
            linked_executables = []
            "#,
        )
        .unwrap();
        fs::write(
            version_path.join("install.toml"),
            r#"
            kind = "package"
            name = "openssl"
            version = "3"
            backend = "test"
            outputs = []
            linked_executables = []
            "#,
        )
        .unwrap();

        let paths = managed_artifact_paths_at(
            ItemKind::Package,
            &exact_target("openssl", "3", "test"),
            &install_root,
            &bin_dir,
        )
        .await
        .unwrap();

        assert!(paths.contains(&version_path));
        assert!(!paths.contains(&latest_path));
    }

    #[tokio::test]
    async fn uninstall_uses_global_config_when_no_project_config_exists() {
        let temp = tempfile::tempdir().unwrap();
        let global = temp.path().join(".config/still/config.toml");
        fs::create_dir_all(global.parent().unwrap()).unwrap();
        fs::write(&global, "[packages]\nlatest = [\"openssl\"]\n").unwrap();

        let result = run(UninstallRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
            target: target("openssl"),
        })
        .await
        .unwrap();

        assert_eq!(result.path, global);
        let config = parse_still_toml(&fs::read_to_string(result.path).unwrap()).unwrap();
        assert!(config.packages.latest.is_empty());
    }

    #[tokio::test]
    async fn uninstall_global_forces_global_config() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("still.toml"),
            "[packages]\nlatest = [\"openssl\"]\n",
        )
        .unwrap();
        let global = temp.path().join(".config/still/config.toml");
        fs::create_dir_all(global.parent().unwrap()).unwrap();
        fs::write(&global, "[packages]\nlatest = [\"llvm\"]\n").unwrap();

        let result = run(UninstallRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: true,
            target: target("llvm"),
        })
        .await
        .unwrap();

        assert_eq!(result.path, global);
        let project =
            parse_still_toml(&fs::read_to_string(temp.path().join("still.toml")).unwrap()).unwrap();
        assert_eq!(project.packages.latest, ["openssl"]);
    }

    fn target(name: &str) -> UninstallTarget {
        UninstallTarget {
            kind: None,
            name: name.to_string(),
            version: "latest".to_string(),
            backend: None,
            exact: false,
        }
    }

    fn exact_target(name: &str, version: &str, backend: &str) -> UninstallTarget {
        UninstallTarget {
            kind: None,
            name: name.to_string(),
            version: version.to_string(),
            backend: Some(backend.parse().unwrap()),
            exact: true,
        }
    }
}
