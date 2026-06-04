//! Feature- and target-aware registry for compiled source crates.

#[cfg(any(feature = "source-kit", test))]
use crate::platform::PlatformId;
use crate::resolve::source::{SourceCapability, SourceResolver};
#[cfg(any(feature = "source-kit", test))]
use crate::specs::item::ItemKind;

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

    fn push_capability(&mut self, capability: SourceCapability) {
        self.sources.push(CompiledSource {
            capability,
        });
    }

    #[cfg(feature = "source-kit")]
    fn push_descriptor(&mut self, descriptor: still_source_kit::SourceDescriptor) {
        self.push_capability(SourceCapability {
            id: descriptor.id,
            kinds: convert_kinds(descriptor.item_kinds),
            platforms: convert_platforms(descriptor.target_os),
        });
    }
}

#[allow(unused_variables)]
fn register_sources(registry: &mut SourceRegistry) {
    #[cfg(feature = "source-aqua")]
    if still_source_aqua::is_host_enabled() {
        registry.push_descriptor(still_source_aqua::descriptor());
    }

    #[cfg(feature = "source-homebrew")]
    if still_source_homebrew::is_host_enabled() {
        registry.push_descriptor(still_source_homebrew::descriptor());
    }

    #[cfg(feature = "source-cargo")]
    if still_source_cargo::is_host_enabled() {
        registry.push_descriptor(still_source_cargo::descriptor());
    }

    #[cfg(feature = "source-npm")]
    if still_source_npm::is_host_enabled() {
        registry.push_descriptor(still_source_npm::descriptor());
    }

    #[cfg(feature = "source-pipx")]
    if still_source_pipx::is_host_enabled() {
        registry.push_descriptor(still_source_pipx::descriptor());
    }

    #[cfg(feature = "source-go")]
    if still_source_go::is_host_enabled() {
        registry.push_descriptor(still_source_go::descriptor());
    }

    #[cfg(feature = "source-apt")]
    if still_source_apt::is_host_enabled() {
        registry.push_descriptor(still_source_apt::descriptor());
    }

    #[cfg(feature = "source-dnf")]
    if still_source_dnf::is_host_enabled() {
        registry.push_descriptor(still_source_dnf::descriptor());
    }

    #[cfg(feature = "source-pacman")]
    if still_source_pacman::is_host_enabled() {
        registry.push_descriptor(still_source_pacman::descriptor());
    }

    #[cfg(feature = "source-winget")]
    if still_source_winget::is_host_enabled() {
        registry.push_descriptor(still_source_winget::descriptor());
    }

    #[cfg(feature = "source-flatpak")]
    if still_source_flatpak::is_host_enabled() {
        registry.push_descriptor(still_source_flatpak::descriptor());
    }
}

#[cfg(feature = "source-kit")]
fn convert_kinds(kinds: &'static [still_source_kit::ItemKind]) -> &'static [ItemKind] {
    match kinds {
        [still_source_kit::ItemKind::Tool] => &[ItemKind::Tool],
        [still_source_kit::ItemKind::Package] => &[ItemKind::Package],
        [still_source_kit::ItemKind::App] => &[ItemKind::App],
        [
            still_source_kit::ItemKind::Tool,
            still_source_kit::ItemKind::Package,
            still_source_kit::ItemKind::App,
        ] => &[ItemKind::Tool, ItemKind::Package, ItemKind::App],
        [still_source_kit::ItemKind::Package, still_source_kit::ItemKind::App] => {
            &[ItemKind::Package, ItemKind::App]
        }
        _ => &[],
    }
}

#[cfg(feature = "source-kit")]
fn convert_platforms(platforms: &'static [still_source_kit::TargetOs]) -> &'static [PlatformId] {
    match platforms {
        [still_source_kit::TargetOs::Macos] => &[PlatformId::Macos],
        [still_source_kit::TargetOs::Linux] => &[PlatformId::Linux],
        [still_source_kit::TargetOs::Windows] => &[PlatformId::Windows],
        [
            still_source_kit::TargetOs::Macos,
            still_source_kit::TargetOs::Linux,
            still_source_kit::TargetOs::Windows,
        ] => &[PlatformId::Macos, PlatformId::Linux, PlatformId::Windows],
        _ => &[],
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
