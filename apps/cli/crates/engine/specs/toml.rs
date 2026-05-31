//! Still TOML config models.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::error::EngineError;
use crate::platform::PlatformId;
use crate::specs::agents::normalize_agents;

/// Parsed project or global Still config.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct StillConfig {
    pub tools: BTreeMap<String, ToolEntry>,
    pub env: EnvConfig,
    pub packages: PackageMap,
    pub apps: PackageMap,
    pub services: BTreeMap<String, ServiceEntry>,
    pub tasks: BTreeMap<String, TaskEntry>,
    pub agents: Option<AgentsConfig>,
}

/// Tool config value from `[tools]`.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ToolEntry {
    Version(String),
    Expanded(ExpandedTool),
}

/// Expanded tool config.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct ExpandedTool {
    pub version: String,
    pub backend: Option<String>,
    pub backends: BTreeMap<String, String>,
    pub components: Vec<String>,
    pub targets: Vec<String>,
}

/// Environment variables and env files.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct EnvConfig {
    pub files: Vec<String>,
    #[serde(flatten)]
    pub vars: BTreeMap<String, String>,
}

/// Shared package/app map.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct PackageMap {
    pub latest: Vec<String>,
    #[serde(flatten)]
    pub entries: BTreeMap<String, PackageEntry>,
}

/// Package or app config value.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum PackageEntry {
    Expanded(ExpandedPackage),
}

/// Inline package/app config.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct ExpandedPackage {
    pub version: Option<String>,
    pub backend: Option<String>,
    pub backends: BTreeMap<String, String>,
    pub names: BTreeMap<String, String>,
    pub platforms: Vec<String>,
    pub ignore: Option<String>,
    pub only: Option<String>,
}

/// Service config value.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ServiceEntry {
    Command(String),
    Expanded(ExpandedService),
}

/// Expanded service config.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct ExpandedService {
    pub preset: Option<String>,
    pub task: Option<String>,
    pub start: Option<ServiceAction>,
    pub stop: Option<ServiceAction>,
    pub check: Option<ServiceAction>,
}

/// Service action configured by task or direct command.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ServiceAction {
    Task(ServiceTaskAction),
    Command(ServiceCommandAction),
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ServiceTaskAction {
    pub task: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ServiceCommandAction {
    pub command: String,
}

/// Task config value.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum TaskEntry {
    Command(String),
    Expanded(ExpandedTask),
}

/// Expanded task config.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct ExpandedTask {
    pub description: Option<String>,
    pub run: TaskRun,
    pub depends: Vec<String>,
    pub requires: Vec<String>,
}

/// Command or command list for a task.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum TaskRun {
    None,
    Command(String),
    Commands(Vec<String>),
}

impl Default for TaskRun {
    fn default() -> Self {
        Self::None
    }
}

/// Agent target and skill config.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct AgentsConfig {
    pub targets: Vec<String>,
    pub instructions: Option<String>,
    pub skills: Option<AgentSkills>,
}

/// Agent skills can be an array or table.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum AgentSkills {
    List(Vec<String>),
    Table(BTreeMap<String, SkillSource>),
}

/// Skill source shorthand or expanded metadata.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum SkillSource {
    Shorthand(String),
    Expanded(ExpandedSkillSource),
}

/// Expanded skill source metadata.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct ExpandedSkillSource {
    pub source: Option<String>,
    pub url: Option<String>,
    pub version: Option<String>,
    pub auto: bool,
    pub tools: Vec<String>,
    pub packages: Vec<String>,
    pub apps: Vec<String>,
}

/// Parses a Still TOML document into typed config.
/// # Errors
/// Fails when the document is invalid TOML or does not match the config model.
pub fn parse_still_toml(input: &str) -> Result<StillConfig> {
    let config: StillConfig =
        toml_edit::de::from_str(input).context("failed to parse still.toml")?;
    validate_config(&config)?;
    Ok(config)
}

fn validate_config(config: &StillConfig) -> Result<()> {
    validate_env_config(&config.env)?;

    for (name, entry) in &config.tools {
        if let ToolEntry::Expanded(tool) = entry
            && tool.version.trim().is_empty()
        {
            return Err(EngineError::InvalidConfig {
                reason: format!("tool \"{name}\" must define version"),
            }
            .into());
        }
        if let ToolEntry::Expanded(tool) = entry {
            validate_platform_map(&format!("tool \"{name}\" backends"), &tool.backends)?;
        }
    }

    validate_package_map("package", &config.packages)?;
    validate_package_map("app", &config.apps)?;

    for (name, entry) in &config.services {
        if let ServiceEntry::Expanded(service) = entry {
            if let Some(preset) = service.preset.as_deref()
                && !is_supported_service_preset(preset)
            {
                return Err(EngineError::InvalidConfig {
                    reason: format!("service \"{name}\" uses unknown preset \"{preset}\""),
                }
                .into());
            }
            if service.preset.is_none()
                && service.task.is_none()
                && service.start.is_none()
                && service.check.is_none()
            {
                return Err(EngineError::InvalidConfig {
                    reason: format!("service \"{name}\" must define preset, task, start, or check"),
                }
                .into());
            }
        }
    }

    for (name, entry) in &config.tasks {
        if let TaskEntry::Expanded(task) = entry {
            match &task.run {
                TaskRun::None => {
                    return Err(EngineError::InvalidConfig {
                        reason: format!("task \"{name}\" must define run"),
                    }
                    .into());
                }
                TaskRun::Commands(commands) if commands.is_empty() => {
                    return Err(EngineError::InvalidConfig {
                        reason: format!("task \"{name}\" must define at least one run command"),
                    }
                    .into());
                }
                TaskRun::Command(_) | TaskRun::Commands(_) => {}
            }
        }
    }

    if let Some(agents) = &config.agents {
        normalize_agents(agents.clone())?;
    }

    Ok(())
}

fn validate_env_config(env: &EnvConfig) -> Result<()> {
    for file in &env.files {
        if file.trim().is_empty() {
            return Err(EngineError::InvalidConfig {
                reason: "env files cannot contain empty entries".to_string(),
            }
            .into());
        }
    }
    Ok(())
}

fn validate_package_map(kind: &str, map: &PackageMap) -> Result<()> {
    let mut latest = BTreeSet::new();
    for name in &map.latest {
        if !latest.insert(name) {
            return Err(EngineError::InvalidConfig {
                reason: format!("{kind} \"{name}\" is listed more than once in latest"),
            }
            .into());
        }
        if map.entries.contains_key(name) {
            return Err(EngineError::InvalidConfig {
                reason: format!("{kind} \"{name}\" is configured in both latest and keyed entries"),
            }
            .into());
        }
    }
    for (name, entry) in &map.entries {
        let PackageEntry::Expanded(package) = entry;
        validate_platform_map(&format!("{kind} \"{name}\" backends"), &package.backends)?;
        validate_platform_map(&format!("{kind} \"{name}\" names"), &package.names)?;
        for field in &package.platforms {
            validate_platform_value(&format!("{kind} \"{name}\" platforms"), field)?;
        }
        if let Some(ignore) = &package.ignore {
            validate_platform_value(&format!("{kind} \"{name}\" ignore"), ignore)?;
        }
        if let Some(only) = &package.only {
            validate_platform_value(&format!("{kind} \"{name}\" only"), only)?;
        }
    }
    Ok(())
}

fn validate_platform_map(label: &str, map: &BTreeMap<String, String>) -> Result<()> {
    for key in map.keys() {
        validate_platform_value(label, key)?;
    }
    Ok(())
}

fn validate_platform_value(label: &str, value: &str) -> Result<()> {
    if value.parse::<PlatformId>().is_err() {
        return Err(EngineError::InvalidConfig {
            reason: format!("{label} contains unsupported platform \"{value}\""),
        }
        .into());
    }
    Ok(())
}

fn is_supported_service_preset(name: &str) -> bool {
    matches!(
        name,
        "docker" | "docker-compose" | "compose" | "postgres" | "postgresql" | "redis"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tool_shorthand_and_expanded_tool() {
        let config = parse_still_toml(
            r#"
            [tools]
            node = "22"

            [tools.rust]
            version = "stable"
            backend = "rustup"
            backends = { macos = "rustup", linux = "mise" }
            components = ["rustfmt", "clippy"]
            targets = ["wasm32-unknown-unknown"]
            "#,
        )
        .unwrap();

        assert_eq!(config.tools["node"], ToolEntry::Version("22".to_string()));
        let ToolEntry::Expanded(rust) = &config.tools["rust"] else {
            panic!("rust should be expanded");
        };
        assert_eq!(rust.version, "stable");
        assert_eq!(rust.backend.as_deref(), Some("rustup"));
        assert_eq!(rust.backends["linux"], "mise");
        assert_eq!(rust.components, ["rustfmt", "clippy"]);
    }

    #[test]
    fn parses_packages_apps_and_platform_names() {
        let config = parse_still_toml(
            r#"
            [packages]
            latest = ["ripgrep"]
            postgresql = { version = "16", backend = "auto", backends = { macos = "homebrew", linux = "apt" } }

            [packages.fd.names]
            macos = "fd"
            linux = "fd-find"

            [apps]
            latest = ["zed"]
            "#,
        )
        .unwrap();

        assert_eq!(config.packages.latest, ["ripgrep"]);
        assert!(config.packages.entries.contains_key("postgresql"));
        let PackageEntry::Expanded(postgresql) = &config.packages.entries["postgresql"];
        assert_eq!(postgresql.backends["linux"], "apt");
        let PackageEntry::Expanded(fd) = &config.packages.entries["fd"];
        assert_eq!(fd.names["linux"], "fd-find");
        assert_eq!(config.apps.latest, ["zed"]);
    }

    #[test]
    fn parses_tasks_services_and_agents() {
        let config = parse_still_toml(
            r#"
            [services]
            docker-compose = "docker compose up"
            dev-server = { task = "dev-server" }

            [tasks]
            test = "cargo test"

            [tasks.ci]
            description = "Run full CI"
            depends = ["test"]
            run = ["cargo fmt --check", "cargo test"]

            [agents]
            targets = ["claude", "codex"]
            instructions = "AGENTS.md"

            [agents.skills]
            rust-review = { source = "rust-review", auto = true, tools = ["rust@stable@rustup"] }
            repo-auditor = "repo-auditor"
            "#,
        )
        .unwrap();

        assert!(matches!(
            config.services["docker-compose"],
            ServiceEntry::Command(_)
        ));
        assert!(matches!(config.tasks["test"], TaskEntry::Command(_)));
        let TaskEntry::Expanded(ci) = &config.tasks["ci"] else {
            panic!("ci should be expanded");
        };
        assert_eq!(ci.depends, ["test"]);
        let agents = config.agents.unwrap();
        assert_eq!(agents.targets, ["claude", "codex"]);
        let Some(AgentSkills::Table(skills)) = agents.skills else {
            panic!("skills should be table");
        };
        assert!(skills.contains_key("rust-review"));
        assert!(skills.contains_key("repo-auditor"));
    }

    #[test]
    fn parses_checked_in_examples() {
        parse_still_toml(include_str!("../../../examples/still.toml")).unwrap();
        serde_json::from_str::<serde_json::Value>(include_str!(
            "../../../examples/still.schema.json"
        ))
        .unwrap();
    }

    #[test]
    fn schema_allows_service_task_reference_shape() {
        let schema: serde_json::Value =
            serde_json::from_str(include_str!("../../../examples/still.schema.json")).unwrap();
        let service_map = &schema["$defs"]["serviceMap"]["additionalProperties"];

        assert!(service_map.get("anyOf").is_some());
        assert!(service_map.get("oneOf").is_none());
    }

    #[test]
    fn rejects_expanded_tool_without_version() {
        let err = parse_still_toml(
            r#"
            [tools.rust]
            backend = "rustup"
            "#,
        )
        .unwrap_err();

        assert!(
            err.to_string()
                .contains("tool \"rust\" must define version")
        );
    }

    #[test]
    fn rejects_empty_env_file_entries() {
        let err = parse_still_toml(
            r#"
            [env]
            files = [".env", " "]
            "#,
        )
        .unwrap_err();

        assert!(
            err.to_string()
                .contains("env files cannot contain empty entries")
        );
    }

    #[test]
    fn rejects_expanded_task_without_run() {
        let err = parse_still_toml(
            r#"
            [tasks.ci]
            depends = ["test"]
            "#,
        )
        .unwrap_err();

        assert!(err.to_string().contains("task \"ci\" must define run"));
    }

    #[test]
    fn rejects_expanded_service_without_action() {
        let err = parse_still_toml(
            r#"
            [services.web]
            stop = { command = "echo stop" }
            "#,
        )
        .unwrap_err();

        assert!(
            err.to_string()
                .contains("service \"web\" must define preset, task, start, or check")
        );
    }

    #[test]
    fn rejects_unknown_service_preset() {
        let err = parse_still_toml(
            r#"
            [services.db]
            preset = "postgresql-typo"
            "#,
        )
        .unwrap_err();

        assert!(
            err.to_string()
                .contains("service \"db\" uses unknown preset \"postgresql-typo\"")
        );
    }

    #[test]
    fn rejects_service_action_with_extra_fields() {
        let err = parse_still_toml(
            r#"
            [services.web]
            start = { task = "web:start", command = "npm run dev" }
            "#,
        )
        .unwrap_err();

        assert!(err.to_string().contains("failed to parse still.toml"));
    }

    #[test]
    fn rejects_service_action_with_unknown_fields() {
        let err = parse_still_toml(
            r#"
            [services.web]
            start = { script = "npm run dev" }
            "#,
        )
        .unwrap_err();

        assert!(err.to_string().contains("failed to parse still.toml"));
    }

    #[test]
    fn rejects_unknown_platform_in_tool_backends() {
        let err = parse_still_toml(
            r#"
            [tools.rust]
            version = "stable"
            backends = { freebsd = "pkg" }
            "#,
        )
        .unwrap_err();

        assert!(
            err.to_string()
                .contains("tool \"rust\" backends contains unsupported platform \"freebsd\"")
        );
    }

    #[test]
    fn rejects_unknown_platform_in_package_metadata() {
        let err = parse_still_toml(
            r#"
            [packages.fd.names]
            freebsd = "fd"
            "#,
        )
        .unwrap_err();

        assert!(
            err.to_string()
                .contains("package \"fd\" names contains unsupported platform \"freebsd\"")
        );
    }

    #[test]
    fn rejects_unknown_platform_in_app_filters() {
        let err = parse_still_toml(
            r#"
            [apps.zed]
            version = "latest"
            only = "freebsd"
            "#,
        )
        .unwrap_err();

        assert!(
            err.to_string()
                .contains("app \"zed\" only contains unsupported platform \"freebsd\"")
        );
    }

    #[test]
    fn rejects_package_configured_in_latest_and_keyed_entry() {
        let err = parse_still_toml(
            r#"
            [packages]
            latest = ["openssl"]
            openssl = { version = "3" }
            "#,
        )
        .unwrap_err();

        assert!(
            err.to_string()
                .contains("package \"openssl\" is configured in both latest and keyed entries")
        );
    }

    #[test]
    fn rejects_duplicate_latest_apps() {
        let err = parse_still_toml(
            r#"
            [apps]
            latest = ["firefox", "firefox"]
            "#,
        )
        .unwrap_err();

        assert!(
            err.to_string()
                .contains("app \"firefox\" is listed more than once in latest")
        );
    }

    #[test]
    fn rejects_unknown_agent_targets_during_config_parse() {
        let err = parse_still_toml(
            r#"
            [agents]
            targets = ["claude", "unknown"]
            "#,
        )
        .unwrap_err();

        assert!(err.to_string().contains("unsupported agent target"));
    }

    #[test]
    fn rejects_invalid_agent_skill_dependencies_during_config_parse() {
        let err = parse_still_toml(
            r#"
            [agents.skills]
            rust-review = { source = "rust-review", tools = ["bad/tool"] }
            "#,
        )
        .unwrap_err();

        assert!(
            err.to_string()
                .contains("invalid agent skill tool dependency \"bad/tool\"")
        );
    }
}
