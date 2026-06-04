//! Engine action for local Still diagnostics.

use std::path::{Path, PathBuf};

use crate::error::Result;

use crate::config::{
    ConfigScope, ConfigSelection, find_project_config, global_config_path, resolve_config_path,
};
use crate::lockfile::{lockfile_path, render_merged_lockfile_for_config, validate_lockfile};
use crate::platform::current_platform;
use crate::specs::toml::parse_still_toml;
use crate::trust::{TrustMarker, config_fingerprint, trust_marker_path};

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
        checks.push(trust_check(path));
        checks.push(lockfile_check(path, &request.home_dir));
    }
    checks.push(global_config_check(&request.start_dir, &request.home_dir)?);
    let global_config = global_config_path(&request.home_dir);
    if global_config.is_file() {
        checks.push(config_parse_check("global config syntax", &global_config));
        checks.push(named_lockfile_check(
            "global lockfile",
            &global_config,
            &request.home_dir,
        ));
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

fn lockfile_check(config_path: &Path, home_dir: &Path) -> DoctorCheck {
    named_lockfile_check("lockfile", config_path, home_dir)
}

fn named_lockfile_check(name: &str, config_path: &Path, home_dir: &Path) -> DoctorCheck {
    let path = lockfile_path(config_path);
    if !path.exists() {
        return DoctorCheck {
            name: name.to_string(),
            status: DoctorStatus::Warning,
            detail: format!("{} is missing; run `still sync`", path.display()),
        };
    }

    match std::fs::read_to_string(&path) {
        Ok(content) => {
            if let Err(err) = validate_lockfile(&content) {
                return DoctorCheck {
                    name: name.to_string(),
                    status: DoctorStatus::Error,
                    detail: format!("{}: {err}", path.display()),
                };
            }
            match expected_lockfile(config_path, home_dir, &content) {
                Ok(expected) if expected == content => DoctorCheck {
                    name: name.to_string(),
                    status: DoctorStatus::Ok,
                    detail: path.display().to_string(),
                },
                Ok(_) => DoctorCheck {
                    name: name.to_string(),
                    status: DoctorStatus::Warning,
                    detail: format!("{} is outdated; run `still sync`", path.display()),
                },
                Err(err) => DoctorCheck {
                    name: name.to_string(),
                    status: DoctorStatus::Error,
                    detail: format!("failed to check {} drift: {err}", path.display()),
                },
            }
        }
        Err(err) => DoctorCheck {
            name: name.to_string(),
            status: DoctorStatus::Error,
            detail: format!("failed to read {}: {err}", path.display()),
        },
    }
}

fn expected_lockfile(
    config_path: &Path,
    home_dir: &Path,
    existing_lockfile: &str,
) -> Result<String> {
    let content = std::fs::read_to_string(config_path)?;
    let global_path = global_config_path(home_dir);
    let items = if config_path == global_path {
        let config = parse_still_toml(&content)?;
        crate::actions::sync::sync_items(config)?
    } else {
        let global_content = match std::fs::read_to_string(&global_path) {
            Ok(content) => Some(content),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
            Err(err) => return Err(err.into()),
        };
        crate::actions::sync::sync_items_for_active_project_config(
            &content,
            global_content.as_deref(),
        )?
    };
    let config = parse_still_toml(&content)?;
    render_merged_lockfile_for_config(Some(existing_lockfile), &items, &config, config_path)
}

fn trust_check(config_path: &Path) -> DoctorCheck {
    let marker_path = trust_marker_path(config_path);
    let content = match std::fs::read(config_path) {
        Ok(content) => content,
        Err(err) => {
            return DoctorCheck {
                name: "trust".to_string(),
                status: DoctorStatus::Error,
                detail: format!("failed to read {}: {err}", config_path.display()),
            };
        }
    };
    let marker_content = match std::fs::read_to_string(&marker_path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return DoctorCheck {
                name: "trust".to_string(),
                status: DoctorStatus::Warning,
                detail: format!("{} is missing; run `still trust`", marker_path.display()),
            };
        }
        Err(err) => {
            return DoctorCheck {
                name: "trust".to_string(),
                status: DoctorStatus::Error,
                detail: format!("failed to read {}: {err}", marker_path.display()),
            };
        }
    };
    let marker = match toml::from_str::<TrustMarker>(&marker_content) {
        Ok(marker) => marker,
        Err(err) => {
            return DoctorCheck {
                name: "trust".to_string(),
                status: DoctorStatus::Error,
                detail: format!("{}: {err}", marker_path.display()),
            };
        }
    };

    let expected_config = config_path.display().to_string();
    if marker.config != expected_config || marker.fingerprint != config_fingerprint(&content) {
        return DoctorCheck {
            name: "trust".to_string(),
            status: DoctorStatus::Warning,
            detail: format!("{} is stale; run `still trust`", marker_path.display()),
        };
    }

    DoctorCheck {
        name: "trust".to_string(),
        status: DoctorStatus::Ok,
        detail: marker_path.display().to_string(),
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
    platform_paths_for_host(home_dir)
}

#[cfg(target_os = "macos")]
fn platform_paths_for_host(home_dir: &Path) -> DoctorPaths {
    DoctorPaths {
        root: PathBuf::from("/opt").join("still"),
        cache: home_dir.join("Library").join("Caches").join("still"),
        bin: PathBuf::from("/opt").join("still").join("bin"),
        config_dir: home_dir.join(".config").join("still"),
    }
}

#[cfg(target_os = "linux")]
fn platform_paths_for_host(home_dir: &Path) -> DoctorPaths {
    let root = home_dir.join(".local").join("share").join("still");
    DoctorPaths {
        root,
        cache: home_dir.join(".cache").join("still"),
        bin: home_dir.join(".local").join("bin"),
        config_dir: home_dir.join(".config").join("still"),
    }
}

#[cfg(target_os = "windows")]
fn platform_paths_for_host(home_dir: &Path) -> DoctorPaths {
    let root = home_dir.join("AppData").join("Local").join("still");
    DoctorPaths {
        root: root.clone(),
        cache: root.join("cache"),
        bin: root.join("bin"),
        config_dir: home_dir.join("AppData").join("Roaming").join("still"),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::lockfile::render_merged_lockfile;
    use crate::trust::{config_fingerprint, trust_marker_path};

    use super::*;

    #[test]
    fn doctor_reports_existing_project_and_global_config() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("still.toml"), "[tools]\n").unwrap();
        fs::write(
            temp.path().join("still.lock.toml"),
            render_merged_lockfile(None, &[]),
        )
        .unwrap();
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
                .any(|check| { check.name == "trust" && check.status == DoctorStatus::Warning })
        );
        assert!(
            result
                .checks
                .iter()
                .any(|check| { check.name == "lockfile" && check.status == DoctorStatus::Ok })
        );
        assert!(result.checks.iter().any(|check| {
            check.name == "global lockfile" && check.status == DoctorStatus::Warning
        }));
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
    fn doctor_reports_trusted_project_config() {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("still.toml");
        let content = b"[tasks]\ntest = \"cargo test\"\n";
        fs::write(&config_path, content).unwrap();
        write_trust_marker(&config_path, content);

        let result = inspect(DoctorRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
        })
        .unwrap();

        let check = result
            .checks
            .iter()
            .find(|check| check.name == "trust")
            .unwrap();
        assert_eq!(check.status, DoctorStatus::Ok);
        assert!(check.detail.contains(".still"));
    }

    #[test]
    fn doctor_warns_when_trust_marker_is_stale() {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("still.toml");
        fs::write(&config_path, "[tasks]\ntest = \"cargo test\"\n").unwrap();
        write_trust_marker(&config_path, b"old");

        let result = inspect(DoctorRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
        })
        .unwrap();

        let check = result
            .checks
            .iter()
            .find(|check| check.name == "trust")
            .unwrap();
        assert_eq!(check.status, DoctorStatus::Warning);
        assert!(check.detail.contains("still trust"));
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
    fn doctor_reports_invalid_project_lockfile_schema() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("still.toml"), "[tools]\n").unwrap();
        fs::write(
            temp.path().join("still.lock.toml"),
            r#"
            [[items]]
            kind = "plugin"
            name = "rust"
            platform = "linux"
            version = "stable"
            source = "backend:auto"
            checksum = "abc123"
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
            .find(|check| check.name == "lockfile")
            .unwrap();
        assert_eq!(check.status, DoctorStatus::Error);
        assert!(check.detail.contains("invalid kind"));
    }

    #[test]
    fn doctor_reports_invalid_global_lockfile_schema() {
        let temp = tempfile::tempdir().unwrap();
        let global = temp.path().join(".config/still/config.toml");
        fs::create_dir_all(global.parent().unwrap()).unwrap();
        fs::write(&global, "[tools]\n").unwrap();
        fs::write(
            global.parent().unwrap().join("still.lock.toml"),
            r#"
            [[items]]
            kind = "package"
            name = "openssl"
            platform = "linux"
            version = ""
            source = "backend:auto"
            checksum = "abc123"
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
            .find(|check| check.name == "global lockfile")
            .unwrap();
        assert_eq!(check.status, DoctorStatus::Error);
        assert!(check.detail.contains("empty version"));
    }

    #[test]
    fn doctor_reports_invalid_lockfile_checksum() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("still.toml"),
            "[packages]\nlatest = [\"openssl\"]\n",
        )
        .unwrap();
        fs::write(
            temp.path().join("still.lock.toml"),
            r#"
            [[items]]
            kind = "package"
            name = "openssl"
            platform = "linux"
            version = "latest"
            source = "backend:auto"
            checksum = "abc123"
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
            .find(|check| check.name == "lockfile")
            .unwrap();
        assert_eq!(check.status, DoctorStatus::Error);
        assert!(check.detail.contains("invalid checksum"));
    }

    #[test]
    fn doctor_warns_when_lockfile_is_outdated() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("still.toml"),
            "[packages]\nlatest = [\"openssl\"]\n",
        )
        .unwrap();
        fs::write(
            temp.path().join("still.lock.toml"),
            render_merged_lockfile(None, &[]),
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
            .find(|check| check.name == "lockfile")
            .unwrap();
        assert_eq!(check.status, DoctorStatus::Warning);
        assert!(check.detail.contains("outdated"));
    }

    #[tokio::test]
    async fn doctor_accepts_project_lockfile_with_global_only_state() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("repo");
        let global = temp.path().join(".config/still/config.toml");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(global.parent().unwrap()).unwrap();
        fs::write(project.join("still.toml"), "[tools]\nrust = \"stable\"\n").unwrap();
        fs::write(&global, "[tools]\nrust = \"1.80.0\"\nnode = \"22\"\n").unwrap();
        crate::actions::sync::refresh_active_lockfile(&project.join("still.toml"), temp.path())
            .await
            .unwrap();

        let result = inspect(DoctorRequest {
            start_dir: project,
            home_dir: temp.path().to_path_buf(),
        })
        .unwrap();

        let check = result
            .checks
            .iter()
            .find(|check| check.name == "lockfile")
            .unwrap();
        assert_eq!(check.status, DoctorStatus::Ok);
    }

    #[test]
    fn doctor_warns_when_global_lockfile_is_outdated() {
        let temp = tempfile::tempdir().unwrap();
        let global = temp.path().join(".config/still/config.toml");
        fs::create_dir_all(global.parent().unwrap()).unwrap();
        fs::write(&global, "[packages]\nlatest = [\"openssl\"]\n").unwrap();
        fs::write(
            global.parent().unwrap().join("still.lock.toml"),
            render_merged_lockfile(None, &[]),
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
            .find(|check| check.name == "global lockfile")
            .unwrap();
        assert_eq!(check.status, DoctorStatus::Warning);
        assert!(check.detail.contains("outdated"));
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
