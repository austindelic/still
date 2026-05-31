//! Runtime boundary between CLI handlers and engine actions.

use engine::actions::{
    activate::{ActivateRequest, ActivateResult},
    agents::{AgentsOperation, AgentsRequest, AgentsResult},
    config::{CheckConfigRequest, CheckConfigResult},
    doctor::{DoctorRequest, DoctorResult},
    env::{EnvRequest, EnvResult},
    init::{InitRequest, InitResult},
    install::{InstallAndRecordRequest, InstallRequest, InstallResult},
    list::{ListRequest, ListResult},
    run::{RunRequest, RunResult},
    services::{ServicesOperation, ServicesRequest, ServicesResult},
    sync::{SyncRequest, SyncResult},
    task::{TaskRequest, TaskResult},
    trust::{TrustRequest, TrustResult},
    uninstall::{UninstallRequest, UninstallResult, UninstallTarget},
};

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
        let start_dir = std::env::current_dir()?;
        let home_dir =
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("failed to find home directory"))?;
        let engine_request = InstallAndRecordRequest {
            start_dir,
            home_dir,
            global: request.global,
            force: request.force,
            install: request.install,
        };
        let runtime = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        runtime.block_on(engine::actions::install::run_and_record(engine_request))
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
