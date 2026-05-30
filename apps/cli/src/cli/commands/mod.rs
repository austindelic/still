//! CLI command dispatch and help rendering.

/// Install command handler.
pub mod install;

use crate::cli::args::{Cli, Command, ConfigCommand};
use crate::cli::output::Output;
use crate::cli::runtime::CliRuntime;
use clap::CommandFactory;

/// Dispatches one parsed subcommand to its CLI handler.
///
/// `cmd` contains the command-specific parsed input from Clap. `runtime` is the
/// mutable engine boundary used by handlers that perform real work. `output`
/// receives all user-facing text. The return value is a process-style exit code:
/// `0` for success and nonzero for command failure.
pub fn run_cli<R, O>(cmd: Command, runtime: &mut R, output: &mut O) -> i32
where
    R: CliRuntime,
    O: Output,
{
    match cmd {
        Command::Init(args) => {
            output.info(&format!("Init command: {:?}", args));
            0
        }
        Command::Trust(args) => {
            output.info(&format!("Trust command: {:?}", args));
            0
        }
        Command::Install(args) => install::run(args, runtime, output),
        Command::Sync(args) => {
            output.info(&format!("Sync command: {:?}", args));
            0
        }
        Command::List(args) => {
            output.info(&format!("List command: {:?}", args));
            0
        }
        Command::Uninstall(args) => {
            output.info(&format!("Uninstall command: {:?}", args));
            0
        }
        Command::Run(args) => {
            output.info(&format!("Run command: {:?}", args));
            0
        }
        Command::Task(args) => {
            output.info(&format!("Task command: {:?}", args));
            0
        }
        Command::Services(args) => {
            output.info(&format!("Services command: {:?}", args));
            0
        }
        Command::Agents(args) => {
            output.info(&format!("Agents command: {:?}", args));
            0
        }
        Command::Config(args) => match args.command {
            ConfigCommand::Check => match runtime.config_check(args.global) {
                Ok(result) => {
                    output.success(&format!("Config OK: {}", result.path.display()));
                    0
                }
                Err(e) => {
                    output.error(&format!("config check failed: {e}"));
                    1
                }
            },
        },
        Command::Doctor(args) => {
            output.info(&format!("Doctor command: {:?}", args));
            0
        }
        Command::Env(args) => {
            output.info(&format!("Env command: {:?}", args));
            0
        }
        Command::Activate(args) => {
            output.info(&format!("Activate command: {:?}", args));
            0
        }
    }
}

/// Dispatches a parsed top-level CLI value using command-module help behavior.
///
/// `cli` is parsed input that may or may not contain a subcommand. `runtime` and
/// `output` are passed through to command handlers unchanged. With no subcommand,
/// this function writes generated help and returns the help-render result.
pub fn run_parsed<R, O>(cli: Cli, runtime: &mut R, output: &mut O) -> i32
where
    R: CliRuntime,
    O: Output,
{
    match cli.command {
        Some(cmd) => run_cli(cmd, runtime, output),
        None => run_without_command(output),
    }
}

fn run_without_command<O: Output>(output: &mut O) -> i32 {
    print_help(output)
}

/// Renders and writes top-level help text to an output sink.
///
/// `output` receives the generated help on stdout when rendering succeeds, or a
/// render error on stderr when Clap fails to write help. Returns `0` for rendered
/// help and `1` for rendering failure.
pub fn print_help<O: Output>(output: &mut O) -> i32 {
    match help_text() {
        Ok(help) => {
            output.info(&help);
            0
        }
        Err(e) => {
            output.error(&format!("failed to render help: {e}"));
            1
        }
    }
}

/// Builds the top-level Clap help text used by no-command behavior and tests.
///
/// The returned string always includes a trailing newline so command handlers
/// can pass it directly to `Output::info`. Errors are I/O failures from Clap's
/// help writer.
pub fn help_text() -> std::io::Result<String> {
    let mut command = Cli::command();
    let mut bytes = Vec::new();
    command.write_help(&mut bytes)?;
    bytes.push(b'\n');
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use anyhow::anyhow;
    use engine::actions::{
        config::CheckConfigResult,
        install::{InstallRequest, InstallResult},
    };
    use engine::specs::toml::StillConfig;

    use super::*;
    use crate::cli::args::{ConfigArgs, ConfigCommand};
    use crate::cli::output::BufferedOutput;

    struct FakeRuntime {
        config_check_result: Option<anyhow::Result<CheckConfigResult>>,
        config_check_globals: Vec<bool>,
    }

    impl CliRuntime for FakeRuntime {
        fn install(&mut self, _request: InstallRequest) -> anyhow::Result<InstallResult> {
            panic!("install should not run in config tests");
        }

        fn config_check(&mut self, global: bool) -> anyhow::Result<CheckConfigResult> {
            self.config_check_globals.push(global);
            self.config_check_result
                .take()
                .expect("test runtime config_check result was not configured")
        }
    }

    #[test]
    fn config_check_formats_success() {
        let mut runtime = FakeRuntime {
            config_check_result: Some(Ok(CheckConfigResult {
                path: PathBuf::from("/repo/still.toml"),
                config: StillConfig::default(),
            })),
            config_check_globals: Vec::new(),
        };
        let mut output = BufferedOutput::default();

        let code = run_cli(
            Command::Config(ConfigArgs {
                global: false,
                command: ConfigCommand::Check,
            }),
            &mut runtime,
            &mut output,
        );

        assert_eq!(code, 0);
        assert_eq!(runtime.config_check_globals, [false]);
        insta::assert_snapshot!(output.stdout, @r###"
✓ Config OK: /repo/still.toml
"###);
        assert_eq!(output.stderr, "");
    }

    #[test]
    fn config_check_formats_error() {
        let mut runtime = FakeRuntime {
            config_check_result: Some(Err(anyhow!("failed to parse still.toml"))),
            config_check_globals: Vec::new(),
        };
        let mut output = BufferedOutput::default();

        let code = run_cli(
            Command::Config(ConfigArgs {
                global: true,
                command: ConfigCommand::Check,
            }),
            &mut runtime,
            &mut output,
        );

        assert_eq!(code, 1);
        assert_eq!(runtime.config_check_globals, [true]);
        assert_eq!(output.stdout, "");
        insta::assert_snapshot!(output.stderr, @r###"
config check failed: failed to parse still.toml
"###);
    }
}
