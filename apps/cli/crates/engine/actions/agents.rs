//! Engine action for inspecting and syncing agent config.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;

use crate::config::{ConfigScope, ConfigSelection, resolve_config_path};
use crate::error::EngineError;
use crate::specs::agents::{
    NormalizedAgents, NormalizedSkill, NormalizedSkillSource, managed_skill_dir_name,
    managed_skills_gitignore, normalize_agents,
};
use crate::specs::toml::parse_still_toml;

const MANAGED_MARKER: &str = ".still-managed";
const SOURCE_METADATA: &str = "source.toml";

/// Agent operation requested by a caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentsOperation {
    List,
    Check,
    Sync,
}

/// Request to inspect or sync configured agents.
#[derive(Debug, Clone)]
pub struct AgentsRequest {
    pub start_dir: PathBuf,
    pub home_dir: PathBuf,
    pub operation: AgentsOperation,
}

/// Normalized agent config and generated managed-skill metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentsResult {
    pub path: PathBuf,
    pub agents: NormalizedAgents,
    pub gitignore: String,
    pub gitignore_path: Option<PathBuf>,
}

/// Reads selected config, normalizes agent skills, and optionally writes ignore metadata.
/// # Errors
/// Fails when config cannot be read, parsed, normalized, or synced.
pub async fn run(request: AgentsRequest) -> Result<AgentsResult> {
    let resolved = resolve_config_path(
        &request.start_dir,
        &request.home_dir,
        ConfigSelection {
            scope: ConfigScope::Project,
            for_write: false,
        },
    )?;
    let content = tokio::fs::read_to_string(&resolved.path)
        .await
        .with_context(|| format!("failed to read {}", resolved.path.display()))?;
    let config = parse_still_toml(&content)?;
    let agents = normalize_agents(config.agents.unwrap_or_default())?;
    let gitignore = managed_skills_gitignore(&agents.skills)?;
    let gitignore_path = if request.operation == AgentsOperation::Sync {
        let skills_dir = resolved
            .path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .join(".agents")
            .join("skills");
        materialize_skills(&skills_dir, &agents.skills).await?;
        let path = skills_dir.join(".gitignore");
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&path, &gitignore).await?;
        Some(path)
    } else {
        None
    };

    Ok(AgentsResult {
        path: resolved.path,
        agents,
        gitignore,
        gitignore_path,
    })
}

async fn materialize_skills(root: &Path, skills: &[NormalizedSkill]) -> Result<()> {
    tokio::fs::create_dir_all(root).await?;
    for skill in skills {
        let dir_name = managed_skill_dir_name(&skill.name)?;
        let path = root.join(&dir_name);
        match tokio::fs::metadata(&path).await {
            Ok(metadata) if metadata.is_dir() => {
                let marker = path.join(MANAGED_MARKER);
                if tokio::fs::metadata(&marker).await.is_err() {
                    return Err(EngineError::Conflict {
                        message: format!(
                            "refusing to overwrite unmarked custom skill directory {}",
                            path.display()
                        ),
                    }
                    .into());
                }
            }
            Ok(_) => {
                return Err(EngineError::Conflict {
                    message: format!(
                        "refusing to overwrite non-directory skill path {}",
                        path.display()
                    ),
                }
                .into());
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                tokio::fs::create_dir_all(&path).await?;
            }
            Err(err) => return Err(err.into()),
        }

        tokio::fs::write(path.join(MANAGED_MARKER), managed_marker(skill)?).await?;
        tokio::fs::write(path.join(SOURCE_METADATA), source_metadata(skill)?).await?;
    }
    Ok(())
}

fn managed_marker(skill: &NormalizedSkill) -> Result<String> {
    toml_edit::ser::to_string(&ManagedSkillMarker {
        managed_by: "still",
        name: &skill.name,
    })
    .map_err(Into::into)
}

fn source_metadata(skill: &NormalizedSkill) -> Result<String> {
    toml_edit::ser::to_string(&ManagedSkillMetadata::from(skill)).map_err(Into::into)
}

#[derive(Debug, Serialize)]
struct ManagedSkillMarker<'a> {
    managed_by: &'a str,
    name: &'a str,
}

#[derive(Debug, Serialize)]
struct ManagedSkillMetadata {
    name: String,
    source_kind: &'static str,
    source: String,
    version: Option<String>,
    auto: bool,
    tools: Vec<String>,
    packages: Vec<String>,
    apps: Vec<String>,
}

impl From<&NormalizedSkill> for ManagedSkillMetadata {
    fn from(skill: &NormalizedSkill) -> Self {
        let (source_kind, source, version) = match &skill.source {
            NormalizedSkillSource::Official { name } => ("official", name.clone(), None),
            NormalizedSkillSource::GitHub { path } => ("github", path.clone(), None),
            NormalizedSkillSource::Url { url, version } => ("url", url.clone(), version.clone()),
        };

        Self {
            name: skill.name.clone(),
            source_kind,
            source,
            version,
            auto: skill.auto,
            tools: spec_strings(&skill.tools),
            packages: spec_strings(&skill.packages),
            apps: spec_strings(&skill.apps),
        }
    }
}

fn spec_strings(specs: &[crate::specs::item::ItemSpec]) -> Vec<String> {
    specs
        .iter()
        .map(|spec| match &spec.backend {
            Some(backend) => format!("{}@{}@{}", spec.name, spec.version, backend),
            None => format!("{}@{}", spec.name, spec.version),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[tokio::test]
    async fn agents_check_normalizes_config_without_writing_gitignore() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("still.toml"),
            r#"
            [agents]
            targets = ["claude", "codex"]
            instructions = "AGENTS.md"
            skills = ["rust-review", "repo-auditor"]
            "#,
        )
        .unwrap();

        let result = run(AgentsRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            operation: AgentsOperation::Check,
        })
        .await
        .unwrap();

        assert_eq!(result.agents.targets, ["claude", "codex"]);
        assert_eq!(
            result.gitignore,
            "# still-managed skills\n/repo-auditor/\n/rust-review/\n"
        );
        assert_eq!(result.gitignore_path, None);
        assert!(!temp.path().join(".agents/skills/.gitignore").exists());
    }

    #[tokio::test]
    async fn agents_sync_writes_managed_skill_gitignore() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("still.toml"),
            r#"
            [agents]
            skills = ["rust-review"]
            "#,
        )
        .unwrap();

        let result = run(AgentsRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            operation: AgentsOperation::Sync,
        })
        .await
        .unwrap();

        let path = temp.path().join(".agents/skills/.gitignore");
        let skill_dir = temp.path().join(".agents/skills/rust-review");
        assert_eq!(result.gitignore_path, Some(path.clone()));
        assert_eq!(
            fs::read_to_string(path).unwrap(),
            "# still-managed skills\n/rust-review/\n"
        );
        assert!(skill_dir.join(".still-managed").is_file());
        let metadata = fs::read_to_string(skill_dir.join("source.toml")).unwrap();
        assert!(metadata.contains("source_kind = \"official\""));
        assert!(metadata.contains("source = \"rust-review\""));
    }

    #[tokio::test]
    async fn agents_sync_refuses_unmarked_custom_skill_directory() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("still.toml"),
            r#"
            [agents]
            skills = ["custom"]
            "#,
        )
        .unwrap();
        fs::create_dir_all(temp.path().join(".agents/skills/custom")).unwrap();
        fs::write(
            temp.path().join(".agents/skills/custom/SKILL.md"),
            "# Custom\n",
        )
        .unwrap();

        let err = run(AgentsRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            operation: AgentsOperation::Sync,
        })
        .await
        .unwrap_err();

        assert!(err.to_string().contains("refusing to overwrite"));
        assert!(temp.path().join(".agents/skills/custom/SKILL.md").is_file());
    }
}
