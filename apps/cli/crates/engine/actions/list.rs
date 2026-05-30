//! Engine action for listing configured desired state.

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::config::{ConfigScope, ConfigSelection, resolve_config_path};
use crate::specs::item::ItemKind;
use crate::specs::toml::{PackageEntry, PackageMap, StillConfig, ToolEntry, parse_still_toml};

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
}

/// Reads selected config and returns configured items.
/// # Errors
/// Fails when the selected config cannot be read or parsed.
pub async fn inspect(request: ListRequest) -> Result<ListResult> {
    let _include_inactive = request.all;
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

    Ok(ListResult {
        path: resolved.path,
        sections: sections_from_config(config),
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
            },
            ToolEntry::Expanded(tool) => ListItem {
                name,
                version: if tool.version.is_empty() {
                    "latest".to_string()
                } else {
                    tool.version
                },
                backend: tool.backend,
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
            },
        );
    }

    items.into_values().collect()
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

    fn item(name: &str, version: &str, backend: Option<&str>) -> ListItem {
        ListItem {
            name: name.to_string(),
            version: version.to_string(),
            backend: backend.map(str::to_string),
        }
    }
}
