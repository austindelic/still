//! Engine action for removing desired install state.

use std::path::PathBuf;

use anyhow::{Context, Result};

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
            scope: ConfigScope::Project,
            for_write: true,
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
    for path in artifact_paths(kind, name) {
        if tokio::fs::symlink_metadata(&path).await.is_err() {
            continue;
        }
        remove_path(&path).await?;
        removed.push(path);
    }
    Ok(removed)
}

fn artifact_paths(kind: ItemKind, name: &str) -> Vec<PathBuf> {
    let install_root = match kind {
        ItemKind::Tool => System::tool_dir(),
        ItemKind::Package => System::root_dir().join("packages"),
        ItemKind::App => System::apps_dir(),
    };
    let mut paths = vec![install_root.join(name)];
    if matches!(kind, ItemKind::Tool | ItemKind::Package) {
        paths.push(System::bin_dir().join(name));
    }
    paths
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
            name: "node".to_string(),
        })
        .await
        .unwrap_err();

        assert!(err.to_string().contains("not configured"));
    }

    #[test]
    fn artifact_paths_stay_under_still_roots() {
        let paths = artifact_paths(ItemKind::Package, "openssl");

        assert!(
            paths
                .iter()
                .any(|path| { path.ends_with(std::path::Path::new("packages").join("openssl")) })
        );
    }
}
