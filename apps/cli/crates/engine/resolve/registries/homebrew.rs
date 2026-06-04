//! Homebrew registry cache loading.

use std::{fs, path::Path};

use crate::{
    error::{EngineError, EngineResult},
    infra::paths::PathOps,
    platform::System,
    specs::brew::{CaskSpec, FormulaSpec},
};

/// Source kind for cached Homebrew package metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CachedHomebrewPackageKind {
    Formula,
    Cask,
}

/// Display-ready package metadata loaded from Homebrew formula and cask caches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedHomebrewPackage {
    pub kind: CachedHomebrewPackageKind,
    pub name: String,
    pub version: String,
    pub installed: bool,
}

/// Loads cached Homebrew formula and cask metadata from Still's cache directory.
/// # Errors
/// Returns an error when an existing cache file cannot be read or is not a JSON array.
pub fn load_cached_homebrew_packages() -> EngineResult<Vec<CachedHomebrewPackage>> {
    let cache_dir = System::cache_dir().join("still");
    let mut packages = Vec::new();
    packages.extend(load_cached_formulae(&cache_dir.join("formula.json"))?);
    packages.extend(load_cached_casks(&cache_dir.join("cask.json"))?);
    Ok(packages)
}

fn load_cached_formulae(path: &Path) -> EngineResult<Vec<CachedHomebrewPackage>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let entries = read_json_array(path)?;
    Ok(entries
        .into_iter()
        .filter_map(|value| serde_json::from_value::<FormulaSpec>(value).ok())
        .map(|formula| {
            let installed = !formula.installed.is_empty();
            CachedHomebrewPackage {
                kind: CachedHomebrewPackageKind::Formula,
                name: formula.name,
                version: formula.versions.stable,
                installed,
            }
        })
        .collect())
}

fn load_cached_casks(path: &Path) -> EngineResult<Vec<CachedHomebrewPackage>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let entries = read_json_array(path)?;
    Ok(entries
        .into_iter()
        .filter_map(|value| serde_json::from_value::<CaskSpec>(value).ok())
        .map(|cask| {
            let installed = !cask.installed.is_empty();
            let version = if cask.version.is_empty() {
                "-".to_string()
            } else {
                cask.version
            };
            CachedHomebrewPackage {
                kind: CachedHomebrewPackageKind::Cask,
                name: cask.token,
                version,
                installed,
            }
        })
        .collect())
}

fn read_json_array(path: &Path) -> EngineResult<Vec<serde_json::Value>> {
    let content = fs::read_to_string(path).map_err(|err| {
        EngineError::from(err).context(format!("failed to read {}", path.display()))
    })?;
    let value: serde_json::Value = serde_json::from_str(&content)?;
    value.as_array().cloned().ok_or_else(|| {
        EngineError::message(format!("{} must contain a JSON array", path.display()))
    })
}
