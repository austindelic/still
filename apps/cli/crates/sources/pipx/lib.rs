//! pipx-style Python CLI source scaffold backed by Python package metadata.

use serde::{Deserialize, Serialize};
use still_source_kit::{
    InstallContext, InstallModel, InstallPlan, ItemKind, MetadataFormat, MetadataInventory,
    RawSourceMetadata, ResolveRequest, ResolvedItem, Source, SourceDescriptor, SourceError,
    SourceResult, TargetOs,
};

pub const SOURCE_ID: &str = "pipx";

const ITEM_KINDS: &[ItemKind] = &[ItemKind::Tool];
const TARGET_OS: &[TargetOs] = &[TargetOs::Macos, TargetOs::Linux, TargetOs::Windows];
const METADATA_FORMATS: &[MetadataFormat] = &[MetadataFormat::Json];
const INSTALL_MODELS: &[InstallModel] = &[InstallModel::VirtualEnvironment, InstallModel::Manual];

/// Python package metadata needed for isolated CLI installs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PythonPackageMetadata {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub requires_dist: Vec<String>,
    #[serde(default)]
    pub entry_points: Vec<PythonEntryPoint>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PythonEntryPoint {
    pub group: String,
    pub name: String,
    pub value: String,
}

/// Source implementation for pipx-style Python metadata.
#[derive(Debug, Default, Clone, Copy)]
pub struct PipxSource;

impl PipxSource {
    /// Creates a pipx source adapter.
    pub const fn new() -> Self {
        Self
    }
}

impl Source for PipxSource {
    fn descriptor(&self) -> SourceDescriptor {
        SourceDescriptor {
            id: SOURCE_ID,
            display_name: "pipx",
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
            reason: "Python package metadata resolution is not implemented yet".to_string(),
        })
    }

    fn plan_install(
        &self,
        resolved: &ResolvedItem,
        _context: &InstallContext,
    ) -> SourceResult<InstallPlan> {
        Ok(InstallPlan::manual(
            resolved.clone(),
            vec!["plan an isolated Python virtual environment and entry-point shims".to_string()],
        ))
    }
}

pub const fn is_host_enabled() -> bool {
    true
}
