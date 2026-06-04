//! Aqua source scaffold backed by registry package metadata.

use serde::{Deserialize, Serialize};
use still_source_kit::{
    InstallContext, InstallModel, InstallPlan, ItemKind, MetadataFormat, MetadataInventory,
    RawSourceMetadata, ResolveRequest, ResolvedItem, Source, SourceDescriptor, SourceError,
    SourceResult, TargetOs,
};

pub const SOURCE_ID: &str = "aqua";

const ITEM_KINDS: &[ItemKind] = &[ItemKind::Tool];
const TARGET_OS: &[TargetOs] = &[TargetOs::Macos, TargetOs::Linux, TargetOs::Windows];
const METADATA_FORMATS: &[MetadataFormat] = &[MetadataFormat::Yaml, MetadataFormat::Json];
const INSTALL_MODELS: &[InstallModel] = &[InstallModel::DownloadArtifact, InstallModel::Manual];

/// Aqua registry metadata needed to select a package asset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AquaPackageMetadata {
    pub name: String,
    pub repo_owner: Option<String>,
    pub repo_name: Option<String>,
    #[serde(default)]
    pub asset: Vec<AquaAssetRule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AquaAssetRule {
    pub name: String,
    pub os: Option<String>,
    pub arch: Option<String>,
}

/// Source implementation for Aqua registry metadata.
#[derive(Debug, Default, Clone, Copy)]
pub struct AquaSource;

impl AquaSource {
    /// Creates an Aqua source adapter.
    pub const fn new() -> Self {
        Self
    }
}

impl Source for AquaSource {
    fn descriptor(&self) -> SourceDescriptor {
        SourceDescriptor {
            id: SOURCE_ID,
            display_name: "Aqua",
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
            reason: "Aqua registry resolution is not implemented yet".to_string(),
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
                "plan an Aqua asset download, checksum verification, and binary discovery"
                    .to_string(),
            ],
        ))
    }
}

pub const fn is_host_enabled() -> bool {
    true
}
