//! winget source scaffold backed by Windows Package Manager manifests.

use serde::{Deserialize, Serialize};
use still_source_kit::{
    InstallContext, InstallModel, InstallPlan, ItemKind, MetadataFormat, MetadataInventory,
    RawSourceMetadata, ResolveRequest, ResolvedItem, Source, SourceDescriptor, SourceError,
    SourceResult, TargetOs,
};

pub const SOURCE_ID: &str = "winget";

const ITEM_KINDS: &[ItemKind] = &[ItemKind::Package, ItemKind::App];
const TARGET_OS: &[TargetOs] = &[TargetOs::Windows];
const METADATA_FORMATS: &[MetadataFormat] = &[MetadataFormat::Yaml, MetadataFormat::Index];
const INSTALL_MODELS: &[InstallModel] = &[
    InstallModel::DownloadArtifact,
    InstallModel::AppBundle,
    InstallModel::Manual,
];

/// winget manifest fields needed for installer selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WingetPackageManifest {
    pub package_identifier: String,
    pub package_version: String,
    #[serde(default)]
    pub installers: Vec<WingetInstaller>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WingetInstaller {
    pub architecture: Option<String>,
    pub installer_url: String,
    pub installer_sha256: Option<String>,
    pub installer_type: Option<String>,
}

/// Source implementation for winget manifests.
#[derive(Debug, Default, Clone, Copy)]
pub struct WingetSource;

impl WingetSource {
    /// Creates a winget source adapter.
    pub const fn new() -> Self {
        Self
    }
}

/// Returns the static winget source descriptor.
pub fn descriptor() -> SourceDescriptor {
    WingetSource::new().descriptor()
}

impl Source for WingetSource {
    fn descriptor(&self) -> SourceDescriptor {
        SourceDescriptor {
            id: SOURCE_ID,
            display_name: "winget",
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
            reason: "winget manifest resolution is not implemented yet".to_string(),
        })
    }

    fn plan_install(
        &self,
        resolved: &ResolvedItem,
        _context: &InstallContext,
    ) -> SourceResult<InstallPlan> {
        Ok(InstallPlan::manual(
            resolved.clone(),
            vec!["plan winget manifest installer selection without invoking winget as the source model".to_string()],
        ))
    }
}

#[cfg(any(target_os = "windows", feature = "portable"))]
pub const fn is_host_enabled() -> bool {
    true
}

#[cfg(not(any(target_os = "windows", feature = "portable")))]
pub const fn is_host_enabled() -> bool {
    false
}
