//! Config discovery and location helpers.

use std::path::{Path, PathBuf};

use crate::error::{EngineError, EngineResult};

/// File name used for project-local desired state.
pub const PROJECT_CONFIG_FILE: &str = "still.toml";

/// Whether a command explicitly selected project or global config.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigScope {
    Project,
    Global,
}

/// Options used when resolving which config a command should use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfigSelection {
    pub scope: ConfigScope,
    pub for_write: bool,
}

/// Resolved config file path and why it was selected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedConfigPath {
    pub scope: ConfigScope,
    pub path: PathBuf,
}

/// Finds the nearest `still.toml` by walking from `start_dir` toward the root.
pub fn find_project_config(start_dir: &Path) -> Option<PathBuf> {
    start_dir
        .ancestors()
        .map(|dir| dir.join(PROJECT_CONFIG_FILE))
        .find(|candidate| candidate.is_file())
}

/// Returns the global Still config path for a supplied home directory.
pub fn global_config_path(home_dir: &Path) -> PathBuf {
    home_dir.join(".config").join("still").join("config.toml")
}

/// Resolves the config path a command should read or mutate.
/// # Errors
/// Fails when project scope was selected for a write and no project config
/// exists. Callers should surface the error with guidance to run `still init` or
/// pass `--global`.
pub fn resolve_config_path(
    start_dir: &Path,
    home_dir: &Path,
    selection: ConfigSelection,
) -> EngineResult<ResolvedConfigPath> {
    match selection.scope {
        ConfigScope::Global => Ok(ResolvedConfigPath {
            scope: ConfigScope::Global,
            path: global_config_path(home_dir),
        }),
        ConfigScope::Project => {
            if let Some(path) = find_project_config(start_dir) {
                return Ok(ResolvedConfigPath {
                    scope: ConfigScope::Project,
                    path,
                });
            }

            if selection.for_write {
                return Err(EngineError::MissingProjectConfig);
            }

            Ok(ResolvedConfigPath {
                scope: ConfigScope::Global,
                path: global_config_path(home_dir),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn finds_nearest_project_config_by_walking_upward() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let project = root.join("repo");
        let nested = project.join("src").join("bin");
        fs::create_dir_all(&nested).unwrap();
        fs::write(project.join(PROJECT_CONFIG_FILE), "").unwrap();

        let found = find_project_config(&nested).unwrap();

        assert_eq!(found, project.join(PROJECT_CONFIG_FILE));
    }

    #[test]
    fn prefers_nested_project_config() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let project = root.join("repo");
        let nested = project.join("workspace");
        fs::create_dir_all(&nested).unwrap();
        fs::write(project.join(PROJECT_CONFIG_FILE), "").unwrap();
        fs::write(nested.join(PROJECT_CONFIG_FILE), "").unwrap();

        let found = find_project_config(&nested).unwrap();

        assert_eq!(found, nested.join(PROJECT_CONFIG_FILE));
    }

    #[test]
    fn resolves_global_when_global_scope_is_selected() {
        let temp = tempfile::tempdir().unwrap();
        let resolved = resolve_config_path(
            temp.path(),
            temp.path(),
            ConfigSelection {
                scope: ConfigScope::Global,
                for_write: true,
            },
        )
        .unwrap();

        assert_eq!(resolved.scope, ConfigScope::Global);
        assert_eq!(resolved.path, temp.path().join(".config/still/config.toml"));
    }

    #[test]
    fn read_without_project_falls_back_to_global() {
        let temp = tempfile::tempdir().unwrap();
        let resolved = resolve_config_path(
            temp.path(),
            temp.path(),
            ConfigSelection {
                scope: ConfigScope::Project,
                for_write: false,
            },
        )
        .unwrap();

        assert_eq!(resolved.scope, ConfigScope::Global);
    }

    #[test]
    fn write_without_project_errors_in_project_scope() {
        let temp = tempfile::tempdir().unwrap();
        let err = resolve_config_path(
            temp.path(),
            temp.path(),
            ConfigSelection {
                scope: ConfigScope::Project,
                for_write: true,
            },
        )
        .unwrap_err();

        assert!(err.to_string().contains("run `still init`"));
        assert!(err.to_string().contains("--global"));
    }
}
