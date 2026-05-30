//! Engine action for inspecting and syncing agent config.

use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::config::{ConfigScope, ConfigSelection, resolve_config_path};
use crate::specs::agents::{NormalizedAgents, managed_skills_gitignore, normalize_agents};
use crate::specs::toml::parse_still_toml;

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
        let path = resolved
            .path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .join(".agents")
            .join("skills")
            .join(".gitignore");
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
        assert_eq!(result.gitignore_path, Some(path.clone()));
        assert_eq!(
            fs::read_to_string(path).unwrap(),
            "# still-managed skills\n/rust-review/\n"
        );
    }
}
