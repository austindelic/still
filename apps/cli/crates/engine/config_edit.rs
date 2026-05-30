//! TOML edits for Still desired-state files.

use toml_edit::{Array, DocumentMut, InlineTable, Item, Table, Value};

use crate::actions::install::InstallItemRequest;
use crate::error::{EngineError, EngineResult};
use crate::specs::item::{ItemKind, ItemSpec};

/// Adds install requests to a Still TOML document.
pub fn add_install_items(input: &str, items: &[InstallItemRequest]) -> EngineResult<String> {
    if items.is_empty() {
        return Err(EngineError::EmptyInstallRequest);
    }

    let mut doc = input
        .parse::<DocumentMut>()
        .map_err(|err| EngineError::InvalidConfig {
            reason: err.to_string(),
        })?;

    for item in items {
        match item.kind {
            ItemKind::Tool => add_tool(&mut doc, &item.spec),
            ItemKind::Package => add_package_like(&mut doc, "packages", &item.spec),
            ItemKind::App => add_package_like(&mut doc, "apps", &item.spec),
        }
    }

    Ok(doc.to_string())
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
