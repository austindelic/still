pub mod args;
pub mod commands;
pub mod output;
pub mod progress;
pub mod runtime;

use clap::Parser;
use miette::miette;

use self::{
    args::Cli,
    commands::run_cli,
    output::{Output, StdOutput},
    runtime::{CliRuntime, RealRuntime},
};

pub fn entry() {
    let code = match try_entry() {
        Ok(code) => code,
        Err(report) => {
            eprintln!("{report:?}");
            1
        }
    };

    if code != 0 {
        std::process::exit(code);
    }
}

fn try_entry() -> miette::Result<i32> {
    let cli = Cli::parse();
    let mut output = StdOutput;

    if cli.command.is_none() {
        return Ok(run_without_command(&mut output));
    }

    let mut runtime =
        RealRuntime::new().map_err(|err| miette!("failed to initialize CLI runtime: {err}"))?;
    Ok(run_parsed_with_no_command(
        cli,
        &mut runtime,
        &mut output,
        run_without_command,
    ))
}

fn run_without_command<O: Output>(output: &mut O) -> i32 {
    #[cfg(feature = "tui")]
    return launch_tui_or_report(output, still_tui::launch_tui);

    #[cfg(not(feature = "tui"))]
    commands::print_help(output)
}

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

#[cfg(all(test, feature = "tui"))]
mod tests {
    use super::*;

    #[derive(Default)]
    struct TestOutput {
        stdout: String,
        stderr: String,
    }

    impl Output for TestOutput {
        fn info(&mut self, msg: &str) {
            self.stdout.push_str(msg);
            self.stdout.push('\n');
        }

        fn error(&mut self, msg: &str) {
            self.stderr.push_str(msg);
            self.stderr.push('\n');
        }

        fn success(&mut self, msg: &str) {
            self.info(&format!("✓ {msg}"));
        }

        fn warning(&mut self, msg: &str) {
            self.error(&format!("⚠ {msg}"));
        }
    }

    #[test]
    fn launch_tui_failure_reports_terminal_guidance() {
        let mut output = TestOutput::default();

        let code = launch_tui_or_report(&mut output, || {
            Err(std::io::Error::other("terminal unavailable"))
        });

        assert_eq!(code, 1);
        assert!(output.stderr.contains("Failed to launch TUI"));
        assert!(output.stderr.contains("Terminal not supporting TUI mode"));
        assert_eq!(output.stdout, "");
    }
}
