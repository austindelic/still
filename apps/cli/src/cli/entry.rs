#[cfg(not(feature = "tui"))]
use clap::CommandFactory;
use clap::Parser;
use miette::miette;

use crate::cli::{
    args::Cli,
    context::CliContext,
    route::route_command,
    session::EngineSession,
    ui::{TerminalUi, Ui},
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
    let mut ui = TerminalUi;

    let Some(command) = cli.command else {
        return Ok(no_command(&mut ui));
    };

    let context = CliContext::load().map_err(|err| miette!("failed to load CLI context: {err}"))?;
    let mut session = EngineSession::new(context)
        .map_err(|err| miette!("failed to initialize engine session: {err}"))?;
    Ok(route_command(command, &mut session, &mut ui))
}

fn no_command<U: Ui>(ui: &mut U) -> i32 {
    #[cfg(feature = "tui")]
    return launch_tui_or_report(ui, still_tui::launch_tui);

    #[cfg(not(feature = "tui"))]
    print_help(ui)
}

#[cfg(not(feature = "tui"))]
fn print_help<U: Ui>(ui: &mut U) -> i32 {
    match help_text() {
        Ok(help) => {
            ui.info(&help);
            0
        }
        Err(e) => {
            ui.error(&format!("failed to render help: {e}"));
            1
        }
    }
}

#[cfg(not(feature = "tui"))]
fn help_text() -> std::io::Result<String> {
    let mut command = Cli::command().bin_name("still");
    let mut bytes = Vec::new();
    command.write_help(&mut bytes)?;
    bytes.push(b'\n');
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(feature = "tui")]
pub fn launch_tui_or_report<U, F>(ui: &mut U, launch: F) -> i32
where
    U: Ui,
    F: FnOnce() -> std::io::Result<()>,
{
    match launch() {
        Ok(()) => 0,
        Err(e) => {
            ui.error(&format!("Failed to launch TUI: {e}"));
            ui.error("");
            ui.error("This may be due to:");
            ui.error("  - Terminal not supporting TUI mode");
            ui.error("  - Terminal size too small");
            ui.error("  - Missing required terminal capabilities");
            1
        }
    }
}

#[cfg(all(test, feature = "tui"))]
mod tests {
    use super::*;
    use crate::cli::ui::BufferedUi;

    #[test]
    fn launch_tui_failure_reports_terminal_guidance() {
        let mut ui = BufferedUi::default();

        let code = launch_tui_or_report(&mut ui, || {
            Err(std::io::Error::other("terminal unavailable"))
        });

        assert_eq!(code, 1);
        assert!(ui.stderr.contains("Failed to launch TUI"));
        assert!(ui.stderr.contains("Terminal not supporting TUI mode"));
        assert_eq!(ui.stdout, "");
    }
}

#[cfg(all(test, not(feature = "tui")))]
mod tests {
    use super::*;

    #[rustfmt::skip]
    #[test]
    fn default_help_matches_help_text() {
        let help = help_text().expect("help text should render");

        insta::assert_snapshot!(help, @r###"
Universal Package Manager + Version Manager

Usage: still [COMMAND]

Commands:
  init       Initialize configuration for a new project
  trust      Trust project-defined executable behavior
  install    Install requested tools, packages, or apps
  sync       Synchronize installed state with config
  list       List configured and installed items
  uninstall  Remove a Still-managed install
  run        Run a command within the managed environment
  task       Run or list tasks defined in config
  services   Inspect, start, stop, or check services
  agents     Inspect, sync, or validate agent instructions and skills
  config     Inspect, validate, or edit Still configuration
  doctor     Diagnose machine, cache, config, and platform health
  env        Display resolved environment information
  activate   Print shell activation code
  help       Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
"###);
    }

    #[test]
    fn default_build_does_not_expose_tui_command() {
        let help = help_text().expect("help text should render");

        assert!(!help.contains("  tui"));
    }

    #[test]
    fn help_does_not_expose_non_priority_commands() {
        let help = help_text().expect("help text should render");

        assert!(!help.contains("translate"));
        assert!(!help.contains("convert"));
        assert!(!help.contains("post-install"));
        assert!(!help.contains("  web"));
    }
}
