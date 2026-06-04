//! Normalized desired state used as the boundary between config and planning.

use std::collections::BTreeMap;

use crate::config::model::{PackageEntry, PackageMap, StillConfig, ToolEntry};
use crate::error::EngineResult;
use crate::platform::{PlatformFilter, PlatformId};
use crate::resolve::SourceIntent;
use crate::specs::item::{ItemKind, VersionReq};

/// Config scope that contributed a desired-state item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DesiredScope {
    Project,
    Global,
}

/// Source selection intent normalized from config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesiredSourceIntent {
    Auto,
    Explicit(crate::resolve::SourceId),
}

impl DesiredSourceIntent {
    fn from_config(value: Option<&str>) -> EngineResult<Self> {
        Ok(match SourceIntent::from_optional(value)? {
            SourceIntent::Auto => Self::Auto,
            SourceIntent::Explicit(source) => Self::Explicit(source),
        })
    }
}

/// One normalized tool, package, or app requested by config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesiredItem {
    pub scope: DesiredScope,
    pub kind: ItemKind,
    pub logical_name: String,
    pub resolved_name: String,
    pub version: VersionReq,
    pub source: DesiredSourceIntent,
    pub platform: PlatformId,
}

/// Fully normalized desired state for tools, packages, and apps.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DesiredState {
    pub items: Vec<DesiredItem>,
}

impl DesiredState {
    /// Normalizes one parsed config for the selected platform.
    /// # Errors
    /// Fails when platform filters or source identifiers are invalid.
    pub fn from_config(
        config: &StillConfig,
        scope: DesiredScope,
        platform: PlatformId,
    ) -> EngineResult<Self> {
        let mut items = Vec::new();
        normalize_tools(config, scope, platform, &mut items)?;
        normalize_package_map(
            &config.packages,
            scope,
            ItemKind::Package,
            platform,
            &mut items,
        )?;
        normalize_package_map(&config.apps, scope, ItemKind::App, platform, &mut items)?;
        Ok(Self { items })
    }

    /// Merges project and global state with project entries taking precedence.
    pub fn merge_project_over_global(project: Self, global: Self) -> Self {
        let mut by_identity = BTreeMap::new();
        for item in global.items {
            by_identity.insert((item.kind, item.logical_name.clone()), item);
        }
        for item in project.items {
            by_identity.insert((item.kind, item.logical_name.clone()), item);
        }
        Self {
            items: by_identity.into_values().collect(),
        }
    }
}

fn normalize_tools(
    config: &StillConfig,
    scope: DesiredScope,
    platform: PlatformId,
    items: &mut Vec<DesiredItem>,
) -> EngineResult<()> {
    for (name, entry) in &config.tools {
        let (version, source) = match entry {
            ToolEntry::Version(version) => (version.as_str(), None),
            ToolEntry::Expanded(tool) => (
                tool.version.as_str(),
                source_for_platform(tool.backend.as_deref(), &tool.backends, platform),
            ),
        };
        items.push(DesiredItem {
            scope,
            kind: ItemKind::Tool,
            logical_name: name.clone(),
            resolved_name: name.clone(),
            version: parse_version(version),
            source: DesiredSourceIntent::from_config(source.as_deref())?,
            platform,
        });
    }
    Ok(())
}

fn normalize_package_map(
    map: &PackageMap,
    scope: DesiredScope,
    kind: ItemKind,
    platform: PlatformId,
    items: &mut Vec<DesiredItem>,
) -> EngineResult<()> {
    for name in &map.latest {
        items.push(DesiredItem {
            scope,
            kind,
            logical_name: name.clone(),
            resolved_name: name.clone(),
            version: VersionReq::default(),
            source: DesiredSourceIntent::Auto,
            platform,
        });
    }

    for (name, entry) in &map.entries {
        let PackageEntry::Expanded(package) = entry;
        let platforms = package.platforms.clone().unwrap_or_default();
        let filter = PlatformFilter::from_config(
            &platforms,
            package.ignore.as_deref(),
            package.only.as_deref(),
        )?;
        if !filter.matches(platform) {
            continue;
        }
        let resolved_name = package
            .names
            .get(platform.to_string().as_str())
            .cloned()
            .unwrap_or_else(|| name.clone());
        let source = source_for_platform(package.backend.as_deref(), &package.backends, platform);
        items.push(DesiredItem {
            scope,
            kind,
            logical_name: name.clone(),
            resolved_name,
            version: package
                .version
                .as_deref()
                .map(parse_version)
                .unwrap_or_default(),
            source: DesiredSourceIntent::from_config(source.as_deref())?,
            platform,
        });
    }
    Ok(())
}

fn source_for_platform(
    fallback: Option<&str>,
    per_platform: &BTreeMap<String, String>,
    platform: PlatformId,
) -> Option<String> {
    per_platform
        .get(platform.to_string().as_str())
        .cloned()
        .or_else(|| fallback.map(str::to_string))
}

fn parse_version(value: &str) -> VersionReq {
    VersionReq::new(value).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use crate::config::parse::parse_still_toml;

    use super::*;

    #[test]
    fn normalizes_source_aliases_and_platform_names() {
        let config = parse_still_toml(
            r#"
            [tools]
            rust = { version = "stable", source = "rustup" }

            [packages.fd]
            version = "latest"
            source = "auto"
            names = { linux = "fd-find" }
            platforms = ["linux"]
            "#,
        )
        .unwrap();

        let desired =
            DesiredState::from_config(&config, DesiredScope::Project, PlatformId::Linux).unwrap();

        assert_eq!(desired.items.len(), 2);
        assert!(matches!(
            desired.items[0].source,
            DesiredSourceIntent::Explicit(_)
        ));
        assert_eq!(desired.items[1].resolved_name, "fd-find");
        assert_eq!(desired.items[1].source, DesiredSourceIntent::Auto);
    }

    #[test]
    fn skips_items_outside_platform_filter() {
        let config = parse_still_toml(
            r#"
            [packages.fd]
            platforms = ["linux"]
            "#,
        )
        .unwrap();

        let desired =
            DesiredState::from_config(&config, DesiredScope::Project, PlatformId::Macos).unwrap();

        assert!(desired.items.is_empty());
    }

    #[test]
    fn project_items_override_global_items_by_kind_and_logical_name() {
        let global = DesiredState {
            items: vec![item(DesiredScope::Global, ItemKind::Tool, "rust", "stable")],
        };
        let project = DesiredState {
            items: vec![item(
                DesiredScope::Project,
                ItemKind::Tool,
                "rust",
                "nightly",
            )],
        };

        let merged = DesiredState::merge_project_over_global(project, global);

        assert_eq!(merged.items.len(), 1);
        assert_eq!(merged.items[0].scope, DesiredScope::Project);
        assert_eq!(merged.items[0].version.as_str(), "nightly");
    }

    fn item(scope: DesiredScope, kind: ItemKind, name: &str, version: &str) -> DesiredItem {
        DesiredItem {
            scope,
            kind,
            logical_name: name.to_string(),
            resolved_name: name.to_string(),
            version: parse_version(version),
            source: DesiredSourceIntent::Auto,
            platform: PlatformId::Linux,
        }
    }
}
