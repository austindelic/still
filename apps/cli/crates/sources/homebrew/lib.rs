//! Homebrew source scaffold backed by raw formula and cask metadata.

use serde::{Deserialize, Serialize};
use still_source_kit::{
    InstallContext, InstallModel, InstallPlan, ItemKind, MetadataFormat, MetadataInventory,
    RawSourceMetadata, ResolveRequest, ResolvedItem, Source, SourceDescriptor, SourceError,
    SourceResult, TargetOs,
};

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

/// Source implementation for Homebrew metadata.
#[derive(Debug, Default, Clone, Copy)]
pub struct HomebrewSource;

impl HomebrewSource {
    /// Creates a Homebrew source adapter.
    pub const fn new() -> Self {
        Self
    }
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
