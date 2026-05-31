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
        reject_conflicting_request_items(items)?;
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

fn reject_conflicting_request_items(items: &[InstallItemRequest]) -> EngineResult<()> {
    for (index, item) in items.iter().enumerate() {
        let Some(existing) = items[..index]
            .iter()
            .find(|existing| existing.kind == item.kind && existing.spec.name == item.spec.name)
        else {
            continue;
        };

        if install_items_match(existing, item) {
            continue;
        }

        return Err(EngineError::Conflict {
            message: format!(
                "{} {} was requested more than once with a different version or backend; pass --force to update it",
                item.kind, item.spec.name
            ),
        });
    }
    Ok(())
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

fn install_items_match(left: &InstallItemRequest, right: &InstallItemRequest) -> bool {
    left.kind == right.kind
        && left.spec.name == right.spec.name
        && left.spec.version == right.spec.version
        && left.spec.backend == right.spec.backend
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

/// Removes one named item from a selected kind, or infers the kind from config.
pub fn remove_item(
    input: &str,
    kind: Option<ItemKind>,
    name: &str,
) -> EngineResult<(String, Option<ItemKind>)> {
    let mut doc = input
        .parse::<DocumentMut>()
        .map_err(|err| EngineError::InvalidConfig {
            reason: err.to_string(),
        })?;

    let removed = match kind {
        Some(ItemKind::Tool) => remove_from_section(&mut doc, "tools", name, ItemKind::Tool),
        Some(ItemKind::Package) => {
            remove_from_section(&mut doc, "packages", name, ItemKind::Package)
        }
        Some(ItemKind::App) => remove_from_section(&mut doc, "apps", name, ItemKind::App),
        None => remove_from_section(&mut doc, "tools", name, ItemKind::Tool)
            .or_else(|| remove_from_section(&mut doc, "packages", name, ItemKind::Package))
            .or_else(|| remove_from_section(&mut doc, "apps", name, ItemKind::App)),
    };

    Ok((doc.to_string(), removed))
}

/// Fully specified desired-state item to remove.
#[derive(Debug, Clone)]
pub struct RemoveItemTarget {
    pub kind: Option<ItemKind>,
    pub name: String,
    pub version: String,
    pub backend: Option<crate::specs::item::BackendId>,
}

/// Removes one item only when the configured version/backend matches `target`.
pub fn remove_item_target(
    input: &str,
    target: &RemoveItemTarget,
) -> EngineResult<(String, Option<ItemKind>)> {
    let mut doc = input
        .parse::<DocumentMut>()
        .map_err(|err| EngineError::InvalidConfig {
            reason: err.to_string(),
        })?;
    let config =
        crate::specs::toml::parse_still_toml(input).map_err(|err| EngineError::InvalidConfig {
            reason: err.to_string(),
        })?;

    let removed = match target.kind {
        Some(ItemKind::Tool) => {
            remove_exact_from_section(&mut doc, &config, "tools", target, ItemKind::Tool)
        }
        Some(ItemKind::Package) => {
            remove_exact_from_section(&mut doc, &config, "packages", target, ItemKind::Package)
        }
        Some(ItemKind::App) => {
            remove_exact_from_section(&mut doc, &config, "apps", target, ItemKind::App)
        }
        None => remove_exact_from_section(&mut doc, &config, "tools", target, ItemKind::Tool)
            .or_else(|| {
                remove_exact_from_section(&mut doc, &config, "packages", target, ItemKind::Package)
            })
            .or_else(|| {
                remove_exact_from_section(&mut doc, &config, "apps", target, ItemKind::App)
            }),
    };

    Ok((doc.to_string(), removed))
}

fn remove_exact_from_section(
    doc: &mut DocumentMut,
    config: &crate::specs::toml::StillConfig,
    section: &str,
    target: &RemoveItemTarget,
    kind: ItemKind,
) -> Option<ItemKind> {
    if !configured_target_matches(config, target, kind)? {
        return None;
    }
    let table = doc.get_mut(section)?.as_table_mut()?;
    let mut removed = false;
    if section == "tools" {
        removed |= table.remove(&target.name).is_some();
    } else if target.version == "latest" && target.backend.is_none() {
        if let Some(latest) = table.get_mut("latest").and_then(Item::as_array_mut) {
            let before = latest.len();
            latest.retain(|value| value.as_str() != Some(target.name.as_str()));
            removed |= latest.len() != before;
        }
        removed |= table.remove(&target.name).is_some();
    } else {
        removed |= table.remove(&target.name).is_some();
    }
    removed.then_some(kind)
}

fn configured_target_matches(
    config: &crate::specs::toml::StillConfig,
    target: &RemoveItemTarget,
    kind: ItemKind,
) -> Option<bool> {
    let spec = ItemSpec {
        name: target.name.clone(),
        version: target.version.parse().ok()?,
        backend: target.backend.clone(),
    };
    match kind {
        ItemKind::Tool => config
            .tools
            .get(&target.name)
            .map(|entry| exact_tool_entry_matches(entry, &spec)),
        ItemKind::Package => exact_package_entry_matches(&config.packages, &spec),
        ItemKind::App => exact_package_entry_matches(&config.apps, &spec),
    }
}

fn exact_tool_entry_matches(entry: &crate::specs::toml::ToolEntry, spec: &ItemSpec) -> bool {
    match entry {
        crate::specs::toml::ToolEntry::Version(version) => {
            version == spec.version.as_str() && spec.backend.is_none()
        }
        crate::specs::toml::ToolEntry::Expanded(tool) => {
            let version = if tool.version.is_empty() {
                "latest"
            } else {
                tool.version.as_str()
            };
            version == spec.version.as_str()
                && spec
                    .backend
                    .as_ref()
                    .is_none_or(|backend| tool.backend.as_deref() == Some(backend.as_str()))
        }
    }
}

fn exact_package_entry_matches(
    map: &crate::specs::toml::PackageMap,
    spec: &ItemSpec,
) -> Option<bool> {
    if spec.version.is_latest() && spec.backend.is_none() {
        if map.latest.iter().any(|name| name == &spec.name) {
            return Some(true);
        }

        return map.entries.get(&spec.name).map(|entry| {
            let crate::specs::toml::PackageEntry::Expanded(package) = entry;
            package.version.as_deref().unwrap_or("latest") == "latest"
        });
    }

    map.entries.get(&spec.name).map(|entry| {
        let crate::specs::toml::PackageEntry::Expanded(package) = entry;
        let version = package.version.as_deref().unwrap_or("latest");
        version == spec.version.as_str()
            && spec
                .backend
                .as_ref()
                .is_none_or(|backend| package.backend.as_deref() == Some(backend.as_str()))
    })
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
        table.remove(&spec.name);
        append_unique_latest(table, &spec.name);
        return;
    }

    remove_latest_value(table, &spec.name);
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

fn remove_latest_value(table: &mut Table, name: &str) {
    if let Some(latest) = table.get_mut("latest").and_then(Item::as_array_mut) {
        latest.retain(|value| value.as_str() != Some(name));
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
    fn rejects_conflicting_duplicate_request_items_without_force() {
        let err = add_install_items(
            "",
            &[
                item(ItemKind::Package, "openssl"),
                item(ItemKind::Package, "openssl@3@homebrew"),
            ],
        )
        .unwrap_err();

        assert!(err.to_string().contains("requested more than once"));
        assert!(err.to_string().contains("--force"));
    }

    #[test]
    fn allows_identical_duplicate_request_items() {
        let output = add_install_items(
            "",
            &[
                item(ItemKind::Package, "openssl"),
                item(ItemKind::Package, "openssl"),
            ],
        )
        .unwrap();

        let config = parse(&output);
        assert_eq!(config.packages.latest, ["openssl"]);
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
    fn force_replaces_latest_package_with_keyed_entry() {
        let output = add_install_items_with_force(
            r#"
            [packages]
            latest = ["openssl", "llvm"]
            "#,
            &[item(ItemKind::Package, "openssl@3@homebrew")],
            true,
        )
        .unwrap();

        let config = parse(&output);
        assert_eq!(config.packages.latest, ["llvm"]);
        let PackageEntry::Expanded(openssl) = &config.packages.entries["openssl"];
        assert_eq!(openssl.version.as_deref(), Some("3"));
        assert_eq!(openssl.backend.as_deref(), Some("homebrew"));
    }

    #[test]
    fn force_replaces_keyed_app_with_latest_entry() {
        let output = add_install_items_with_force(
            r#"
            [apps]
            firefox = { version = "121", backend = "homebrew-cask" }
            latest = ["zed"]
            "#,
            &[item(ItemKind::App, "firefox")],
            true,
        )
        .unwrap();

        let config = parse(&output);
        assert_eq!(config.apps.latest, ["zed", "firefox"]);
        assert!(!config.apps.entries.contains_key("firefox"));
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
        let (output, removed) = remove_item("[tools]\nrust = \"stable\"\n", None, "rust").unwrap();

        let config = parse(&output);
        assert_eq!(removed, Some(ItemKind::Tool));
        assert!(!config.tools.contains_key("rust"));
    }

    #[test]
    fn removes_latest_package_entries() {
        let (output, removed) = remove_item(
            "[packages]\nlatest = [\"openssl\", \"llvm\"]\n",
            None,
            "openssl",
        )
        .unwrap();

        let config = parse(&output);
        assert_eq!(removed, Some(ItemKind::Package));
        assert_eq!(config.packages.latest, ["llvm"]);
    }

    #[test]
    fn typed_remove_only_removes_requested_kind() {
        let (output, removed) = remove_item(
            r#"
            [tools]
            zed = "latest"

            [apps]
            latest = ["zed"]
            "#,
            Some(ItemKind::App),
            "zed",
        )
        .unwrap();

        let config = parse(&output);
        assert_eq!(removed, Some(ItemKind::App));
        assert!(config.tools.contains_key("zed"));
        assert!(config.apps.latest.is_empty());
    }

    #[test]
    fn removes_exact_matching_entries() {
        let (output, removed) = remove_item_target(
            r#"
            [packages]
            latest = ["llvm"]
            openssl = { version = "3", backend = "homebrew" }
            "#,
            &remove_target("openssl@3@homebrew"),
        )
        .unwrap();

        let config = parse(&output);
        assert_eq!(removed, Some(ItemKind::Package));
        assert_eq!(config.packages.latest, ["llvm"]);
        assert!(!config.packages.entries.contains_key("openssl"));
    }

    #[test]
    fn exact_remove_version_matches_any_backend_when_backend_is_unspecified() {
        let (output, removed) = remove_item_target(
            r#"
            [tools]
            rust = { version = "stable", backend = "rustup" }
            "#,
            &remove_target("rust@stable"),
        )
        .unwrap();

        let config = parse(&output);
        assert_eq!(removed, Some(ItemKind::Tool));
        assert!(!config.tools.contains_key("rust"));
    }

    #[test]
    fn exact_remove_latest_matches_keyed_package_or_app_with_backend() {
        let (output, removed) = remove_item_target(
            r#"
            [apps]
            firefox = { backend = "homebrew-cask" }
            "#,
            &remove_target("firefox@latest"),
        )
        .unwrap();

        let config = parse(&output);
        assert_eq!(removed, Some(ItemKind::App));
        assert!(!config.apps.entries.contains_key("firefox"));
    }

    #[test]
    fn exact_remove_does_not_remove_mismatched_entries() {
        let (output, removed) = remove_item_target(
            r#"
            [packages]
            openssl = { version = "3", backend = "homebrew" }
            "#,
            &remove_target("openssl@1.1@homebrew"),
        )
        .unwrap();

        let config = parse(&output);
        assert_eq!(removed, None);
        assert!(config.packages.entries.contains_key("openssl"));
    }

    #[test]
    fn reports_when_remove_target_is_missing() {
        let (output, removed) = remove_item("[tools]\nrust = \"stable\"\n", None, "node").unwrap();

        assert_eq!(removed, None);
        assert_eq!(parse(&output).tools.len(), 1);
    }

    fn item(kind: ItemKind, spec: &str) -> InstallItemRequest {
        InstallItemRequest {
            kind,
            spec: spec.parse::<ItemSpec>().unwrap(),
        }
    }

    fn remove_target(spec: &str) -> RemoveItemTarget {
        let spec = spec.parse::<ItemSpec>().unwrap();
        RemoveItemTarget {
            kind: None,
            name: spec.name,
            version: spec.version.to_string(),
            backend: spec.backend,
        }
    }

    fn parse(input: &str) -> StillConfig {
        parse_still_toml(input).unwrap()
    }
}
