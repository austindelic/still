//! Trust verification for project-defined executable behavior.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::error::EngineError;
use crate::utils::hashing::Hashing;

/// Trust marker file written under the project-local `.still` directory.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TrustMarker {
    pub config: String,
    pub fingerprint: String,
}

/// Returns the marker path for a resolved project config.
pub fn trust_marker_path(config_path: &Path) -> PathBuf {
    config_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(".still")
        .join("trust.toml")
}

/// Computes the fingerprint stored in project trust markers.
pub fn config_fingerprint(content: &[u8]) -> String {
    Hashing::sha256(content)
}

/// Fails unless the current config bytes match the project trust marker.
/// # Errors
/// Fails when the marker is missing, malformed, scoped to another config path,
/// or was written for different config contents.
pub async fn assert_config_trusted(
    config_path: &Path,
    content: &[u8],
    behavior: impl Into<String>,
) -> Result<()> {
    let behavior = behavior.into();
    let marker_path = trust_marker_path(config_path);
    let marker = match read_marker(&marker_path).await {
        Ok(marker) => marker,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(untrusted(config_path, behavior).into());
        }
        Err(err) => {
            return Err(err)
                .with_context(|| format!("failed to read trust marker {}", marker_path.display()));
        }
    };
    let expected_config = config_path.display().to_string();
    if marker.config != expected_config || marker.fingerprint != config_fingerprint(content) {
        return Err(untrusted(config_path, behavior).into());
    }

    Ok(())
}

async fn read_marker(path: &Path) -> std::io::Result<TrustMarker> {
    let content = tokio::fs::read_to_string(path).await?;
    let marker = toml_edit::de::from_str(&content)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
    Ok(marker)
}

fn untrusted(config_path: &Path, behavior: String) -> EngineError {
    EngineError::UntrustedProjectConfig {
        config_path: config_path.to_path_buf(),
        behavior,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[tokio::test]
    async fn accepts_matching_trust_marker() {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("still.toml");
        let content = b"[tasks]\ntest = \"echo ok\"\n";
        fs::write(&config_path, content).unwrap();
        let marker_path = trust_marker_path(&config_path);
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

        assert_config_trusted(&config_path, content, "task execution")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn rejects_stale_trust_marker() {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("still.toml");
        let content = b"[tasks]\ntest = \"echo ok\"\n";
        let marker_path = trust_marker_path(&config_path);
        fs::create_dir_all(marker_path.parent().unwrap()).unwrap();
        fs::write(
            marker_path,
            format!(
                "config = \"{}\"\nfingerprint = \"{}\"\n",
                config_path.display(),
                config_fingerprint(b"old")
            ),
        )
        .unwrap();

        let err = assert_config_trusted(&config_path, content, "task execution")
            .await
            .unwrap_err();

        assert!(err.to_string().contains("not trusted"));
        assert!(err.to_string().contains("task execution"));
    }
}
