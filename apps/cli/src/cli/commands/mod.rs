//! CLI command dispatch and help rendering.

/// Install command handler.
pub mod install;

use crate::cli::args::{Cli, Command};
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
        Command::Install(args) => install::run(args, runtime, output),
        Command::Uninstall(args) => {
            output.info(&format!("Uninstall command: {:?}", args));
            0
        }
        Command::Use(args) => {
            output.info(&format!("Use command: {:?}", args));
            0
        }
        Command::Run(args) => {
            output.info(&format!("Run command: {:?}", args));
            0
        }
        Command::Translate(args) => {
            output.info(&format!("Translate command: {:?}", args));
            0
        }
        Command::Doctor(args) => {
            output.info(&format!("Doctor command: {:?}", args));
            0
        }
        Command::Init(args) => {
            output.info(&format!("Init command: {:?}", args));
            0
        }
        Command::Convert(args) => {
            output.info(&format!("Convert command: {:?}", args));
            0
        }
        _ => 0,
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
