//! pacman source scaffold backed by Arch package database metadata.

use serde::{Deserialize, Serialize};
use still_source_kit::{
    InstallContext, InstallModel, InstallPlan, ItemKind, MetadataFormat, MetadataInventory,
    RawSourceMetadata, ResolveRequest, ResolvedItem, Source, SourceDescriptor, SourceError,
    SourceResult, TargetOs,
};

pub const SOURCE_ID: &str = "pacman";

const ITEM_KINDS: &[ItemKind] = &[ItemKind::Package];
const TARGET_OS: &[TargetOs] = &[TargetOs::Linux];
const METADATA_FORMATS: &[MetadataFormat] = &[MetadataFormat::PlainText, MetadataFormat::Index];
const INSTALL_MODELS: &[InstallModel] =
    &[InstallModel::SystemPackageMetadata, InstallModel::Manual];

/// Arch package database fields needed for package planning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PacmanPackageMetadata {
    pub name: String,
    pub version: String,
    pub arch: String,
    pub filename: Option<String>,
    pub sha256: Option<String>,
    #[serde(default)]
    pub depends: Vec<String>,
}

/// Source implementation for pacman package metadata.
#[derive(Debug, Default, Clone, Copy)]
pub struct PacmanSource;

impl PacmanSource {
    /// Creates a pacman source adapter.
    pub const fn new() -> Self {
        Self
    }
}

/// Returns the static pacman source descriptor.
pub fn descriptor() -> SourceDescriptor {
    PacmanSource::new().descriptor()
}

impl Source for PacmanSource {
    fn descriptor(&self) -> SourceDescriptor {
        SourceDescriptor {
            id: SOURCE_ID,
            display_name: "pacman",
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
            reason: "Arch package database resolution is not implemented yet".to_string(),
        })
    }

    fn plan_install(
        &self,
        resolved: &ResolvedItem,
        _context: &InstallContext,
    ) -> SourceResult<InstallPlan> {
        Ok(InstallPlan::manual(
            resolved.clone(),
            vec!["plan pacman package metadata reconciliation without invoking pacman as the source model".to_string()],
        ))
    }
}

#[cfg(any(target_os = "linux", feature = "portable"))]
pub const fn is_host_enabled() -> bool {
    true
}

#[cfg(not(any(target_os = "linux", feature = "portable")))]
pub const fn is_host_enabled() -> bool {
    false
}
