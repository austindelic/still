//! dnf source scaffold backed by RPM repository metadata.

use serde::{Deserialize, Serialize};
use still_source_kit::{
    InstallContext, InstallModel, InstallPlan, ItemKind, MetadataFormat, MetadataInventory,
    RawSourceMetadata, ResolveRequest, ResolvedItem, Source, SourceDescriptor, SourceError,
    SourceResult, TargetOs,
};

pub const SOURCE_ID: &str = "dnf";

const ITEM_KINDS: &[ItemKind] = &[ItemKind::Package];
const TARGET_OS: &[TargetOs] = &[TargetOs::Linux];
const METADATA_FORMATS: &[MetadataFormat] = &[MetadataFormat::Xml, MetadataFormat::Index];
const INSTALL_MODELS: &[InstallModel] =
    &[InstallModel::SystemPackageMetadata, InstallModel::Manual];

/// RPM package metadata needed for dnf-style package planning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnfPackageMetadata {
    pub name: String,
    pub version: String,
    pub release: Option<String>,
    pub arch: String,
    pub location: Option<String>,
    pub checksum: Option<String>,
}

/// Source implementation for dnf repository metadata.
#[derive(Debug, Default, Clone, Copy)]
pub struct DnfSource;

impl DnfSource {
    /// Creates a dnf source adapter.
    pub const fn new() -> Self {
        Self
    }
}

impl Source for DnfSource {
    fn descriptor(&self) -> SourceDescriptor {
        SourceDescriptor {
            id: SOURCE_ID,
            display_name: "dnf",
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
            reason: "RPM repository metadata resolution is not implemented yet".to_string(),
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
                "plan dnf package metadata reconciliation without invoking dnf as the source model"
                    .to_string(),
            ],
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
