//! Engine action for listing configured desired state.

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::config::{
    ConfigScope, ConfigSelection, find_project_config, global_config_path, resolve_config_path,
};
use crate::error::EngineError;
use crate::platform::{PlatformFilter, PlatformId, current_platform};
use crate::specs::backend::normalize_auto_backend;
use crate::specs::item::ItemKind;
use crate::specs::toml::{PackageEntry, PackageMap, StillConfig, ToolEntry, parse_still_toml};
use crate::system::System;
use crate::utils::paths::PathOps;

/// Request to list configured state.
#[derive(Debug, Clone)]
pub struct ListRequest {
    pub start_dir: PathBuf,
    pub home_dir: PathBuf,
    pub global: bool,
    pub all: bool,
}

/// Configured tools, packages, and apps from one selected config file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListResult {
    pub path: PathBuf,
    pub sections: Vec<ListSection>,
}

/// One item group in list output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListSection {
    pub kind: ItemKind,
    pub items: Vec<ListItem>,
}

/// One configured desired-state item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListItem {
    pub logical_name: String,
    pub name: String,
    pub version: String,
    pub backend: Option<String>,
    pub outputs: Vec<String>,
    pub linked_executables: Vec<String>,
    pub configured: bool,
    pub project: bool,
    pub global: bool,
    pub installed: bool,
}

#[derive(Debug, Deserialize)]
struct InstallMarker {
    kind: String,
    name: String,
    version: String,
    backend: Option<String>,
    #[serde(default)]
    outputs: Vec<String>,
    #[serde(default)]
    linked_executables: Vec<String>,
}

/// Reads selected config and returns configured items.
/// # Errors
/// Fails when the selected config cannot be read or parsed.
pub async fn inspect(request: ListRequest) -> Result<ListResult> {
    let selection = ConfigSelection {
        scope: if request.global {
            ConfigScope::Global
        } else {
            ConfigScope::Project
        },
        for_write: false,
    };
    let resolved = resolve_config_path(&request.start_dir, &request.home_dir, selection)?;
    let content = tokio::fs::read_to_string(&resolved.path)
        .await
        .with_context(|| format!("failed to read {}", resolved.path.display()))?;
    let config = parse_still_toml(&content)?;
    let mut sections = sections_from_config(config, resolved.scope, false)?;
    if !request.global && resolved.scope == ConfigScope::Project {
        merge_global_only_config_items(&mut sections, &request.home_dir).await?;
    }
    if request.all {
        merge_inactive_config_items(&mut sections, &request.start_dir, &request.home_dir).await?;
        merge_installed_items(&mut sections, discover_installed_items().await?);
    }

    Ok(ListResult {
        path: resolved.path,
        sections,
    })
}

async fn merge_global_only_config_items(
    sections: &mut [ListSection],
    home_dir: &std::path::Path,
) -> Result<()> {
    let global_path = global_config_path(home_dir);
    let content = match tokio::fs::read_to_string(&global_path).await {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => {
            return Err(err).with_context(|| format!("failed to read {}", global_path.display()));
        }
    };
    let config = parse_still_toml(&content)?;
    merge_global_only_items(
        sections,
        sections_from_config(config, ConfigScope::Global, false)?,
    );
    Ok(())
}

async fn merge_inactive_config_items(
    sections: &mut [ListSection],
    start_dir: &std::path::Path,
    home_dir: &std::path::Path,
) -> Result<()> {
    if let Some(project_path) = find_project_config(start_dir) {
        merge_config_path(sections, &project_path, ConfigScope::Project, true).await?;
    }

    let global_path = global_config_path(home_dir);
    merge_config_path(sections, &global_path, ConfigScope::Global, true).await?;

    Ok(())
}

async fn merge_config_path(
    sections: &mut [ListSection],
    path: &std::path::Path,
    scope: ConfigScope,
    include_inactive_platforms: bool,
) -> Result<()> {
    let content = match tokio::fs::read_to_string(path).await {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err).with_context(|| format!("failed to read {}", path.display())),
    };
    let config = parse_still_toml(&content)?;
    merge_config_items(
        sections,
        sections_from_config(config, scope, include_inactive_platforms)?,
    );
    Ok(())
}

fn sections_from_config(
    config: StillConfig,
    scope: ConfigScope,
    include_inactive_platforms: bool,
) -> Result<Vec<ListSection>> {
    let platform = current_platform();
    Ok(vec![
        ListSection {
            kind: ItemKind::Tool,
            items: tool_items(config.tools, scope, platform),
        },
        ListSection {
            kind: ItemKind::Package,
            items: package_items(
                ItemKind::Package,
                config.packages,
                scope,
                platform,
                include_inactive_platforms,
            )?,
        },
        ListSection {
            kind: ItemKind::App,
            items: package_items(
                ItemKind::App,
                config.apps,
                scope,
                platform,
                include_inactive_platforms,
            )?,
        },
    ])
}

fn tool_items(
    tools: BTreeMap<String, ToolEntry>,
    scope: ConfigScope,
    platform: PlatformId,
) -> Vec<ListItem> {
    tools
        .into_iter()
        .map(|(name, entry)| match entry {
            ToolEntry::Version(version) => ListItem {
                logical_name: name.clone(),
                name,
                version,
                backend: None,
                outputs: Vec::new(),
                linked_executables: Vec::new(),
                configured: true,
                project: scope == ConfigScope::Project,
                global: scope == ConfigScope::Global,
                installed: false,
            },
            ToolEntry::Expanded(tool) => ListItem {
                logical_name: name.clone(),
                name,
                version: if tool.version.is_empty() {
                    "latest".to_string()
                } else {
                    tool.version
                },
                backend: backend_for_platform(
                    ItemKind::Tool,
                    tool.backend,
                    tool.backends,
                    platform,
                ),
                outputs: Vec::new(),
                linked_executables: Vec::new(),
                configured: true,
                project: scope == ConfigScope::Project,
                global: scope == ConfigScope::Global,
                installed: false,
            },
        })
        .collect()
}

fn package_items(
    kind: ItemKind,
    map: PackageMap,
    scope: ConfigScope,
    platform: PlatformId,
    include_inactive_platforms: bool,
) -> Result<Vec<ListItem>> {
    let mut items = BTreeMap::new();
    let mut resolved_names = BTreeMap::new();
    for name in map.latest {
        reject_duplicate_resolved_name(kind, &mut resolved_names, &name, &name)?;
        items.insert(
            name.clone(),
            ListItem {
                logical_name: name.clone(),
                name,
                version: "latest".to_string(),
                backend: None,
                outputs: Vec::new(),
                linked_executables: Vec::new(),
                configured: true,
                project: scope == ConfigScope::Project,
                global: scope == ConfigScope::Global,
                installed: false,
            },
        );
    }

    for (name, entry) in map.entries {
        let PackageEntry::Expanded(package) = entry;
        let filter = PlatformFilter::from_config(
            package.platforms.as_deref().unwrap_or(&[]),
            package.ignore.as_deref(),
            package.only.as_deref(),
        )?;
        if !include_inactive_platforms && !filter.matches(platform) {
            continue;
        }
        let logical_name = name.clone();
        let resolved_name = name_for_platform(name, package.names, platform)?;
        reject_duplicate_resolved_name(kind, &mut resolved_names, &logical_name, &resolved_name)?;
        items.insert(
            resolved_name.clone(),
            ListItem {
                logical_name,
                name: resolved_name,
                version: package.version.unwrap_or_else(|| "latest".to_string()),
                backend: backend_for_platform(kind, package.backend, package.backends, platform),
                outputs: Vec::new(),
                linked_executables: Vec::new(),
                configured: true,
                project: scope == ConfigScope::Project,
                global: scope == ConfigScope::Global,
                installed: false,
            },
        );
    }

    Ok(items.into_values().collect())
}

fn reject_duplicate_resolved_name(
    kind: ItemKind,
    resolved_names: &mut BTreeMap<String, String>,
    logical_name: &str,
    resolved_name: &str,
) -> Result<()> {
    if let Some(existing) =
        resolved_names.insert(resolved_name.to_string(), logical_name.to_string())
        && existing != logical_name
    {
        return Err(EngineError::Conflict {
            message: format!(
                "{kind} \"{logical_name}\" resolves to \"{resolved_name}\", already used by \"{existing}\""
            ),
        }
        .into());
    }
    Ok(())
}

fn name_for_platform(
    name: String,
    names: BTreeMap<String, String>,
    platform: PlatformId,
) -> Result<String> {
    for (key, value) in names {
        let key_platform: PlatformId = key.parse()?;
        if key_platform == platform {
            return Ok(value);
        }
    }
    Ok(name)
}

fn backend_for_platform(
    kind: ItemKind,
    backend: Option<String>,
    backends: BTreeMap<String, String>,
    platform: PlatformId,
) -> Option<String> {
    backends
        .into_iter()
        .find_map(|(key, value)| match key.parse::<PlatformId>() {
            Ok(key_platform) if key_platform == platform => Some(value),
            _ => None,
        })
        .or(backend)
        .and_then(|backend| normalize_auto_backend(kind, Some(backend), platform))
}

async fn discover_installed_items() -> Result<Vec<ListSection>> {
    discover_installed_items_from_roots(
        System::tool_dir(),
        System::root_dir().join("packages"),
        System::apps_dir(),
    )
    .await
}

async fn discover_installed_items_from_roots(
    tool_root: PathBuf,
    package_root: PathBuf,
    app_root: PathBuf,
) -> Result<Vec<ListSection>> {
    Ok(vec![
        ListSection {
            kind: ItemKind::Tool,
            items: installed_items(ItemKind::Tool, tool_root).await?,
        },
        ListSection {
            kind: ItemKind::Package,
            items: installed_items(ItemKind::Package, package_root).await?,
        },
        ListSection {
            kind: ItemKind::App,
            items: installed_items(ItemKind::App, app_root).await?,
        },
    ])
}

async fn installed_items(kind: ItemKind, root: PathBuf) -> Result<Vec<ListItem>> {
    let mut items = Vec::new();
    let Ok(mut names) = tokio::fs::read_dir(&root).await else {
        return Ok(items);
    };

    while let Some(name_entry) = names.next_entry().await? {
        let name_path = name_entry.path();
        let metadata = name_entry.metadata().await?;
        if !metadata.is_dir() {
            continue;
        }

        if let Some(item) = read_marker_item(kind, name_path.join("install.toml")).await? {
            items.push(item);
            continue;
        }

        let mut versions = tokio::fs::read_dir(&name_path)
            .await
            .with_context(|| format!("failed to read {}", name_path.display()))?;
        while let Some(version_entry) = versions.next_entry().await? {
            if !version_entry.metadata().await?.is_dir() {
                continue;
            }
            if let Some(item) =
                read_marker_item(kind, version_entry.path().join("install.toml")).await?
            {
                items.push(item);
            }
        }
    }

    items.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.version.cmp(&right.version))
            .then_with(|| left.backend.cmp(&right.backend))
    });
    Ok(items)
}

async fn read_marker_item(kind: ItemKind, path: PathBuf) -> Result<Option<ListItem>> {
    let content = match tokio::fs::read_to_string(&path).await {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err).with_context(|| format!("failed to read {}", path.display())),
    };
    let marker: InstallMarker = toml_edit::de::from_str(&content)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    if marker.kind.parse::<ItemKind>()? != kind {
        return Ok(None);
    }
    Ok(Some(ListItem {
        logical_name: marker.name.clone(),
        name: marker.name,
        version: marker.version,
        backend: marker.backend,
        outputs: marker.outputs,
        linked_executables: marker.linked_executables,
        configured: false,
        project: false,
        global: false,
        installed: true,
    }))
}

fn merge_global_only_items(sections: &mut [ListSection], global_sections: Vec<ListSection>) {
    for global_section in global_sections {
        let Some(section) = sections
            .iter_mut()
            .find(|section| section.kind == global_section.kind)
        else {
            continue;
        };
        for global_item in global_section.items {
            if section
                .items
                .iter()
                .any(|item| item_overrides_global(item, &global_item))
            {
                continue;
            }
            section.items.push(global_item);
        }
        sort_items(&mut section.items);
    }
}

fn item_overrides_global(project: &ListItem, global: &ListItem) -> bool {
    project.logical_name == global.logical_name || project.name == global.name
}

fn merge_config_items(sections: &mut [ListSection], config_sections: Vec<ListSection>) {
    for config_section in config_sections {
        let Some(section) = sections
            .iter_mut()
            .find(|section| section.kind == config_section.kind)
        else {
            continue;
        };
        for config_item in config_section.items {
            if let Some(existing) = section.items.iter_mut().find(|item| {
                item.name == config_item.name
                    && item.version == config_item.version
                    && item.backend == config_item.backend
            }) {
                existing.configured = true;
                existing.project |= config_item.project;
                existing.global |= config_item.global;
            } else {
                section.items.push(config_item);
            }
        }
        sort_items(&mut section.items);
    }
}

fn merge_installed_items(sections: &mut [ListSection], installed_sections: Vec<ListSection>) {
    for installed_section in installed_sections {
        let Some(section) = sections
            .iter_mut()
            .find(|section| section.kind == installed_section.kind)
        else {
            continue;
        };
        for installed_item in installed_section.items {
            if let Some(existing) = section.items.iter_mut().find(|item| {
                item.name == installed_item.name
                    && item.version == installed_item.version
                    && backends_match_for_installed_merge(
                        item.backend.as_deref(),
                        installed_item.backend.as_deref(),
                    )
            }) {
                existing.installed = true;
                existing.outputs = installed_item.outputs;
                existing.linked_executables = installed_item.linked_executables;
            } else {
                section.items.push(installed_item);
            }
        }
        sort_items(&mut section.items);
    }
}

fn backends_match_for_installed_merge(configured: Option<&str>, installed: Option<&str>) -> bool {
    configured.is_none() || installed.is_none() || configured == installed
}

fn sort_items(items: &mut [ListItem]) {
    items.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.version.cmp(&right.version))
            .then_with(|| left.backend.cmp(&right.backend))
    });
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[tokio::test]
    async fn list_reads_configured_tools_packages_and_apps() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("still.toml"),
            r#"
            [tools]
            jq = "latest"
            rust = { version = "stable", backend = "rustup" }

            [packages]
            latest = ["openssl"]
            llvm = { version = "18", backend = "homebrew" }

            [apps]
            latest = ["firefox"]
            zed = { backend = "homebrew-cask" }
            "#,
        )
        .unwrap();

        let result = inspect(ListRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
            all: false,
        })
        .await
        .unwrap();

        assert_eq!(result.path, temp.path().join("still.toml"));
        assert_eq!(
            result.sections[0].items,
            [
                item("jq", "latest", None),
                item("rust", "stable", Some("rustup"))
            ]
        );
        assert_eq!(
            result.sections[1].items,
            [
                item("llvm", "18", Some("homebrew")),
                item("openssl", "latest", None)
            ]
        );
        assert_eq!(
            result.sections[2].items,
            [
                item("firefox", "latest", None),
                item("zed", "latest", Some("homebrew-cask"))
            ]
        );
    }

    #[tokio::test]
    async fn list_uses_platform_filters_and_names() {
        let temp = tempfile::tempdir().unwrap();
        let platform = current_platform().to_string();
        let other_platform = if cfg!(target_os = "windows") {
            "linux"
        } else {
            "windows"
        };
        fs::write(
            temp.path().join("still.toml"),
            format!(
                r#"
                [packages.fd]
                version = "latest"
                names = {{ {platform} = "fd-find" }}

                [packages.skip-me]
                version = "latest"
                only = "{other_platform}"
                "#
            ),
        )
        .unwrap();

        let result = inspect(ListRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
            all: false,
        })
        .await
        .unwrap();

        assert_eq!(
            result.sections[1].items,
            [item_with_logical_name("fd", "fd-find", "latest", None)]
        );
    }

    #[tokio::test]
    async fn list_rejects_duplicate_resolved_package_names() {
        let temp = tempfile::tempdir().unwrap();
        let platform = current_platform().to_string();
        fs::write(
            temp.path().join("still.toml"),
            format!(
                r#"
                [packages.fd]
                version = "latest"
                names = {{ {platform} = "fd-find" }}

                [packages.fd-find]
                version = "latest"
                "#
            ),
        )
        .unwrap();

        let err = inspect(ListRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
            all: false,
        })
        .await
        .unwrap_err();

        assert!(err.to_string().contains("resolves to \"fd-find\""));
        assert!(err.to_string().contains("already used"));
    }

    #[tokio::test]
    async fn list_all_includes_project_items_inactive_on_current_platform() {
        let temp = tempfile::tempdir().unwrap();
        let other_platform = if cfg!(target_os = "windows") {
            "linux"
        } else {
            "windows"
        };
        fs::write(
            temp.path().join("still.toml"),
            format!(
                r#"
                [packages.inactive-lib]
                version = "latest"
                only = "{other_platform}"
                "#
            ),
        )
        .unwrap();

        let active = inspect(ListRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
            all: false,
        })
        .await
        .unwrap();
        assert!(active.sections[1].items.is_empty());

        let all = inspect(ListRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
            all: true,
        })
        .await
        .unwrap();

        assert_eq!(
            all.sections[1].items,
            [item("inactive-lib", "latest", None)]
        );
    }

    #[tokio::test]
    async fn list_global_all_includes_global_items_inactive_on_current_platform() {
        let temp = tempfile::tempdir().unwrap();
        let other_platform = if cfg!(target_os = "windows") {
            "linux"
        } else {
            "windows"
        };
        let global = temp.path().join(".config/still/config.toml");
        fs::create_dir_all(global.parent().unwrap()).unwrap();
        fs::write(
            &global,
            format!(
                r#"
                [apps.platform-app]
                version = "latest"
                only = "{other_platform}"
                "#
            ),
        )
        .unwrap();

        let active = inspect(ListRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: true,
            all: false,
        })
        .await
        .unwrap();
        assert!(active.sections[2].items.is_empty());

        let all = inspect(ListRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: true,
            all: true,
        })
        .await
        .unwrap();

        assert_eq!(
            all.sections[2].items,
            [global_item("platform-app", "latest", None)]
        );
    }

    #[tokio::test]
    async fn list_can_read_global_config() {
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join(".config/still/config.toml");
        fs::create_dir_all(config.parent().unwrap()).unwrap();
        fs::write(&config, "[tools]\nnode = \"22\"\n").unwrap();

        let result = inspect(ListRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: true,
            all: false,
        })
        .await
        .unwrap();

        assert_eq!(result.path, config);
        assert_eq!(result.sections[0].items, [global_item("node", "22", None)]);
    }

    #[tokio::test]
    async fn list_uses_platform_specific_backend_overrides() {
        let temp = tempfile::tempdir().unwrap();
        let platform = current_platform().to_string();
        fs::write(
            temp.path().join("still.toml"),
            format!(
                r#"
                [tools.rust]
                version = "stable"
                backend = "rustup"
                backends = {{ {platform} = "mise" }}

                [apps.zed]
                backend = "homebrew-cask"
                backends = {{ {platform} = "flatpak" }}
                "#
            ),
        )
        .unwrap();

        let result = inspect(ListRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
            all: false,
        })
        .await
        .unwrap();

        assert_eq!(
            result.sections[0].items,
            [item("rust", "stable", Some("mise"))]
        );
        assert_eq!(
            result.sections[2].items,
            [item("zed", "latest", Some("flatpak"))]
        );
    }

    #[tokio::test]
    async fn list_resolves_literal_auto_backends() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("still.toml"),
            r#"
            [tools.rust]
            version = "stable"
            backend = "auto"

            [packages.openssl]
            version = "3"
            backend = "auto"

            [apps.firefox]
            version = "latest"
            backend = "auto"
            "#,
        )
        .unwrap();

        let result = inspect(ListRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
            all: false,
        })
        .await
        .unwrap();

        assert_eq!(
            result.sections[0].items,
            [item("rust", "stable", Some("homebrew"))]
        );
        assert_eq!(
            result.sections[1].items,
            [item(
                "openssl",
                "3",
                Some(crate::specs::backend::default_backend(
                    ItemKind::Package,
                    current_platform()
                ))
            )]
        );
        assert_eq!(
            result.sections[2].items,
            [item(
                "firefox",
                "latest",
                Some(crate::specs::backend::default_backend(
                    ItemKind::App,
                    current_platform()
                ))
            )]
        );
    }

    #[tokio::test]
    async fn list_all_discovers_still_managed_install_markers() {
        let temp = tempfile::tempdir().unwrap();
        let tools = temp.path().join("tools");
        let packages = temp.path().join("packages");
        let apps = temp.path().join("apps");
        write_marker(
            tools.join("ripgrep/14.1.1/install.toml"),
            "tool",
            "ripgrep",
            "14.1.1",
            "homebrew",
        );
        write_marker(
            packages.join("openssl/3.4.0/install.toml"),
            "package",
            "openssl",
            "3.4.0",
            "homebrew",
        );
        write_marker(
            apps.join("zed/latest/install.toml"),
            "app",
            "zed",
            "latest",
            "homebrew-cask",
        );

        let sections =
            discover_installed_items_from_roots(tools.clone(), packages.clone(), apps.clone())
                .await
                .unwrap();

        assert_eq!(
            sections[0].items,
            [installed_item_with_output(
                "ripgrep",
                "14.1.1",
                Some("homebrew"),
                &tools.join("ripgrep/14.1.1")
            )]
        );
        assert_eq!(
            sections[1].items,
            [installed_item_with_output(
                "openssl",
                "3.4.0",
                Some("homebrew"),
                &packages.join("openssl/3.4.0")
            )]
        );
        assert_eq!(
            sections[2].items,
            [installed_item_with_output(
                "zed",
                "latest",
                Some("homebrew-cask"),
                &apps.join("zed/latest")
            )]
        );
    }

    #[test]
    fn merge_installed_marks_matching_configured_items() {
        let mut sections = vec![ListSection {
            kind: ItemKind::Tool,
            items: vec![item("ripgrep", "14.1.1", Some("homebrew"))],
        }];

        merge_installed_items(
            &mut sections,
            vec![ListSection {
                kind: ItemKind::Tool,
                items: vec![
                    installed_item("fd", "10.2.0", Some("homebrew")),
                    installed_item("ripgrep", "14.1.1", Some("homebrew")),
                ],
            }],
        );

        assert_eq!(
            sections[0].items,
            [
                installed_item("fd", "10.2.0", Some("homebrew")),
                ListItem {
                    logical_name: "ripgrep".to_string(),
                    name: "ripgrep".to_string(),
                    version: "14.1.1".to_string(),
                    backend: Some("homebrew".to_string()),
                    outputs: Vec::new(),
                    linked_executables: Vec::new(),
                    configured: true,
                    project: true,
                    global: false,
                    installed: true,
                },
            ]
        );
    }

    #[test]
    fn merge_installed_matches_configured_auto_backend() {
        let mut sections = vec![ListSection {
            kind: ItemKind::Package,
            items: vec![item("openssl", "latest", None)],
        }];

        merge_installed_items(
            &mut sections,
            vec![ListSection {
                kind: ItemKind::Package,
                items: vec![installed_item("openssl", "latest", Some("apt"))],
            }],
        );

        assert_eq!(
            sections[0].items,
            [ListItem {
                logical_name: "openssl".to_string(),
                name: "openssl".to_string(),
                version: "latest".to_string(),
                backend: None,
                outputs: Vec::new(),
                linked_executables: Vec::new(),
                configured: true,
                project: true,
                global: false,
                installed: true,
            }]
        );
    }

    #[tokio::test]
    async fn list_all_merges_global_config_items() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("repo");
        let global = temp.path().join(".config/still/config.toml");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(global.parent().unwrap()).unwrap();
        fs::write(project.join("still.toml"), "[tools]\nrust = \"stable\"\n").unwrap();
        fs::write(
            &global,
            r#"
            [tools]
            node = "22"

            [apps]
            latest = ["firefox"]
            "#,
        )
        .unwrap();

        let result = inspect(ListRequest {
            start_dir: project,
            home_dir: temp.path().to_path_buf(),
            global: false,
            all: true,
        })
        .await
        .unwrap();

        assert!(
            result.sections[0]
                .items
                .contains(&global_item("node", "22", None))
        );
        assert!(
            result.sections[0]
                .items
                .contains(&item("rust", "stable", None))
        );
        assert!(
            result.sections[2]
                .items
                .contains(&global_item("firefox", "latest", None))
        );
    }

    #[tokio::test]
    async fn list_all_includes_global_items_inactive_on_current_platform() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("repo");
        let global = temp.path().join(".config/still/config.toml");
        let other_platform = if cfg!(target_os = "windows") {
            "linux"
        } else {
            "windows"
        };
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(global.parent().unwrap()).unwrap();
        fs::write(project.join("still.toml"), "[tools]\nrust = \"stable\"\n").unwrap();
        fs::write(
            &global,
            format!(
                r#"
                [apps.platform-app]
                version = "latest"
                only = "{other_platform}"
                "#
            ),
        )
        .unwrap();

        let result = inspect(ListRequest {
            start_dir: project,
            home_dir: temp.path().to_path_buf(),
            global: false,
            all: true,
        })
        .await
        .unwrap();

        assert!(
            result.sections[2]
                .items
                .contains(&global_item("platform-app", "latest", None))
        );
    }

    #[tokio::test]
    async fn list_active_merges_global_only_config_items() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("repo");
        let global = temp.path().join(".config/still/config.toml");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(global.parent().unwrap()).unwrap();
        fs::write(project.join("still.toml"), "[tools]\nrust = \"stable\"\n").unwrap();
        fs::write(&global, "[tools]\nnode = \"22\"\n").unwrap();

        let result = inspect(ListRequest {
            start_dir: project,
            home_dir: temp.path().to_path_buf(),
            global: false,
            all: false,
        })
        .await
        .unwrap();

        assert_eq!(
            result.sections[0].items,
            [
                global_item("node", "22", None),
                item("rust", "stable", None),
            ]
        );
    }

    #[tokio::test]
    async fn list_active_project_item_overrides_global_item() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("repo");
        let global = temp.path().join(".config/still/config.toml");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(global.parent().unwrap()).unwrap();
        fs::write(project.join("still.toml"), "[tools]\nrust = \"stable\"\n").unwrap();
        fs::write(&global, "[tools]\nrust = \"1.80.0\"\nnode = \"22\"\n").unwrap();

        let result = inspect(ListRequest {
            start_dir: project,
            home_dir: temp.path().to_path_buf(),
            global: false,
            all: false,
        })
        .await
        .unwrap();

        assert_eq!(
            result.sections[0].items,
            [
                global_item("node", "22", None),
                item("rust", "stable", None),
            ]
        );
    }

    #[tokio::test]
    async fn list_active_project_item_overrides_global_item_by_logical_name() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("repo");
        let global = temp.path().join(".config/still/config.toml");
        let platform = current_platform().to_string();
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(global.parent().unwrap()).unwrap();
        fs::write(
            project.join("still.toml"),
            format!(
                r#"
                [packages.fd]
                version = "latest"
                names = {{ {platform} = "fd-find" }}
                "#
            ),
        )
        .unwrap();
        fs::write(&global, "[packages]\nlatest = [\"fd\"]\n").unwrap();

        let result = inspect(ListRequest {
            start_dir: project,
            home_dir: temp.path().to_path_buf(),
            global: false,
            all: false,
        })
        .await
        .unwrap();

        assert_eq!(
            result.sections[1].items,
            [item_with_logical_name("fd", "fd-find", "latest", None)]
        );
    }

    #[tokio::test]
    async fn list_active_project_item_overrides_global_item_by_resolved_name() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("repo");
        let global = temp.path().join(".config/still/config.toml");
        let platform = current_platform().to_string();
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(global.parent().unwrap()).unwrap();
        fs::write(
            project.join("still.toml"),
            format!(
                r#"
                [packages.fd]
                version = "latest"
                names = {{ {platform} = "fd-find" }}
                "#
            ),
        )
        .unwrap();
        fs::write(&global, "[packages]\nlatest = [\"fd-find\"]\n").unwrap();

        let result = inspect(ListRequest {
            start_dir: project,
            home_dir: temp.path().to_path_buf(),
            global: false,
            all: false,
        })
        .await
        .unwrap();

        assert_eq!(
            result.sections[1].items,
            [item_with_logical_name("fd", "fd-find", "latest", None)]
        );
    }

    #[tokio::test]
    async fn list_global_all_merges_project_config_items() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("repo");
        let global = temp.path().join(".config/still/config.toml");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(global.parent().unwrap()).unwrap();
        fs::write(project.join("still.toml"), "[tools]\nrust = \"stable\"\n").unwrap();
        fs::write(
            &global,
            r#"
            [tools]
            node = "22"
            "#,
        )
        .unwrap();

        let result = inspect(ListRequest {
            start_dir: project,
            home_dir: temp.path().to_path_buf(),
            global: true,
            all: true,
        })
        .await
        .unwrap();

        assert_eq!(result.path, global);
        assert!(
            result.sections[0]
                .items
                .contains(&global_item("node", "22", None))
        );
        assert!(
            result.sections[0]
                .items
                .contains(&item("rust", "stable", None))
        );
    }

    fn item(name: &str, version: &str, backend: Option<&str>) -> ListItem {
        item_with_logical_name(name, name, version, backend)
    }

    fn item_with_logical_name(
        logical_name: &str,
        name: &str,
        version: &str,
        backend: Option<&str>,
    ) -> ListItem {
        ListItem {
            logical_name: logical_name.to_string(),
            name: name.to_string(),
            version: version.to_string(),
            backend: backend.map(str::to_string),
            outputs: Vec::new(),
            linked_executables: Vec::new(),
            configured: true,
            project: true,
            global: false,
            installed: false,
        }
    }

    fn global_item(name: &str, version: &str, backend: Option<&str>) -> ListItem {
        ListItem {
            logical_name: name.to_string(),
            name: name.to_string(),
            version: version.to_string(),
            backend: backend.map(str::to_string),
            outputs: Vec::new(),
            linked_executables: Vec::new(),
            configured: true,
            project: false,
            global: true,
            installed: false,
        }
    }

    fn installed_item(name: &str, version: &str, backend: Option<&str>) -> ListItem {
        ListItem {
            logical_name: name.to_string(),
            name: name.to_string(),
            version: version.to_string(),
            backend: backend.map(str::to_string),
            outputs: Vec::new(),
            linked_executables: Vec::new(),
            configured: false,
            project: false,
            global: false,
            installed: true,
        }
    }

    fn installed_item_with_output(
        name: &str,
        version: &str,
        backend: Option<&str>,
        output: &std::path::Path,
    ) -> ListItem {
        ListItem {
            outputs: vec![output.display().to_string()],
            ..installed_item(name, version, backend)
        }
    }

    fn write_marker(path: PathBuf, kind: &str, name: &str, version: &str, backend: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let output = path.parent().unwrap().display().to_string();
        fs::write(
            path,
            format!(
                "kind = \"{kind}\"\nname = \"{name}\"\nversion = \"{version}\"\nbackend = \"{backend}\"\noutputs = [\"{output}\"]\nlinked_executables = []\n"
            ),
        )
        .unwrap();
    }
}
