//! Synchronous runtime facade for frontends that call engine actions.

use std::{future::Future, path::PathBuf};

use crate::actions::{
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
use crate::error::{EngineContext, EngineError, Result};

/// Install request plus desired-state write scope.
#[derive(Debug, Clone)]
pub struct ScopedInstallRequest {
    /// Parsed install items to execute.
    pub install: InstallRequest,
    /// Whether the successful request should be recorded in global config.
    pub global: bool,
    /// Whether recording may replace changed existing desired-state entries.
    pub force: bool,
}

/// Operations synchronous frontends need from the engine layer.
///
/// This trait is the test boundary between parsed frontend input and real
/// engine side effects. Keep frontend parser types and user-facing formatting
/// out of these methods.
pub trait ActionRuntime {
    /// Installs requested items and records desired state.
    fn install(&mut self, request: ScopedInstallRequest) -> Result<InstallResult>;

    /// Checks the selected config file.
    fn config_check(&mut self, global: bool) -> Result<CheckConfigResult>;

    /// Initializes a starter config in the current project directory.
    fn init(&mut self, force: bool) -> Result<InitResult>;

    /// Reads configured environment values.
    fn env(&mut self, global: bool) -> Result<EnvResult>;

    /// Lists configured desired-state items.
    fn list(&mut self, all: bool, global: bool) -> Result<ListResult>;

    /// Inspects or syncs configured agent state.
    fn agents(&mut self, operation: AgentsOperation, global: bool) -> Result<AgentsResult>;

    /// Runs a child command inside the managed environment.
    fn run_command(&mut self, command: Vec<String>, global: bool) -> Result<RunResult>;

    /// Lists or runs configured tasks.
    fn task(&mut self, name: Option<String>, global: bool) -> Result<TaskResult>;

    /// Generates shell activation code.
    fn activate(&mut self, shell: Option<String>, global: bool) -> Result<ActivateResult>;

    /// Runs local diagnostics.
    fn doctor(&mut self) -> Result<DoctorResult>;

    /// Synchronizes installed state against configured desired state.
    fn sync(&mut self, global: bool) -> Result<SyncResult>;

    /// Inspects or runs configured service actions.
    fn services(
        &mut self,
        operation: ServicesOperation,
        name: Option<String>,
        global: bool,
    ) -> Result<ServicesResult>;

    /// Marks the current project config trusted.
    fn trust(&mut self) -> Result<TrustResult>;

    /// Removes an item from desired state.
    fn uninstall(&mut self, target: UninstallTarget, global: bool) -> Result<UninstallResult>;
}

/// Production runtime implementation that calls real engine actions.
///
/// The type carries no state today. It owns discovery of the current working
/// directory and home directory, then bridges synchronous frontends into async
/// engine actions with a Tokio runtime.
#[derive(Debug, Default)]
pub struct RealRuntime;

impl ActionRuntime for RealRuntime {
    fn install(&mut self, request: ScopedInstallRequest) -> Result<InstallResult> {
        let context = RuntimeContext::load()?;
        block_on(crate::actions::install::run_and_record(
            InstallAndRecordRequest {
                start_dir: context.start_dir,
                home_dir: context.home_dir,
                global: request.global,
                force: request.force,
                install: request.install,
            },
        ))
    }

    fn config_check(&mut self, global: bool) -> Result<CheckConfigResult> {
        let context = RuntimeContext::load()?;
        block_on(crate::actions::config::check(CheckConfigRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
            global,
        }))
    }

    fn init(&mut self, force: bool) -> Result<InitResult> {
        block_on(crate::actions::init::run(InitRequest {
            start_dir: current_dir()?,
            force,
        }))
    }

    fn env(&mut self, global: bool) -> Result<EnvResult> {
        let context = RuntimeContext::load()?;
        block_on(crate::actions::env::inspect(EnvRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
            global,
        }))
    }

    fn list(&mut self, all: bool, global: bool) -> Result<ListResult> {
        let context = RuntimeContext::load()?;
        block_on(crate::actions::list::inspect(ListRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
            global,
            all,
        }))
    }

    fn agents(&mut self, operation: AgentsOperation, global: bool) -> Result<AgentsResult> {
        let context = RuntimeContext::load()?;
        block_on(crate::actions::agents::run(AgentsRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
            global,
            operation,
        }))
    }

    fn run_command(&mut self, command: Vec<String>, global: bool) -> Result<RunResult> {
        let context = RuntimeContext::load()?;
        block_on(crate::actions::run::run(RunRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
            global,
            command,
        }))
    }

    fn task(&mut self, name: Option<String>, global: bool) -> Result<TaskResult> {
        let context = RuntimeContext::load()?;
        block_on(crate::actions::task::run(TaskRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
            global,
            name,
        }))
    }

    fn activate(&mut self, shell: Option<String>, global: bool) -> Result<ActivateResult> {
        let context = RuntimeContext::load()?;
        block_on(crate::actions::activate::run(ActivateRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
            global,
            shell,
        }))
    }

    fn doctor(&mut self) -> Result<DoctorResult> {
        let context = RuntimeContext::load()?;
        crate::actions::doctor::inspect(DoctorRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
        })
    }

    fn sync(&mut self, global: bool) -> Result<SyncResult> {
        let context = RuntimeContext::load()?;
        block_on(crate::actions::sync::run(SyncRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
            global,
        }))
    }

    fn services(
        &mut self,
        operation: ServicesOperation,
        name: Option<String>,
        global: bool,
    ) -> Result<ServicesResult> {
        let context = RuntimeContext::load()?;
        block_on(crate::actions::services::run(ServicesRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
            global,
            operation,
            name,
        }))
    }

    fn trust(&mut self) -> Result<TrustResult> {
        let context = RuntimeContext::load()?;
        block_on(crate::actions::trust::run(TrustRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
        }))
    }

    fn uninstall(&mut self, target: UninstallTarget, global: bool) -> Result<UninstallResult> {
        let context = RuntimeContext::load()?;
        block_on(crate::actions::uninstall::run(UninstallRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
            global,
            target,
        }))
    }
}

#[derive(Debug, Clone)]
struct RuntimeContext {
    start_dir: PathBuf,
    home_dir: PathBuf,
}

impl RuntimeContext {
    fn load() -> Result<Self> {
        Ok(Self {
            start_dir: current_dir()?,
            home_dir: home_dir()?,
        })
    }
}

fn current_dir() -> Result<PathBuf> {
    std::env::current_dir().context("failed to read current directory")
}

fn home_dir() -> Result<PathBuf> {
    dirs::home_dir().ok_or_else(|| EngineError::message("failed to find home directory"))
}

fn block_on<T>(future: impl Future<Output = Result<T>>) -> Result<T> {
    let runtime = tokio::runtime::Runtime::new().context("failed to create tokio runtime")?;
    runtime.block_on(future)
}
