//! Receipt marker helpers for Still-managed installs.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::Result;
use crate::specs::item::ItemKind;

/// File name used for per-install receipt markers.
pub const INSTALL_MARKER_FILE: &str = "install.toml";

/// Writes an install receipt marker under an install directory.
pub(crate) async fn write_install_marker(
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
    let install_path = install_path.display().to_string();
    let marker = InstallMarker {
        kind: kind.to_string(),
        name,
        version,
        backend: source,
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
