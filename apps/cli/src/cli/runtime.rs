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
    uninstall::{UninstallRequest, UninstallResult},
};
use engine::config::{ConfigScope, ConfigSelection, resolve_config_path};
use engine::config_edit::add_install_items;

/// CLI install request plus desired-state write scope.
#[derive(Debug, Clone)]
pub struct InstallCommandRequest {
    pub install: InstallRequest,
    pub global: bool,
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
    fn list(&mut self, all: bool) -> anyhow::Result<ListResult>;

    /// Inspects or syncs configured agent state.
    fn agents(&mut self, operation: AgentsOperation) -> anyhow::Result<AgentsResult>;

    /// Runs a child command inside the managed environment.
    fn run_command(&mut self, command: Vec<String>) -> anyhow::Result<RunResult>;

    /// Lists or runs configured tasks.
    fn task(&mut self, name: Option<String>) -> anyhow::Result<TaskResult>;

    /// Generates shell activation code.
    fn activate(&mut self, shell: Option<String>) -> anyhow::Result<ActivateResult>;

    /// Runs local diagnostics.
    fn doctor(&mut self) -> anyhow::Result<DoctorResult>;

    /// Plans synchronization against configured desired state.
    fn sync(&mut self) -> anyhow::Result<SyncResult>;

    /// Inspects or runs configured service actions.
    fn services(
        &mut self,
        operation: ServicesOperation,
        name: Option<String>,
    ) -> anyhow::Result<ServicesResult>;

    /// Marks the current project config trusted.
    fn trust(&mut self) -> anyhow::Result<TrustResult>;

    /// Removes an item from desired state.
    fn uninstall(&mut self, name: String) -> anyhow::Result<UninstallResult>;
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
        let runtime = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        let result = runtime.block_on(engine::actions::install::run(request.install))?;
        record_install_items(install_request, request.global)?;
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

    fn list(&mut self, all: bool) -> anyhow::Result<ListResult> {
        let start_dir = std::env::current_dir()?;
        let home_dir =
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("failed to find home directory"))?;
        let request = ListRequest {
            start_dir,
            home_dir,
            global: false,
            all,
        };
        let runtime = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        runtime.block_on(engine::actions::list::inspect(request))
    }

    fn agents(&mut self, operation: AgentsOperation) -> anyhow::Result<AgentsResult> {
        let start_dir = std::env::current_dir()?;
        let home_dir =
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("failed to find home directory"))?;
        let request = AgentsRequest {
            start_dir,
            home_dir,
            operation,
        };
        let runtime = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        runtime.block_on(engine::actions::agents::run(request))
    }

    fn run_command(&mut self, command: Vec<String>) -> anyhow::Result<RunResult> {
        let start_dir = std::env::current_dir()?;
        let home_dir =
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("failed to find home directory"))?;
        let request = RunRequest {
            start_dir,
            home_dir,
            command,
        };
        let runtime = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        runtime.block_on(engine::actions::run::run(request))
    }

    fn task(&mut self, name: Option<String>) -> anyhow::Result<TaskResult> {
        let start_dir = std::env::current_dir()?;
        let home_dir =
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("failed to find home directory"))?;
        let request = TaskRequest {
            start_dir,
            home_dir,
            name,
        };
        let runtime = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        runtime.block_on(engine::actions::task::run(request))
    }

    fn activate(&mut self, shell: Option<String>) -> anyhow::Result<ActivateResult> {
        let home_dir =
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("failed to find home directory"))?;
        Ok(engine::actions::activate::run(ActivateRequest {
            home_dir,
            shell,
        })?)
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

    fn sync(&mut self) -> anyhow::Result<SyncResult> {
        let start_dir = std::env::current_dir()?;
        let home_dir =
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("failed to find home directory"))?;
        let request = SyncRequest {
            start_dir,
            home_dir,
        };
        let runtime = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        runtime.block_on(engine::actions::sync::plan(request))
    }

    fn services(
        &mut self,
        operation: ServicesOperation,
        name: Option<String>,
    ) -> anyhow::Result<ServicesResult> {
        let start_dir = std::env::current_dir()?;
        let home_dir =
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("failed to find home directory"))?;
        let request = ServicesRequest {
            start_dir,
            home_dir,
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

    fn uninstall(&mut self, name: String) -> anyhow::Result<UninstallResult> {
        let start_dir = std::env::current_dir()?;
        let home_dir =
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("failed to find home directory"))?;
        let request = UninstallRequest {
            start_dir,
            home_dir,
            name,
        };
        let runtime = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        runtime.block_on(engine::actions::uninstall::run(request))
    }
}

fn record_install_items(request: InstallRequest, global: bool) -> anyhow::Result<()> {
    let start_dir = std::env::current_dir()?;
    let home_dir =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("failed to find home directory"))?;
    let resolved = resolve_config_path(
        &start_dir,
        &home_dir,
        ConfigSelection {
            scope: if global {
                ConfigScope::Global
            } else {
                ConfigScope::Project
            },
            for_write: false,
        },
    )?;

    let content = match std::fs::read_to_string(&resolved.path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err.into()),
    };
    let updated = add_install_items(&content, &request.items)?;
    if let Some(parent) = resolved.path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&resolved.path, updated)?;
    Ok(())
}
