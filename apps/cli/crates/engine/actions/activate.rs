//! Engine action for shell activation snippets.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::actions::run::resolve_env_with_scope;
use crate::config::{ConfigScope, find_project_config, global_config_path};
use crate::error::EngineError;
use crate::system::System;
use crate::utils::paths::PathOps;

/// Request to generate shell activation code.
#[derive(Debug, Clone)]
pub struct ActivateRequest {
    pub start_dir: PathBuf,
    pub home_dir: PathBuf,
    pub global: bool,
    pub shell: Option<String>,
}

/// Shell activation code for Still-managed paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivateResult {
    pub shell: ShellKind,
    pub code: String,
}

/// Supported shell syntaxes for activation output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellKind {
    Posix,
    Fish,
    PowerShell,
    Cmd,
}

/// Generates shell code that applies Still-managed PATH and configured env.
/// # Errors
/// Fails when the shell is unsupported or project env cannot be resolved.
pub async fn run(request: ActivateRequest) -> Result<ActivateResult> {
    let shell = request
        .shell
        .as_deref()
        .map(parse_shell)
        .transpose()?
        .unwrap_or(ShellKind::Posix);
    let root = still_root(&request.home_dir);
    let vars =
        activation_vars(&request.start_dir, &request.home_dir, &root, request.global).await?;
    let code = activation_code(shell, &vars)?;

    Ok(ActivateResult { shell, code })
}

fn parse_shell(value: &str) -> Result<ShellKind, EngineError> {
    match value {
        "sh" | "bash" | "zsh" | "posix" => Ok(ShellKind::Posix),
        "fish" => Ok(ShellKind::Fish),
        "powershell" | "pwsh" => Ok(ShellKind::PowerShell),
        "cmd" => Ok(ShellKind::Cmd),
        value => Err(EngineError::Conflict {
            message: format!("unsupported shell \"{value}\""),
        }),
    }
}

async fn activation_vars(
    start_dir: &Path,
    home_dir: &Path,
    root: &Path,
    global: bool,
) -> Result<BTreeMap<String, String>> {
    let scope = if global {
        ConfigScope::Global
    } else {
        ConfigScope::Project
    };
    let has_readable_config = if global {
        global_config_path(home_dir).is_file()
    } else {
        find_project_config(start_dir).is_some() || global_config_path(home_dir).is_file()
    };
    let mut vars = if has_readable_config {
        resolve_env_with_scope(start_dir, home_dir, scope)
            .await?
            .vars
    } else {
        BTreeMap::from([("PATH".to_string(), managed_path(None))])
    };
    vars.insert("STILL_HOME".to_string(), root.display().to_string());
    Ok(vars)
}

fn managed_path(configured: Option<&str>) -> String {
    let mut paths = vec![System::bin_dir()];
    if let Some(configured) = configured {
        paths.extend(std::env::split_paths(configured));
    } else if let Some(existing) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&existing));
    }
    std::env::join_paths(paths)
        .unwrap_or_else(|_| System::bin_dir().into_os_string())
        .to_string_lossy()
        .into_owned()
}

fn activation_code(shell: ShellKind, vars: &BTreeMap<String, String>) -> Result<String> {
    let mut lines = Vec::new();
    for (key, value) in vars {
        if !is_portable_env_name(key) {
            return Err(EngineError::InvalidConfig {
                reason: format!("invalid environment variable name \"{key}\""),
            }
            .into());
        }
        lines.push(match shell {
            ShellKind::Posix => format!("export {key}={}", quote_posix(value)),
            ShellKind::Fish => format!("set -gx {key} {}", quote_fish(value)),
            ShellKind::PowerShell => format!("$env:{key} = {}", quote_powershell(value)),
            ShellKind::Cmd => format!("set \"{key}={}\"", quote_cmd_set_value(value)),
        });
    }
    Ok(lines.join("\n"))
}

fn is_portable_env_name(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some('_') | Some('A'..='Z') | Some('a'..='z'))
        && chars.all(|char| matches!(char, '_' | 'A'..='Z' | 'a'..='z' | '0'..='9'))
}

fn quote_posix(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn quote_fish(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
}

fn quote_powershell(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn quote_cmd_set_value(value: &str) -> String {
    value.replace('^', "^^").replace('"', "^\"")
}

fn still_root(home_dir: &Path) -> PathBuf {
    if cfg!(target_os = "macos") {
        PathBuf::from("/opt").join("still")
    } else if cfg!(target_os = "windows") {
        home_dir.join("AppData").join("Local").join("still")
    } else {
        home_dir.join(".local").join("share").join("still")
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::trust::{config_fingerprint, trust_marker_path};

    use super::*;

    #[tokio::test]
    async fn generates_posix_activation_by_default() {
        let temp = tempfile::tempdir().unwrap();
        let result = run(ActivateRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
            shell: None,
        })
        .await
        .unwrap();

        assert_eq!(result.shell, ShellKind::Posix);
        assert!(result.code.contains("export STILL_HOME="));
        assert!(result.code.contains("export PATH="));
    }

    #[tokio::test]
    async fn generates_fish_activation() {
        let temp = tempfile::tempdir().unwrap();
        let result = run(ActivateRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
            shell: Some("fish".to_string()),
        })
        .await
        .unwrap();

        assert_eq!(result.shell, ShellKind::Fish);
        assert!(result.code.contains("set -gx PATH"));
    }

    #[tokio::test]
    async fn activation_applies_project_env() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("still.toml"),
            r#"
            [env]
            RUST_LOG = "debug"
            MESSAGE = "hello 'still'"
            "#,
        )
        .unwrap();

        let result = run(ActivateRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
            shell: None,
        })
        .await
        .unwrap();

        assert!(result.code.contains("export RUST_LOG='debug'"));
        assert!(
            result
                .code
                .contains("export MESSAGE='hello '\"'\"'still'\"'\"''")
        );
    }

    #[tokio::test]
    async fn activation_prepends_still_bin_to_configured_path() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("still.toml"),
            "[env]\nPATH = \"project-bin\"\n",
        )
        .unwrap();

        let vars = activation_vars(temp.path(), temp.path(), &still_root(temp.path()), false)
            .await
            .unwrap();
        let path = vars.get("PATH").unwrap();
        let paths = std::env::split_paths(path).collect::<Vec<_>>();

        assert_eq!(paths.first(), Some(&System::bin_dir()));
        assert_eq!(paths.get(1), Some(&PathBuf::from("project-bin")));
    }

    #[tokio::test]
    async fn activation_can_use_global_env_when_project_config_exists() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("repo");
        let global = temp.path().join(".config/still/config.toml");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(global.parent().unwrap()).unwrap();
        fs::write(project.join("still.toml"), "[env]\nVALUE = \"project\"\n").unwrap();
        fs::write(&global, "[env]\nVALUE = \"global\"\n").unwrap();

        let result = run(ActivateRequest {
            start_dir: project,
            home_dir: temp.path().to_path_buf(),
            global: true,
            shell: Some("fish".to_string()),
        })
        .await
        .unwrap();

        assert!(result.code.contains("set -gx VALUE 'global'"));
        assert!(!result.code.contains("project"));
    }

    #[tokio::test]
    async fn activation_env_file_loading_requires_trust() {
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

        let err = run(ActivateRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
            shell: None,
        })
        .await
        .unwrap_err();

        assert!(err.to_string().contains("not trusted"));
        assert!(err.to_string().contains("env file loading"));
    }

    #[tokio::test]
    async fn activation_loads_trusted_env_files() {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("still.toml");
        let config = r#"
            [env]
            files = [".env"]
            INLINE = "config"
            "#;
        fs::write(&config_path, config).unwrap();
        fs::write(temp.path().join(".env"), "FROM_FILE=loaded\n").unwrap();
        write_trust_marker(&config_path, config.as_bytes());

        let result = run(ActivateRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
            shell: Some("powershell".to_string()),
        })
        .await
        .unwrap();

        assert!(result.code.contains("$env:FROM_FILE = 'loaded'"));
        assert!(result.code.contains("$env:INLINE = 'config'"));
    }

    #[tokio::test]
    async fn activation_applies_global_env_without_project_config() {
        let temp = tempfile::tempdir().unwrap();
        let global = temp.path().join(".config/still/config.toml");
        fs::create_dir_all(global.parent().unwrap()).unwrap();
        fs::write(
            &global,
            r#"
            [env]
            GLOBAL_ONLY = "yes"
            "#,
        )
        .unwrap();

        let result = run(ActivateRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
            shell: None,
        })
        .await
        .unwrap();

        assert!(result.code.contains("export GLOBAL_ONLY='yes'"));
    }

    #[tokio::test]
    async fn rejects_unknown_shells() {
        let temp = tempfile::tempdir().unwrap();
        let err = run(ActivateRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            global: false,
            shell: Some("elvish".to_string()),
        })
        .await
        .unwrap_err();

        assert!(err.to_string().contains("unsupported shell"));
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
