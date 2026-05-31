//! Normalized agent and skill config models.

use crate::error::{EngineError, EngineResult};
use crate::specs::item::ItemSpec;
use crate::specs::toml::{AgentSkills, AgentsConfig, SkillSource};

const SUPPORTED_TARGETS: &[&str] = &["claude", "codex"];

/// Normalized agent desired state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedAgents {
    pub targets: Vec<String>,
    pub instructions: Option<String>,
    pub skills: Vec<NormalizedSkill>,
}

/// One normalized agent skill source and its dependency policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedSkill {
    pub name: String,
    pub source: NormalizedSkillSource,
    pub auto: bool,
    pub tools: Vec<ItemSpec>,
    pub packages: Vec<ItemSpec>,
    pub apps: Vec<ItemSpec>,
}

/// Supported skill source forms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NormalizedSkillSource {
    Official {
        name: String,
        version: Option<String>,
    },
    GitHub {
        path: String,
        version: Option<String>,
    },
    Url {
        url: String,
        version: Option<String>,
    },
}

/// Builds `.agents/skills/.gitignore` content for Still-managed skill folders.
pub fn managed_skills_gitignore(skills: &[NormalizedSkill]) -> EngineResult<String> {
    let mut names = skills
        .iter()
        .map(|skill| managed_skill_dir_name(&skill.name))
        .collect::<EngineResult<Vec<_>>>()?;
    names.sort();
    names.dedup();

    let mut output = String::from("# still-managed skills\n");
    for name in names {
        output.push('/');
        output.push_str(&name);
        output.push_str("/\n");
    }
    Ok(output)
}

/// Validates and returns the directory name for a managed skill.
pub fn managed_skill_dir_name(name: &str) -> EngineResult<String> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains('/')
        || name.contains('\\')
        || name.contains('*')
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
    {
        return Err(EngineError::InvalidConfig {
            reason: format!("invalid managed skill directory name \"{name}\""),
        });
    }

    Ok(name.to_string())
}

/// Converts parsed `[agents]` config into command/planner friendly data.
pub fn normalize_agents(config: AgentsConfig) -> EngineResult<NormalizedAgents> {
    let targets = normalize_targets(config.targets)?;
    if let Some(instructions) = config.instructions.as_deref()
        && instructions.trim().is_empty()
    {
        return Err(EngineError::InvalidConfig {
            reason: "agent instructions cannot be empty".to_string(),
        });
    }
    let skills = match config.skills {
        Some(AgentSkills::List(skills)) => skills
            .into_iter()
            .map(|skill| {
                validate_non_empty("agent skill", &skill)?;
                let source = source_from_shorthand(&skill);
                Ok(NormalizedSkill {
                    name: skill_name(&skill),
                    source,
                    auto: false,
                    tools: Vec::new(),
                    packages: Vec::new(),
                    apps: Vec::new(),
                })
            })
            .collect::<EngineResult<Vec<_>>>()?,
        Some(AgentSkills::Table(skills)) => skills
            .into_iter()
            .map(|(name, source)| {
                validate_non_empty("agent skill name", &name)?;
                normalize_skill_entry(name, source)
            })
            .collect::<EngineResult<Vec<_>>>()?,
        None => Vec::new(),
    };

    Ok(NormalizedAgents {
        targets,
        instructions: config.instructions,
        skills,
    })
}

fn normalize_targets(targets: Vec<String>) -> EngineResult<Vec<String>> {
    let mut normalized = Vec::new();
    for target in targets {
        if !SUPPORTED_TARGETS.contains(&target.as_str()) {
            return Err(EngineError::InvalidConfig {
                reason: format!(
                    "unsupported agent target \"{target}\"; supported targets are claude, codex"
                ),
            });
        }
        if normalized.contains(&target) {
            return Err(EngineError::InvalidConfig {
                reason: format!("duplicate agent target \"{target}\""),
            });
        }
        normalized.push(target);
    }
    Ok(normalized)
}

fn normalize_skill_entry(name: String, source: SkillSource) -> EngineResult<NormalizedSkill> {
    match source {
        SkillSource::Shorthand(value) => {
            validate_non_empty(&format!("agent skill \"{name}\" source"), &value)?;
            Ok(NormalizedSkill {
                name,
                source: source_from_shorthand(&value),
                auto: false,
                tools: Vec::new(),
                packages: Vec::new(),
                apps: Vec::new(),
            })
        }
        SkillSource::Expanded(expanded) => {
            if let Some(version) = expanded.version.as_deref() {
                validate_non_empty(&format!("agent skill \"{name}\" version"), version)?;
            }
            let source = match (expanded.source, expanded.url) {
                (Some(source), None) => {
                    validate_non_empty(&format!("agent skill \"{name}\" source"), &source)?;
                    source_from_shorthand_with_version(&source, expanded.version)
                }
                (None, Some(url)) => NormalizedSkillSource::Url {
                    url: {
                        validate_non_empty(&format!("agent skill \"{name}\" url"), &url)?;
                        url
                    },
                    version: expanded.version,
                },
                (None, None) => source_from_shorthand_with_version(&name, expanded.version),
                (Some(_), Some(_)) => {
                    return Err(EngineError::InvalidConfig {
                        reason: format!("agent skill \"{name}\" cannot set both source and url"),
                    });
                }
            };

            Ok(NormalizedSkill {
                name,
                source,
                auto: expanded.auto,
                tools: parse_specs("tool", expanded.tools)?,
                packages: parse_specs("package", expanded.packages)?,
                apps: parse_specs("app", expanded.apps)?,
            })
        }
    }
}

fn validate_non_empty(label: &str, value: &str) -> EngineResult<()> {
    if value.trim().is_empty() {
        return Err(EngineError::InvalidConfig {
            reason: format!("{label} cannot be empty"),
        });
    }
    Ok(())
}

fn source_from_shorthand(value: &str) -> NormalizedSkillSource {
    source_from_shorthand_with_version(value, None)
}

fn source_from_shorthand_with_version(
    value: &str,
    explicit_version: Option<String>,
) -> NormalizedSkillSource {
    if value.contains("://") {
        let (url, version) = split_url_version(value);
        return NormalizedSkillSource::Url {
            url,
            version: explicit_version.or(version),
        };
    }

    if value.contains('/') {
        return NormalizedSkillSource::GitHub {
            path: value.to_string(),
            version: explicit_version,
        };
    }

    NormalizedSkillSource::Official {
        name: value.to_string(),
        version: explicit_version,
    }
}

fn split_url_version(value: &str) -> (String, Option<String>) {
    let Some((prefix, suffix)) = value.rsplit_once(':') else {
        return (value.to_string(), None);
    };

    if prefix.ends_with("http") || prefix.ends_with("https") || suffix.contains('/') {
        return (value.to_string(), None);
    }

    (prefix.to_string(), Some(suffix.to_string()))
}

fn skill_name(source: &str) -> String {
    source
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(source)
        .split(':')
        .next()
        .unwrap_or(source)
        .to_string()
}

pub(crate) fn parse_skill_dependency_specs(
    label: &str,
    values: Vec<String>,
) -> EngineResult<Vec<ItemSpec>> {
    values
        .into_iter()
        .map(|value| {
            value
                .parse()
                .map_err(|err: anyhow::Error| EngineError::InvalidConfig {
                    reason: format!("invalid agent skill {label} dependency \"{value}\": {err}"),
                })
        })
        .collect()
}

fn parse_specs(label: &str, values: Vec<String>) -> EngineResult<Vec<ItemSpec>> {
    parse_skill_dependency_specs(label, values)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::specs::toml::{ExpandedSkillSource, parse_still_toml};

    use super::*;

    #[test]
    fn normalizes_list_skills_from_official_and_github_sources() {
        let config = parse_still_toml(
            r#"
            [agents]
            targets = ["claude", "codex"]
            instructions = "AGENTS.md"
            skills = ["rust-review", "owner/repo-auditor"]
            "#,
        )
        .unwrap()
        .agents
        .unwrap();

        let agents = normalize_agents(config).unwrap();

        assert_eq!(agents.targets, ["claude", "codex"]);
        assert_eq!(agents.instructions.as_deref(), Some("AGENTS.md"));
        assert_eq!(
            agents.skills[0].source,
            NormalizedSkillSource::Official {
                name: "rust-review".to_string(),
                version: None
            }
        );
        assert_eq!(
            agents.skills[1].source,
            NormalizedSkillSource::GitHub {
                path: "owner/repo-auditor".to_string(),
                version: None
            }
        );
    }

    #[test]
    fn normalizes_table_skill_source_forms() {
        let config = parse_still_toml(
            r#"
            [agents]
            targets = ["codex"]

            [agents.skills]
            external = { url = "https://example.com/skill", version = "1.2.0" }
            pinned = "https://example.com/skill:2.0.0"
            latest = "https://example.com/skill"
            rust-review = { source = "rust-review", auto = true, tools = ["rust@stable@rustup"], packages = ["llvm"] }
            pinned-official = { source = "repo-auditor", version = "v1.2.3" }
            pinned-github = { source = "owner/repo-auditor", version = "abcdef" }
            "#,
        )
        .unwrap()
        .agents
        .unwrap();

        let agents = normalize_agents(config).unwrap();

        assert!(agents.skills.iter().any(|skill| {
            skill.name == "external"
                && skill.source
                    == NormalizedSkillSource::Url {
                        url: "https://example.com/skill".to_string(),
                        version: Some("1.2.0".to_string()),
                    }
        }));
        let rust = agents
            .skills
            .iter()
            .find(|skill| skill.name == "rust-review")
            .unwrap();
        assert!(rust.auto);
        assert_eq!(rust.tools[0].name, "rust");
        assert_eq!(rust.tools[0].backend.as_ref().unwrap().as_str(), "rustup");
        assert_eq!(rust.packages[0].name, "llvm");
        assert!(agents.skills.iter().any(|skill| {
            skill.name == "pinned-official"
                && skill.source
                    == NormalizedSkillSource::Official {
                        name: "repo-auditor".to_string(),
                        version: Some("v1.2.3".to_string()),
                    }
        }));
        assert!(agents.skills.iter().any(|skill| {
            skill.name == "pinned-github"
                && skill.source
                    == NormalizedSkillSource::GitHub {
                        path: "owner/repo-auditor".to_string(),
                        version: Some("abcdef".to_string()),
                    }
        }));
    }

    #[test]
    fn rejects_expanded_skill_with_source_and_url() {
        let config = AgentsConfig {
            skills: Some(AgentSkills::Table(BTreeMap::from([(
                "bad".to_string(),
                SkillSource::Expanded(ExpandedSkillSource {
                    source: Some("rust-review".to_string()),
                    url: Some("https://example.com/skill".to_string()),
                    ..ExpandedSkillSource::default()
                }),
            )]))),
            ..AgentsConfig::default()
        };

        let err = normalize_agents(config).unwrap_err();

        assert!(err.to_string().contains("cannot set both source and url"));
    }

    #[test]
    fn rejects_empty_agent_instructions() {
        let err = parse_still_toml(
            r#"
            [agents]
            instructions = " "
            "#,
        )
        .unwrap_err();

        assert!(
            err.to_string()
                .contains("agent instructions cannot be empty")
        );
    }

    #[test]
    fn rejects_empty_agent_skill_sources() {
        let err = parse_still_toml(
            r#"
            [agents]
            skills = [" "]
            "#,
        )
        .unwrap_err();

        assert!(err.to_string().contains("agent skill cannot be empty"));

        let err = parse_still_toml(
            r#"
            [agents.skills]
            rust-review = { url = " " }
            "#,
        )
        .unwrap_err();

        assert!(
            err.to_string()
                .contains("agent skill \"rust-review\" url cannot be empty")
        );
    }

    #[test]
    fn rejects_unknown_agent_targets() {
        let config = AgentsConfig {
            targets: vec!["claude".to_string(), "unknown".to_string()],
            ..AgentsConfig::default()
        };

        let err = normalize_agents(config).unwrap_err();

        assert!(err.to_string().contains("unsupported agent target"));
    }

    #[test]
    fn rejects_duplicate_agent_targets() {
        let config = AgentsConfig {
            targets: vec![
                "codex".to_string(),
                "claude".to_string(),
                "codex".to_string(),
            ],
            ..AgentsConfig::default()
        };

        let err = normalize_agents(config).unwrap_err();

        assert!(err.to_string().contains("duplicate agent target \"codex\""));
    }

    #[test]
    fn generates_anchored_gitignore_for_managed_skills() {
        let skills = vec![
            normalized_skill("repo-auditor"),
            normalized_skill("rust-review"),
            normalized_skill("repo-auditor"),
        ];

        let output = managed_skills_gitignore(&skills).unwrap();

        assert_eq!(
            output,
            "# still-managed skills\n/repo-auditor/\n/rust-review/\n"
        );
        assert!(!output.contains('*'));
    }

    #[test]
    fn rejects_unsafe_managed_skill_directory_names() {
        let err = managed_skills_gitignore(&[normalized_skill("../custom")]).unwrap_err();

        assert!(
            err.to_string()
                .contains("invalid managed skill directory name")
        );
    }

    fn normalized_skill(name: &str) -> NormalizedSkill {
        NormalizedSkill {
            name: name.to_string(),
            source: NormalizedSkillSource::Official {
                name: name.to_string(),
                version: None,
            },
            auto: false,
            tools: Vec::new(),
            packages: Vec::new(),
            apps: Vec::new(),
        }
    }
}
