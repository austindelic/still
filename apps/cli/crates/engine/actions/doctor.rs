//! Engine action for local Still diagnostics.

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::config::{
    ConfigScope, ConfigSelection, find_project_config, global_config_path, resolve_config_path,
};
use crate::lockfile::lockfile_path;
use crate::platform::{PlatformId, current_platform};
use crate::specs::toml::parse_still_toml;

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
    let paths = platform_paths(&request.home_dir);
    let project_config = find_project_config(&request.start_dir);

    checks.push(platform_check());
    checks.push(home_check(&request.home_dir));
    checks.push(project_config_check(project_config.as_deref()));
    if let Some(path) = project_config.as_deref() {
        checks.push(config_parse_check("project config syntax", path));
        checks.push(lockfile_check(path));
    }
    checks.push(global_config_check(&request.start_dir, &request.home_dir)?);
    let global_config = global_config_path(&request.home_dir);
    if global_config.is_file() {
        checks.push(config_parse_check("global config syntax", &global_config));
    }
    checks.push(path_readiness_check("still root", &paths.root));
    checks.push(path_readiness_check("cache", &paths.cache));
    checks.push(path_readiness_check("bin", &paths.bin));
    checks.push(path_readiness_check("config dir", &paths.config_dir));
    checks.push(path_on_path_check(&paths.bin));

    Ok(DoctorResult { checks })
}

fn platform_check() -> DoctorCheck {
    DoctorCheck {
        name: "platform".to_string(),
        status: DoctorStatus::Ok,
        detail: format!("detected {}", current_platform()),
    }
}

fn home_check(home_dir: &Path) -> DoctorCheck {
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

fn project_config_check(config_path: Option<&Path>) -> DoctorCheck {
    match config_path {
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

fn config_parse_check(name: &str, path: &Path) -> DoctorCheck {
    match std::fs::read_to_string(path) {
        Ok(content) => match parse_still_toml(&content) {
            Ok(_) => DoctorCheck {
                name: name.to_string(),
                status: DoctorStatus::Ok,
                detail: format!("{} parsed successfully", path.display()),
            },
            Err(err) => DoctorCheck {
                name: name.to_string(),
                status: DoctorStatus::Error,
                detail: format!("{}: {err}", path.display()),
            },
        },
        Err(err) => DoctorCheck {
            name: name.to_string(),
            status: DoctorStatus::Error,
            detail: format!("failed to read {}: {err}", path.display()),
        },
    }
}

fn lockfile_check(config_path: &Path) -> DoctorCheck {
    let path = lockfile_path(config_path);
    if !path.exists() {
        return DoctorCheck {
            name: "lockfile".to_string(),
            status: DoctorStatus::Warning,
            detail: format!("{} is missing; run `still sync`", path.display()),
        };
    }

    match std::fs::read_to_string(&path) {
        Ok(content) => match content.parse::<toml_edit::DocumentMut>() {
            Ok(_) => DoctorCheck {
                name: "lockfile".to_string(),
                status: DoctorStatus::Ok,
                detail: path.display().to_string(),
            },
            Err(err) => DoctorCheck {
                name: "lockfile".to_string(),
                status: DoctorStatus::Error,
                detail: format!("{}: {err}", path.display()),
            },
        },
        Err(err) => DoctorCheck {
            name: "lockfile".to_string(),
            status: DoctorStatus::Error,
            detail: format!("failed to read {}: {err}", path.display()),
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

fn path_readiness_check(name: &str, path: &Path) -> DoctorCheck {
    let status = if path.is_dir() {
        if is_readonly(path) {
            DoctorStatus::Error
        } else {
            DoctorStatus::Ok
        }
    } else if nearest_existing_ancestor(path)
        .map(|ancestor| !is_readonly(&ancestor))
        .unwrap_or(false)
    {
        DoctorStatus::Warning
    } else {
        DoctorStatus::Error
    };
    let detail = match status {
        DoctorStatus::Ok => path.display().to_string(),
        DoctorStatus::Warning => format!("{} does not exist yet", path.display()),
        DoctorStatus::Error => format!("{} is not writable or cannot be reached", path.display()),
    }
    .to_string();

    DoctorCheck {
        name: name.to_string(),
        status,
        detail,
    }
}

fn path_on_path_check(bin: &Path) -> DoctorCheck {
    let present = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).any(|entry| entry == bin))
        .unwrap_or(false);
    DoctorCheck {
        name: "PATH".to_string(),
        status: if present {
            DoctorStatus::Ok
        } else {
            DoctorStatus::Warning
        },
        detail: if present {
            format!("{} is on PATH", bin.display())
        } else {
            format!("{} is not on PATH; run `still activate`", bin.display())
        },
    }
}

fn nearest_existing_ancestor(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .find(|candidate| candidate.exists())
        .map(Path::to_path_buf)
}

fn is_readonly(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|metadata| metadata.permissions().readonly())
        .unwrap_or(true)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DoctorPaths {
    root: PathBuf,
    cache: PathBuf,
    bin: PathBuf,
    config_dir: PathBuf,
}

fn platform_paths(home_dir: &Path) -> DoctorPaths {
    match current_platform() {
        PlatformId::Macos => DoctorPaths {
            root: PathBuf::from("/opt").join("still"),
            cache: home_dir.join("Library").join("Caches").join("still"),
            bin: PathBuf::from("/opt").join("still").join("bin"),
            config_dir: home_dir.join(".config").join("still"),
        },
        PlatformId::Linux => {
            let root = home_dir.join(".local").join("share").join("still");
            DoctorPaths {
                root,
                cache: home_dir.join(".cache").join("still"),
                bin: home_dir.join(".local").join("bin"),
                config_dir: home_dir.join(".config").join("still"),
            }
        }
        PlatformId::Windows => {
            let root = home_dir.join("AppData").join("Local").join("still");
            DoctorPaths {
                root: root.clone(),
                cache: root.join("cache"),
                bin: root.join("bin"),
                config_dir: home_dir.join("AppData").join("Roaming").join("still"),
            }
        }
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
        fs::write(temp.path().join("still.lock.toml"), "# generated\n").unwrap();
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
        assert!(result.checks.iter().any(|check| {
            check.name == "project config syntax" && check.status == DoctorStatus::Ok
        }));
        assert!(
            result
                .checks
                .iter()
                .any(|check| { check.name == "lockfile" && check.status == DoctorStatus::Ok })
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

    #[test]
    fn doctor_reports_invalid_project_config() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("still.toml"),
            r#"
            [tools]
            rust = {}
            "#,
        )
        .unwrap();

        let result = inspect(DoctorRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
        })
        .unwrap();

        let check = result
            .checks
            .iter()
            .find(|check| check.name == "project config syntax")
            .unwrap();
        assert_eq!(check.status, DoctorStatus::Error);
        assert!(check.detail.contains("still.toml"));
    }

    #[test]
    fn doctor_warns_when_project_lockfile_is_missing() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("still.toml"), "[tools]\n").unwrap();

        let result = inspect(DoctorRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
        })
        .unwrap();

        let check = result
            .checks
            .iter()
            .find(|check| check.name == "lockfile")
            .unwrap();
        assert_eq!(check.status, DoctorStatus::Warning);
        assert!(check.detail.contains("still sync"));
    }

    #[test]
    fn doctor_reports_path_readiness_and_path_activation() {
        let temp = tempfile::tempdir().unwrap();

        let result = inspect(DoctorRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
        })
        .unwrap();

        for name in ["still root", "cache", "bin", "config dir", "PATH"] {
            assert!(
                result.checks.iter().any(|check| check.name == name),
                "missing {name} check"
            );
        }
    }
}
