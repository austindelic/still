//! Homebrew source scaffold backed by raw formula and cask metadata.

use std::path::Path;

use serde::{Deserialize, Serialize};
use still_source_kit::{
    InstallContext, InstallModel, InstallPlan, ItemKind, MetadataFormat, MetadataInventory,
    RawSourceMetadata, ResolveRequest, ResolvedItem, Source, SourceDescriptor, SourceError,
    SourceResult, TargetOs,
};

pub mod brew;
pub use brew::{BottleFileSpec, BottleSpec, CaskSpec, FormulaSpec};

pub const SOURCE_ID: &str = "homebrew";

const ITEM_KINDS: &[ItemKind] = &[ItemKind::Tool, ItemKind::Package, ItemKind::App];
const TARGET_OS: &[TargetOs] = &[TargetOs::Macos];
const METADATA_FORMATS: &[MetadataFormat] = &[MetadataFormat::Json];
const INSTALL_MODELS: &[InstallModel] = &[
    InstallModel::DownloadArtifact,
    InstallModel::BuildFromSource,
    InstallModel::AppBundle,
    InstallModel::Manual,
];

/// Homebrew formula metadata fields Still initially cares about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormulaMetadata {
    pub name: String,
    pub full_name: Option<String>,
    pub desc: Option<String>,
    pub versions: HomebrewVersions,
    #[serde(default)]
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HomebrewVersions {
    pub stable: Option<String>,
    pub head: Option<String>,
}

/// Homebrew cask metadata fields used for app resolution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaskMetadata {
    pub token: String,
    pub name: Vec<String>,
    pub desc: Option<String>,
    pub version: Option<String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

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

/// Loads cached Homebrew formula and cask metadata from a cache root.
pub fn load_cached_homebrew_packages(cache_root: &Path) -> SourceResult<Vec<CachedHomebrewPackage>> {
    let mut packages = Vec::new();
    packages.extend(load_cached_formulae(&cache_root.join("formula.json"))?);
    packages.extend(load_cached_casks(&cache_root.join("cask.json"))?);
    Ok(packages)
}

fn load_cached_formulae(path: &Path) -> SourceResult<Vec<CachedHomebrewPackage>> {
    let Some(array) = load_json_array(path)? else {
        return Ok(Vec::new());
    };
    Ok(array
        .into_iter()
        .filter_map(|value| serde_json::from_value::<FormulaSpec>(value).ok())
        .map(|formula| CachedHomebrewPackage {
            kind: CachedHomebrewPackageKind::Formula,
            name: formula.name,
            version: formula.versions.stable,
            installed: !formula.installed.is_empty(),
        })
        .collect())
}

fn load_cached_casks(path: &Path) -> SourceResult<Vec<CachedHomebrewPackage>> {
    let Some(array) = load_json_array(path)? else {
        return Ok(Vec::new());
    };
    Ok(array
        .into_iter()
        .filter_map(|value| serde_json::from_value::<CaskSpec>(value).ok())
        .map(|cask| CachedHomebrewPackage {
            kind: CachedHomebrewPackageKind::Cask,
            name: cask.token,
            version: if cask.version.is_empty() {
                "latest".to_string()
            } else {
                cask.version
            },
            installed: !cask.installed.is_empty(),
        })
        .collect())
}

fn load_json_array(path: &Path) -> SourceResult<Option<Vec<serde_json::Value>>> {
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(path).map_err(|err| SourceError::InvalidMetadata {
        source_id: SOURCE_ID,
        reason: format!("failed to read {}: {err}", path.display()),
    })?;
    let value = serde_json::from_str::<serde_json::Value>(&content).map_err(|err| {
        SourceError::InvalidMetadata {
            source_id: SOURCE_ID,
            reason: format!("failed to parse {}: {err}", path.display()),
        }
    })?;
    value
        .as_array()
        .cloned()
        .map(Some)
        .ok_or_else(|| SourceError::InvalidMetadata {
            source_id: SOURCE_ID,
            reason: format!("{} is not a JSON array", path.display()),
        })
}

/// Source implementation for Homebrew metadata.
#[derive(Debug, Default, Clone, Copy)]
pub struct HomebrewSource;

impl HomebrewSource {
    /// Creates a Homebrew source adapter.
    pub const fn new() -> Self {
        Self
    }
}

/// Returns the static Homebrew source descriptor.
pub fn descriptor() -> SourceDescriptor {
    HomebrewSource::new().descriptor()
}

impl Source for HomebrewSource {
    fn descriptor(&self) -> SourceDescriptor {
        SourceDescriptor {
            id: SOURCE_ID,
            display_name: "Homebrew",
            item_kinds: ITEM_KINDS,
            target_os: TARGET_OS,
            metadata_formats: METADATA_FORMATS,
            install_models: INSTALL_MODELS,
        }
    }

    fn parse_metadata(&self, raw: RawSourceMetadata) -> SourceResult<MetadataInventory> {
        Ok(MetadataInventory::from_raw(SOURCE_ID, raw))
    }

    fn resolve(
        &self,
        request: &ResolveRequest,
        _inventory: &MetadataInventory,
    ) -> SourceResult<ResolvedItem> {
        Err(SourceError::unsupported(SOURCE_ID, "homebrew resolution")).map_err(|err| match err {
            SourceError::Unsupported { .. } => SourceError::ResolutionFailed {
                source_id: SOURCE_ID,
                name: request.name.clone(),
                reason: "raw Homebrew formula/cask resolution is not implemented yet".to_string(),
            },
            other => other,
        })
    }

    fn plan_install(
        &self,
        resolved: &ResolvedItem,
        _context: &InstallContext,
    ) -> SourceResult<InstallPlan> {
        Ok(InstallPlan::manual(
            resolved.clone(),
            vec![
                "plan Homebrew bottle, source build, or cask app install from raw metadata"
                    .to_string(),
            ],
        ))
    }
}

#[cfg(any(target_os = "macos", feature = "portable"))]
pub const fn is_host_enabled() -> bool {
    true
}

#[cfg(not(any(target_os = "macos", feature = "portable")))]
pub const fn is_host_enabled() -> bool {
    false
}
