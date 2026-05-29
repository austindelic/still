pub mod install;

use crate::cli::args::{Cli, Command};
use crate::cli::output::{Output, StdOutput};
use crate::cli::runtime::{CliRuntime, RealRuntime};
#[cfg(feature = "tui")]
use crate::tui;
#[cfg(not(feature = "tui"))]
use clap::CommandFactory;
use clap::Parser;

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
        #[cfg(feature = "tui")]
        Command::Tui => launch_tui_or_report(output),
        _ => 0,
    }
}

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

#[cfg(feature = "tui")]
fn run_without_command<O: Output>(output: &mut O) -> i32 {
    launch_tui_or_report(output)
}

#[cfg(not(feature = "tui"))]
fn run_without_command<O: Output>(output: &mut O) -> i32 {
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

#[cfg(feature = "tui")]
fn launch_tui_or_report<O: Output>(output: &mut O) -> i32 {
    match tui::launch_tui() {
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

#[cfg(not(feature = "tui"))]
fn help_text() -> std::io::Result<String> {
    let mut command = Cli::command();
    let mut bytes = Vec::new();
    command.write_help(&mut bytes)?;
    bytes.push(b'\n');
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

pub fn entry() {
    let cli = Cli::parse();
    let mut runtime = RealRuntime;
    let mut output = StdOutput;
    let code = run_parsed(cli, &mut runtime, &mut output);

    if code != 0 {
        std::process::exit(code);
    }
}
