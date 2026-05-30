//! Engine action for creating starter project config.

use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::config::PROJECT_CONFIG_FILE;
use crate::error::EngineError;
use crate::trust::{config_fingerprint, trust_marker_path};

const STARTER_CONFIG: &str = r#"[tools]

[packages]
latest = []

[apps]
latest = []

[env]
files = []

[tasks]

[services]

[agents]
targets = []
"#;

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

    tokio::fs::write(&path, STARTER_CONFIG)
        .await
        .with_context(|| format!("failed to write {}", path.display()))?;
    let trust_path = trust_marker_path(&path);
    if let Some(parent) = trust_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(
        &trust_path,
        format!(
            "config = \"{}\"\nfingerprint = \"{}\"\n",
            path.display(),
            config_fingerprint(STARTER_CONFIG.as_bytes())
        ),
    )
    .await
    .with_context(|| format!("failed to write {}", trust_path.display()))?;

    Ok(InitResult { path })
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
