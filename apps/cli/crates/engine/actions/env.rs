//! Engine action for resolved environment inspection.

use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::config::{ConfigScope, ConfigSelection, resolve_config_path};
use crate::specs::toml::parse_still_toml;

/// Request to inspect configured environment values.
#[derive(Debug, Clone)]
pub struct EnvRequest {
    pub start_dir: PathBuf,
    pub home_dir: PathBuf,
    pub global: bool,
}

/// Configured environment values and source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvResult {
    pub path: PathBuf,
    pub vars: Vec<(String, String)>,
    pub files: Vec<String>,
}

/// Reads configured environment values without executing project code.
/// # Errors
/// Fails when config cannot be found, read, or parsed.
pub async fn inspect(request: EnvRequest) -> Result<EnvResult> {
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
    let vars = config.env.vars.into_iter().collect();

    Ok(EnvResult {
        path: resolved.path,
        vars,
        files: config.env.files,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[tokio::test]
    async fn inspect_reads_env_vars_and_files() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("still.toml"),
            r#"
            [env]
            RUST_LOG = "debug"
            NODE_ENV = "development"
            files = [".env", ".env.local"]
            "#,
        )
        .unwrap();

        let result = inspect(EnvRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
        })
        .await
        .unwrap();

        assert_eq!(result.path, temp.path().join("still.toml"));
        assert_eq!(
            result.vars,
            [
                ("NODE_ENV".to_string(), "development".to_string()),
                ("RUST_LOG".to_string(), "debug".to_string())
            ]
        );
        assert_eq!(result.files, [".env", ".env.local"]);
    }
}
