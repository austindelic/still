//! Flatpak source scaffold backed by appstream and remote metadata.

use serde::{Deserialize, Serialize};
use still_source_kit::{
    InstallContext, InstallModel, InstallPlan, ItemKind, MetadataFormat, MetadataInventory,
    RawSourceMetadata, ResolveRequest, ResolvedItem, Source, SourceDescriptor, SourceError,
    SourceResult, TargetOs,
};

pub const SOURCE_ID: &str = "flatpak";

const ITEM_KINDS: &[ItemKind] = &[ItemKind::App];
const TARGET_OS: &[TargetOs] = &[TargetOs::Linux];
const METADATA_FORMATS: &[MetadataFormat] = &[MetadataFormat::Xml, MetadataFormat::Index];
const INSTALL_MODELS: &[InstallModel] = &[InstallModel::AppBundle, InstallModel::Manual];

/// Flatpak app metadata needed to resolve an application.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlatpakAppMetadata {
    pub app_id: String,
    pub branch: Option<String>,
    pub remote: Option<String>,
    pub runtime: Option<String>,
}

/// Source implementation for Flatpak metadata.
#[derive(Debug, Default, Clone, Copy)]
pub struct FlatpakSource;

impl FlatpakSource {
    /// Creates a Flatpak source adapter.
    pub const fn new() -> Self {
        Self
    }
}

impl Source for FlatpakSource {
    fn descriptor(&self) -> SourceDescriptor {
        SourceDescriptor {
            id: SOURCE_ID,
            display_name: "Flatpak",
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
            reason: "Flatpak app metadata resolution is not implemented yet".to_string(),
        })
    }

    fn plan_install(
        &self,
        resolved: &ResolvedItem,
        _context: &InstallContext,
    ) -> SourceResult<InstallPlan> {
        Ok(InstallPlan::manual(
            resolved.clone(),
            vec!["plan Flatpak app metadata reconciliation without invoking flatpak as the source model".to_string()],
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
