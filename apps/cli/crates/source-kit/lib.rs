//! Shared contracts and planning types for Still source implementations.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Result type used by source-kit traits and source crates.
pub type SourceResult<T> = Result<T, SourceError>;

/// Errors returned while reading source metadata or creating install plans.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SourceError {
    #[error("{source_id} does not support {operation}")]
    Unsupported {
        source_id: &'static str,
        operation: &'static str,
    },
    #[error("{source_id} metadata is invalid: {reason}")]
    InvalidMetadata {
        source_id: &'static str,
        reason: String,
    },
    #[error("{source_id} could not resolve {name}: {reason}")]
    ResolutionFailed {
        source_id: &'static str,
        name: String,
        reason: String,
    },
    #[error("{source_id} cannot plan installation for {name}: {reason}")]
    PlanningFailed {
        source_id: &'static str,
        name: String,
        reason: String,
    },
}

impl SourceError {
    /// Creates an unsupported-operation error for a source scaffold.
    pub fn unsupported(source_id: &'static str, operation: &'static str) -> Self {
        Self::Unsupported {
            source_id,
            operation,
        }
    }
}

/// Stable Still item kinds a source can provide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ItemKind {
    Tool,
    Package,
    App,
}

/// Operating systems used for source compatibility decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TargetOs {
    Macos,
    Linux,
    Windows,
}

/// CPU architecture used when selecting source artifacts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TargetArch {
    X86_64,
    Aarch64,
    Armv7,
    I686,
}

/// Host or target platform used by resolvers and planners.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TargetTriple {
    pub os: TargetOs,
    pub arch: TargetArch,
}

impl TargetTriple {
    /// Creates a target identity from a Still OS and architecture.
    pub const fn new(os: TargetOs, arch: TargetArch) -> Self {
        Self { os, arch }
    }
}

/// Version requested by config or CLI input before source resolution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VersionRequest {
    Latest,
    Exact(String),
}

impl Default for VersionRequest {
    fn default() -> Self {
        Self::Latest
    }
}

/// One item lookup after parsing and source selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolveRequest {
    pub kind: ItemKind,
    pub name: String,
    #[serde(default)]
    pub version: VersionRequest,
    pub target: TargetTriple,
}

/// Metadata formats accepted by a source crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MetadataFormat {
    Json,
    Toml,
    Yaml,
    Xml,
    PlainText,
    Index,
}

/// Raw source metadata captured before source-specific parsing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawSourceMetadata {
    pub format: MetadataFormat,
    pub location: Option<String>,
    pub bytes: Vec<u8>,
}

impl RawSourceMetadata {
    /// Captures a raw metadata document with an optional source location.
    pub fn new(format: MetadataFormat, location: Option<String>, bytes: Vec<u8>) -> Self {
        Self {
            format,
            location,
            bytes,
        }
    }
}

/// Parsed or preserved metadata for one source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataInventory {
    pub source_id: String,
    pub entries: Vec<MetadataEntry>,
    pub raw_documents: Vec<RawSourceMetadata>,
}

impl MetadataInventory {
    /// Creates an empty inventory for tests or source metadata that is not parsed yet.
    pub fn empty(source_id: impl Into<String>) -> Self {
        Self {
            source_id: source_id.into(),
            entries: Vec::new(),
            raw_documents: Vec::new(),
        }
    }

    /// Preserves one raw metadata document until a source-specific parser is added.
    pub fn from_raw(source_id: impl Into<String>, raw: RawSourceMetadata) -> Self {
        Self {
            source_id: source_id.into(),
            entries: Vec::new(),
            raw_documents: vec![raw],
        }
    }
}

/// One package, tool, or app discovered in source metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataEntry {
    pub kind: ItemKind,
    pub name: String,
    pub version: String,
    pub summary: Option<String>,
    pub dependencies: Vec<SourceDependency>,
    pub artifacts: Vec<SourceArtifact>,
    pub raw: BTreeMap<String, String>,
}

/// Source-level dependency discovered from raw metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceDependency {
    pub name: String,
    pub kind: Option<ItemKind>,
    pub version: Option<VersionRequest>,
    pub optional: bool,
}

/// Resolved installable item returned by a source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedItem {
    pub source_id: String,
    pub kind: ItemKind,
    pub name: String,
    pub version: String,
    pub target: TargetTriple,
    pub dependencies: Vec<SourceDependency>,
    pub artifacts: Vec<SourceArtifact>,
    pub metadata: BTreeMap<String, String>,
}

/// Downloadable, buildable, or locally staged artifact selected by a source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceArtifact {
    pub location: ArtifactLocation,
    pub format: ArtifactFormat,
    pub checksum: Option<Checksum>,
    pub target: Option<TargetTriple>,
}

/// Location for an artifact before Still stages it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArtifactLocation {
    Url(String),
    RegistryObject(String),
    LocalPath(PathBuf),
    SourceTree(String),
}

/// File or directory shape expected by source-kit installers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArtifactFormat {
    TarGz,
    TarXz,
    Tgz,
    Zip,
    Executable,
    Directory,
    SourceTree,
    Unknown(String),
}

/// Integrity metadata attached to a source artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checksum {
    pub algorithm: ChecksumAlgorithm,
    pub value: String,
}

/// Checksum algorithms understood by source-kit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChecksumAlgorithm {
    Sha256,
    Sha512,
    Blake3,
}

/// Filesystem context needed to build a side-effect-free install plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallContext {
    pub install_root: PathBuf,
    pub cache_root: PathBuf,
    pub bin_dir: PathBuf,
}

/// Source-side install model advertised to the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstallModel {
    DownloadArtifact,
    BuildFromSource,
    VirtualEnvironment,
    SystemPackageMetadata,
    AppBundle,
    Manual,
}

/// Side-effect-free install plan produced by a source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallPlan {
    pub item: ResolvedItem,
    pub steps: Vec<InstallStep>,
    pub receipt: InstallReceipt,
}

impl InstallPlan {
    /// Creates a plan that records manual instructions without shelling out.
    pub fn manual(item: ResolvedItem, instructions: Vec<String>) -> Self {
        Self {
            receipt: InstallReceipt::for_item(&item),
            item,
            steps: vec![InstallStep::Manual { instructions }],
        }
    }
}

/// One planned action for a future executor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstallStep {
    Download {
        artifact: SourceArtifact,
        destination: PathBuf,
    },
    VerifyChecksum {
        path: PathBuf,
        checksum: Checksum,
    },
    ExtractArchive {
        archive: PathBuf,
        destination: PathBuf,
    },
    StageFiles {
        source: PathBuf,
        destination: PathBuf,
    },
    LinkExecutable {
        source: PathBuf,
        link: PathBuf,
    },
    BuildFromSource {
        source: ArtifactLocation,
        destination: PathBuf,
    },
    CreateVirtualEnvironment {
        package: String,
        destination: PathBuf,
    },
    RegisterApp {
        bundle_path: PathBuf,
    },
    Manual {
        instructions: Vec<String>,
    },
}

/// Receipt data a source expects the executor to persist after installation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallReceipt {
    pub source_id: String,
    pub name: String,
    pub version: String,
    pub files: Vec<PathBuf>,
    pub executables: Vec<PathBuf>,
    pub metadata: BTreeMap<String, String>,
}

impl InstallReceipt {
    /// Creates an empty receipt identity for a resolved item.
    pub fn for_item(item: &ResolvedItem) -> Self {
        Self {
            source_id: item.source_id.clone(),
            name: item.name.clone(),
            version: item.version.clone(),
            files: Vec::new(),
            executables: Vec::new(),
            metadata: item.metadata.clone(),
        }
    }
}

/// Static capability metadata for a source crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceDescriptor {
    pub id: &'static str,
    pub display_name: &'static str,
    pub item_kinds: &'static [ItemKind],
    pub target_os: &'static [TargetOs],
    pub metadata_formats: &'static [MetadataFormat],
    pub install_models: &'static [InstallModel],
}

impl SourceDescriptor {
    /// Returns whether this source advertises support for an item kind.
    pub fn supports_kind(&self, kind: ItemKind) -> bool {
        self.item_kinds.contains(&kind)
    }

    /// Returns whether this source advertises support for a target OS.
    pub fn supports_os(&self, os: TargetOs) -> bool {
        self.target_os.contains(&os)
    }
}

/// Common behavior implemented by every Still source crate.
pub trait Source {
    /// Returns static source capabilities used during source selection.
    fn descriptor(&self) -> SourceDescriptor;

    /// Converts raw metadata into Still's intermediate source inventory.
    fn parse_metadata(&self, raw: RawSourceMetadata) -> SourceResult<MetadataInventory>;

    /// Resolves a typed request against parsed source metadata.
    fn resolve(
        &self,
        request: &ResolveRequest,
        inventory: &MetadataInventory,
    ) -> SourceResult<ResolvedItem>;

    /// Creates a side-effect-free plan for a resolved item.
    fn plan_install(
        &self,
        resolved: &ResolvedItem,
        context: &InstallContext,
    ) -> SourceResult<InstallPlan>;
}
