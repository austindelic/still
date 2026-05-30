//! Engine action for running a command in the Still environment.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

use crate::config::{ConfigScope, ConfigSelection, resolve_config_path};
use crate::error::EngineError;
use crate::specs::toml::parse_still_toml;
use crate::trust::assert_config_trusted;

/// Request to run a child command with configured environment values.
#[derive(Debug, Clone)]
pub struct RunRequest {
    pub start_dir: PathBuf,
    pub home_dir: PathBuf,
    pub command: Vec<String>,
}

/// Captured result from a child command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunResult {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Runs a command with Still config environment applied.
/// # Errors
/// Fails when no command is supplied, config/env files cannot be read, or the
/// child process cannot be spawned.
pub async fn run(request: RunRequest) -> Result<RunResult> {
    if request.command.is_empty() {
        return Err(EngineError::Conflict {
            message: "run requires a command".to_string(),
        }
        .into());
    }

    let resolved_env = resolve_env(&request.start_dir, &request.home_dir).await?;
    let mut command = Command::new(&request.command[0]);
    command.args(&request.command[1..]);
    command.current_dir(resolved_env.working_dir);
    command.envs(resolved_env.vars);

    let output = command
        .output()
        .with_context(|| format!("failed to run {}", request.command[0]))?;

    Ok(RunResult {
        status: output.status.code().unwrap_or(1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedRunEnv {
    working_dir: PathBuf,
    vars: BTreeMap<String, String>,
}

async fn resolve_env(start_dir: &Path, home_dir: &Path) -> Result<ResolvedRunEnv> {
    let resolved = resolve_config_path(
        start_dir,
        home_dir,
        ConfigSelection {
            scope: ConfigScope::Project,
            for_write: false,
        },
    )?;
    let config_dir = resolved
        .path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let content = tokio::fs::read_to_string(&resolved.path)
        .await
        .with_context(|| format!("failed to read {}", resolved.path.display()))?;
    let config = parse_still_toml(&content)?;
    if !config.env.files.is_empty() {
        assert_config_trusted(&resolved.path, content.as_bytes(), "env file loading").await?;
    }
    let mut vars = BTreeMap::new();

    for file in config.env.files {
        let path = config_dir.join(&file);
        let content = tokio::fs::read_to_string(&path)
            .await
            .with_context(|| format!("failed to read env file {}", path.display()))?;
        vars.extend(parse_env_file(&content)?);
    }
    vars.extend(config.env.vars);

    Ok(ResolvedRunEnv {
        working_dir: config_dir,
        vars,
    })
}

fn parse_env_file(input: &str) -> Result<BTreeMap<String, String>> {
    let mut vars = BTreeMap::new();
    for (index, line) in input.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            return Err(EngineError::InvalidConfig {
                reason: format!("invalid env file line {}", index + 1),
            }
            .into());
        };
        let key = key.trim();
        if key.is_empty() {
            return Err(EngineError::InvalidConfig {
                reason: format!("invalid env file line {}", index + 1),
            }
            .into());
        }
        vars.insert(key.to_string(), unquote_env_value(value.trim()));
    }
    Ok(vars)
}

fn unquote_env_value(value: &str) -> String {
    if value.len() >= 2 {
        let bytes = value.as_bytes();
        if (bytes[0] == b'"' && bytes[value.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[value.len() - 1] == b'\'')
        {
            return value[1..value.len() - 1].to_string();
        }
    }
    value.to_string()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::trust::{config_fingerprint, trust_marker_path};

    use super::*;

    #[tokio::test]
    async fn resolves_env_files_then_config_vars() {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("still.toml");
        let config = r#"
            [env]
            files = [".env", ".env.local"]
            SHARED = "config"
            INLINE = "yes"
            "#;
        fs::write(&config_path, config).unwrap();
        write_trust_marker(&config_path, config.as_bytes());
        fs::write(temp.path().join(".env"), "SHARED=file\nFROM_FILE=one\n").unwrap();
        fs::write(
            temp.path().join(".env.local"),
            "FROM_LOCAL='two'\nFROM_FILE=override\n",
        )
        .unwrap();

        let result = resolve_env(temp.path(), temp.path()).await.unwrap();

        assert_eq!(result.working_dir, temp.path());
        assert_eq!(result.vars["SHARED"], "config");
        assert_eq!(result.vars["INLINE"], "yes");
        assert_eq!(result.vars["FROM_FILE"], "override");
        assert_eq!(result.vars["FROM_LOCAL"], "two");
    }

    #[tokio::test]
    async fn run_rejects_empty_command() {
        let temp = tempfile::tempdir().unwrap();

        let err = run(RunRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            command: Vec::new(),
        })
        .await
        .unwrap_err();

        assert!(err.to_string().contains("run requires a command"));
    }

    #[test]
    fn env_file_parser_rejects_invalid_lines() {
        let err = parse_env_file("ok=yes\ninvalid\n").unwrap_err();

        assert!(err.to_string().contains("line 2"));
    }

    #[test]
    fn env_file_parser_keeps_values_literal() {
        let vars = parse_env_file("HOME_COPY=$HOME\nMESSAGE=\"hello $USER\"\n").unwrap();

        assert_eq!(vars["HOME_COPY"], "$HOME");
        assert_eq!(vars["MESSAGE"], "hello $USER");
    }

    #[tokio::test]
    async fn env_file_loading_requires_trust() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("still.toml"),
            r#"
            [env]
            files = [".env"]
            "#,
        )
        .unwrap();
        fs::write(temp.path().join(".env"), "TOKEN=secret\n").unwrap();

        let err = resolve_env(temp.path(), temp.path()).await.unwrap_err();

        assert!(err.to_string().contains("not trusted"));
        assert!(err.to_string().contains("env file loading"));
    }

    #[tokio::test]
    async fn missing_env_files_are_errors() {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("still.toml");
        let config = r#"
            [env]
            files = [".env.missing"]
            "#;
        fs::write(&config_path, config).unwrap();
        write_trust_marker(&config_path, config.as_bytes());

        let err = resolve_env(temp.path(), temp.path()).await.unwrap_err();

        assert!(err.to_string().contains("failed to read env file"));
        assert!(err.to_string().contains(".env.missing"));
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
