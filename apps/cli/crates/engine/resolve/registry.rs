//! Feature- and target-aware registry for compiled source crates.

use crate::platform::PlatformId;
use crate::resolve::source::{SourceCapability, SourceResolver};
use crate::specs::item::ItemKind;

#[allow(dead_code)]
const TOOL: &[ItemKind] = &[ItemKind::Tool];
#[allow(dead_code)]
const PACKAGE: &[ItemKind] = &[ItemKind::Package];
#[allow(dead_code)]
const APP: &[ItemKind] = &[ItemKind::App];
#[allow(dead_code)]
const TOOL_PACKAGE_APP: &[ItemKind] = &[ItemKind::Tool, ItemKind::Package, ItemKind::App];
#[allow(dead_code)]
const PACKAGE_APP: &[ItemKind] = &[ItemKind::Package, ItemKind::App];
#[allow(dead_code)]
const ALL_PLATFORMS: &[PlatformId] = &[PlatformId::Macos, PlatformId::Linux, PlatformId::Windows];
#[allow(dead_code)]
const MACOS: &[PlatformId] = &[PlatformId::Macos];
#[allow(dead_code)]
const LINUX: &[PlatformId] = &[PlatformId::Linux];
#[allow(dead_code)]
const WINDOWS: &[PlatformId] = &[PlatformId::Windows];

/// Source capability compiled into this engine build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledSource {
    pub capability: SourceCapability,
}

/// Registry of source crates available in this binary.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceRegistry {
    sources: Vec<CompiledSource>,
}

impl SourceRegistry {
    /// Builds a registry from enabled engine source features and target OS cfg.
    pub fn bundled() -> Self {
        let mut registry = Self::default();

        register_sources(&mut registry);

        registry
    }

    /// Returns the compiled source capabilities.
    pub fn sources(&self) -> &[CompiledSource] {
        &self.sources
    }

    /// Builds a resolver from compiled source capabilities.
    pub fn resolver(&self) -> SourceResolver {
        SourceResolver::with_sources(
            self.sources
                .iter()
                .map(|source| source.capability.clone())
                .collect(),
        )
    }

    #[allow(dead_code)]
    fn push(
        &mut self,
        id: &'static str,
        kinds: &'static [ItemKind],
        platforms: &'static [PlatformId],
    ) {
        self.sources.push(CompiledSource {
            capability: SourceCapability {
                id,
                kinds,
                platforms,
            },
        });
    }
}

#[allow(unused_variables)]
fn register_sources(registry: &mut SourceRegistry) {
    #[cfg(feature = "source-aqua")]
    if still_source_aqua::is_host_enabled() {
        registry.push(still_source_aqua::SOURCE_ID, TOOL, ALL_PLATFORMS);
    }

    #[cfg(feature = "source-homebrew")]
    if still_source_homebrew::is_host_enabled() {
        registry.push(still_source_homebrew::SOURCE_ID, TOOL_PACKAGE_APP, MACOS);
    }

    #[cfg(feature = "source-cargo")]
    if still_source_cargo::is_host_enabled() {
        registry.push(still_source_cargo::SOURCE_ID, TOOL, ALL_PLATFORMS);
    }

    #[cfg(feature = "source-npm")]
    if still_source_npm::is_host_enabled() {
        registry.push(still_source_npm::SOURCE_ID, TOOL, ALL_PLATFORMS);
    }

    #[cfg(feature = "source-pipx")]
    if still_source_pipx::is_host_enabled() {
        registry.push(still_source_pipx::SOURCE_ID, TOOL, ALL_PLATFORMS);
    }

    #[cfg(feature = "source-go")]
    if still_source_go::is_host_enabled() {
        registry.push(still_source_go::SOURCE_ID, TOOL, ALL_PLATFORMS);
    }

    #[cfg(feature = "source-apt")]
    if still_source_apt::is_host_enabled() {
        registry.push(still_source_apt::SOURCE_ID, PACKAGE, LINUX);
    }

    #[cfg(feature = "source-dnf")]
    if still_source_dnf::is_host_enabled() {
        registry.push(still_source_dnf::SOURCE_ID, PACKAGE, LINUX);
    }

    #[cfg(feature = "source-pacman")]
    if still_source_pacman::is_host_enabled() {
        registry.push(still_source_pacman::SOURCE_ID, PACKAGE, LINUX);
    }

    #[cfg(feature = "source-winget")]
    if still_source_winget::is_host_enabled() {
        registry.push(still_source_winget::SOURCE_ID, PACKAGE_APP, WINDOWS);
    }

    #[cfg(feature = "source-flatpak")]
    if still_source_flatpak::is_host_enabled() {
        registry.push(still_source_flatpak::SOURCE_ID, APP, LINUX);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::{Architecture, HostPlatform, current_platform};

    #[test]
    fn bundled_registry_matches_enabled_features_for_host() {
        let registry = SourceRegistry::bundled();
        let source_ids = registry
            .sources()
            .iter()
            .map(|source| source.capability.id)
            .collect::<Vec<_>>();

        #[cfg(not(feature = "sources"))]
        assert!(source_ids.is_empty());

        #[cfg(all(feature = "sources", target_os = "macos"))]
        assert!(source_ids.contains(&"homebrew"));
        #[cfg(all(feature = "sources", target_os = "linux"))]
        assert!(source_ids.contains(&"apt"));
        #[cfg(all(feature = "sources", target_os = "windows"))]
        assert!(source_ids.contains(&"winget"));
    }

    #[test]
    fn compiled_registry_can_feed_source_resolver() {
        let registry = SourceRegistry::bundled();
        let platform = HostPlatform::new(current_platform(), Architecture::detect());
        let candidates = registry.resolver().candidates(ItemKind::Tool, platform);

        #[cfg(not(feature = "sources"))]
        assert!(candidates.is_empty());

        #[cfg(feature = "sources")]
        assert!(!candidates.is_empty());
    }
}
