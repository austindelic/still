//! Engine action for inspecting and syncing agent config.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::actions::install::InstallItemRequest;
use crate::actions::sync::refresh_lockfile;
use crate::config::{ConfigScope, ConfigSelection, resolve_config_path};
use crate::config_edit::add_install_items;
use crate::error::EngineError;
use crate::specs::agents::{
    NormalizedAgents, NormalizedSkill, NormalizedSkillSource, managed_skill_dir_name,
    managed_skills_gitignore, normalize_agents,
};
use crate::specs::item::{ItemKind, ItemSpec};
use crate::specs::toml::{PackageMap, StillConfig, parse_still_toml};
use crate::trust::assert_config_trusted;

const MANAGED_MARKER: &str = ".still-managed";
const SOURCE_METADATA: &str = "source.toml";
const CONTENT_DIR: &str = "content";

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
    pub auto_added: Vec<InstallItemRequest>,
    pub missing_dependencies: Vec<InstallItemRequest>,
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
    let agents = normalize_agents(config.agents.clone().unwrap_or_default())?;
    let gitignore = managed_skills_gitignore(&agents.skills)?;
    let mut auto_added = Vec::new();
    let mut missing_dependencies = missing_skill_dependencies(&agents.skills, &config, false);
    let gitignore_path = if request.operation == AgentsOperation::Sync {
        assert_config_trusted(&resolved.path, content.as_bytes(), "agent sync").await?;
        auto_added = auto_dependency_items(&agents.skills, &config);
        if !auto_added.is_empty() {
            let updated = add_install_items(&content, &auto_added)?;
            tokio::fs::write(&resolved.path, updated)
                .await
                .with_context(|| format!("failed to write {}", resolved.path.display()))?;
            refresh_lockfile(&resolved.path).await?;
            let updated_config = parse_still_toml(
                &tokio::fs::read_to_string(&resolved.path)
                    .await
                    .with_context(|| format!("failed to read {}", resolved.path.display()))?,
            )?;
            missing_dependencies =
                missing_skill_dependencies(&agents.skills, &updated_config, false);
        }
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
        auto_added,
        missing_dependencies,
    })
}

fn missing_skill_dependencies(
    skills: &[NormalizedSkill],
    config: &StillConfig,
    include_auto: bool,
) -> Vec<InstallItemRequest> {
    let mut items = Vec::new();
    for skill in skills.iter().filter(|skill| include_auto || !skill.auto) {
        extend_missing(&mut items, ItemKind::Tool, &skill.tools, |spec| {
            !config.tools.contains_key(&spec.name)
        });
        extend_missing(&mut items, ItemKind::Package, &skill.packages, |spec| {
            !package_map_contains(&config.packages, &spec.name)
        });
        extend_missing(&mut items, ItemKind::App, &skill.apps, |spec| {
            !package_map_contains(&config.apps, &spec.name)
        });
    }
    items
}

fn auto_dependency_items(
    skills: &[NormalizedSkill],
    config: &StillConfig,
) -> Vec<InstallItemRequest> {
    missing_skill_dependencies(
        &skills
            .iter()
            .filter(|skill| skill.auto)
            .cloned()
            .collect::<Vec<_>>(),
        config,
        true,
    )
}

fn extend_missing(
    items: &mut Vec<InstallItemRequest>,
    kind: ItemKind,
    specs: &[ItemSpec],
    is_missing: impl Fn(&ItemSpec) -> bool,
) {
    for spec in specs {
        if is_missing(spec)
            && !items
                .iter()
                .any(|item| item.kind == kind && item.spec.name == spec.name)
        {
            items.push(InstallItemRequest {
                kind,
                spec: spec.clone(),
            });
        }
    }
}

fn package_map_contains(map: &PackageMap, name: &str) -> bool {
    map.latest.iter().any(|item| item == name) || map.entries.contains_key(name)
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
        materialize_skill_source(skill, &path).await?;
    }
    Ok(())
}

async fn materialize_skill_source(skill: &NormalizedSkill, path: &Path) -> Result<()> {
    match &skill.source {
        NormalizedSkillSource::Official { .. } => Ok(()),
        NormalizedSkillSource::GitHub { path: repo } => {
            let url = format!("https://github.com/{repo}.git");
            clone_skill_source(&url, path).await
        }
        NormalizedSkillSource::Url { url, .. } if url.starts_with("file://") => {
            let source = PathBuf::from(url.trim_start_matches("file://"));
            copy_skill_source(&source, &path.join(CONTENT_DIR)).await
        }
        NormalizedSkillSource::Url { url, .. } if is_git_source(url) => {
            clone_skill_source(url, path).await
        }
        NormalizedSkillSource::Url { url, .. } => {
            download_skill_file(url, &path.join(CONTENT_DIR)).await
        }
    }
}

fn is_git_source(url: &str) -> bool {
    url.ends_with(".git") || url.starts_with("git@")
}

async fn clone_skill_source(url: &str, path: &Path) -> Result<()> {
    let content_path = path.join(CONTENT_DIR);
    if tokio::fs::metadata(&content_path).await.is_ok() {
        tokio::fs::remove_dir_all(&content_path).await?;
    }
    let status = Command::new("git")
        .args(["clone", "--depth", "1", url])
        .arg(&content_path)
        .status()
        .with_context(|| format!("failed to run git clone for agent skill source {url}"))?;
    if !status.success() {
        return Err(EngineError::Conflict {
            message: format!("failed to clone agent skill source {url}"),
        }
        .into());
    }
    Ok(())
}

async fn download_skill_file(url: &str, destination: &Path) -> Result<()> {
    if tokio::fs::metadata(destination).await.is_ok() {
        tokio::fs::remove_dir_all(destination).await?;
    }
    tokio::fs::create_dir_all(destination).await?;
    let response = reqwest::get(url)
        .await
        .with_context(|| format!("failed to download agent skill source {url}"))?;
    if !response.status().is_success() {
        return Err(EngineError::Conflict {
            message: format!(
                "failed to download agent skill source {url}: HTTP {}",
                response.status()
            ),
        }
        .into());
    }
    let body = response.bytes().await?;
    tokio::fs::write(destination.join("SKILL.md"), body).await?;
    Ok(())
}

async fn copy_skill_source(source: &Path, destination: &Path) -> Result<()> {
    let metadata = tokio::fs::metadata(source)
        .await
        .with_context(|| format!("failed to read skill source {}", source.display()))?;
    if !metadata.is_dir() {
        return Err(EngineError::InvalidConfig {
            reason: format!("skill source {} is not a directory", source.display()),
        }
        .into());
    }
    if tokio::fs::metadata(destination).await.is_ok() {
        tokio::fs::remove_dir_all(destination).await?;
    }
    tokio::fs::create_dir_all(destination).await?;
    copy_dir_recursive(source, destination).await
}

async fn copy_dir_recursive(source: &Path, destination: &Path) -> Result<()> {
    let mut stack = vec![(source.to_path_buf(), destination.to_path_buf())];
    while let Some((from, to)) = stack.pop() {
        tokio::fs::create_dir_all(&to).await?;
        let mut entries = tokio::fs::read_dir(&from).await?;
        while let Some(entry) = entries.next_entry().await? {
            let entry_path = entry.path();
            let target_path = to.join(entry.file_name());
            let metadata = entry.metadata().await?;
            if metadata.is_dir() {
                stack.push((entry_path, target_path));
            } else if metadata.is_file() {
                tokio::fs::copy(&entry_path, &target_path).await?;
            }
        }
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

    use crate::trust::{config_fingerprint, trust_marker_path};

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
        let config_path = temp.path().join("still.toml");
        let config = r#"
            [agents]
            skills = ["rust-review"]
            "#;
        fs::write(&config_path, config).unwrap();
        write_trust_marker(&config_path, config.as_bytes());

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
        let config_path = temp.path().join("still.toml");
        let config = r#"
            [agents]
            skills = ["custom"]
            "#;
        fs::write(&config_path, config).unwrap();
        write_trust_marker(&config_path, config.as_bytes());
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

    #[tokio::test]
    async fn agents_sync_auto_adds_missing_inline_skill_dependencies() {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("still.toml");
        let config = r#"
            [tools]
            rust = { version = "stable", backend = "rustup" }

            [packages]
            latest = ["openssl"]

            [agents]

            [agents.skills]
            rust-review = { auto = true, tools = ["rust@stable@rustup", "cargo-nextest"], packages = ["openssl", "llvm"], apps = ["zed"] }
            "#;
        fs::write(&config_path, config).unwrap();
        write_trust_marker(&config_path, config.as_bytes());

        let result = run(AgentsRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            operation: AgentsOperation::Sync,
        })
        .await
        .unwrap();

        assert_eq!(result.auto_added.len(), 3);
        assert!(
            result
                .auto_added
                .iter()
                .any(|item| item.kind == ItemKind::Tool && item.spec.name == "cargo-nextest")
        );
        let updated = fs::read_to_string(temp.path().join("still.toml")).unwrap();
        let config = parse_still_toml(&updated).unwrap();
        assert!(config.tools.contains_key("cargo-nextest"));
        assert!(config.packages.latest.contains(&"llvm".to_string()));
        assert!(config.apps.latest.contains(&"zed".to_string()));

        let lockfile = fs::read_to_string(temp.path().join("still.lock.toml")).unwrap();
        assert!(lockfile.contains("name = \"cargo-nextest\""));
        assert!(lockfile.contains("name = \"llvm\""));
        assert!(lockfile.contains("name = \"zed\""));
    }

    #[tokio::test]
    async fn agents_check_reports_missing_non_auto_skill_dependencies() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("still.toml"),
            r#"
            [agents]

            [agents.skills]
            repo-auditor = { source = "repo-auditor", tools = ["cargo-audit"], packages = ["jq"], apps = ["zed"] }
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

        assert_eq!(result.auto_added, []);
        assert_eq!(result.missing_dependencies.len(), 3);
        assert!(
            result
                .missing_dependencies
                .iter()
                .any(|item| { item.kind == ItemKind::Tool && item.spec.name == "cargo-audit" })
        );
        let content = fs::read_to_string(temp.path().join("still.toml")).unwrap();
        assert!(!content.contains("cargo-audit ="));
    }

    #[tokio::test]
    async fn agents_sync_copies_file_url_skill_source() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source-skill");
        fs::create_dir_all(source.join("nested")).unwrap();
        fs::write(source.join("SKILL.md"), "# Local skill\n").unwrap();
        fs::write(source.join("nested/config.toml"), "ok = true\n").unwrap();
        let config_path = temp.path().join("still.toml");
        let config = format!(
            r#"
                [agents]

                [agents.skills]
                local-skill = "file://{}"
                "#,
            source.display()
        );
        fs::write(&config_path, &config).unwrap();
        write_trust_marker(&config_path, config.as_bytes());

        run(AgentsRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            operation: AgentsOperation::Sync,
        })
        .await
        .unwrap();

        let content = temp.path().join(".agents/skills/local-skill/content");
        assert_eq!(
            fs::read_to_string(content.join("SKILL.md")).unwrap(),
            "# Local skill\n"
        );
        assert_eq!(
            fs::read_to_string(content.join("nested/config.toml")).unwrap(),
            "ok = true\n"
        );
    }

    #[tokio::test]
    async fn agents_sync_requires_trust() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("still.toml"),
            r#"
            [agents]
            skills = ["rust-review"]
            "#,
        )
        .unwrap();

        let err = run(AgentsRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            operation: AgentsOperation::Sync,
        })
        .await
        .unwrap_err();

        assert!(err.to_string().contains("not trusted"));
        assert!(err.to_string().contains("agent sync"));
    }

    fn write_trust_marker(config_path: &Path, content: &[u8]) {
        let marker_path = trust_marker_path(config_path);
        fs::create_dir_all(marker_path.parent().unwrap()).unwrap();
        fs::write(
            marker_path,
            format!(
                "config = \"{}\"\nfingerprint = \"{}\"\n",
                config_path.display(),
                config_fingerprint(content)
            ),
        )
        .unwrap();
    }
}
