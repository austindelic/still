//! CLI parsing, command dispatch, and output formatting.

/// Command-line argument definitions.
#[path = "contract.rs"]
pub mod args;
/// CLI command dispatch and command handlers.
pub mod commands;
/// Output abstractions used by real commands and tests.
pub mod output;

use clap::Parser;

use crate::runtime::{ActionRuntime, RealRuntime};

use self::{
    args::Cli,
    commands::run_cli,
    output::{Output, StdOutput},
};

/// Parses process arguments, dispatches the selected command, and exits on failure.
pub fn entry() {
    entry_with_no_command(commands::print_help);
}

/// Parses process arguments and dispatches with an injected no-command handler.
pub fn entry_with_no_command<F>(no_command: F)
where
    F: FnOnce(&mut StdOutput) -> i32,
{
    let cli = Cli::parse();
    let mut runtime = RealRuntime;
    let mut output = StdOutput;
    let code = run_parsed_with_no_command(cli, &mut runtime, &mut output, no_command);

    if code != 0 {
        std::process::exit(code);
    }
}

/// Runs parsed CLI input with help output as the default no-command behavior.
pub fn run_parsed<R, O>(cli: Cli, runtime: &mut R, output: &mut O) -> i32
where
    R: ActionRuntime,
    O: Output,
{
    run_parsed_with_no_command(cli, runtime, output, commands::print_help)
}

/// Runs parsed CLI input with an injected no-command handler.
pub fn run_parsed_with_no_command<R, O, F>(
    cli: Cli,
    runtime: &mut R,
    output: &mut O,
    no_command: F,
) -> i32
where
    R: ActionRuntime,
    O: Output,
    F: FnOnce(&mut O) -> i32,
{
    match cli.command {
        Some(command) => run_cli(command, runtime, output),
        None => no_command(output),
    }
}
