//! Engine action for creating starter project config.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::error::{EngineContext, Result};
use serde::Serialize;

use crate::config::PROJECT_CONFIG_FILE;
use crate::error::EngineError;
use crate::trust::{config_fingerprint, trust_marker_content, trust_marker_path};

/// Request to initialize a project config.
#[derive(Debug, Clone)]
pub struct InitRequest {
    pub start_dir: PathBuf,
    pub force: bool,
}

/// Result of creating a starter config.
#[derive(Debug, Clone)]
pub struct InitResult {
    pub path: PathBuf,
}

/// Creates a starter `still.toml` in the selected directory.
/// # Errors
/// Fails if the file exists and `force` is false, or if the file cannot be
/// written.
/// # Side Effects
/// Writes `still.toml` and a matching project trust marker.
pub async fn run(request: InitRequest) -> Result<InitResult> {
    let path = request.start_dir.join(PROJECT_CONFIG_FILE);
    if path.exists() && !request.force {
        return Err(EngineError::ConfigAlreadyExists { path }.into());
    }

    let content = starter_config(&request.start_dir).await?;
    tokio::fs::write(&path, &content)
        .await
        .with_context(|| format!("failed to write {}", path.display()))?;
    let trust_path = trust_marker_path(&path);
    if let Some(parent) = trust_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(
        &trust_path,
        trust_marker_content(&path, config_fingerprint(content.as_bytes()))?,
    )
    .await
    .with_context(|| format!("failed to write {}", trust_path.display()))?;

    Ok(InitResult { path })
}

async fn starter_config(project_dir: &Path) -> Result<String> {
    let mut tools = BTreeMap::new();
    if file_exists(project_dir.join("Cargo.toml")).await {
        tools.insert("rust".to_string(), "stable".to_string());
    }
    if file_exists(project_dir.join("package.json")).await {
        tools.insert("node".to_string(), "latest".to_string());
    }
    if file_exists(project_dir.join("go.mod")).await {
        tools.insert("go".to_string(), "latest".to_string());
    }
    if file_exists(project_dir.join("pyproject.toml")).await {
        tools.insert("python".to_string(), "latest".to_string());
    }

    let config = StarterConfig {
        tools,
        packages: LatestSection::default(),
        apps: LatestSection::default(),
        env: EnvStarter::default(),
        tasks: BTreeMap::new(),
        services: BTreeMap::new(),
        agents: AgentsStarter::default(),
    };
    toml::to_string_pretty(&config).map_err(Into::into)
}

#[derive(Debug, Default, Serialize)]
struct StarterConfig {
    tools: BTreeMap<String, String>,
    packages: LatestSection,
    apps: LatestSection,
    env: EnvStarter,
    tasks: BTreeMap<String, String>,
    services: BTreeMap<String, String>,
    agents: AgentsStarter,
}

#[derive(Debug, Default, Serialize)]
struct LatestSection {
    latest: Vec<String>,
}

#[derive(Debug, Default, Serialize)]
struct EnvStarter {
    files: Vec<String>,
}

#[derive(Debug, Default, Serialize)]
struct AgentsStarter {
    targets: Vec<String>,
}

async fn file_exists(path: PathBuf) -> bool {
    tokio::fs::metadata(path)
        .await
        .is_ok_and(|metadata| metadata.is_file())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[tokio::test]
    async fn init_creates_starter_config() {
        let temp = tempfile::tempdir().unwrap();

        let result = run(InitRequest {
            start_dir: temp.path().to_path_buf(),
            force: false,
        })
        .await
        .unwrap();

        assert_eq!(result.path, temp.path().join("still.toml"));
        let content = fs::read_to_string(result.path).unwrap();
        assert!(content.contains("[tools]"));
        assert!(content.contains("[agents]"));
        let marker = fs::read_to_string(temp.path().join(".still/trust.toml")).unwrap();
        assert!(marker.contains("fingerprint"));
        assert!(marker.contains("still.toml"));
    }

    #[tokio::test]
    async fn init_infers_common_project_tools() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("Cargo.toml"),
            "[package]\nname = \"demo\"\n",
        )
        .unwrap();
        fs::write(temp.path().join("package.json"), "{}\n").unwrap();

        let result = run(InitRequest {
            start_dir: temp.path().to_path_buf(),
            force: false,
        })
        .await
        .unwrap();

        let content = fs::read_to_string(&result.path).unwrap();
        assert!(content.contains("rust = \"stable\""));
        assert!(content.contains("node = \"latest\""));
        let marker = fs::read_to_string(temp.path().join(".still/trust.toml")).unwrap();
        assert!(marker.contains(&config_fingerprint(content.as_bytes())));
    }

    #[tokio::test]
    async fn init_refuses_to_overwrite_existing_config() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("still.toml"), "[tools]\nnode = \"22\"\n").unwrap();

        let err = run(InitRequest {
            start_dir: temp.path().to_path_buf(),
            force: false,
        })
        .await
        .unwrap_err();

        assert!(err.to_string().contains("already exists"));
        assert!(err.to_string().contains("--force"));
    }

    #[tokio::test]
    async fn init_force_overwrites_existing_config() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("still.toml");
        fs::write(&path, "[tools]\nnode = \"22\"\n").unwrap();

        run(InitRequest {
            start_dir: temp.path().to_path_buf(),
            force: true,
        })
        .await
        .unwrap();

        let content = fs::read_to_string(path).unwrap();
        assert!(content.contains("[agents]"));
        assert!(!content.contains("node = \"22\""));
    }
}
