//! Engine action for listing configured desired state.

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::config::{ConfigScope, ConfigSelection, resolve_config_path};
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
    pub name: String,
    pub version: String,
    pub backend: Option<String>,
    pub configured: bool,
    pub installed: bool,
}

#[derive(Debug, Deserialize)]
struct InstallMarker {
    kind: String,
    name: String,
    version: String,
    backend: Option<String>,
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
    let mut sections = sections_from_config(config);
    if request.all {
        merge_installed_items(&mut sections, discover_installed_items().await?);
    }

    Ok(ListResult {
        path: resolved.path,
        sections,
    })
}

fn sections_from_config(config: StillConfig) -> Vec<ListSection> {
    vec![
        ListSection {
            kind: ItemKind::Tool,
            items: tool_items(config.tools),
        },
        ListSection {
            kind: ItemKind::Package,
            items: package_items(config.packages),
        },
        ListSection {
            kind: ItemKind::App,
            items: package_items(config.apps),
        },
    ]
}

fn tool_items(tools: BTreeMap<String, ToolEntry>) -> Vec<ListItem> {
    tools
        .into_iter()
        .map(|(name, entry)| match entry {
            ToolEntry::Version(version) => ListItem {
                name,
                version,
                backend: None,
                configured: true,
                installed: false,
            },
            ToolEntry::Expanded(tool) => ListItem {
                name,
                version: if tool.version.is_empty() {
                    "latest".to_string()
                } else {
                    tool.version
                },
                backend: tool.backend,
                configured: true,
                installed: false,
            },
        })
        .collect()
}

fn package_items(map: PackageMap) -> Vec<ListItem> {
    let mut items = BTreeMap::new();
    for name in map.latest {
        items.insert(
            name.clone(),
            ListItem {
                name,
                version: "latest".to_string(),
                backend: None,
                configured: true,
                installed: false,
            },
        );
    }

    for (name, entry) in map.entries {
        let PackageEntry::Expanded(package) = entry;
        items.insert(
            name.clone(),
            ListItem {
                name,
                version: package.version.unwrap_or_else(|| "latest".to_string()),
                backend: package.backend,
                configured: true,
                installed: false,
            },
        );
    }

    items.into_values().collect()
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
        name: marker.name,
        version: marker.version,
        backend: marker.backend,
        configured: false,
        installed: true,
    }))
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
                    && item.backend == installed_item.backend
            }) {
                existing.installed = true;
            } else {
                section.items.push(installed_item);
            }
        }
        section.items.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| left.version.cmp(&right.version))
                .then_with(|| left.backend.cmp(&right.backend))
        });
    }
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
        assert_eq!(result.sections[0].items, [item("node", "22", None)]);
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

        let sections = discover_installed_items_from_roots(tools, packages, apps)
            .await
            .unwrap();

        assert_eq!(
            sections[0].items,
            [installed_item("ripgrep", "14.1.1", Some("homebrew"))]
        );
        assert_eq!(
            sections[1].items,
            [installed_item("openssl", "3.4.0", Some("homebrew"))]
        );
        assert_eq!(
            sections[2].items,
            [installed_item("zed", "latest", Some("homebrew-cask"))]
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
                    name: "ripgrep".to_string(),
                    version: "14.1.1".to_string(),
                    backend: Some("homebrew".to_string()),
                    configured: true,
                    installed: true,
                },
            ]
        );
    }

    fn item(name: &str, version: &str, backend: Option<&str>) -> ListItem {
        ListItem {
            name: name.to_string(),
            version: version.to_string(),
            backend: backend.map(str::to_string),
            configured: true,
            installed: false,
        }
    }

    fn installed_item(name: &str, version: &str, backend: Option<&str>) -> ListItem {
        ListItem {
            name: name.to_string(),
            version: version.to_string(),
            backend: backend.map(str::to_string),
            configured: false,
            installed: true,
        }
    }

    fn write_marker(path: PathBuf, kind: &str, name: &str, version: &str, backend: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            path,
            format!(
                "kind = \"{kind}\"\nname = \"{name}\"\nversion = \"{version}\"\nbackend = \"{backend}\"\n"
            ),
        )
        .unwrap();
    }
}
