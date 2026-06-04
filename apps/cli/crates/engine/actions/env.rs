//! Engine action for resolved environment inspection.

use std::path::PathBuf;

use crate::error::{EngineContext, Result};

use crate::actions::run::resolve_env_with_scope;
use crate::config::{ConfigScope, ConfigSelection, global_config_path, resolve_config_path};
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
    let mut files = Vec::new();
    if !request.global && resolved.scope == ConfigScope::Project {
        files.extend(env_files_from_config(&global_config_path(&request.home_dir)).await?);
    }
    files.extend(config.env.files);
    let resolved_env =
        resolve_env_with_scope(&request.start_dir, &request.home_dir, resolved.scope).await?;
    let vars = resolved_env.vars.into_iter().collect();

    Ok(EnvResult {
        path: resolved.path,
        vars,
        files,
    })
}

async fn env_files_from_config(path: &std::path::Path) -> Result<Vec<String>> {
    let content = match tokio::fs::read_to_string(path).await {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err).with_context(|| format!("failed to read {}", path.display())),
    };
    Ok(parse_still_toml(&content)?.env.files)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::trust::{config_fingerprint, trust_marker_path};

    use super::*;

    #[tokio::test]
    async fn inspect_reads_env_vars_and_files() {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("still.toml");
        let config = r#"
            [env]
            RUST_LOG = "debug"
            NODE_ENV = "development"
            files = [".env", ".env.local"]
            "#;
        fs::write(&config_path, config).unwrap();
        fs::write(temp.path().join(".env"), "").unwrap();
        fs::write(temp.path().join(".env.local"), "").unwrap();
        write_trust_marker(&config_path, config.as_bytes());

        let result = inspect(EnvRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
        })
        .await
        .unwrap();

        assert_eq!(result.path, temp.path().join("still.toml"));
        assert!(
            result
                .vars
                .contains(&("NODE_ENV".to_string(), "development".to_string()))
        );
        assert!(
            result
                .vars
                .contains(&("RUST_LOG".to_string(), "debug".to_string()))
        );
        assert_eq!(result.files, [".env", ".env.local"]);
        assert!(result.vars.iter().any(|(key, _)| key == "PATH"));
    }

    #[tokio::test]
    async fn inspect_resolves_trusted_env_files() {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("still.toml");
        let config = r#"
            [env]
            files = [".env", ".env.local"]
            SHARED = "config"
            "#;
        fs::write(&config_path, config).unwrap();
        fs::write(temp.path().join(".env"), "SHARED=file\nFROM_FILE=one\n").unwrap();
        fs::write(temp.path().join(".env.local"), "FROM_LOCAL='two'\n").unwrap();
        write_trust_marker(&config_path, config.as_bytes());

        let result = inspect(EnvRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
        })
        .await
        .unwrap();

        assert!(
            result
                .vars
                .contains(&("SHARED".to_string(), "config".to_string()))
        );
        assert!(
            result
                .vars
                .contains(&("FROM_FILE".to_string(), "one".to_string()))
        );
        assert!(
            result
                .vars
                .contains(&("FROM_LOCAL".to_string(), "two".to_string()))
        );
    }

    #[tokio::test]
    async fn inspect_reports_global_and_project_env_files() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("repo");
        let global = temp.path().join(".config/still/config.toml");
        let project_config = project.join("still.toml");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(global.parent().unwrap()).unwrap();
        fs::write(
            &global,
            r#"
            [env]
            files = ["global.env"]
            GLOBAL = "yes"
            "#,
        )
        .unwrap();
        fs::write(
            global.parent().unwrap().join("global.env"),
            "GLOBAL_FILE=yes\n",
        )
        .unwrap();
        let project_content = r#"
            [env]
            files = ["project.env"]
            PROJECT = "yes"
            "#;
        fs::write(&project_config, project_content).unwrap();
        fs::write(project.join("project.env"), "PROJECT_FILE=yes\n").unwrap();
        write_trust_marker(&project_config, project_content.as_bytes());

        let result = inspect(EnvRequest {
            start_dir: project,
            home_dir: temp.path().to_path_buf(),
            global: false,
        })
        .await
        .unwrap();

        assert_eq!(result.files, ["global.env", "project.env"]);
        assert!(
            result
                .vars
                .contains(&("GLOBAL_FILE".to_string(), "yes".to_string()))
        );
        assert!(
            result
                .vars
                .contains(&("PROJECT_FILE".to_string(), "yes".to_string()))
        );
    }

    #[tokio::test]
    async fn inspect_requires_trust_before_loading_env_files() {
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

        let err = inspect(EnvRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
        })
        .await
        .unwrap_err();

        assert!(err.to_string().contains("not trusted"));
        assert!(err.to_string().contains("env file loading"));
    }

    fn write_trust_marker(config_path: &std::path::Path, content: &[u8]) {
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
