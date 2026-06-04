//! Engine action for inspecting and syncing agent config.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{EngineContext, Result};
use serde::{Deserialize, Serialize};

use crate::actions::install::{InstallItemRequest, InstallRequest};
use crate::actions::sync::refresh_active_lockfile;
use crate::config::edit::add_install_items;
use crate::config::{ConfigScope, ConfigSelection, resolve_config_path};
use crate::error::EngineError;
use crate::lockfile::lockfile_path;
use crate::specs::agents::{
    NormalizedAgents, NormalizedSkill, NormalizedSkillSource, managed_skill_dir_name,
    managed_skills_gitignore, normalize_agents, parse_skill_dependency_specs,
};
use crate::specs::item::{ItemKind, ItemSpec};
use crate::specs::toml::{PackageEntry, PackageMap, StillConfig, ToolEntry, parse_still_toml};
use crate::trust::assert_config_trusted;

const MANAGED_MARKER: &str = ".still-managed";
const SOURCE_METADATA: &str = "source.toml";
const CONTENT_DIR: &str = "content";
const TARGET_MANIFEST_MARKER: &str = "managed_by = \"still\"";

/// Agent operation requested by a caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentsOperation {
    List,
    Check,
    Sync,
    SyncAcceptAutoDependencies,
}

/// Request to inspect or sync configured agents.
#[derive(Debug, Clone)]
pub struct AgentsRequest {
    pub start_dir: PathBuf,
    pub home_dir: PathBuf,
    pub global: bool,
    pub operation: AgentsOperation,
}

/// Normalized agent config and generated managed-skill metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentsResult {
    pub path: PathBuf,
    pub agents: NormalizedAgents,
    pub gitignore: String,
    pub gitignore_path: Option<PathBuf>,
    pub target_manifests: Vec<PathBuf>,
    pub pending_auto_dependencies: Vec<InstallItemRequest>,
    pub auto_added: Vec<InstallItemRequest>,
    pub missing_dependencies: Vec<InstallItemRequest>,
    pub review_required: bool,
}

/// Installs auto-added skill dependencies after they become normal desired state.
pub trait AgentDependencyInstaller {
    /// Reconciles the supplied tool/package/app dependencies.
    /// # Errors
    /// Fails when dependency installation fails through the selected backend.
    async fn install(&mut self, items: Vec<InstallItemRequest>) -> Result<()>;
}

/// Production installer for agent-linked dependencies.
#[derive(Debug, Default)]
pub struct RealAgentDependencyInstaller;

impl AgentDependencyInstaller for RealAgentDependencyInstaller {
    async fn install(&mut self, items: Vec<InstallItemRequest>) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }
        crate::actions::install::run(InstallRequest { items }).await?;
        Ok(())
    }
}

/// Reads selected config, normalizes agent skills, and optionally writes ignore metadata.
/// # Errors
/// Fails when config cannot be read, parsed, normalized, or synced.
pub async fn run(request: AgentsRequest) -> Result<AgentsResult> {
    let mut installer = RealAgentDependencyInstaller;
    run_with_installer(request, &mut installer).await
}

/// Runs agent inspection or sync with an injected dependency installer.
/// # Errors
/// Fails when config cannot be read, parsed, normalized, synced, or dependency
/// installation fails.
pub async fn run_with_installer(
    request: AgentsRequest,
    installer: &mut impl AgentDependencyInstaller,
) -> Result<AgentsResult> {
    let resolved = resolve_config_path(
        &request.start_dir,
        &request.home_dir,
        ConfigSelection {
            scope: if request.global {
                ConfigScope::Global
            } else {
                ConfigScope::Project
            },
            for_write: false,
        },
    )?;
    let content = tokio::fs::read_to_string(&resolved.path)
        .await
        .with_context(|| format!("failed to read {}", resolved.path.display()))?;
    let config = parse_still_toml(&content)?;
    let mut agents = normalize_agents(config.agents.clone().unwrap_or_default())?;
    let project_root = resolved
        .path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    if request.operation != AgentsOperation::List {
        validate_instructions_file(project_root, agents.instructions.as_deref()).await?;
    }
    let gitignore = managed_skills_gitignore(&agents.skills)?;
    let mut dependency_skills = agents.skills.clone();
    let mut pending_auto_dependencies = auto_dependency_items(&dependency_skills, &config);
    let mut auto_added = Vec::new();
    let mut missing_dependencies = missing_skill_dependencies(&dependency_skills, &config, false);
    let mut target_manifests = Vec::new();
    if request.operation == AgentsOperation::Check {
        if resolved.scope == ConfigScope::Project && has_url_skill_source(&agents.skills) {
            assert_config_trusted(
                &resolved.path,
                content.as_bytes(),
                "external agent skill inspection",
            )
            .await?;
        }
        dependency_skills = skills_with_source_manifest_dependencies(&agents.skills).await?;
        agents.skills = dependency_skills.clone();
        pending_auto_dependencies = auto_dependency_items(&dependency_skills, &config);
        missing_dependencies = missing_skill_dependencies(&dependency_skills, &config, false);
    }
    let mut review_required = false;
    let gitignore_path = if matches!(
        request.operation,
        AgentsOperation::Sync | AgentsOperation::SyncAcceptAutoDependencies
    ) {
        if resolved.scope == ConfigScope::Project {
            assert_config_trusted(&resolved.path, content.as_bytes(), "agent sync").await?;
        }
        if !pending_auto_dependencies.is_empty() && request.operation == AgentsOperation::Sync {
            review_required = true;
            return Ok(AgentsResult {
                path: resolved.path,
                agents,
                gitignore,
                gitignore_path: None,
                target_manifests,
                pending_auto_dependencies,
                auto_added,
                missing_dependencies,
                review_required,
            });
        }
        let skills_dir = project_root.join(".agents").join("skills");
        materialize_skills(&skills_dir, &agents.skills).await?;
        dependency_skills = skills_with_manifest_dependencies(&skills_dir, &agents.skills).await?;
        agents.skills = dependency_skills.clone();
        pending_auto_dependencies = auto_dependency_items(&dependency_skills, &config);
        missing_dependencies = missing_skill_dependencies(&dependency_skills, &config, false);
        auto_added = pending_auto_dependencies.clone();
        if !auto_added.is_empty() && request.operation == AgentsOperation::Sync {
            review_required = true;
            auto_added.clear();
            return Ok(AgentsResult {
                path: resolved.path,
                agents,
                gitignore,
                gitignore_path: None,
                target_manifests,
                pending_auto_dependencies,
                auto_added,
                missing_dependencies,
                review_required,
            });
        }
        if !auto_added.is_empty() {
            let lockfile_path = lockfile_path(&resolved.path);
            let original_lockfile = read_optional_file(&lockfile_path).await?;
            let updated = add_install_items(&content, &auto_added)?;
            tokio::fs::write(&resolved.path, updated)
                .await
                .with_context(|| format!("failed to write {}", resolved.path.display()))?;
            if let Err(err) = refresh_active_lockfile(&resolved.path, &request.home_dir).await {
                restore_auto_dependency_state(
                    &resolved.path,
                    &content,
                    &lockfile_path,
                    original_lockfile,
                )
                .await?;
                return Err(err);
            }
            let updated_config = match parse_still_toml(
                &tokio::fs::read_to_string(&resolved.path)
                    .await
                    .with_context(|| format!("failed to read {}", resolved.path.display()))?,
            ) {
                Ok(config) => config,
                Err(err) => {
                    restore_auto_dependency_state(
                        &resolved.path,
                        &content,
                        &lockfile_path,
                        original_lockfile,
                    )
                    .await?;
                    return Err(err);
                }
            };
            missing_dependencies =
                missing_skill_dependencies(&dependency_skills, &updated_config, false);
            if let Err(err) = installer.install(auto_added.clone()).await {
                restore_auto_dependency_state(
                    &resolved.path,
                    &content,
                    &lockfile_path,
                    original_lockfile,
                )
                .await?;
                return Err(err.context("failed to install auto-added agent dependencies"));
            }
        }
        prune_stale_managed_skills(&skills_dir, &agents.skills).await?;
        target_manifests =
            materialize_target_manifests(&project_root.join(".agents").join("targets"), &agents)
                .await?;
        let path = skills_dir.join(".gitignore");
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&path, &gitignore).await?;
        refresh_active_lockfile(&resolved.path, &request.home_dir).await?;
        Some(path)
    } else {
        None
    };

    Ok(AgentsResult {
        path: resolved.path,
        agents,
        gitignore,
        gitignore_path,
        target_manifests,
        pending_auto_dependencies,
        auto_added,
        missing_dependencies,
        review_required,
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
            !tool_config_satisfies(config.tools.get(&spec.name), spec)
        });
        extend_missing(&mut items, ItemKind::Package, &skill.packages, |spec| {
            !package_map_satisfies(&config.packages, spec)
        });
        extend_missing(&mut items, ItemKind::App, &skill.apps, |spec| {
            !package_map_satisfies(&config.apps, spec)
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

fn has_url_skill_source(skills: &[NormalizedSkill]) -> bool {
    skills
        .iter()
        .any(|skill| matches!(skill.source, NormalizedSkillSource::Url { .. }))
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
                .any(|item| item.kind == Some(kind) && item.spec.name == spec.name)
        {
            items.push(InstallItemRequest {
                kind: Some(kind),
                spec: spec.clone(),
                tool: Default::default(),
            });
        }
    }
}

fn tool_config_satisfies(entry: Option<&ToolEntry>, spec: &ItemSpec) -> bool {
    let Some(entry) = entry else {
        return false;
    };
    match entry {
        ToolEntry::Version(version) => {
            version == spec.version.as_str() && required_backend_matches(spec, None)
        }
        ToolEntry::Expanded(tool) => {
            let version = if tool.version.is_empty() {
                "latest"
            } else {
                tool.version.as_str()
            };
            version == spec.version.as_str()
                && required_backend_matches(spec, tool.backend.as_deref())
        }
    }
}

fn package_map_satisfies(map: &PackageMap, spec: &ItemSpec) -> bool {
    if spec.version.is_latest()
        && map.latest.iter().any(|item| item == &spec.name)
        && required_backend_matches(spec, None)
    {
        return true;
    }

    map.entries.get(&spec.name).is_some_and(|entry| {
        let PackageEntry::Expanded(package) = entry;
        let version = package.version.as_deref().unwrap_or("latest");
        version == spec.version.as_str()
            && required_backend_matches(spec, package.backend.as_deref())
    })
}

fn required_backend_matches(spec: &ItemSpec, configured: Option<&str>) -> bool {
    spec.backend
        .as_ref()
        .is_none_or(|backend| configured == Some(backend.as_str()))
}

async fn skills_with_manifest_dependencies(
    root: &Path,
    skills: &[NormalizedSkill],
) -> Result<Vec<NormalizedSkill>> {
    let mut enriched = Vec::new();
    for skill in skills {
        let dir_name = managed_skill_dir_name(&skill.name)?;
        let manifest = read_skill_manifest(&root.join(dir_name).join(CONTENT_DIR)).await?;
        let mut skill = skill.clone();
        merge_specs(&mut skill.tools, manifest.tools);
        merge_specs(&mut skill.packages, manifest.packages);
        merge_specs(&mut skill.apps, manifest.apps);
        enriched.push(skill);
    }
    Ok(enriched)
}

async fn skills_with_source_manifest_dependencies(
    skills: &[NormalizedSkill],
) -> Result<Vec<NormalizedSkill>> {
    let mut enriched = Vec::new();
    for skill in skills {
        let manifest = match &skill.source {
            NormalizedSkillSource::Url { url, .. } if url.starts_with("file://") => {
                let source = PathBuf::from(url.trim_start_matches("file://"));
                read_skill_manifest(&source).await?
            }
            _ => SkillManifestDependencies::default(),
        };
        let mut skill = skill.clone();
        merge_specs(&mut skill.tools, manifest.tools);
        merge_specs(&mut skill.packages, manifest.packages);
        merge_specs(&mut skill.apps, manifest.apps);
        enriched.push(skill);
    }
    Ok(enriched)
}

async fn read_skill_manifest(content_dir: &Path) -> Result<SkillManifestDependencies> {
    let path = content_dir.join("still.skill.toml");
    let content = match tokio::fs::read_to_string(&path).await {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(SkillManifestDependencies::default());
        }
        Err(err) => return Err(err.into()),
    };
    let manifest: SkillManifest =
        toml::from_str(&content).with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(manifest.dependencies)
}

fn merge_specs(target: &mut Vec<ItemSpec>, incoming: Vec<ItemSpec>) {
    for spec in incoming {
        if !target.iter().any(|existing| existing.name == spec.name) {
            target.push(spec);
        }
    }
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

async fn prune_stale_managed_skills(root: &Path, skills: &[NormalizedSkill]) -> Result<()> {
    let mut desired = BTreeSet::new();
    for skill in skills {
        desired.insert(managed_skill_dir_name(&skill.name)?);
    }
    let mut entries = tokio::fs::read_dir(root).await?;
    while let Some(entry) = entries.next_entry().await? {
        let metadata = entry.metadata().await?;
        if !metadata.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if desired.contains(&name) {
            continue;
        }
        let path = entry.path();
        if tokio::fs::metadata(path.join(MANAGED_MARKER)).await.is_ok() {
            tokio::fs::remove_dir_all(path).await?;
        }
    }
    Ok(())
}

async fn materialize_target_manifests(
    root: &Path,
    agents: &NormalizedAgents,
) -> Result<Vec<PathBuf>> {
    if agents.targets.is_empty() {
        prune_stale_target_manifests(root, &[]).await?;
        return Ok(Vec::new());
    }

    tokio::fs::create_dir_all(root).await?;
    let mut written = Vec::new();
    for target in &agents.targets {
        let path = root.join(format!("{target}.toml"));
        refuse_unmanaged_target_manifest(&path).await?;
        tokio::fs::write(&path, target_manifest(target, agents)?).await?;
        written.push(path);
    }
    prune_stale_target_manifests(root, &agents.targets).await?;
    Ok(written)
}

async fn refuse_unmanaged_target_manifest(path: &Path) -> Result<()> {
    match tokio::fs::read_to_string(path).await {
        Ok(content) if content.contains(TARGET_MANIFEST_MARKER) => Ok(()),
        Ok(_) => Err(EngineError::Conflict {
            message: format!(
                "refusing to overwrite unmanaged agent target {}",
                path.display()
            ),
        }
        .into()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}

async fn prune_stale_target_manifests(root: &Path, targets: &[String]) -> Result<()> {
    let Ok(mut entries) = tokio::fs::read_dir(root).await else {
        return Ok(());
    };
    let desired = targets
        .iter()
        .map(|target| format!("{target}.toml"))
        .collect::<BTreeSet<_>>();
    while let Some(entry) = entries.next_entry().await? {
        let metadata = entry.metadata().await?;
        if !metadata.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if desired.contains(&name) || !name.ends_with(".toml") {
            continue;
        }
        let path = entry.path();
        let content = tokio::fs::read_to_string(&path).await?;
        if content.contains(TARGET_MANIFEST_MARKER) {
            tokio::fs::remove_file(path).await?;
        }
    }
    Ok(())
}

fn target_manifest(target: &str, agents: &NormalizedAgents) -> Result<String> {
    let skills = agents
        .skills
        .iter()
        .map(|skill| managed_skill_dir_name(&skill.name))
        .collect::<crate::error::EngineResult<Vec<_>>>()?
        .into_iter()
        .map(|name| format!(".agents/skills/{name}"))
        .collect();
    toml::to_string_pretty(&AgentTargetManifest {
        managed_by: "still",
        target,
        instructions: agents.instructions.as_deref(),
        skills,
    })
    .map_err(Into::into)
}

async fn validate_instructions_file(project_root: &Path, instructions: Option<&str>) -> Result<()> {
    let Some(instructions) = instructions else {
        return Ok(());
    };
    let path = project_root.join(instructions);
    let metadata = tokio::fs::metadata(&path).await.with_context(|| {
        format!(
            "failed to read configured agent instructions {}",
            path.display()
        )
    })?;
    if !metadata.is_file() {
        return Err(EngineError::InvalidConfig {
            reason: format!(
                "configured agent instructions {} is not a file",
                path.display()
            ),
        }
        .into());
    }
    Ok(())
}

async fn materialize_skill_source(skill: &NormalizedSkill, path: &Path) -> Result<()> {
    match &skill.source {
        NormalizedSkillSource::Official { name, .. } => {
            materialize_official_skill(name, &path.join(CONTENT_DIR)).await
        }
        NormalizedSkillSource::GitHub {
            path: repo,
            version,
        } => {
            let url = format!("https://github.com/{repo}.git");
            clone_skill_source(&url, path, version.as_deref()).await
        }
        NormalizedSkillSource::Url { url, .. } if url.starts_with("file://") => {
            let source = PathBuf::from(url.trim_start_matches("file://"));
            copy_skill_source(&source, &path.join(CONTENT_DIR)).await
        }
        NormalizedSkillSource::Url { url, version } if is_git_source(url) => {
            clone_skill_source(url, path, version.as_deref()).await
        }
        NormalizedSkillSource::Url { url, .. } => {
            download_skill_file(url, &path.join(CONTENT_DIR)).await
        }
    }
}

async fn materialize_official_skill(name: &str, destination: &Path) -> Result<()> {
    let Some(content) = official_skill_content(name) else {
        return Err(EngineError::Conflict {
            message: format!("official agent skill \"{name}\" is not available"),
        }
        .into());
    };
    if tokio::fs::metadata(destination).await.is_ok() {
        tokio::fs::remove_dir_all(destination).await?;
    }
    tokio::fs::create_dir_all(destination).await?;
    tokio::fs::write(destination.join("SKILL.md"), content).await?;
    Ok(())
}

fn official_skill_content(name: &str) -> Option<&'static str> {
    match name {
        "rust-review" => Some(
            r#"# rust-review

Review Rust changes for correctness, maintainability, and test coverage.

## Focus

- Check ownership, lifetimes, error handling, and async boundaries.
- Look for platform-specific behavior hidden behind generic code.
- Prefer small, actionable findings with file and line references.
- Call out missing tests when behavior changes.
"#,
        ),
        "repo-auditor" => Some(
            r#"# repo-auditor

Audit a repository against its documented architecture and product spec.

## Focus

- Compare implementation, docs, examples, and tests for drift.
- Identify unowned side effects, unsafe filesystem behavior, and stale generated artifacts.
- Report concrete gaps before summaries.
- Recommend the narrowest next verification step.
"#,
        ),
        _ => None,
    }
}

fn is_git_source(url: &str) -> bool {
    url.ends_with(".git") || url.starts_with("git@")
}

async fn clone_skill_source(url: &str, path: &Path, version: Option<&str>) -> Result<()> {
    let content_path = path.join(CONTENT_DIR);
    if tokio::fs::metadata(&content_path).await.is_ok() {
        tokio::fs::remove_dir_all(&content_path).await?;
    }
    let status = Command::new("git")
        .args(git_clone_args(url, &content_path, version))
        .status()
        .with_context(|| format!("failed to run git clone for agent skill source {url}"))?;
    if !status.success() {
        return Err(EngineError::Conflict {
            message: format!("failed to clone agent skill source {url}"),
        }
        .into());
    }
    if let Some(version) = version {
        let status = Command::new("git")
            .args(git_checkout_args(&content_path, version))
            .status()
            .with_context(|| {
                format!("failed to run git checkout for agent skill source {url}@{version}")
            })?;
        if !status.success() {
            return Err(EngineError::Conflict {
                message: format!("failed to checkout agent skill source {url}@{version}"),
            }
            .into());
        }
    }
    Ok(())
}

fn git_clone_args(url: &str, destination: &Path, version: Option<&str>) -> Vec<OsString> {
    let mut args = vec![OsString::from("clone")];
    if version.is_none() {
        args.extend([OsString::from("--depth"), OsString::from("1")]);
    }
    args.extend([OsString::from(url), destination.as_os_str().to_owned()]);
    args
}

fn git_checkout_args(destination: &Path, version: &str) -> Vec<OsString> {
    vec![
        OsString::from("-C"),
        destination.as_os_str().to_owned(),
        OsString::from("checkout"),
        OsString::from(version),
    ]
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
    toml::to_string_pretty(&ManagedSkillMarker {
        managed_by: "still",
        name: &skill.name,
    })
    .map_err(Into::into)
}

fn source_metadata(skill: &NormalizedSkill) -> Result<String> {
    toml::to_string_pretty(&ManagedSkillMetadata::from(skill)).map_err(Into::into)
}

#[derive(Debug, Serialize)]
struct ManagedSkillMarker<'a> {
    managed_by: &'a str,
    name: &'a str,
}

#[derive(Debug, Serialize)]
struct AgentTargetManifest<'a> {
    managed_by: &'a str,
    target: &'a str,
    instructions: Option<&'a str>,
    skills: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct SkillManifest {
    dependencies: SkillManifestDependencies,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct SkillManifestDependencies {
    #[serde(deserialize_with = "deserialize_dependency_specs")]
    tools: Vec<ItemSpec>,
    #[serde(deserialize_with = "deserialize_dependency_specs")]
    packages: Vec<ItemSpec>,
    #[serde(deserialize_with = "deserialize_dependency_specs")]
    apps: Vec<ItemSpec>,
}

fn deserialize_dependency_specs<'de, D>(deserializer: D) -> Result<Vec<ItemSpec>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let values = Vec::<String>::deserialize(deserializer)?;
    parse_skill_dependency_specs("manifest", values).map_err(serde::de::Error::custom)
}

async fn read_optional_file(path: &Path) -> Result<Option<String>> {
    match tokio::fs::read_to_string(path).await {
        Ok(content) => Ok(Some(content)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).with_context(|| format!("failed to read {}", path.display())),
    }
}

async fn restore_auto_dependency_state(
    config_path: &Path,
    original_config: &str,
    lockfile_path: &Path,
    original_lockfile: Option<String>,
) -> Result<()> {
    tokio::fs::write(config_path, original_config)
        .await
        .with_context(|| format!("failed to restore {}", config_path.display()))?;
    match original_lockfile {
        Some(content) => {
            tokio::fs::write(lockfile_path, content)
                .await
                .with_context(|| format!("failed to restore {}", lockfile_path.display()))?;
        }
        None => match tokio::fs::remove_file(lockfile_path).await {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("failed to remove {}", lockfile_path.display()));
            }
        },
    }
    Ok(())
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
            NormalizedSkillSource::Official { name, version } => {
                ("official", name.clone(), version.clone())
            }
            NormalizedSkillSource::GitHub { path, version } => {
                ("github", path.clone(), version.clone())
            }
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

    #[derive(Default)]
    struct FakeInstaller {
        installed: Vec<InstallItemRequest>,
    }

    impl AgentDependencyInstaller for FakeInstaller {
        async fn install(&mut self, items: Vec<InstallItemRequest>) -> Result<()> {
            self.installed.extend(items);
            Ok(())
        }
    }

    #[derive(Default)]
    struct FailingInstaller {
        attempted: Vec<InstallItemRequest>,
    }

    impl AgentDependencyInstaller for FailingInstaller {
        async fn install(&mut self, items: Vec<InstallItemRequest>) -> Result<()> {
            self.attempted.extend(items);
            Err(EngineError::message("installer failed"))
        }
    }

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
        fs::write(temp.path().join("AGENTS.md"), "# Project instructions\n").unwrap();

        let result = run(AgentsRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
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
        let source = write_local_skill_source(temp.path(), "rust-review");
        let config = format!(
            r#"
            [agents]

            [agents.skills]
            rust-review = "{}"
            "#,
            source
        );
        fs::write(&config_path, &config).unwrap();
        write_trust_marker(&config_path, config.as_bytes());

        let result = run(AgentsRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
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
        assert!(metadata.contains("source_kind = \"url\""));
        assert!(metadata.contains(&format!("source = \"{}\"", source)));
    }

    #[tokio::test]
    async fn agents_sync_materializes_official_skill() {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("still.toml");
        let config = r#"
            [agents.skills]
            rust-review = { source = "rust-review", version = "v1.2.3" }
            "#;
        fs::write(&config_path, config).unwrap();
        write_trust_marker(&config_path, config.as_bytes());

        run(AgentsRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
            operation: AgentsOperation::Sync,
        })
        .await
        .unwrap();

        let skill_dir = temp.path().join(".agents/skills/rust-review");
        assert!(skill_dir.join(".still-managed").is_file());
        let content = fs::read_to_string(skill_dir.join("content/SKILL.md")).unwrap();
        assert!(content.contains("# rust-review"));
        let metadata = fs::read_to_string(skill_dir.join("source.toml")).unwrap();
        assert!(metadata.contains("source_kind = \"official\""));
        assert!(metadata.contains("source = \"rust-review\""));
        assert!(metadata.contains("version = \"v1.2.3\""));
    }

    #[tokio::test]
    async fn agents_sync_rejects_unknown_official_skill() {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("still.toml");
        let config = r#"
            [agents.skills]
            unknown-skill = { source = "unknown-skill" }
            "#;
        fs::write(&config_path, config).unwrap();
        write_trust_marker(&config_path, config.as_bytes());

        let err = run(AgentsRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
            operation: AgentsOperation::Sync,
        })
        .await
        .unwrap_err();

        assert!(err.to_string().contains("official agent skill"));
    }

    #[test]
    fn unpinned_git_skill_clone_uses_shallow_default_branch() {
        let destination = Path::new(".agents/skills/repo/content");

        let args = git_clone_args("https://github.com/owner/repo.git", destination, None);

        assert_eq!(
            os_args(&args),
            [
                "clone",
                "--depth",
                "1",
                "https://github.com/owner/repo.git",
                ".agents/skills/repo/content"
            ]
        );
    }

    #[test]
    fn pinned_git_skill_clone_allows_commit_checkout() {
        let destination = Path::new(".agents/skills/repo/content");

        let clone_args = git_clone_args(
            "https://github.com/owner/repo.git",
            destination,
            Some("abc123"),
        );
        let checkout_args = git_checkout_args(destination, "abc123");

        assert_eq!(
            os_args(&clone_args),
            [
                "clone",
                "https://github.com/owner/repo.git",
                ".agents/skills/repo/content"
            ]
        );
        assert_eq!(
            os_args(&checkout_args),
            ["-C", ".agents/skills/repo/content", "checkout", "abc123"]
        );
    }

    #[tokio::test]
    async fn agents_sync_writes_target_manifests() {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("still.toml");
        let rust_review = write_local_skill_source(temp.path(), "rust-review");
        let repo_auditor = write_local_skill_source(temp.path(), "repo-auditor");
        let config = format!(
            r#"
            [agents]
            targets = ["claude", "codex"]
            instructions = "AGENTS.md"

            [agents.skills]
            rust-review = "{}"
            repo-auditor = "{}"
            "#,
            rust_review, repo_auditor
        );
        fs::write(&config_path, &config).unwrap();
        fs::write(temp.path().join("AGENTS.md"), "# Project instructions\n").unwrap();
        write_trust_marker(&config_path, config.as_bytes());

        let result = run(AgentsRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
            operation: AgentsOperation::Sync,
        })
        .await
        .unwrap();

        let claude = temp.path().join(".agents/targets/claude.toml");
        let codex = temp.path().join(".agents/targets/codex.toml");
        assert_eq!(result.target_manifests, [claude.clone(), codex.clone()]);
        let content = fs::read_to_string(claude).unwrap();
        assert!(content.contains("managed_by = \"still\""));
        assert!(content.contains("target = \"claude\""));
        assert!(content.contains("instructions = \"AGENTS.md\""));
        assert!(content.contains("\".agents/skills/rust-review\""));
        assert!(content.contains("\".agents/skills/repo-auditor\""));
        assert!(
            fs::read_to_string(codex)
                .unwrap()
                .contains("target = \"codex\"")
        );
    }

    #[tokio::test]
    async fn agents_sync_writes_agent_skill_lockfile_entry() {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("still.toml");
        let rust_review = write_local_skill_source(temp.path(), "rust-review");
        let config = format!(
            r#"
            [agents.skills]
            rust-review = {{ url = "{}", version = "v1.2.3" }}
            "#,
            rust_review
        );
        fs::write(&config_path, &config).unwrap();
        write_trust_marker(&config_path, config.as_bytes());

        run(AgentsRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
            operation: AgentsOperation::Sync,
        })
        .await
        .unwrap();

        let lockfile = fs::read_to_string(temp.path().join("still.lock.toml")).unwrap();
        assert!(lockfile.contains("kind = \"agent-skill\""));
        assert!(lockfile.contains("name = \"rust-review\""));
        assert!(lockfile.contains("version = \"v1.2.3\""));
    }

    #[tokio::test]
    async fn agents_check_rejects_missing_instructions_file() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("still.toml"),
            r#"
            [agents]
            instructions = "AGENTS.md"
            "#,
        )
        .unwrap();

        let err = run(AgentsRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
            operation: AgentsOperation::Check,
        })
        .await
        .unwrap_err();

        assert!(
            err.to_string()
                .contains("failed to read configured agent instructions")
        );
    }

    #[tokio::test]
    async fn agents_list_allows_missing_instructions_file() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("still.toml"),
            r#"
            [agents]
            instructions = "AGENTS.md"
            skills = ["rust-review"]
            "#,
        )
        .unwrap();

        let result = run(AgentsRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
            operation: AgentsOperation::List,
        })
        .await
        .unwrap();

        assert_eq!(result.agents.instructions.as_deref(), Some("AGENTS.md"));
        assert_eq!(result.agents.skills[0].name, "rust-review");
    }

    #[tokio::test]
    async fn agents_global_reads_global_config_when_project_exists() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("repo");
        let global = temp.path().join(".config/still/config.toml");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(global.parent().unwrap()).unwrap();
        fs::write(
            project.join("still.toml"),
            r#"
            [agents]
            targets = ["codex"]
            skills = ["project-skill"]
            "#,
        )
        .unwrap();
        fs::write(
            &global,
            r#"
            [agents]
            targets = ["claude"]
            skills = ["global-skill"]
            "#,
        )
        .unwrap();

        let result = run(AgentsRequest {
            start_dir: project,
            home_dir: temp.path().to_path_buf(),
            global: true,
            operation: AgentsOperation::List,
        })
        .await
        .unwrap();

        assert_eq!(result.path, global);
        assert_eq!(result.agents.targets, ["claude"]);
        assert_eq!(result.agents.skills[0].name, "global-skill");
    }

    #[tokio::test]
    async fn agents_global_sync_uses_global_config_without_project_trust() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("repo");
        let global = temp.path().join(".config/still/config.toml");
        let global_skill = write_local_skill_source(temp.path(), "global-skill");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(global.parent().unwrap()).unwrap();
        fs::write(
            project.join("still.toml"),
            r#"
            [agents]
            skills = ["project-skill"]
            "#,
        )
        .unwrap();
        fs::write(
            &global,
            format!(
                r#"
            [agents]

            [agents.skills]
            global-skill = "{}"
            "#,
                global_skill
            ),
        )
        .unwrap();

        let result = run(AgentsRequest {
            start_dir: project,
            home_dir: temp.path().to_path_buf(),
            global: true,
            operation: AgentsOperation::Sync,
        })
        .await
        .unwrap();

        let skills_dir = global.parent().unwrap().join(".agents/skills");
        assert_eq!(result.path, global);
        assert_eq!(result.gitignore_path, Some(skills_dir.join(".gitignore")));
        assert!(skills_dir.join("global-skill/.still-managed").is_file());
        assert!(!skills_dir.join("project-skill").exists());
    }

    #[tokio::test]
    async fn agents_sync_prunes_stale_managed_target_manifests_only() {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("still.toml");
        let config = r#"
            [agents]
            targets = ["codex"]
            "#;
        fs::write(&config_path, config).unwrap();
        write_trust_marker(&config_path, config.as_bytes());
        let targets = temp.path().join(".agents/targets");
        fs::create_dir_all(&targets).unwrap();
        fs::write(
            targets.join("claude.toml"),
            "managed_by = \"still\"\ntarget = \"claude\"\nskills = []\n",
        )
        .unwrap();
        fs::write(targets.join("custom.toml"), "target = \"custom\"\n").unwrap();

        run(AgentsRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
            operation: AgentsOperation::Sync,
        })
        .await
        .unwrap();

        assert!(!targets.join("claude.toml").exists());
        assert!(targets.join("codex.toml").is_file());
        assert!(targets.join("custom.toml").is_file());
    }

    #[tokio::test]
    async fn agents_sync_refuses_unmanaged_target_manifest() {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("still.toml");
        let config = r#"
            [agents]
            targets = ["claude"]
            "#;
        fs::write(&config_path, config).unwrap();
        write_trust_marker(&config_path, config.as_bytes());
        let targets = temp.path().join(".agents/targets");
        fs::create_dir_all(&targets).unwrap();
        fs::write(targets.join("claude.toml"), "target = \"claude\"\n").unwrap();

        let err = run(AgentsRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
            operation: AgentsOperation::Sync,
        })
        .await
        .unwrap_err();

        assert!(err.to_string().contains("refusing to overwrite unmanaged"));
    }

    #[tokio::test]
    async fn agents_sync_refuses_unmarked_custom_skill_directory() {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("still.toml");
        let source = write_local_skill_source(temp.path(), "custom");
        let config = format!(
            r#"
            [agents]

            [agents.skills]
            custom = "{}"
            "#,
            source
        );
        fs::write(&config_path, &config).unwrap();
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
            global: false,
            operation: AgentsOperation::Sync,
        })
        .await
        .unwrap_err();

        assert!(err.to_string().contains("refusing to overwrite"));
        assert!(temp.path().join(".agents/skills/custom/SKILL.md").is_file());
    }

    #[tokio::test]
    async fn agents_sync_prunes_only_stale_managed_skill_directories() {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("still.toml");
        let source = write_local_skill_source(temp.path(), "rust-review");
        let config = format!(
            r#"
            [agents]

            [agents.skills]
            rust-review = "{}"
            "#,
            source
        );
        fs::write(&config_path, &config).unwrap();
        write_trust_marker(&config_path, config.as_bytes());

        let stale = temp.path().join(".agents/skills/old-managed");
        fs::create_dir_all(&stale).unwrap();
        fs::write(stale.join(".still-managed"), "managed\n").unwrap();
        fs::write(stale.join("SKILL.md"), "# Old\n").unwrap();
        let custom = temp.path().join(".agents/skills/custom");
        fs::create_dir_all(&custom).unwrap();
        fs::write(custom.join("SKILL.md"), "# Custom\n").unwrap();

        run(AgentsRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
            operation: AgentsOperation::Sync,
        })
        .await
        .unwrap();

        assert!(!stale.exists());
        assert!(custom.join("SKILL.md").is_file());
        assert!(temp.path().join(".agents/skills/rust-review").is_dir());
        assert_eq!(
            fs::read_to_string(temp.path().join(".agents/skills/.gitignore")).unwrap(),
            "# still-managed skills\n/rust-review/\n"
        );
    }

    #[tokio::test]
    async fn agents_sync_auto_adds_missing_inline_skill_dependencies() {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("still.toml");
        let source = write_local_skill_source(temp.path(), "rust-review");
        let config = format!(
            r#"
            [tools]
            rust = {{ version = "stable", backend = "rustup" }}

            [packages]
            latest = ["openssl"]

            [agents]

            [agents.skills]
            rust-review = {{ url = "{}", auto = true, tools = ["rust@stable@rustup", "cargo-nextest"], packages = ["openssl", "llvm"], apps = ["zed"] }}
        "#,
            source
        );
        fs::write(&config_path, &config).unwrap();
        write_trust_marker(&config_path, config.as_bytes());
        let mut installer = FakeInstaller::default();

        let result = run_with_installer(
            AgentsRequest {
                start_dir: temp.path().to_path_buf(),
                home_dir: temp.path().to_path_buf(),
                global: false,
                operation: AgentsOperation::SyncAcceptAutoDependencies,
            },
            &mut installer,
        )
        .await
        .unwrap();

        assert_eq!(result.auto_added.len(), 3);
        assert_eq!(installer.installed, result.auto_added);
        assert!(
            result
                .auto_added
                .iter()
                .any(|item| item.kind == Some(ItemKind::Tool) && item.spec.name == "cargo-nextest")
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
    async fn agents_sync_reports_auto_dependencies_without_writing_until_accepted() {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("still.toml");
        let config = r#"
            [agents]

            [agents.skills]
            rust-review = { auto = true, tools = ["cargo-nextest"], packages = ["llvm"], apps = ["zed"] }
        "#;
        fs::write(&config_path, config).unwrap();
        write_trust_marker(&config_path, config.as_bytes());
        let mut installer = FakeInstaller::default();

        let result = run_with_installer(
            AgentsRequest {
                start_dir: temp.path().to_path_buf(),
                home_dir: temp.path().to_path_buf(),
                global: false,
                operation: AgentsOperation::Sync,
            },
            &mut installer,
        )
        .await
        .unwrap();

        assert!(result.review_required);
        assert_eq!(result.pending_auto_dependencies.len(), 3);
        assert_eq!(result.auto_added, []);
        assert_eq!(installer.installed, []);
        assert_eq!(fs::read_to_string(&config_path).unwrap(), config);
        assert!(!temp.path().join("still.lock.toml").exists());
    }

    #[tokio::test]
    async fn agents_sync_restores_config_and_lockfile_when_auto_dependency_install_fails() {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("still.toml");
        let lockfile_path = temp.path().join("still.lock.toml");
        let source = write_local_skill_source(temp.path(), "rust-review");
        let config = format!(
            r#"
            [tools]
            rust = {{ version = "stable", backend = "rustup" }}

            [agents]

            [agents.skills]
            rust-review = {{ url = "{}", auto = true, tools = ["cargo-nextest"] }}
        "#,
            source
        );
        let lockfile = "[[items]]\nkind = \"tool\"\nname = \"rust\"\nplatform = \"macos\"\nversion = \"stable\"\n";
        fs::write(&config_path, &config).unwrap();
        fs::write(&lockfile_path, lockfile).unwrap();
        write_trust_marker(&config_path, config.as_bytes());
        let mut installer = FailingInstaller::default();

        let err = run_with_installer(
            AgentsRequest {
                start_dir: temp.path().to_path_buf(),
                home_dir: temp.path().to_path_buf(),
                global: false,
                operation: AgentsOperation::SyncAcceptAutoDependencies,
            },
            &mut installer,
        )
        .await
        .unwrap_err();

        assert!(
            err.to_string()
                .contains("failed to install auto-added agent dependencies")
        );
        assert_eq!(installer.attempted.len(), 1);
        assert_eq!(fs::read_to_string(&config_path).unwrap(), config);
        assert_eq!(fs::read_to_string(&lockfile_path).unwrap(), lockfile);
    }

    #[tokio::test]
    async fn agents_sync_auto_adds_missing_skill_manifest_dependencies() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source-skill");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("SKILL.md"), "# Local skill\n").unwrap();
        fs::write(
            source.join("still.skill.toml"),
            r#"
            [dependencies]
            tools = ["cargo-nextest@0.9.99@cargo"]
            packages = ["llvm"]
            apps = ["zed"]
            "#,
        )
        .unwrap();
        let config_path = temp.path().join("still.toml");
        let config = format!(
            r#"
            [agents]

            [agents.skills]
            local-skill = {{ url = "file://{}", auto = true }}
            "#,
            source.display()
        );
        fs::write(&config_path, &config).unwrap();
        write_trust_marker(&config_path, config.as_bytes());
        let mut installer = FakeInstaller::default();

        let result = run_with_installer(
            AgentsRequest {
                start_dir: temp.path().to_path_buf(),
                home_dir: temp.path().to_path_buf(),
                global: false,
                operation: AgentsOperation::SyncAcceptAutoDependencies,
            },
            &mut installer,
        )
        .await
        .unwrap();

        assert_eq!(result.auto_added.len(), 3);
        assert_eq!(installer.installed, result.auto_added);
        assert!(
            result
                .auto_added
                .iter()
                .any(|item| item.kind == Some(ItemKind::Tool) && item.spec.name == "cargo-nextest")
        );
        let updated = fs::read_to_string(temp.path().join("still.toml")).unwrap();
        let config = parse_still_toml(&updated).unwrap();
        assert!(config.tools.contains_key("cargo-nextest"));
        assert!(config.packages.latest.contains(&"llvm".to_string()));
        assert!(config.apps.latest.contains(&"zed".to_string()));
    }

    #[tokio::test]
    async fn agents_sync_reports_missing_manifest_dependencies_when_auto_is_false() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source-skill");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("SKILL.md"), "# Local skill\n").unwrap();
        fs::write(
            source.join("still.skill.toml"),
            r#"
            [dependencies]
            tools = ["cargo-audit"]
            packages = ["jq"]
            apps = ["zed"]
            "#,
        )
        .unwrap();
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
        let mut installer = FakeInstaller::default();

        let result = run_with_installer(
            AgentsRequest {
                start_dir: temp.path().to_path_buf(),
                home_dir: temp.path().to_path_buf(),
                global: false,
                operation: AgentsOperation::Sync,
            },
            &mut installer,
        )
        .await
        .unwrap();

        assert_eq!(result.auto_added, []);
        assert_eq!(installer.installed, []);
        assert_eq!(result.missing_dependencies.len(), 3);
        assert!(
            result
                .missing_dependencies
                .iter()
                .any(|item| item.kind == Some(ItemKind::Package) && item.spec.name == "jq")
        );
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
            global: false,
            operation: AgentsOperation::Check,
        })
        .await
        .unwrap();

        assert_eq!(result.auto_added, []);
        assert_eq!(result.missing_dependencies.len(), 3);
        assert!(
            result.missing_dependencies.iter().any(|item| {
                item.kind == Some(ItemKind::Tool) && item.spec.name == "cargo-audit"
            })
        );
        let content = fs::read_to_string(temp.path().join("still.toml")).unwrap();
        assert!(!content.contains("cargo-audit ="));
    }

    #[tokio::test]
    async fn agents_check_reports_versioned_dependency_mismatches() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("still.toml"),
            r#"
            [tools]
            cargo-nextest = "latest"

            [packages]
            latest = ["llvm"]

            [apps]
            zed = { backend = "homebrew-cask" }

            [agents]

            [agents.skills]
            rust-review = { source = "rust-review", tools = ["cargo-nextest@0.9.99@cargo"], packages = ["llvm@18@homebrew"], apps = ["zed@1.0.0@homebrew-cask"] }
            "#,
        )
        .unwrap();

        let result = run(AgentsRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
            operation: AgentsOperation::Check,
        })
        .await
        .unwrap();

        assert_eq!(result.missing_dependencies.len(), 3);
        assert!(result.missing_dependencies.iter().any(|item| {
            item.kind == Some(ItemKind::Tool)
                && item.spec.name == "cargo-nextest"
                && item.spec.version.as_str() == "0.9.99"
        }));
        assert!(result.missing_dependencies.iter().any(|item| {
            item.kind == Some(ItemKind::Package)
                && item.spec.name == "llvm"
                && item.spec.backend.as_ref().unwrap().as_str() == "homebrew"
        }));
        assert!(result.missing_dependencies.iter().any(|item| {
            item.kind == Some(ItemKind::App)
                && item.spec.name == "zed"
                && item.spec.version.as_str() == "1.0.0"
        }));
    }

    #[tokio::test]
    async fn agents_check_accepts_matching_version_with_any_backend_when_unspecified() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("still.toml"),
            r#"
            [tools]
            cargo-nextest = { version = "0.9.99", backend = "cargo" }

            [packages]
            llvm = { version = "18", backend = "homebrew" }

            [apps]
            zed = { backend = "homebrew-cask" }

            [agents]

            [agents.skills]
            rust-review = { source = "rust-review", tools = ["cargo-nextest@0.9.99"], packages = ["llvm@18"], apps = ["zed"] }
            "#,
        )
        .unwrap();

        let result = run(AgentsRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
            operation: AgentsOperation::Check,
        })
        .await
        .unwrap();

        assert_eq!(result.missing_dependencies, []);
    }

    #[tokio::test]
    async fn agents_check_reports_pending_auto_dependencies_without_writing() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("still.toml"),
            r#"
            [agents]

            [agents.skills]
            rust-review = { source = "rust-review", auto = true, tools = ["cargo-nextest"], packages = ["llvm"], apps = ["zed"] }
            "#,
        )
        .unwrap();

        let result = run(AgentsRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
            operation: AgentsOperation::Check,
        })
        .await
        .unwrap();

        assert_eq!(result.auto_added, []);
        assert_eq!(result.pending_auto_dependencies.len(), 3);
        assert!(
            result
                .pending_auto_dependencies
                .iter()
                .any(|item| item.kind == Some(ItemKind::Tool) && item.spec.name == "cargo-nextest")
        );
        let content = fs::read_to_string(temp.path().join("still.toml")).unwrap();
        let config = parse_still_toml(&content).unwrap();
        assert!(!config.tools.contains_key("cargo-nextest"));
        assert!(!config.packages.latest.contains(&"llvm".to_string()));
        assert!(!config.apps.latest.contains(&"zed".to_string()));
        assert!(!temp.path().join("still.lock.toml").exists());
    }

    #[tokio::test]
    async fn agents_check_reports_pending_local_manifest_dependencies_without_writing() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source-skill");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("SKILL.md"), "# Local skill\n").unwrap();
        fs::write(
            source.join("still.skill.toml"),
            r#"
            [dependencies]
            tools = ["cargo-nextest"]
            packages = ["llvm"]
            apps = ["zed"]
            "#,
        )
        .unwrap();
        let config_path = temp.path().join("still.toml");
        let config = format!(
            r#"
                [agents]

                [agents.skills]
                local-skill = {{ url = "file://{}", auto = true }}
                "#,
            source.display()
        );
        fs::write(&config_path, &config).unwrap();
        write_trust_marker(&config_path, config.as_bytes());

        let result = run(AgentsRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
            operation: AgentsOperation::Check,
        })
        .await
        .unwrap();

        assert_eq!(result.auto_added, []);
        assert_eq!(result.pending_auto_dependencies.len(), 3);
        assert!(
            result
                .pending_auto_dependencies
                .iter()
                .any(|item| item.kind == Some(ItemKind::Tool) && item.spec.name == "cargo-nextest")
        );
        assert!(!temp.path().join(".agents/skills").exists());
        assert!(!temp.path().join("still.lock.toml").exists());
    }

    #[tokio::test]
    async fn agents_check_requires_trust_before_reading_file_url_skill_manifest() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source-skill");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("SKILL.md"), "# Local skill\n").unwrap();
        fs::write(
            source.join("still.skill.toml"),
            r#"
            [dependencies]
            tools = ["cargo-nextest"]
            "#,
        )
        .unwrap();
        fs::write(
            temp.path().join("still.toml"),
            format!(
                r#"
                [agents.skills]
                local-skill = {{ url = "file://{}", auto = true }}
                "#,
                source.display()
            ),
        )
        .unwrap();

        let err = run(AgentsRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
            operation: AgentsOperation::Check,
        })
        .await
        .unwrap_err();

        assert!(
            err.to_string()
                .contains("not trusted for external agent skill inspection")
        );
    }

    #[tokio::test]
    async fn agents_global_check_reads_file_url_skill_manifest_without_project_trust() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("repo");
        let global = temp.path().join(".config/still/config.toml");
        let source = temp.path().join("source-skill");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(global.parent().unwrap()).unwrap();
        fs::create_dir_all(&source).unwrap();
        fs::write(project.join("still.toml"), "[agents]\n").unwrap();
        fs::write(source.join("SKILL.md"), "# Local skill\n").unwrap();
        fs::write(
            source.join("still.skill.toml"),
            r#"
            [dependencies]
            tools = ["cargo-nextest"]
            "#,
        )
        .unwrap();
        fs::write(
            &global,
            format!(
                r#"
                [agents.skills]
                local-skill = {{ url = "file://{}", auto = true }}
                "#,
                source.display()
            ),
        )
        .unwrap();

        let result = run(AgentsRequest {
            start_dir: project,
            home_dir: temp.path().to_path_buf(),
            global: true,
            operation: AgentsOperation::Check,
        })
        .await
        .unwrap();

        assert_eq!(result.path, global);
        assert_eq!(result.pending_auto_dependencies.len(), 1);
        assert_eq!(
            result.pending_auto_dependencies[0].spec.name,
            "cargo-nextest"
        );
        assert!(!global.parent().unwrap().join(".agents/skills").exists());
    }

    #[tokio::test]
    async fn agents_list_does_not_read_file_url_skill_manifest() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("still.toml"),
            r#"
            [agents.skills]
            local-skill = { url = "file:///missing/source", auto = true }
            "#,
        )
        .unwrap();

        let result = run(AgentsRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
            operation: AgentsOperation::List,
        })
        .await
        .unwrap();

        assert_eq!(result.agents.skills[0].name, "local-skill");
        assert_eq!(result.pending_auto_dependencies, []);
    }

    #[tokio::test]
    async fn agents_check_reports_missing_local_manifest_dependencies_when_auto_is_false() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source-skill");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("SKILL.md"), "# Local skill\n").unwrap();
        fs::write(
            source.join("still.skill.toml"),
            r#"
            [dependencies]
            tools = ["cargo-audit"]
            packages = ["jq"]
            apps = ["zed"]
            "#,
        )
        .unwrap();
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

        let result = run(AgentsRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
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
                .any(|item| item.kind == Some(ItemKind::Package) && item.spec.name == "jq")
        );
        assert!(!temp.path().join(".agents/skills").exists());
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
            global: false,
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
            global: false,
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

    fn write_local_skill_source(root: &Path, name: &str) -> String {
        let source = root.join(format!("{name}-source"));
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("SKILL.md"), format!("# {name}\n")).unwrap();
        format!("file://{}", source.display())
    }

    fn os_args(args: &[OsString]) -> Vec<String> {
        args.iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }
}
