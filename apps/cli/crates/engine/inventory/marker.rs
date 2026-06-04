//! Receipt marker helpers for Still-managed installs.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::specs::item::ItemKind;

/// File name used for per-install receipt markers.
pub const INSTALL_MARKER_FILE: &str = "install.toml";

/// Still-managed install receipt marker.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InstallMarker {
    pub kind: String,
    pub name: String,
    #[serde(default = "latest_version")]
    pub version: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_path: Option<String>,
    #[serde(default)]
    pub outputs: Vec<String>,
    #[serde(default)]
    pub linked_executables: Vec<String>,
}

/// Reads an install receipt marker from an install directory.
pub async fn read_install_marker(install_path: &Path) -> Result<Option<InstallMarker>> {
    let marker_path = install_path.join(INSTALL_MARKER_FILE);
    let content = match tokio::fs::read_to_string(&marker_path).await {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err.into()),
    };
    Ok(Some(toml::from_str(&content)?))
}

/// Writes an install receipt marker under an install directory.
pub async fn write_install_marker(
    install_path: &Path,
    kind: ItemKind,
    name: &str,
    version: &str,
    source: &str,
    outputs: &[PathBuf],
    linked_executables: &[PathBuf],
) -> Result<()> {
    tokio::fs::create_dir_all(install_path).await?;
    let marker_path = install_path.join(INSTALL_MARKER_FILE);
    let output_paths = paths_to_strings(outputs);
    let linked_paths = paths_to_strings(linked_executables);
    let install_path_display = install_path.display().to_string();
    let marker = InstallMarker {
        kind: kind.to_string(),
        name: name.to_string(),
        version: version.to_string(),
        backend: Some(source.to_string()),
        install_path: Some(install_path_display),
        outputs: output_paths,
        linked_executables: linked_paths,
    };
    let content = toml::to_string_pretty(&marker)?;
    tokio::fs::write(marker_path, content).await?;
    Ok(())
}

fn paths_to_strings(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .map(|path| path.display().to_string())
        .collect()
}

fn latest_version() -> String {
    "latest".to_string()
}
