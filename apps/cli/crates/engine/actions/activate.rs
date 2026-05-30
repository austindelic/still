//! Engine action for shell activation snippets.

use std::path::{Path, PathBuf};

use crate::error::{EngineError, EngineResult};

/// Request to generate shell activation code.
#[derive(Debug, Clone)]
pub struct ActivateRequest {
    pub home_dir: PathBuf,
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

/// Generates shell code that prepends Still's bin directory to PATH.
pub fn run(request: ActivateRequest) -> EngineResult<ActivateResult> {
    let shell = request
        .shell
        .as_deref()
        .map(parse_shell)
        .transpose()?
        .unwrap_or(ShellKind::Posix);
    let root = still_root(&request.home_dir);
    let bin = root.join("bin");
    let code = activation_code(shell, &root, &bin);

    Ok(ActivateResult { shell, code })
}

fn parse_shell(value: &str) -> EngineResult<ShellKind> {
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

fn activation_code(shell: ShellKind, root: &Path, bin: &Path) -> String {
    let root = root.display();
    let bin = bin.display();
    match shell {
        ShellKind::Posix => format!("export STILL_HOME=\"{root}\"\nexport PATH=\"{bin}:$PATH\""),
        ShellKind::Fish => format!("set -gx STILL_HOME \"{root}\"\nfish_add_path \"{bin}\""),
        ShellKind::PowerShell => {
            format!("$env:STILL_HOME = \"{root}\"\n$env:Path = \"{bin};$env:Path\"")
        }
        ShellKind::Cmd => format!("set \"STILL_HOME={root}\"\nset \"PATH={bin};%PATH%\""),
    }
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
    use super::*;

    #[test]
    fn generates_posix_activation_by_default() {
        let result = run(ActivateRequest {
            home_dir: PathBuf::from("/home/alex"),
            shell: None,
        })
        .unwrap();

        assert_eq!(result.shell, ShellKind::Posix);
        assert!(result.code.contains("export STILL_HOME="));
        assert!(result.code.contains("export PATH="));
    }

    #[test]
    fn generates_fish_activation() {
        let result = run(ActivateRequest {
            home_dir: PathBuf::from("/home/alex"),
            shell: Some("fish".to_string()),
        })
        .unwrap();

        assert_eq!(result.shell, ShellKind::Fish);
        assert!(result.code.contains("fish_add_path"));
    }

    #[test]
    fn rejects_unknown_shells() {
        let err = run(ActivateRequest {
            home_dir: PathBuf::from("/home/alex"),
            shell: Some("elvish".to_string()),
        })
        .unwrap_err();

        assert!(err.to_string().contains("unsupported shell"));
    }
}
