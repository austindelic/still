//! Engine actions for config validation.

use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::config::{ConfigScope, ConfigSelection, resolve_config_path};
use crate::specs::toml::{StillConfig, parse_still_toml};

/// Request to validate the selected Still config.
#[derive(Debug, Clone)]
pub struct CheckConfigRequest {
    pub start_dir: PathBuf,
    pub home_dir: PathBuf,
    pub global: bool,
}

/// Result of loading and validating a Still config.
#[derive(Debug, Clone)]
pub struct CheckConfigResult {
    pub path: PathBuf,
    pub config: StillConfig,
}

/// Loads and parses the selected Still config.
/// # Errors
/// Fails when the selected config cannot be found, read, or parsed.
pub async fn check(request: CheckConfigRequest) -> Result<CheckConfigResult> {
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

    Ok(CheckConfigResult {
        path: resolved.path,
        config,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[tokio::test]
    async fn check_reads_nearest_project_config() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("repo");
        let nested = project.join("src");
        fs::create_dir_all(&nested).unwrap();
        fs::write(project.join("still.toml"), "[tools]\nnode = \"22\"\n").unwrap();

        let result = check(CheckConfigRequest {
            start_dir: nested,
            home_dir: temp.path().to_path_buf(),
            global: false,
        })
        .await
        .unwrap();

        assert_eq!(result.path, project.join("still.toml"));
        assert!(result.config.tools.contains_key("node"));
    }

    #[tokio::test]
    async fn check_can_read_global_config() {
        let temp = tempfile::tempdir().unwrap();
        let config_dir = temp.path().join(".config/still");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(
            config_dir.join("config.toml"),
            "[tools]\npython = \"3.12\"\n",
        )
        .unwrap();

        let result = check(CheckConfigRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: true,
        })
        .await
        .unwrap();

        assert_eq!(result.path, config_dir.join("config.toml"));
        assert!(result.config.tools.contains_key("python"));
    }
}
