//! Engine action for trusting project-defined executable behavior.

use std::path::PathBuf;

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

use crate::config::{ConfigScope, ConfigSelection, resolve_config_path};

/// Request to mark the current project config trusted.
#[derive(Debug, Clone)]
pub struct TrustRequest {
    pub start_dir: PathBuf,
    pub home_dir: PathBuf,
}

/// Trust marker written for a project config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustResult {
    pub config_path: PathBuf,
    pub trust_path: PathBuf,
    pub fingerprint: String,
}

/// Writes a project-local trust marker for the selected config file.
/// # Errors
/// Fails when no project config exists or the trust marker cannot be written.
pub async fn run(request: TrustRequest) -> Result<TrustResult> {
    let resolved = resolve_config_path(
        &request.start_dir,
        &request.home_dir,
        ConfigSelection {
            scope: ConfigScope::Project,
            for_write: true,
        },
    )?;
    let content = tokio::fs::read(&resolved.path)
        .await
        .with_context(|| format!("failed to read {}", resolved.path.display()))?;
    let fingerprint = fingerprint(&content);
    let trust_path = resolved
        .path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join(".still")
        .join("trust.toml");

    if let Some(parent) = trust_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(
        &trust_path,
        format!(
            "config = \"{}\"\nfingerprint = \"{}\"\n",
            resolved.path.display(),
            fingerprint
        ),
    )
    .await
    .with_context(|| format!("failed to write {}", trust_path.display()))?;

    Ok(TrustResult {
        config_path: resolved.path,
        trust_path,
        fingerprint,
    })
}

fn fingerprint(content: &[u8]) -> String {
    format!("{:x}", Sha256::digest(content))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[tokio::test]
    async fn trust_writes_project_marker() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("still.toml"), "[tools]\n").unwrap();

        let result = run(TrustRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
        })
        .await
        .unwrap();

        assert_eq!(result.config_path, temp.path().join("still.toml"));
        assert_eq!(result.trust_path, temp.path().join(".still/trust.toml"));
        let marker = fs::read_to_string(result.trust_path).unwrap();
        assert!(marker.contains("fingerprint"));
        assert!(marker.contains("still.toml"));
    }

    #[tokio::test]
    async fn trust_requires_project_config() {
        let temp = tempfile::tempdir().unwrap();

        let err = run(TrustRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
        })
        .await
        .unwrap_err();

        assert!(err.to_string().contains("still init"));
    }
}
