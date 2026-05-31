//! Runtime boundary between CLI handlers and engine actions.

use engine::actions::{
    activate::{ActivateRequest, ActivateResult},
    agents::{AgentsOperation, AgentsRequest, AgentsResult},
    config::{CheckConfigRequest, CheckConfigResult},
    doctor::{DoctorRequest, DoctorResult},
    env::{EnvRequest, EnvResult},
    init::{InitRequest, InitResult},
    install::{InstallRequest, InstallResult},
    list::{ListRequest, ListResult},
    run::{RunRequest, RunResult},
    services::{ServicesOperation, ServicesRequest, ServicesResult},
    sync::{SyncRequest, SyncResult},
    task::{TaskRequest, TaskResult},
    trust::{TrustRequest, TrustResult},
    uninstall::{UninstallRequest, UninstallResult, UninstallTarget},
};
use engine::config::{ConfigScope, ConfigSelection, resolve_config_path};
use engine::lockfile::lockfile_path;

/// CLI install request plus desired-state write scope.
#[derive(Debug, Clone)]
pub struct InstallCommandRequest {
    pub install: InstallRequest,
    pub global: bool,
    pub force: bool,
}

/// Operations command handlers need from the engine layer.
///
/// This trait is the test seam between parsed CLI commands and real engine side
/// effects. Add methods here when a command needs engine behavior; keep Clap
/// types and user-facing formatting out of the engine-facing request structs.
pub trait CliRuntime {
    /// Installs a requested item through the engine.
    ///
    /// `request` is the command-neutral install request built by the CLI handler.
    /// Implementations return the engine result on success or an error that the
    /// handler will format for the user.
    fn install(&mut self, request: InstallCommandRequest) -> anyhow::Result<InstallResult>;

    /// Checks the selected config file through the engine.
    fn config_check(&mut self, global: bool) -> anyhow::Result<CheckConfigResult>;

    /// Initializes a starter config in the current project directory.
    fn init(&mut self, force: bool) -> anyhow::Result<InitResult>;

    /// Reads configured environment values.
    fn env(&mut self, global: bool) -> anyhow::Result<EnvResult>;

    /// Lists configured desired-state items.
    fn list(&mut self, all: bool, global: bool) -> anyhow::Result<ListResult>;

    /// Inspects or syncs configured agent state.
    fn agents(&mut self, operation: AgentsOperation, global: bool) -> anyhow::Result<AgentsResult>;

    /// Runs a child command inside the managed environment.
    fn run_command(&mut self, command: Vec<String>, global: bool) -> anyhow::Result<RunResult>;

    /// Lists or runs configured tasks.
    fn task(&mut self, name: Option<String>, global: bool) -> anyhow::Result<TaskResult>;

    /// Generates shell activation code.
    fn activate(&mut self, shell: Option<String>, global: bool) -> anyhow::Result<ActivateResult>;

    /// Runs local diagnostics.
    fn doctor(&mut self) -> anyhow::Result<DoctorResult>;

    /// Plans synchronization against configured desired state.
    fn sync(&mut self, global: bool) -> anyhow::Result<SyncResult>;

    /// Inspects or runs configured service actions.
    fn services(
        &mut self,
        operation: ServicesOperation,
        name: Option<String>,
        global: bool,
    ) -> anyhow::Result<ServicesResult>;

    /// Marks the current project config trusted.
    fn trust(&mut self) -> anyhow::Result<TrustResult>;

    /// Removes an item from desired state.
    fn uninstall(
        &mut self,
        target: UninstallTarget,
        global: bool,
    ) -> anyhow::Result<UninstallResult>;
}

/// Production runtime implementation that calls real engine actions.
///
/// The type carries no state today, but it owns the decision to bridge sync CLI
/// handlers into async engine actions.
#[derive(Debug, Default)]
pub struct RealRuntime;

impl CliRuntime for RealRuntime {
    fn install(&mut self, request: InstallCommandRequest) -> anyhow::Result<InstallResult> {
        let install_request = request.install.clone();
        let pending_config =
            prepare_install_config_write(&install_request, request.global, request.force)?;
        let runtime = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        let result = runtime.block_on(engine::actions::install::run(request.install))?;
        if let Err(err) = record_install_items(pending_config) {
            if let Err(rollback_err) = runtime.block_on(
                engine::actions::install::rollback_installed_items(&result.installed),
            ) {
                return Err(err.context(format!(
                    "failed to roll back installed artifacts after config recording failed: {rollback_err}"
                )));
            }
            return Err(
                err.context("failed to record installed items; rolled back installed artifacts")
            );
        }
        Ok(result)
    }

    fn config_check(&mut self, global: bool) -> anyhow::Result<CheckConfigResult> {
        let start_dir = std::env::current_dir()?;
        let home_dir =
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("failed to find home directory"))?;
        let request = CheckConfigRequest {
            start_dir,
            home_dir,
            global,
        };
        let runtime = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        runtime.block_on(engine::actions::config::check(request))
    }

    fn init(&mut self, force: bool) -> anyhow::Result<InitResult> {
        let start_dir = std::env::current_dir()?;
        let request = InitRequest { start_dir, force };
        let runtime = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        runtime.block_on(engine::actions::init::run(request))
    }

    fn env(&mut self, global: bool) -> anyhow::Result<EnvResult> {
        let start_dir = std::env::current_dir()?;
        let home_dir =
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("failed to find home directory"))?;
        let request = EnvRequest {
            start_dir,
            home_dir,
            global,
        };
        let runtime = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        runtime.block_on(engine::actions::env::inspect(request))
    }

    fn list(&mut self, all: bool, global: bool) -> anyhow::Result<ListResult> {
        let start_dir = std::env::current_dir()?;
        let home_dir =
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("failed to find home directory"))?;
        let request = ListRequest {
            start_dir,
            home_dir,
            global,
            all,
        };
        let runtime = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        runtime.block_on(engine::actions::list::inspect(request))
    }

    fn agents(&mut self, operation: AgentsOperation, global: bool) -> anyhow::Result<AgentsResult> {
        let start_dir = std::env::current_dir()?;
        let home_dir =
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("failed to find home directory"))?;
        let request = AgentsRequest {
            start_dir,
            home_dir,
            global,
            operation,
        };
        let runtime = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        runtime.block_on(engine::actions::agents::run(request))
    }

    fn run_command(&mut self, command: Vec<String>, global: bool) -> anyhow::Result<RunResult> {
        let start_dir = std::env::current_dir()?;
        let home_dir =
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("failed to find home directory"))?;
        let request = RunRequest {
            start_dir,
            home_dir,
            global,
            command,
        };
        let runtime = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        runtime.block_on(engine::actions::run::run(request))
    }

    fn task(&mut self, name: Option<String>, global: bool) -> anyhow::Result<TaskResult> {
        let start_dir = std::env::current_dir()?;
        let home_dir =
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("failed to find home directory"))?;
        let request = TaskRequest {
            start_dir,
            home_dir,
            global,
            name,
        };
        let runtime = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        runtime.block_on(engine::actions::task::run(request))
    }

    fn activate(&mut self, shell: Option<String>, global: bool) -> anyhow::Result<ActivateResult> {
        let start_dir = std::env::current_dir()?;
        let home_dir =
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("failed to find home directory"))?;
        let request = ActivateRequest {
            start_dir,
            home_dir,
            global,
            shell,
        };
        let runtime = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        runtime.block_on(engine::actions::activate::run(request))
    }

    fn doctor(&mut self) -> anyhow::Result<DoctorResult> {
        let start_dir = std::env::current_dir()?;
        let home_dir =
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("failed to find home directory"))?;
        engine::actions::doctor::inspect(DoctorRequest {
            start_dir,
            home_dir,
        })
    }

    fn sync(&mut self, global: bool) -> anyhow::Result<SyncResult> {
        let start_dir = std::env::current_dir()?;
        let home_dir =
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("failed to find home directory"))?;
        let request = SyncRequest {
            start_dir,
            home_dir,
            global,
        };
        let runtime = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        runtime.block_on(engine::actions::sync::run(request))
    }

    fn services(
        &mut self,
        operation: ServicesOperation,
        name: Option<String>,
        global: bool,
    ) -> anyhow::Result<ServicesResult> {
        let start_dir = std::env::current_dir()?;
        let home_dir =
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("failed to find home directory"))?;
        let request = ServicesRequest {
            start_dir,
            home_dir,
            global,
            operation,
            name,
        };
        let runtime = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        runtime.block_on(engine::actions::services::run(request))
    }

    fn trust(&mut self) -> anyhow::Result<TrustResult> {
        let start_dir = std::env::current_dir()?;
        let home_dir =
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("failed to find home directory"))?;
        let request = TrustRequest {
            start_dir,
            home_dir,
        };
        let runtime = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        runtime.block_on(engine::actions::trust::run(request))
    }

    fn uninstall(
        &mut self,
        target: UninstallTarget,
        global: bool,
    ) -> anyhow::Result<UninstallResult> {
        let start_dir = std::env::current_dir()?;
        let home_dir =
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("failed to find home directory"))?;
        let request = UninstallRequest {
            start_dir,
            home_dir,
            global,
            target,
        };
        let runtime = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        runtime.block_on(engine::actions::uninstall::run(request))
    }
}

#[derive(Debug, Clone)]
struct PendingInstallConfigWrite {
    path: std::path::PathBuf,
    home_dir: std::path::PathBuf,
    original: Option<String>,
    updated: String,
}

fn prepare_install_config_write(
    request: &InstallRequest,
    global: bool,
    force: bool,
) -> anyhow::Result<PendingInstallConfigWrite> {
    let start_dir = std::env::current_dir()?;
    let home_dir =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("failed to find home directory"))?;
    prepare_install_config_write_at(request, global, force, &start_dir, &home_dir)
}

fn prepare_install_config_write_at(
    request: &InstallRequest,
    global: bool,
    force: bool,
    start_dir: &std::path::Path,
    home_dir: &std::path::Path,
) -> anyhow::Result<PendingInstallConfigWrite> {
    let resolved = resolve_config_path(
        start_dir,
        home_dir,
        ConfigSelection {
            scope: if global {
                ConfigScope::Global
            } else {
                ConfigScope::Project
            },
            for_write: false,
        },
    )?;

    let original = match std::fs::read_to_string(&resolved.path) {
        Ok(content) => Some(content),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => return Err(err.into()),
    };
    let content = original.as_deref().unwrap_or("");
    let updated =
        engine::config_edit::add_install_items_with_force(content, &request.items, force)?;
    Ok(PendingInstallConfigWrite {
        path: resolved.path,
        home_dir: home_dir.to_path_buf(),
        original,
        updated,
    })
}

fn record_install_items(pending: PendingInstallConfigWrite) -> anyhow::Result<()> {
    if let Some(parent) = pending.path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let lockfile_path = lockfile_path(&pending.path);
    let original_lockfile = read_optional_file(&lockfile_path)?;
    std::fs::write(&pending.path, pending.updated)?;
    let runtime = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    if let Err(err) = runtime.block_on(engine::actions::sync::refresh_active_lockfile(
        &pending.path,
        &pending.home_dir,
    )) {
        restore_install_config(&pending.path, pending.original)?;
        restore_install_lockfile(&lockfile_path, original_lockfile)?;
        return Err(err);
    }
    Ok(())
}

fn read_optional_file(path: &std::path::Path) -> anyhow::Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(content) => Ok(Some(content)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err.into()),
    }
}

fn restore_install_config(path: &std::path::Path, original: Option<String>) -> anyhow::Result<()> {
    match original {
        Some(original) => std::fs::write(path, original)?,
        None if path.exists() => std::fs::remove_file(path)?,
        None => {}
    }
    Ok(())
}

fn restore_install_lockfile(
    path: &std::path::Path,
    original: Option<String>,
) -> anyhow::Result<()> {
    match original {
        Some(original) => std::fs::write(path, original)?,
        None if path.exists() => std::fs::remove_file(path)?,
        None => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use engine::actions::install::InstallItemRequest;
    use engine::specs::item::{ItemKind, ItemSpec};

    use super::*;

    #[test]
    fn install_config_preflight_rejects_changed_existing_entries_before_write() {
        let temp = test_dir("install-preflight-conflict");
        let config_path = temp.join("still.toml");
        fs::write(
            &config_path,
            r#"
            [tools]
            rust = "stable"
            "#,
        )
        .unwrap();
        let request = InstallRequest {
            items: vec![install_item(
                ItemKind::Tool,
                "rust",
                "1.76.0",
                Some("rustup"),
            )],
        };

        let err =
            prepare_install_config_write_at(&request, false, false, &temp, &temp).unwrap_err();

        assert!(err.to_string().contains("--force"));
        assert_eq!(
            fs::read_to_string(config_path).unwrap(),
            r#"
            [tools]
            rust = "stable"
            "#
        );
    }

    #[test]
    fn install_config_preflight_uses_global_when_no_project_config_exists() {
        let temp = test_dir("install-preflight-global");
        let request = InstallRequest {
            items: vec![install_item(ItemKind::Package, "openssl", "latest", None)],
        };

        let pending =
            prepare_install_config_write_at(&request, false, false, &temp, &temp).unwrap();

        assert_eq!(pending.path, temp.join(".config/still/config.toml"));
        assert!(pending.updated.contains("latest = [\"openssl\"]"));
    }

    #[test]
    fn install_recording_does_not_mutate_config_when_lockfile_backup_fails() {
        let temp = test_dir("install-recording-preflight");
        let config_path = temp.join("still.toml");
        let original = "[tools]\nrust = \"stable\"\n";
        fs::write(&config_path, original).unwrap();
        let lockfile_path = temp.join("still.lock.toml");
        fs::create_dir(&lockfile_path).unwrap();

        let err = record_install_items(PendingInstallConfigWrite {
            path: config_path.clone(),
            home_dir: temp.clone(),
            original: Some(original.to_string()),
            updated: "[tools]\nrust = \"stable\"\nnode = \"latest\"\n".to_string(),
        })
        .unwrap_err();

        assert!(!err.to_string().is_empty());
        assert_eq!(fs::read_to_string(config_path).unwrap(), original);
        assert!(lockfile_path.is_dir());
    }

    #[test]
    fn install_recording_restores_lockfile_when_refresh_fails() {
        let temp = test_dir("install-lockfile-rollback");
        let config_path = temp.join("still.toml");
        let lockfile_path = temp.join("still.lock.toml");
        let original_config = "[tools]\nrust = \"stable\"\n";
        let original_lockfile = "# original lockfile\n";
        fs::write(&config_path, original_config).unwrap();
        fs::write(&lockfile_path, original_lockfile).unwrap();

        let err = record_install_items(PendingInstallConfigWrite {
            path: config_path.clone(),
            home_dir: temp.clone(),
            original: Some(original_config.to_string()),
            updated: "[tools]\nrust = {}\n".to_string(),
        })
        .unwrap_err();

        assert!(
            err.to_string()
                .contains("tool \"rust\" must define version")
        );
        assert_eq!(fs::read_to_string(config_path).unwrap(), original_config);
        assert_eq!(
            fs::read_to_string(lockfile_path).unwrap(),
            original_lockfile
        );
    }

    #[test]
    fn install_restore_lockfile_puts_original_content_back() {
        let temp = test_dir("install-lockfile-restore-helper");
        let lockfile_path = temp.join("still.lock.toml");
        fs::write(&lockfile_path, "# updated\n").unwrap();

        restore_install_lockfile(&lockfile_path, Some("# original\n".to_string())).unwrap();

        assert_eq!(fs::read_to_string(lockfile_path).unwrap(), "# original\n");
    }

    fn install_item(
        kind: ItemKind,
        name: &str,
        version: &str,
        backend: Option<&str>,
    ) -> InstallItemRequest {
        InstallItemRequest {
            kind,
            spec: ItemSpec {
                name: name.to_string(),
                version: version.parse().unwrap(),
                backend: backend.map(|backend| backend.parse().unwrap()),
            },
            tool: Default::default(),
        }
    }

    fn test_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("still-{name}-{suffix}"));
        fs::create_dir_all(&path).unwrap();
        path
    }
}
