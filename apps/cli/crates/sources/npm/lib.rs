//! npm source scaffold backed by npm-compatible registry metadata.

use serde::{Deserialize, Serialize};
use still_source_kit::{
    InstallContext, InstallModel, InstallPlan, ItemKind, MetadataFormat, MetadataInventory,
    RawSourceMetadata, ResolveRequest, ResolvedItem, Source, SourceDescriptor, SourceError,
    SourceResult, TargetOs,
};

pub const SOURCE_ID: &str = "npm";

const ITEM_KINDS: &[ItemKind] = &[ItemKind::Tool];
const TARGET_OS: &[TargetOs] = &[TargetOs::Macos, TargetOs::Linux, TargetOs::Windows];
const METADATA_FORMATS: &[MetadataFormat] = &[MetadataFormat::Json];
const INSTALL_MODELS: &[InstallModel] = &[InstallModel::DownloadArtifact, InstallModel::Manual];

/// npm package metadata fields needed for CLI package resolution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NpmPackageMetadata {
    pub name: String,
    #[serde(default)]
    pub versions: Vec<NpmVersionMetadata>,
    #[serde(default)]
    pub dist_tags: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NpmVersionMetadata {
    pub version: String,
    pub bin: Option<NpmBin>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum NpmBin {
    Single(String),
    Many(std::collections::BTreeMap<String, String>),
}

/// Source implementation for npm registry metadata.
#[derive(Debug, Default, Clone, Copy)]
pub struct NpmSource;

impl NpmSource {
    /// Creates an npm source adapter.
    pub const fn new() -> Self {
        Self
    }
}

/// Returns the static npm source descriptor.
pub fn descriptor() -> SourceDescriptor {
    NpmSource::new().descriptor()
}

impl Source for NpmSource {
    fn descriptor(&self) -> SourceDescriptor {
        SourceDescriptor {
            id: SOURCE_ID,
            display_name: "npm",
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
        Err(SourceError::ResolutionFailed {
            source_id: SOURCE_ID,
            name: request.name.clone(),
            reason: "npm registry resolution is not implemented yet".to_string(),
        })
    }

    fn plan_install(
        &self,
        resolved: &ResolvedItem,
        _context: &InstallContext,
    ) -> SourceResult<InstallPlan> {
        Ok(InstallPlan::manual(
            resolved.clone(),
            vec!["plan an npm package unpack and binary shim into Still storage".to_string()],
        ))
    }
}

pub const fn is_host_enabled() -> bool {
    true
}
