//! Cargo source scaffold backed by crates.io-compatible metadata.

use serde::{Deserialize, Serialize};
use still_source_kit::{
    InstallContext, InstallModel, InstallPlan, ItemKind, MetadataFormat, MetadataInventory,
    RawSourceMetadata, ResolveRequest, ResolvedItem, Source, SourceDescriptor, SourceError,
    SourceResult, TargetOs,
};

pub const SOURCE_ID: &str = "cargo";

const ITEM_KINDS: &[ItemKind] = &[ItemKind::Tool];
const TARGET_OS: &[TargetOs] = &[TargetOs::Macos, TargetOs::Linux, TargetOs::Windows];
const METADATA_FORMATS: &[MetadataFormat] = &[MetadataFormat::Json, MetadataFormat::Index];
const INSTALL_MODELS: &[InstallModel] = &[InstallModel::BuildFromSource, InstallModel::Manual];

/// Crate metadata needed to resolve a Rust CLI package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrateMetadata {
    pub name: String,
    pub newest_version: Option<String>,
    #[serde(default)]
    pub versions: Vec<CrateVersion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrateVersion {
    pub num: String,
    pub yanked: bool,
    #[serde(default)]
    pub bins: Vec<String>,
}

/// Source implementation for Cargo registry metadata.
#[derive(Debug, Default, Clone, Copy)]
pub struct CargoSource;

impl CargoSource {
    /// Creates a Cargo source adapter.
    pub const fn new() -> Self {
        Self
    }
}

/// Returns the static Cargo source descriptor.
pub fn descriptor() -> SourceDescriptor {
    CargoSource::new().descriptor()
}

impl Source for CargoSource {
    fn descriptor(&self) -> SourceDescriptor {
        SourceDescriptor {
            id: SOURCE_ID,
            display_name: "Cargo",
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
            reason: "crates.io index resolution is not implemented yet".to_string(),
        })
    }

    fn plan_install(
        &self,
        resolved: &ResolvedItem,
        _context: &InstallContext,
    ) -> SourceResult<InstallPlan> {
        Ok(InstallPlan::manual(
            resolved.clone(),
            vec!["plan a cargo package build into a Still-owned install root".to_string()],
        ))
    }
}

pub const fn is_host_enabled() -> bool {
    true
}
