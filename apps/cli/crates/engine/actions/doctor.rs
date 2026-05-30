//! Engine action for local Still diagnostics.

use std::path::PathBuf;

use anyhow::Result;

use crate::config::{ConfigScope, ConfigSelection, find_project_config, resolve_config_path};
use crate::platform::PlatformId;

/// Request to diagnose local Still state.
#[derive(Debug, Clone)]
pub struct DoctorRequest {
    pub start_dir: PathBuf,
    pub home_dir: PathBuf,
}

/// Diagnostic check result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorCheck {
    pub name: String,
    pub status: DoctorStatus,
    pub detail: String,
}

/// Diagnostic severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoctorStatus {
    Ok,
    Warning,
    Error,
}

/// Local diagnostic report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorResult {
    pub checks: Vec<DoctorCheck>,
}

/// Runs non-mutating diagnostics for config, paths, and platform support.
pub fn inspect(request: DoctorRequest) -> Result<DoctorResult> {
    let mut checks = Vec::new();
    checks.push(platform_check());
    checks.push(home_check(&request.home_dir));
    checks.push(project_config_check(&request.start_dir));
    checks.push(global_config_check(&request.start_dir, &request.home_dir)?);
    checks.push(still_paths_check(&request.home_dir));

    Ok(DoctorResult { checks })
}

fn platform_check() -> DoctorCheck {
    DoctorCheck {
        name: "platform".to_string(),
        status: DoctorStatus::Ok,
        detail: format!("detected {}", current_platform()),
    }
}

fn home_check(home_dir: &PathBuf) -> DoctorCheck {
    DoctorCheck {
        name: "home".to_string(),
        status: if home_dir.is_dir() {
            DoctorStatus::Ok
        } else {
            DoctorStatus::Error
        },
        detail: home_dir.display().to_string(),
    }
}

fn project_config_check(start_dir: &std::path::Path) -> DoctorCheck {
    match find_project_config(start_dir) {
        Some(path) => DoctorCheck {
            name: "project config".to_string(),
            status: DoctorStatus::Ok,
            detail: path.display().to_string(),
        },
        None => DoctorCheck {
            name: "project config".to_string(),
            status: DoctorStatus::Warning,
            detail: "no still.toml found; project commands will use global config where allowed"
                .to_string(),
        },
    }
}

fn global_config_check(
    start_dir: &std::path::Path,
    home_dir: &std::path::Path,
) -> Result<DoctorCheck> {
    let resolved = resolve_config_path(
        start_dir,
        home_dir,
        ConfigSelection {
            scope: ConfigScope::Global,
            for_write: false,
        },
    )?;
    Ok(DoctorCheck {
        name: "global config".to_string(),
        status: if resolved.path.is_file() {
            DoctorStatus::Ok
        } else {
            DoctorStatus::Warning
        },
        detail: resolved.path.display().to_string(),
    })
}

fn still_paths_check(home_dir: &std::path::Path) -> DoctorCheck {
    let root = if cfg!(target_os = "macos") {
        PathBuf::from("/opt").join("still")
    } else if cfg!(target_os = "windows") {
        home_dir.join("AppData").join("Local").join("still")
    } else {
        home_dir.join(".local").join("share").join("still")
    };
    DoctorCheck {
        name: "still root".to_string(),
        status: DoctorStatus::Ok,
        detail: root.display().to_string(),
    }
}

fn current_platform() -> PlatformId {
    if cfg!(target_os = "macos") {
        PlatformId::Macos
    } else if cfg!(target_os = "windows") {
        PlatformId::Windows
    } else {
        PlatformId::Linux
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn doctor_reports_existing_project_and_global_config() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("still.toml"), "[tools]\n").unwrap();
        let global = temp.path().join(".config/still/config.toml");
        fs::create_dir_all(global.parent().unwrap()).unwrap();
        fs::write(&global, "[tools]\n").unwrap();

        let result = inspect(DoctorRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
        })
        .unwrap();

        assert!(
            result.checks.iter().any(|check| {
                check.name == "project config" && check.status == DoctorStatus::Ok
            })
        );
        assert!(
            result
                .checks
                .iter()
                .any(|check| { check.name == "global config" && check.status == DoctorStatus::Ok })
        );
    }

    #[test]
    fn doctor_warns_when_project_config_is_missing() {
        let temp = tempfile::tempdir().unwrap();

        let result = inspect(DoctorRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
        })
        .unwrap();

        assert!(result.checks.iter().any(|check| {
            check.name == "project config" && check.status == DoctorStatus::Warning
        }));
    }
}
