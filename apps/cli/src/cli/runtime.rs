//! Runtime boundary between CLI handlers and engine actions.

use engine::actions::{
    config::{CheckConfigRequest, CheckConfigResult},
    init::{InitRequest, InitResult},
    install::{InstallRequest, InstallResult},
};

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
    fn install(&mut self, request: InstallRequest) -> anyhow::Result<InstallResult>;

    /// Checks the selected config file through the engine.
    fn config_check(&mut self, global: bool) -> anyhow::Result<CheckConfigResult>;

    /// Initializes a starter config in the current project directory.
    fn init(&mut self, force: bool) -> anyhow::Result<InitResult>;
}

/// Production runtime implementation that calls real engine actions.
///
/// The type carries no state today, but it owns the decision to bridge sync CLI
/// handlers into async engine actions.
#[derive(Debug, Default)]
pub struct RealRuntime;

impl CliRuntime for RealRuntime {
    fn install(&mut self, request: InstallRequest) -> anyhow::Result<InstallResult> {
        let runtime = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        runtime.block_on(engine::actions::install::run(request))
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
}
