//! Normalized agent and skill config models.

use crate::error::{EngineError, EngineResult};
use crate::specs::item::ItemSpec;
use crate::specs::toml::{AgentSkills, AgentsConfig, SkillSource};

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
    },
    GitHub {
        path: String,
    },
    Url {
        url: String,
        version: Option<String>,
    },
}

/// Converts parsed `[agents]` config into command/planner friendly data.
pub fn normalize_agents(config: AgentsConfig) -> EngineResult<NormalizedAgents> {
    let skills = match config.skills {
        Some(AgentSkills::List(skills)) => skills
            .into_iter()
            .map(|skill| {
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
            .map(|(name, source)| normalize_skill_entry(name, source))
            .collect::<EngineResult<Vec<_>>>()?,
        None => Vec::new(),
    };

    Ok(NormalizedAgents {
        targets: config.targets,
        instructions: config.instructions,
        skills,
    })
}

fn normalize_skill_entry(name: String, source: SkillSource) -> EngineResult<NormalizedSkill> {
    match source {
        SkillSource::Shorthand(value) => Ok(NormalizedSkill {
            name,
            source: source_from_shorthand(&value),
            auto: false,
            tools: Vec::new(),
            packages: Vec::new(),
            apps: Vec::new(),
        }),
        SkillSource::Expanded(expanded) => {
            let source = match (expanded.source, expanded.url) {
                (Some(source), None) => source_from_shorthand(&source),
                (None, Some(url)) => NormalizedSkillSource::Url {
                    url,
                    version: expanded.version,
                },
                (None, None) => source_from_shorthand(&name),
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

fn source_from_shorthand(value: &str) -> NormalizedSkillSource {
    if value.contains("://") {
        let (url, version) = split_url_version(value);
        return NormalizedSkillSource::Url { url, version };
    }

    if value.contains('/') {
        return NormalizedSkillSource::GitHub {
            path: value.to_string(),
        };
    }

    NormalizedSkillSource::Official {
        name: value.to_string(),
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

fn parse_specs(label: &str, values: Vec<String>) -> EngineResult<Vec<ItemSpec>> {
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
                name: "rust-review".to_string()
            }
        );
        assert_eq!(
            agents.skills[1].source,
            NormalizedSkillSource::GitHub {
                path: "owner/repo-auditor".to_string()
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
}
