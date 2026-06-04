//! Source selection models for tools, packages, and apps.

use std::{fmt, str::FromStr};

use crate::error::{EngineError, EngineResult};
use crate::platform::{HostPlatform, PlatformId};
use crate::specs::item::ItemKind;

/// Source family selected for metadata and artifact resolution.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceId(String);

impl SourceId {
    /// Creates a source id after validating config or caller input.
    pub fn new(value: impl Into<String>) -> EngineResult<Self> {
        let value = value.into();
        validate_source_id(&value)?;
        Ok(Self(value))
    }

    /// Returns the normalized source id.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for SourceId {
    type Err = EngineError;

    fn from_str(input: &str) -> EngineResult<Self> {
        Self::new(input)
    }
}

/// Caller intent before source resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceIntent {
    Auto,
    Explicit(SourceId),
}

impl SourceIntent {
    /// Normalizes absent, blank, or literal `auto` input into automatic selection.
    pub fn from_optional(value: Option<&str>) -> EngineResult<Self> {
        match value.map(str::trim).filter(|value| !value.is_empty()) {
            Some("auto") | None => Ok(Self::Auto),
            Some(value) => Ok(Self::Explicit(SourceId::new(value)?)),
        }
    }

    /// Returns the explicit source id when the caller disabled fallback.
    pub fn explicit(&self) -> Option<&SourceId> {
        match self {
            Self::Auto => None,
            Self::Explicit(source) => Some(source),
        }
    }
}

/// Source selected for one request after auto/default normalization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSelection {
    pub intent: SourceIntent,
    pub candidates: Vec<SourceId>,
}

impl SourceSelection {
    /// Returns the first source Still should attempt during resolution.
    pub fn primary(&self) -> Option<&SourceId> {
        match &self.intent {
            SourceIntent::Explicit(source) => Some(source),
            SourceIntent::Auto => self.candidates.first(),
        }
    }

    /// Returns whether fallback to later candidates is allowed.
    pub fn allows_fallback(&self) -> bool {
        matches!(self.intent, SourceIntent::Auto)
    }
}

/// Resolves source intent against compiled source capabilities and host data.
#[derive(Debug, Clone)]
pub struct SourceResolver {
    sources: Vec<SourceCapability>,
}

impl SourceResolver {
    /// Builds a resolver from the source families currently modeled by Still.
    pub fn bundled() -> Self {
        Self {
            sources: bundled_source_capabilities().to_vec(),
        }
    }

    /// Builds a resolver with injected source capabilities for tests.
    pub fn with_sources(sources: Vec<SourceCapability>) -> Self {
        Self { sources }
    }

    /// Resolves a source for an item kind on a host platform.
    /// # Errors
    /// Fails when an explicit source is unknown, unsupported for the item kind,
    /// unsupported for the platform, or when auto has no viable candidates.
    pub fn resolve(
        &self,
        kind: ItemKind,
        intent: SourceIntent,
        platform: HostPlatform,
    ) -> EngineResult<SourceSelection> {
        match intent {
            SourceIntent::Explicit(source) => {
                self.validate_explicit_source(kind, &source, platform)?;
                Ok(SourceSelection {
                    intent: SourceIntent::Explicit(source),
                    candidates: Vec::new(),
                })
            }
            SourceIntent::Auto => {
                let candidates = self.candidates(kind, platform);
                if candidates.is_empty() {
                    return Err(EngineError::NoSourceCandidates {
                        kind: kind.to_string(),
                        platform: platform.id().to_string(),
                    });
                }
                Ok(SourceSelection {
                    intent: SourceIntent::Auto,
                    candidates,
                })
            }
        }
    }

    /// Returns auto source candidates in product order for the item kind and host.
    pub fn candidates(&self, kind: ItemKind, platform: HostPlatform) -> Vec<SourceId> {
        auto_order(kind, platform.id())
            .iter()
            .filter(|source| {
                self.find(source)
                    .is_some_and(|capability| capability.supports(kind, platform.id()))
            })
            .map(|source| SourceId::new(*source).expect("bundled source ids are valid"))
            .collect()
    }

    fn validate_explicit_source(
        &self,
        kind: ItemKind,
        source: &SourceId,
        platform: HostPlatform,
    ) -> EngineResult<()> {
        let Some(capability) = self.find(source.as_str()) else {
            return Err(EngineError::UnknownSource {
                id: source.to_string(),
            });
        };
        if !capability.kinds.contains(&kind) {
            return Err(EngineError::UnsupportedSourceForKind {
                id: source.to_string(),
                kind: kind.to_string(),
            });
        }
        if !capability.platforms.contains(&platform.id()) {
            return Err(EngineError::UnsupportedSourceForPlatform {
                id: source.to_string(),
                platform: platform.id().to_string(),
            });
        }
        Ok(())
    }

    fn find(&self, source: &str) -> Option<&SourceCapability> {
        self.sources
            .iter()
            .find(|capability| capability.id == source)
    }
}

/// Source capabilities advertised by a future `still-source-*` implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceCapability {
    pub id: &'static str,
    pub kinds: &'static [ItemKind],
    pub platforms: &'static [PlatformId],
}

impl SourceCapability {
    fn supports(&self, kind: ItemKind, platform: PlatformId) -> bool {
        self.kinds.contains(&kind) && self.platforms.contains(&platform)
    }
}

const TOOL: &[ItemKind] = &[ItemKind::Tool];
const PACKAGE: &[ItemKind] = &[ItemKind::Package];
const APP: &[ItemKind] = &[ItemKind::App];
const TOOL_PACKAGE_APP: &[ItemKind] = &[ItemKind::Tool, ItemKind::Package, ItemKind::App];
const PACKAGE_APP: &[ItemKind] = &[ItemKind::Package, ItemKind::App];
const ALL_PLATFORMS: &[PlatformId] = &[PlatformId::Macos, PlatformId::Linux, PlatformId::Windows];
const MACOS: &[PlatformId] = &[PlatformId::Macos];
const LINUX: &[PlatformId] = &[PlatformId::Linux];
const WINDOWS: &[PlatformId] = &[PlatformId::Windows];

fn bundled_source_capabilities() -> &'static [SourceCapability] {
    &[
        SourceCapability {
            id: "aqua",
            kinds: TOOL,
            platforms: ALL_PLATFORMS,
        },
        SourceCapability {
            id: "homebrew",
            kinds: TOOL_PACKAGE_APP,
            platforms: MACOS,
        },
        SourceCapability {
            id: "cargo",
            kinds: TOOL,
            platforms: ALL_PLATFORMS,
        },
        SourceCapability {
            id: "npm",
            kinds: TOOL,
            platforms: ALL_PLATFORMS,
        },
        SourceCapability {
            id: "pipx",
            kinds: TOOL,
            platforms: ALL_PLATFORMS,
        },
        SourceCapability {
            id: "go",
            kinds: TOOL,
            platforms: ALL_PLATFORMS,
        },
        SourceCapability {
            id: "apt",
            kinds: PACKAGE,
            platforms: LINUX,
        },
        SourceCapability {
            id: "dnf",
            kinds: PACKAGE,
            platforms: LINUX,
        },
        SourceCapability {
            id: "pacman",
            kinds: PACKAGE,
            platforms: LINUX,
        },
        SourceCapability {
            id: "winget",
            kinds: PACKAGE_APP,
            platforms: WINDOWS,
        },
        SourceCapability {
            id: "flatpak",
            kinds: APP,
            platforms: LINUX,
        },
    ]
}

fn auto_order(kind: ItemKind, platform: PlatformId) -> &'static [&'static str] {
    match (kind, platform) {
        (ItemKind::Tool, PlatformId::Macos) => &["aqua", "homebrew", "cargo", "npm", "pipx", "go"],
        (ItemKind::Tool, PlatformId::Linux | PlatformId::Windows) => {
            &["aqua", "cargo", "npm", "pipx", "go"]
        }
        (ItemKind::Package, PlatformId::Macos) => &["homebrew"],
        (ItemKind::Package, PlatformId::Linux) => &["apt", "dnf", "pacman"],
        (ItemKind::Package, PlatformId::Windows) => &["winget"],
        (ItemKind::App, PlatformId::Macos) => &["homebrew"],
        (ItemKind::App, PlatformId::Linux) => &["flatpak"],
        (ItemKind::App, PlatformId::Windows) => &["winget"],
    }
}

fn validate_source_id(value: &str) -> EngineResult<()> {
    if value.is_empty() {
        return Err(EngineError::InvalidItemSpec {
            reason: "source name cannot be empty".to_string(),
        });
    }
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return Err(EngineError::InvalidItemSpec {
            reason: "source name cannot be empty".to_string(),
        });
    };
    if !first.is_ascii_alphanumeric() {
        return Err(EngineError::InvalidItemSpec {
            reason: "source name must start with a letter or number".to_string(),
        });
    }
    for c in chars {
        if !(c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.')) {
            return Err(EngineError::InvalidItemSpec {
                reason: format!("source name contains invalid character '{c}'"),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::platform::Architecture;

    use super::*;

    #[test]
    fn auto_tool_candidates_follow_product_order_for_macos() {
        let resolver = SourceResolver::bundled();
        let platform = HostPlatform::new(PlatformId::Macos, Architecture::Aarch64);
        let candidates = resolver.candidates(ItemKind::Tool, platform);

        assert_eq!(
            candidates.iter().map(SourceId::as_str).collect::<Vec<_>>(),
            ["aqua", "homebrew", "cargo", "npm", "pipx", "go"]
        );
    }

    #[test]
    fn auto_package_candidates_follow_product_order_for_linux() {
        let resolver = SourceResolver::bundled();
        let platform = HostPlatform::new(PlatformId::Linux, Architecture::X86_64);
        let selection = resolver
            .resolve(ItemKind::Package, SourceIntent::Auto, platform)
            .unwrap();

        assert!(selection.allows_fallback());
        assert_eq!(
            selection
                .candidates
                .iter()
                .map(SourceId::as_str)
                .collect::<Vec<_>>(),
            ["apt", "dnf", "pacman"]
        );
    }

    #[test]
    fn explicit_source_selection_does_not_include_fallback_candidates() {
        let resolver = SourceResolver::bundled();
        let platform = HostPlatform::new(PlatformId::Linux, Architecture::X86_64);
        let selection = resolver
            .resolve(
                ItemKind::Tool,
                SourceIntent::Explicit(SourceId::new("cargo").unwrap()),
                platform,
            )
            .unwrap();

        assert!(!selection.allows_fallback());
        assert_eq!(selection.primary().unwrap().as_str(), "cargo");
        assert!(selection.candidates.is_empty());
    }

    #[test]
    fn explicit_source_validation_rejects_unknown_sources() {
        let resolver = SourceResolver::bundled();
        let platform = HostPlatform::new(PlatformId::Linux, Architecture::X86_64);
        let err = resolver
            .resolve(
                ItemKind::Tool,
                SourceIntent::Explicit(SourceId::new("unknown").unwrap()),
                platform,
            )
            .unwrap_err();

        assert!(matches!(
            err,
            EngineError::UnknownSource { id } if id == "unknown"
        ));
    }

    #[test]
    fn explicit_source_validation_rejects_wrong_item_kind() {
        let resolver = SourceResolver::bundled();
        let platform = HostPlatform::new(PlatformId::Linux, Architecture::X86_64);
        let err = resolver
            .resolve(
                ItemKind::Package,
                SourceIntent::Explicit(SourceId::new("cargo").unwrap()),
                platform,
            )
            .unwrap_err();

        assert!(matches!(
            err,
            EngineError::UnsupportedSourceForKind { id, kind }
                if id == "cargo" && kind == "package"
        ));
    }

    #[test]
    fn explicit_source_validation_rejects_wrong_platform() {
        let resolver = SourceResolver::bundled();
        let platform = HostPlatform::new(PlatformId::Windows, Architecture::X86_64);
        let err = resolver
            .resolve(
                ItemKind::Package,
                SourceIntent::Explicit(SourceId::new("apt").unwrap()),
                platform,
            )
            .unwrap_err();

        assert!(matches!(
            err,
            EngineError::UnsupportedSourceForPlatform { id, platform }
                if id == "apt" && platform == "windows"
        ));
    }

    #[test]
    fn auto_can_report_no_candidates_from_injected_sources() {
        let resolver = SourceResolver::with_sources(Vec::new());
        let platform = HostPlatform::new(PlatformId::Windows, Architecture::X86_64);
        let err = resolver
            .resolve(ItemKind::App, SourceIntent::Auto, platform)
            .unwrap_err();

        assert!(matches!(
            err,
            EngineError::NoSourceCandidates { kind, platform }
                if kind == "app" && platform == "windows"
        ));
    }

    #[test]
    fn source_intent_normalizes_auto_and_explicit_values() {
        assert_eq!(
            SourceIntent::from_optional(None).unwrap(),
            SourceIntent::Auto
        );
        assert_eq!(
            SourceIntent::from_optional(Some("auto")).unwrap(),
            SourceIntent::Auto
        );
        assert_eq!(
            SourceIntent::from_optional(Some("cargo"))
                .unwrap()
                .explicit()
                .unwrap()
                .as_str(),
            "cargo"
        );
    }
}
