//! CLI entrypoint, command routing, and feature-gated no-argument behavior.

/// Command-line argument definitions.
pub mod args;
/// CLI command dispatch and command handlers.
pub mod commands;
/// Output abstractions used by real commands and tests.
pub mod output;
/// Boundary between CLI handlers and engine actions.
pub mod runtime;

use clap::Parser;

use self::{
    args::Cli,
    commands::run_cli,
    output::{Output, StdOutput},
    runtime::{CliRuntime, RealRuntime},
};

/// Parses process arguments, dispatches the selected command, and exits on failure.
///
/// This is the production binary entrypoint. It owns the real runtime and output
/// sink, reads argv through Clap, and converts any nonzero command result into
/// the process exit code. Tests should call `run_parsed` or
/// `run_parsed_with_no_command` so they can inject parsed input, runtime state,
/// and output capture.
pub fn entry() {
    let cli = Cli::parse();
    let mut runtime = RealRuntime;
    let mut output = StdOutput;
    let code = run_parsed(cli, &mut runtime, &mut output);

    if code != 0 {
        std::process::exit(code);
    }
}

/// Runs parsed CLI input with the build's default no-command behavior.
///
/// `cli` is the already parsed user input. `runtime` is the mutable boundary
/// used by commands that need engine behavior. `output` receives all user-facing
/// stdout/stderr text. The return value is a process-style exit code where `0`
/// means success.
pub fn run_parsed<R, O>(cli: Cli, runtime: &mut R, output: &mut O) -> i32
where
    R: CliRuntime,
    O: Output,
{
    run_parsed_with_no_command(cli, runtime, output, run_without_command)
}

/// Runs parsed CLI input with an injected no-command handler.
///
/// `cli`, `runtime`, and `output` have the same roles as `run_parsed`.
/// `no_command` is called only when the user supplied no subcommand, allowing
/// tests and feature-gated builds to choose between help output and TUI launch
/// without changing command dispatch. The return value is the selected handler's
/// process-style exit code.
pub fn run_parsed_with_no_command<R, O, F>(
    cli: Cli,
    runtime: &mut R,
    output: &mut O,
    no_command: F,
) -> i32
where
    R: CliRuntime,
    O: Output,
    F: FnOnce(&mut O) -> i32,
{
    match cli.command {
        Some(command) => run_cli(command, runtime, output),
        None => no_command(output),
    }
}

#[cfg(not(feature = "tui"))]
fn run_without_command<O: Output>(output: &mut O) -> i32 {
    commands::print_help(output)
}

#[cfg(feature = "tui")]
fn run_without_command<O: Output>(output: &mut O) -> i32 {
    launch_tui_or_report(output, still_tui::launch_tui)
}

/// Launches the TUI and maps terminal startup failures into CLI output.
///
/// `output` is used only for failure reporting so terminal errors still appear
/// in the same stderr path as normal commands. `launch` owns the actual TUI
/// startup function, which makes the lifecycle testable without entering raw
/// terminal mode. Returns `0` when the TUI exits normally and `1` when launch
/// fails.
#[cfg(feature = "tui")]
pub fn launch_tui_or_report<O, F>(output: &mut O, launch: F) -> i32
where
    O: Output,
    F: FnOnce() -> std::io::Result<()>,
{
    match launch() {
        Ok(()) => 0,
        Err(e) => {
            output.error(&format!("Failed to launch TUI: {e}"));
            output.error("");
            output.error("This may be due to:");
            output.error("  - Terminal not supporting TUI mode");
            output.error("  - Terminal size too small");
            output.error("  - Missing required terminal capabilities");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{
        args::{Cli, Command, DoctorArgs},
        output::BufferedOutput,
        runtime::CliRuntime,
    };
    use engine::actions::{
        activate::ActivateResult,
        agents::{AgentsOperation, AgentsResult},
        config::CheckConfigResult,
        doctor::{DoctorCheck, DoctorResult, DoctorStatus},
        env::EnvResult,
        init::InitResult,
        list::ListResult,
        run::RunResult,
        services::{ServicesOperation, ServicesResult},
        sync::SyncResult,
        task::TaskResult,
        trust::TrustResult,
        uninstall::{UninstallResult, UninstallTarget},
    };

    #[derive(Debug, Default)]
    struct FakeRuntime;

    impl CliRuntime for FakeRuntime {
        fn install(
            &mut self,
            _request: crate::cli::runtime::InstallCommandRequest,
        ) -> anyhow::Result<engine::actions::install::InstallResult> {
            panic!("install should not run in these routing tests");
        }

        fn config_check(&mut self, _global: bool) -> anyhow::Result<CheckConfigResult> {
            panic!("config_check should not run in these routing tests");
        }

        fn init(&mut self, _force: bool) -> anyhow::Result<InitResult> {
            panic!("init should not run in these routing tests");
        }

        fn env(&mut self, _global: bool) -> anyhow::Result<EnvResult> {
            panic!("env should not run in these routing tests");
        }

        fn list(&mut self, _all: bool, _global: bool) -> anyhow::Result<ListResult> {
            panic!("list should not run in these routing tests");
        }

        fn agents(&mut self, _operation: AgentsOperation) -> anyhow::Result<AgentsResult> {
            panic!("agents should not run in these routing tests");
        }

        fn run_command(
            &mut self,
            _command: Vec<String>,
            _global: bool,
        ) -> anyhow::Result<RunResult> {
            panic!("run_command should not run in these routing tests");
        }

        fn task(&mut self, _name: Option<String>, _global: bool) -> anyhow::Result<TaskResult> {
            panic!("task should not run in these routing tests");
        }

        fn activate(
            &mut self,
            _shell: Option<String>,
            _global: bool,
        ) -> anyhow::Result<ActivateResult> {
            panic!("activate should not run in these routing tests");
        }

        fn doctor(&mut self) -> anyhow::Result<DoctorResult> {
            Ok(DoctorResult {
                checks: vec![DoctorCheck {
                    name: "platform".to_string(),
                    status: DoctorStatus::Ok,
                    detail: "detected test".to_string(),
                }],
            })
        }

        fn sync(&mut self, _global: bool) -> anyhow::Result<SyncResult> {
            panic!("sync should not run in these routing tests");
        }

        fn services(
            &mut self,
            _operation: ServicesOperation,
            _name: Option<String>,
            _global: bool,
        ) -> anyhow::Result<ServicesResult> {
            panic!("services should not run in these routing tests");
        }

        fn trust(&mut self) -> anyhow::Result<TrustResult> {
            panic!("trust should not run in these routing tests");
        }

        fn uninstall(
            &mut self,
            _target: UninstallTarget,
            _global: bool,
        ) -> anyhow::Result<UninstallResult> {
            panic!("uninstall should not run in these routing tests");
        }
    }

    #[cfg(not(feature = "tui"))]
    #[test]
    fn no_command_prints_help_without_tui_feature() {
        let cli = Cli { command: None };
        let mut runtime = FakeRuntime;
        let mut output = BufferedOutput::default();

        let code = run_parsed(cli, &mut runtime, &mut output);

        assert_eq!(code, 0);
        assert!(output.stdout.contains("Usage: Still [COMMAND]"));
        assert_eq!(output.stderr, "");
    }

    #[cfg(feature = "tui")]
    #[test]
    fn no_command_uses_tui_feature_path() {
        let cli = Cli { command: None };
        let mut runtime = FakeRuntime;
        let mut output = BufferedOutput::default();

        let code = run_parsed_with_no_command(cli, &mut runtime, &mut output, |output| {
            output.info("opened tui");
            0
        });

        assert_eq!(code, 0);
        assert_eq!(output.stdout, "opened tui\n");
        assert_eq!(output.stderr, "");
    }

    #[test]
    fn command_delegates_to_cli_runner() {
        let cli = Cli {
            command: Some(Command::Doctor(DoctorArgs {})),
        };
        let mut runtime = FakeRuntime;
        let mut output = BufferedOutput::default();

        let code = run_parsed(cli, &mut runtime, &mut output);

        assert_eq!(code, 0);
        assert_eq!(output.stdout, "[ok] platform: detected test\n");
        assert_eq!(output.stderr, "");
    }

    #[cfg(feature = "tui")]
    #[test]
    fn tui_launch_error_is_reported() {
        let mut output = BufferedOutput::default();

        let code = launch_tui_or_report(&mut output, || {
            Err(std::io::Error::other("terminal failed"))
        });

        assert_eq!(code, 1);
        insta::assert_snapshot!(output.stderr, @r###"
Failed to launch TUI: terminal failed

This may be due to:
  - Terminal not supporting TUI mode
  - Terminal size too small
  - Missing required terminal capabilities
"###);
        assert_eq!(output.stdout, "");
    }
}
