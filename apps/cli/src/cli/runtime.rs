use std::{future::Future, path::PathBuf};

use engine::{
    actions::{
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
    },
    error::{EngineContext, EngineError, Result},
};

#[derive(Debug, Clone)]
pub struct ScopedInstallRequest {
    pub install: InstallRequest,
    pub global: bool,
    pub force: bool,
}

pub trait CliRuntime {
    fn install(&mut self, request: ScopedInstallRequest) -> Result<InstallResult>;
    fn config_check(&mut self, global: bool) -> Result<CheckConfigResult>;
    fn init(&mut self, force: bool) -> Result<InitResult>;
    fn env(&mut self, global: bool) -> Result<EnvResult>;
    fn list(&mut self, all: bool, global: bool) -> Result<ListResult>;
    fn agents(&mut self, operation: AgentsOperation, global: bool) -> Result<AgentsResult>;
    fn run_command(&mut self, command: Vec<String>, global: bool) -> Result<RunResult>;
    fn task(&mut self, name: Option<String>, global: bool) -> Result<TaskResult>;
    fn activate(&mut self, shell: Option<String>, global: bool) -> Result<ActivateResult>;
    fn doctor(&mut self) -> Result<DoctorResult>;
    fn sync(&mut self, global: bool) -> Result<SyncResult>;
    fn services(
        &mut self,
        operation: ServicesOperation,
        name: Option<String>,
        global: bool,
    ) -> Result<ServicesResult>;
    fn trust(&mut self) -> Result<TrustResult>;
    fn uninstall(&mut self, target: UninstallTarget, global: bool) -> Result<UninstallResult>;
}

#[derive(Debug)]
pub struct RealRuntime {
    tokio: tokio::runtime::Runtime,
}

impl RealRuntime {
    pub fn new() -> Result<Self> {
        let tokio = tokio::runtime::Runtime::new().context("failed to create tokio runtime")?;
        Ok(Self { tokio })
    }

    fn block_on<T>(&self, future: impl Future<Output = Result<T>>) -> Result<T> {
        self.tokio.block_on(future)
    }
}

impl CliRuntime for RealRuntime {
    fn install(&mut self, request: ScopedInstallRequest) -> Result<InstallResult> {
        let context = RuntimeContext::load()?;
        self.block_on(engine::actions::install::run_and_record(
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
        self.block_on(engine::actions::config::check(CheckConfigRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
            global,
        }))
    }

    fn init(&mut self, force: bool) -> Result<InitResult> {
        self.block_on(engine::actions::init::run(InitRequest {
            start_dir: current_dir()?,
            force,
        }))
    }

    fn env(&mut self, global: bool) -> Result<EnvResult> {
        let context = RuntimeContext::load()?;
        self.block_on(engine::actions::env::inspect(EnvRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
            global,
        }))
    }

    fn list(&mut self, all: bool, global: bool) -> Result<ListResult> {
        let context = RuntimeContext::load()?;
        self.block_on(engine::actions::list::inspect(ListRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
            global,
            all,
        }))
    }

    fn agents(&mut self, operation: AgentsOperation, global: bool) -> Result<AgentsResult> {
        let context = RuntimeContext::load()?;
        self.block_on(engine::actions::agents::run(AgentsRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
            global,
            operation,
        }))
    }

    fn run_command(&mut self, command: Vec<String>, global: bool) -> Result<RunResult> {
        let context = RuntimeContext::load()?;
        self.block_on(engine::actions::run::run(RunRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
            global,
            command,
        }))
    }

    fn task(&mut self, name: Option<String>, global: bool) -> Result<TaskResult> {
        let context = RuntimeContext::load()?;
        self.block_on(engine::actions::task::run(TaskRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
            global,
            name,
        }))
    }

    fn activate(&mut self, shell: Option<String>, global: bool) -> Result<ActivateResult> {
        let context = RuntimeContext::load()?;
        self.block_on(engine::actions::activate::run(ActivateRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
            global,
            shell,
        }))
    }

    fn doctor(&mut self) -> Result<DoctorResult> {
        let context = RuntimeContext::load()?;
        engine::actions::doctor::inspect(DoctorRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
        })
    }

    fn sync(&mut self, global: bool) -> Result<SyncResult> {
        let context = RuntimeContext::load()?;
        self.block_on(engine::actions::sync::run(SyncRequest {
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
        self.block_on(engine::actions::services::run(ServicesRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
            global,
            operation,
            name,
        }))
    }

    fn trust(&mut self) -> Result<TrustResult> {
        let context = RuntimeContext::load()?;
        self.block_on(engine::actions::trust::run(TrustRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
        }))
    }

    fn uninstall(&mut self, target: UninstallTarget, global: bool) -> Result<UninstallResult> {
        let context = RuntimeContext::load()?;
        self.block_on(engine::actions::uninstall::run(UninstallRequest {
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
