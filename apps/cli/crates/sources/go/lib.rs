//! Go source scaffold backed by module metadata.

use serde::{Deserialize, Serialize};
use still_source_kit::{
    InstallContext, InstallModel, InstallPlan, ItemKind, MetadataFormat, MetadataInventory,
    RawSourceMetadata, ResolveRequest, ResolvedItem, Source, SourceDescriptor, SourceError,
    SourceResult, TargetOs,
};

pub const SOURCE_ID: &str = "go";

const ITEM_KINDS: &[ItemKind] = &[ItemKind::Tool];
const TARGET_OS: &[TargetOs] = &[TargetOs::Macos, TargetOs::Linux, TargetOs::Windows];
const METADATA_FORMATS: &[MetadataFormat] = &[MetadataFormat::Json, MetadataFormat::PlainText];
const INSTALL_MODELS: &[InstallModel] = &[InstallModel::BuildFromSource, InstallModel::Manual];

/// Go module metadata needed to build a command package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoModuleMetadata {
    pub module_path: String,
    pub version: String,
    pub time: Option<String>,
    pub origin: Option<String>,
}

/// Source implementation for Go module metadata.
#[derive(Debug, Default, Clone, Copy)]
pub struct GoSource;

impl GoSource {
    /// Creates a Go source adapter.
    pub const fn new() -> Self {
        Self
    }
}

/// Returns the static Go source descriptor.
pub fn descriptor() -> SourceDescriptor {
    GoSource::new().descriptor()
}

impl Source for GoSource {
    fn descriptor(&self) -> SourceDescriptor {
        SourceDescriptor {
            id: SOURCE_ID,
            display_name: "Go",
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
            reason: "Go module metadata resolution is not implemented yet".to_string(),
        })
    }

    fn plan_install(
        &self,
        resolved: &ResolvedItem,
        _context: &InstallContext,
    ) -> SourceResult<InstallPlan> {
        Ok(InstallPlan::manual(
            resolved.clone(),
            vec!["plan a Go module build into a Still-owned GOBIN".to_string()],
        ))
    }
}

pub const fn is_host_enabled() -> bool {
    true
}
