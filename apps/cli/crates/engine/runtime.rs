//! Host runtime adapter selected at compile time.

use std::future::Future;
use std::path::PathBuf;

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
use crate::error::{EngineError, Result};

/// Runtime implementation for the target host platform.
pub type Runtime = crate::system::System;

/// Constructs the runtime selected for this build target.
pub fn get_runtime() -> Runtime {
    crate::system::init_system()
}

/// Install request plus desired-state write scope.
#[derive(Debug, Clone)]
pub struct RecordedInstallRequest {
    pub install: InstallRequest,
    pub global: bool,
    pub force: bool,
}

/// Install capability exposed by the host runtime.
pub trait InstallRuntime {
    fn install(&mut self, request: RecordedInstallRequest) -> Result<InstallResult>;
}

/// Config inspection capability exposed by the host runtime.
pub trait ConfigRuntime {
    fn config_check(&mut self, global: bool) -> Result<CheckConfigResult>;
}

/// Project initialization capability exposed by the host runtime.
pub trait InitRuntime {
    fn init(&mut self, force: bool) -> Result<InitResult>;
}

/// Environment inspection capability exposed by the host runtime.
pub trait EnvRuntime {
    fn env(&mut self, global: bool) -> Result<EnvResult>;
}

/// Desired-state listing capability exposed by the host runtime.
pub trait ListRuntime {
    fn list(&mut self, all: bool, global: bool) -> Result<ListResult>;
}

/// Agent management capability exposed by the host runtime.
pub trait AgentsRuntime {
    fn agents(&mut self, operation: AgentsOperation, global: bool) -> Result<AgentsResult>;
}

/// Child process execution capability exposed by the host runtime.
pub trait RunRuntime {
    fn run_command(&mut self, command: Vec<String>, global: bool) -> Result<RunResult>;
}

/// Task capability exposed by the host runtime.
pub trait TaskRuntime {
    fn task(&mut self, name: Option<String>, global: bool) -> Result<TaskResult>;
}

/// Shell activation capability exposed by the host runtime.
pub trait ActivateRuntime {
    fn activate(&mut self, shell: Option<String>, global: bool) -> Result<ActivateResult>;
}

/// Diagnostic capability exposed by the host runtime.
pub trait DoctorRuntime {
    fn doctor(&mut self) -> Result<DoctorResult>;
}

/// Sync capability exposed by the host runtime.
pub trait SyncRuntime {
    fn sync(&mut self, global: bool) -> Result<SyncResult>;
}

/// Service capability exposed by the host runtime.
pub trait ServicesRuntime {
    fn services(
        &mut self,
        operation: ServicesOperation,
        name: Option<String>,
        global: bool,
    ) -> Result<ServicesResult>;
}

/// Trust marker capability exposed by the host runtime.
pub trait TrustRuntime {
    fn trust(&mut self) -> Result<TrustResult>;
}

/// Uninstall capability exposed by the host runtime.
pub trait UninstallRuntime {
    fn uninstall(&mut self, target: UninstallTarget, global: bool) -> Result<UninstallResult>;
}

/// Full host runtime surface used by the CLI adapter.
pub trait RuntimeOps:
    InstallRuntime
    + ConfigRuntime
    + InitRuntime
    + EnvRuntime
    + ListRuntime
    + AgentsRuntime
    + RunRuntime
    + TaskRuntime
    + ActivateRuntime
    + DoctorRuntime
    + SyncRuntime
    + ServicesRuntime
    + TrustRuntime
    + UninstallRuntime
{
}

impl<T> RuntimeOps for T where
    T: InstallRuntime
        + ConfigRuntime
        + InitRuntime
        + EnvRuntime
        + ListRuntime
        + AgentsRuntime
        + RunRuntime
        + TaskRuntime
        + ActivateRuntime
        + DoctorRuntime
        + SyncRuntime
        + ServicesRuntime
        + TrustRuntime
        + UninstallRuntime
{
}

impl InstallRuntime for Runtime {
    fn install(&mut self, request: RecordedInstallRequest) -> Result<InstallResult> {
        let context = HostContext::resolve()?;
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
}

impl ConfigRuntime for Runtime {
    fn config_check(&mut self, global: bool) -> Result<CheckConfigResult> {
        let context = HostContext::resolve()?;
        block_on(crate::actions::config::check(CheckConfigRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
            global,
        }))
    }
}

impl InitRuntime for Runtime {
    fn init(&mut self, force: bool) -> Result<InitResult> {
        block_on(crate::actions::init::run(InitRequest {
            start_dir: current_dir()?,
            force,
        }))
    }
}

impl EnvRuntime for Runtime {
    fn env(&mut self, global: bool) -> Result<EnvResult> {
        let context = HostContext::resolve()?;
        block_on(crate::actions::env::inspect(EnvRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
            global,
        }))
    }
}

impl ListRuntime for Runtime {
    fn list(&mut self, all: bool, global: bool) -> Result<ListResult> {
        let context = HostContext::resolve()?;
        block_on(crate::actions::list::inspect(ListRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
            global,
            all,
        }))
    }
}

impl AgentsRuntime for Runtime {
    fn agents(&mut self, operation: AgentsOperation, global: bool) -> Result<AgentsResult> {
        let context = HostContext::resolve()?;
        block_on(crate::actions::agents::run(AgentsRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
            global,
            operation,
        }))
    }
}

impl RunRuntime for Runtime {
    fn run_command(&mut self, command: Vec<String>, global: bool) -> Result<RunResult> {
        let context = HostContext::resolve()?;
        block_on(crate::actions::run::run(RunRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
            global,
            command,
        }))
    }
}

impl TaskRuntime for Runtime {
    fn task(&mut self, name: Option<String>, global: bool) -> Result<TaskResult> {
        let context = HostContext::resolve()?;
        block_on(crate::actions::task::run(TaskRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
            global,
            name,
        }))
    }
}

impl ActivateRuntime for Runtime {
    fn activate(&mut self, shell: Option<String>, global: bool) -> Result<ActivateResult> {
        let context = HostContext::resolve()?;
        block_on(crate::actions::activate::run(ActivateRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
            global,
            shell,
        }))
    }
}

impl DoctorRuntime for Runtime {
    fn doctor(&mut self) -> Result<DoctorResult> {
        let context = HostContext::resolve()?;
        crate::actions::doctor::inspect(DoctorRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
        })
    }
}

impl SyncRuntime for Runtime {
    fn sync(&mut self, global: bool) -> Result<SyncResult> {
        let context = HostContext::resolve()?;
        block_on(crate::actions::sync::run(SyncRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
            global,
        }))
    }
}

impl ServicesRuntime for Runtime {
    fn services(
        &mut self,
        operation: ServicesOperation,
        name: Option<String>,
        global: bool,
    ) -> Result<ServicesResult> {
        let context = HostContext::resolve()?;
        block_on(crate::actions::services::run(ServicesRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
            global,
            operation,
            name,
        }))
    }
}

impl TrustRuntime for Runtime {
    fn trust(&mut self) -> Result<TrustResult> {
        let context = HostContext::resolve()?;
        block_on(crate::actions::trust::run(TrustRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
        }))
    }
}

impl UninstallRuntime for Runtime {
    fn uninstall(&mut self, target: UninstallTarget, global: bool) -> Result<UninstallResult> {
        let context = HostContext::resolve()?;
        block_on(crate::actions::uninstall::run(UninstallRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
            global,
            target,
        }))
    }
}

#[derive(Debug)]
struct HostContext {
    start_dir: PathBuf,
    home_dir: PathBuf,
}

impl HostContext {
    fn resolve() -> Result<Self> {
        Ok(Self {
            start_dir: current_dir()?,
            home_dir: home_dir()?,
        })
    }
}

fn current_dir() -> Result<PathBuf> {
    Ok(std::env::current_dir()?)
}

fn home_dir() -> Result<PathBuf> {
    dirs::home_dir().ok_or_else(|| EngineError::message("failed to find home directory"))
}

fn block_on<T>(future: impl Future<Output = Result<T>>) -> Result<T> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| EngineError::message(format!("failed to create tokio runtime: {err}")))?;
    runtime.block_on(future)
}
