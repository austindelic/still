use engine::actions::install::{InstallRequest, InstallResult};

pub trait CliRuntime {
    fn install(&mut self, request: InstallRequest) -> anyhow::Result<InstallResult>;
}

#[derive(Debug, Default)]
pub struct RealRuntime;

impl CliRuntime for RealRuntime {
    fn install(&mut self, request: InstallRequest) -> anyhow::Result<InstallResult> {
        let runtime = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        runtime.block_on(engine::actions::install::run(request))
    }
}
