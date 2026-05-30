//! TOML edits for Still desired-state files.

use toml_edit::{Array, DocumentMut, InlineTable, Item, Table, Value};

use crate::actions::install::InstallItemRequest;
use crate::error::{EngineError, EngineResult};
use crate::specs::item::{ItemKind, ItemSpec};

/// Adds install requests to a Still TOML document.
pub fn add_install_items(input: &str, items: &[InstallItemRequest]) -> EngineResult<String> {
    add_install_items_with_force(input, items, false)
}

/// Adds install requests, optionally allowing changed existing entries.
pub fn add_install_items_with_force(
    input: &str,
    items: &[InstallItemRequest],
    force: bool,
) -> EngineResult<String> {
    if items.is_empty() {
        return Err(EngineError::EmptyInstallRequest);
    }

    let mut doc = input
        .parse::<DocumentMut>()
        .map_err(|err| EngineError::InvalidConfig {
            reason: err.to_string(),
        })?;
    let config =
        crate::specs::toml::parse_still_toml(input).map_err(|err| EngineError::InvalidConfig {
            reason: err.to_string(),
        })?;

    if !force {
        reject_conflicting_install_items(&config, items)?;
    }

    for item in items {
        match item.kind {
            ItemKind::Tool => add_tool(&mut doc, &item.spec),
            ItemKind::Package => add_package_like(&mut doc, "packages", &item.spec),
            ItemKind::App => add_package_like(&mut doc, "apps", &item.spec),
        }
    }

    Ok(doc.to_string())
}

fn reject_conflicting_install_items(
    config: &crate::specs::toml::StillConfig,
    items: &[InstallItemRequest],
) -> EngineResult<()> {
    for item in items {
        if existing_item_matches(config, item) != Some(false) {
            continue;
        }
        return Err(EngineError::Conflict {
            message: format!(
                "{} {} is already configured with a different version or backend; pass --force to update it",
                item.kind, item.spec.name
            ),
        });
    }
    Ok(())
}

fn existing_item_matches(
    config: &crate::specs::toml::StillConfig,
    item: &InstallItemRequest,
) -> Option<bool> {
    match item.kind {
        ItemKind::Tool => config
            .tools
            .get(&item.spec.name)
            .map(|entry| tool_entry_matches(entry, &item.spec)),
        ItemKind::Package => package_entry_matches(&config.packages, &item.spec),
        ItemKind::App => package_entry_matches(&config.apps, &item.spec),
    }
}

fn tool_entry_matches(entry: &crate::specs::toml::ToolEntry, spec: &ItemSpec) -> bool {
    match entry {
        crate::specs::toml::ToolEntry::Version(version) => {
            spec.backend.is_none() && version == spec.version.as_str()
        }
        crate::specs::toml::ToolEntry::Expanded(tool) => {
            let version = if tool.version.is_empty() {
                "latest"
            } else {
                tool.version.as_str()
            };
            version == spec.version.as_str()
                && tool.backend.as_deref() == spec.backend.as_ref().map(|backend| backend.as_str())
        }
    }
}

fn package_entry_matches(map: &crate::specs::toml::PackageMap, spec: &ItemSpec) -> Option<bool> {
    if let Some(entry) = map.entries.get(&spec.name) {
        let crate::specs::toml::PackageEntry::Expanded(package) = entry;
        let version = package.version.as_deref().unwrap_or("latest");
        return Some(
            version == spec.version.as_str()
                && package.backend.as_deref()
                    == spec.backend.as_ref().map(|backend| backend.as_str()),
        );
    }

    if map.latest.iter().any(|name| name == &spec.name) {
        return Some(spec.version.is_latest() && spec.backend.is_none());
    }

    None
}

/// Removes one named item from tools, packages, or apps.
pub fn remove_item(input: &str, name: &str) -> EngineResult<(String, Option<ItemKind>)> {
    let mut doc = input
        .parse::<DocumentMut>()
        .map_err(|err| EngineError::InvalidConfig {
            reason: err.to_string(),
        })?;

    let removed = remove_from_section(&mut doc, "tools", name, ItemKind::Tool)
        .or_else(|| remove_from_section(&mut doc, "packages", name, ItemKind::Package))
        .or_else(|| remove_from_section(&mut doc, "apps", name, ItemKind::App));

    Ok((doc.to_string(), removed))
}

fn add_tool(doc: &mut DocumentMut, spec: &ItemSpec) {
    let tools = table_mut(doc, "tools");
    if should_use_latest_shorthand(spec) {
        tools[&spec.name] = Item::Value(Value::from("latest"));
        return;
    }

    let mut table = Table::new();
    table["version"] = Item::Value(Value::from(spec.version.as_str()));
    if let Some(backend) = &spec.backend {
        table["backend"] = Item::Value(Value::from(backend.as_str()));
    }
    tools[&spec.name] = Item::Table(table);
}

fn add_package_like(doc: &mut DocumentMut, section: &str, spec: &ItemSpec) {
    let table = table_mut(doc, section);
    if should_use_latest_shorthand(spec) {
        append_unique_latest(table, &spec.name);
        return;
    }

    let mut entry = InlineTable::new();
    if !spec.version.is_latest() {
        entry.insert("version", Value::from(spec.version.as_str()));
    }
    if let Some(backend) = &spec.backend {
        entry.insert("backend", Value::from(backend.as_str()));
    }
    table[&spec.name] = Item::Value(Value::InlineTable(entry));
}

fn table_mut<'a>(doc: &'a mut DocumentMut, name: &str) -> &'a mut Table {
    if !doc.as_table().contains_key(name) || !doc[name].is_table() {
        doc[name] = Item::Table(Table::new());
    }
    doc[name]
        .as_table_mut()
        .expect("section was set to a table")
}

fn append_unique_latest(table: &mut Table, name: &str) {
    if !table.contains_key("latest") || !table["latest"].is_array() {
        table["latest"] = Item::Value(Value::Array(Array::new()));
    }

    let latest = table["latest"]
        .as_array_mut()
        .expect("latest was set to an array");
    if !latest.iter().any(|value| value.as_str() == Some(name)) {
        latest.push(name);
    }
}

fn remove_from_section(
    doc: &mut DocumentMut,
    section: &str,
    name: &str,
    kind: ItemKind,
) -> Option<ItemKind> {
    let item = doc.get_mut(section)?;
    let table = item.as_table_mut()?;
    let mut removed = false;

    if table.remove(name).is_some() {
        removed = true;
    }

    if let Some(latest) = table.get_mut("latest").and_then(Item::as_array_mut) {
        let before = latest.len();
        latest.retain(|value| value.as_str() != Some(name));
        removed |= latest.len() != before;
    }

    removed.then_some(kind)
}

fn should_use_latest_shorthand(spec: &ItemSpec) -> bool {
    spec.version.is_latest() && spec.backend.is_none()
}

#[cfg(test)]
mod tests {
    use crate::actions::install::InstallItemRequest;
    use crate::specs::item::{ItemKind, ItemSpec};
    use crate::specs::toml::{PackageEntry, StillConfig, ToolEntry, parse_still_toml};

    use super::*;

    #[test]
    fn adds_grouped_latest_items_to_empty_config() {
        let output = add_install_items(
            "",
            &[
                item(ItemKind::Tool, "jq"),
                item(ItemKind::Package, "openssl"),
                item(ItemKind::App, "firefox"),
            ],
        )
        .unwrap();

        let config = parse(&output);
        assert_eq!(config.tools["jq"], ToolEntry::Version("latest".to_string()));
        assert_eq!(config.packages.latest, ["openssl"]);
        assert_eq!(config.apps.latest, ["firefox"]);
    }

    #[test]
    fn writes_pinned_or_backend_specific_items_as_keyed_entries() {
        let output = add_install_items(
            "",
            &[
                item(ItemKind::Tool, "rust@stable@rustup"),
                item(ItemKind::Package, "llvm@18@homebrew"),
                item(ItemKind::App, "firefox@latest@homebrew-cask"),
            ],
        )
        .unwrap();

        let config = parse(&output);
        let ToolEntry::Expanded(rust) = &config.tools["rust"] else {
            panic!("rust should be expanded");
        };
        assert_eq!(rust.version, "stable");
        assert_eq!(rust.backend.as_deref(), Some("rustup"));

        let PackageEntry::Expanded(llvm) = &config.packages.entries["llvm"];
        assert_eq!(llvm.version.as_deref(), Some("18"));
        assert_eq!(llvm.backend.as_deref(), Some("homebrew"));

        let PackageEntry::Expanded(firefox) = &config.apps.entries["firefox"];
        assert_eq!(firefox.version.as_deref(), None);
        assert_eq!(firefox.backend.as_deref(), Some("homebrew-cask"));
    }

    #[test]
    fn dedupes_latest_package_and_app_arrays() {
        let output = add_install_items(
            r#"
            [packages]
            latest = ["openssl"]

            [apps]
            latest = ["firefox"]
            "#,
            &[
                item(ItemKind::Package, "openssl"),
                item(ItemKind::App, "firefox"),
            ],
        )
        .unwrap();

        let config = parse(&output);
        assert_eq!(config.packages.latest, ["openssl"]);
        assert_eq!(config.apps.latest, ["firefox"]);
    }

    #[test]
    fn preserves_unrelated_config_sections() {
        let output = add_install_items(
            r#"
            [env]
            RUST_LOG = "debug"

            [tasks]
            test = "cargo test"
            "#,
            &[item(ItemKind::Tool, "ripgrep")],
        )
        .unwrap();

        let config = parse(&output);
        assert_eq!(config.env.vars["RUST_LOG"], "debug");
        assert!(config.tasks.contains_key("test"));
        assert!(config.tools.contains_key("ripgrep"));
    }

    #[test]
    fn rejects_changed_existing_entries_without_force() {
        let err = add_install_items(
            r#"
            [tools]
            rust = { version = "stable", backend = "rustup" }

            [packages]
            llvm = { version = "18", backend = "homebrew" }
            "#,
            &[
                item(ItemKind::Tool, "rust@stable@mise"),
                item(ItemKind::Package, "llvm@19@homebrew"),
            ],
        )
        .unwrap_err();

        assert!(err.to_string().contains("--force"));
    }

    #[test]
    fn force_allows_changed_existing_entries() {
        let output = add_install_items_with_force(
            r#"
            [tools]
            rust = { version = "stable", backend = "rustup" }
            "#,
            &[item(ItemKind::Tool, "rust@stable@mise")],
            true,
        )
        .unwrap();

        let config = parse(&output);
        let ToolEntry::Expanded(rust) = &config.tools["rust"] else {
            panic!("rust should be expanded");
        };
        assert_eq!(rust.backend.as_deref(), Some("mise"));
    }

    #[test]
    fn identical_existing_entries_remain_noops() {
        let output = add_install_items(
            r#"
            [tools]
            rust = { version = "stable", backend = "rustup" }

            [packages]
            latest = ["openssl"]
            "#,
            &[
                item(ItemKind::Tool, "rust@stable@rustup"),
                item(ItemKind::Package, "openssl"),
            ],
        )
        .unwrap();

        let config = parse(&output);
        assert_eq!(config.packages.latest, ["openssl"]);
        let ToolEntry::Expanded(rust) = &config.tools["rust"] else {
            panic!("rust should be expanded");
        };
        assert_eq!(rust.backend.as_deref(), Some("rustup"));
    }

    #[test]
    fn removes_tool_entries() {
        let (output, removed) = remove_item("[tools]\nrust = \"stable\"\n", "rust").unwrap();

        let config = parse(&output);
        assert_eq!(removed, Some(ItemKind::Tool));
        assert!(!config.tools.contains_key("rust"));
    }

    #[test]
    fn removes_latest_package_entries() {
        let (output, removed) =
            remove_item("[packages]\nlatest = [\"openssl\", \"llvm\"]\n", "openssl").unwrap();

        let config = parse(&output);
        assert_eq!(removed, Some(ItemKind::Package));
        assert_eq!(config.packages.latest, ["llvm"]);
    }

    #[test]
    fn reports_when_remove_target_is_missing() {
        let (output, removed) = remove_item("[tools]\nrust = \"stable\"\n", "node").unwrap();

        assert_eq!(removed, None);
        assert_eq!(parse(&output).tools.len(), 1);
    }

    fn item(kind: ItemKind, spec: &str) -> InstallItemRequest {
        InstallItemRequest {
            kind,
            spec: spec.parse::<ItemSpec>().unwrap(),
        }
    }

    fn parse(input: &str) -> StillConfig {
        parse_still_toml(input).unwrap()
    }
}
