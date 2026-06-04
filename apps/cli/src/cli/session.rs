use std::future::Future;

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
    error::{EngineContext, Result},
};

use crate::cli::context::CliContext;

#[derive(Debug, Clone)]
pub struct InstallCommandRequest {
    pub install: InstallRequest,
    pub global: bool,
    pub force: bool,
}

/// CLI-owned session for calling engine actions through one Tokio runtime.
#[derive(Debug)]
pub struct EngineSession {
    context: CliContext,
    tokio: tokio::runtime::Runtime,
}

impl EngineSession {
    pub fn new(context: CliContext) -> Result<Self> {
        let tokio = tokio::runtime::Runtime::new().context("failed to create tokio runtime")?;
        Ok(Self { context, tokio })
    }

    fn block_on<T>(&self, future: impl Future<Output = Result<T>>) -> Result<T> {
        self.tokio.block_on(future)
    }

    pub fn install_and_record(&mut self, request: InstallCommandRequest) -> Result<InstallResult> {
        let context = self.context.clone();
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

    pub fn config_check(&mut self, global: bool) -> Result<CheckConfigResult> {
        let context = self.context.clone();
        self.block_on(engine::actions::config::check(CheckConfigRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
            global,
        }))
    }

    pub fn init(&mut self, force: bool) -> Result<InitResult> {
        let context = self.context.clone();
        self.block_on(engine::actions::init::run(InitRequest {
            start_dir: context.start_dir,
            force,
        }))
    }

    pub fn env(&mut self, global: bool) -> Result<EnvResult> {
        let context = self.context.clone();
        self.block_on(engine::actions::env::inspect(EnvRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
            global,
        }))
    }

    pub fn list(&mut self, all: bool, global: bool) -> Result<ListResult> {
        let context = self.context.clone();
        self.block_on(engine::actions::list::inspect(ListRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
            global,
            all,
        }))
    }

    pub fn agents(&mut self, operation: AgentsOperation, global: bool) -> Result<AgentsResult> {
        let context = self.context.clone();
        self.block_on(engine::actions::agents::run(AgentsRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
            global,
            operation,
        }))
    }

    pub fn run_command(&mut self, command: Vec<String>, global: bool) -> Result<RunResult> {
        let context = self.context.clone();
        self.block_on(engine::actions::run::run(RunRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
            global,
            command,
        }))
    }

    pub fn task(&mut self, name: Option<String>, global: bool) -> Result<TaskResult> {
        let context = self.context.clone();
        self.block_on(engine::actions::task::run(TaskRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
            global,
            name,
        }))
    }

    pub fn activate(&mut self, shell: Option<String>, global: bool) -> Result<ActivateResult> {
        let context = self.context.clone();
        self.block_on(engine::actions::activate::run(ActivateRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
            global,
            shell,
        }))
    }

    pub fn doctor(&mut self) -> Result<DoctorResult> {
        let context = self.context.clone();
        engine::actions::doctor::inspect(DoctorRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
        })
    }

    pub fn sync(&mut self, global: bool) -> Result<SyncResult> {
        let context = self.context.clone();
        self.block_on(engine::actions::sync::run(SyncRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
            global,
        }))
    }

    pub fn services(
        &mut self,
        operation: ServicesOperation,
        name: Option<String>,
        global: bool,
    ) -> Result<ServicesResult> {
        let context = self.context.clone();
        self.block_on(engine::actions::services::run(ServicesRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
            global,
            operation,
            name,
        }))
    }

    pub fn trust(&mut self) -> Result<TrustResult> {
        let context = self.context.clone();
        self.block_on(engine::actions::trust::run(TrustRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
        }))
    }

    pub fn uninstall(&mut self, target: UninstallTarget, global: bool) -> Result<UninstallResult> {
        let context = self.context.clone();
        self.block_on(engine::actions::uninstall::run(UninstallRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
            global,
            target,
        }))
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn new_keeps_loaded_cli_context() {
        let context = CliContext {
            start_dir: PathBuf::from("/tmp/still/project"),
            home_dir: PathBuf::from("/tmp/still/home"),
        };

        let session = EngineSession::new(context.clone()).expect("session should initialize");

        assert_eq!(session.context, context);
    }
}
