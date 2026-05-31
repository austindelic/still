//! Engine action for removing desired install state.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::actions::sync::refresh_lockfile;
use crate::config::{ConfigScope, ConfigSelection, resolve_config_path};
use crate::config_edit::remove_item;
use crate::error::EngineError;
use crate::specs::item::ItemKind;
use crate::system::{Linux, MacOS, System, Windows};
use crate::utils::paths::PathOps;

/// Platform-specific uninstall operations.
pub trait UninstallOps {}

// macOS currently uses the marker trait until backend removal behavior exists.
impl UninstallOps for MacOS {}
impl UninstallOps for Linux {}
impl UninstallOps for Windows {}

/// Request to remove one configured item.
#[derive(Debug, Clone)]
pub struct UninstallRequest {
    pub start_dir: PathBuf,
    pub home_dir: PathBuf,
    pub global: bool,
    pub name: String,
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
    let (updated, removed) = remove_item(&content, &request.name)?;
    let Some(kind) = removed else {
        return Err(EngineError::Conflict {
            message: format!("{} is not configured", request.name),
        }
        .into());
    };

    tokio::fs::write(&resolved.path, updated)
        .await
        .with_context(|| format!("failed to write {}", resolved.path.display()))?;
    refresh_lockfile(&resolved.path).await?;
    let removed_paths = remove_installed_artifacts(kind, &request.name).await?;

    Ok(UninstallResult {
        path: resolved.path,
        kind,
        name: request.name,
        removed_paths,
    })
}

async fn remove_installed_artifacts(kind: ItemKind, name: &str) -> Result<Vec<PathBuf>> {
    let mut removed = Vec::new();
    for path in managed_artifact_paths(kind, name).await? {
        if tokio::fs::symlink_metadata(&path).await.is_err() {
            continue;
        }
        remove_path(&path).await?;
        removed.push(path);
    }
    Ok(removed)
}

async fn managed_artifact_paths(kind: ItemKind, name: &str) -> Result<Vec<PathBuf>> {
    managed_artifact_paths_at(kind, name, &install_root(kind), &System::bin_dir()).await
}

async fn managed_artifact_paths_at(
    kind: ItemKind,
    name: &str,
    install_root: &Path,
    bin_dir: &Path,
) -> Result<Vec<PathBuf>> {
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
            Some(marker) if marker.matches(kind, name) => marker,
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
    #[serde(default)]
    outputs: Vec<String>,
    #[serde(default)]
    linked_executables: Vec<String>,
}

impl InstallMarker {
    fn matches(&self, kind: ItemKind, name: &str) -> bool {
        self.kind == kind.to_string() && self.name == name
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
        ItemKind::Package => System::root_dir().join("packages"),
        ItemKind::App => System::apps_dir(),
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
            name: "openssl".to_string(),
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
            name: "node".to_string(),
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

        let paths =
            managed_artifact_paths_at(ItemKind::Package, "openssl", &install_root, &bin_dir)
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

        let paths =
            managed_artifact_paths_at(ItemKind::Package, "openssl", &install_root, &bin_dir)
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

        let paths =
            managed_artifact_paths_at(ItemKind::Package, "openssl", &install_root, &bin_dir)
                .await
                .unwrap();

        assert!(paths.contains(&install_path));
        assert!(!paths.contains(&outside));
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
            name: "openssl".to_string(),
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
            name: "llvm".to_string(),
        })
        .await
        .unwrap();

        assert_eq!(result.path, global);
        let project =
            parse_still_toml(&fs::read_to_string(temp.path().join("still.toml")).unwrap()).unwrap();
        assert_eq!(project.packages.latest, ["openssl"]);
    }
}
