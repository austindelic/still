use crate::cli::args::{Cli, Command};
use crate::cli::install;
use crate::tui;
use clap::Parser;
use engine::platform::policy::system::{System, system};

pub fn run_cli(system: System, cmd: Command) {
    match cmd {
        Command::Install(args) => {
            install::run(system, args);
        }
        Command::Uninstall(args) => {
            println!("Uninstall command: {:?}", args);
        }
        Command::Use(args) => {
            println!("Use command: {:?}", args);
        }
        Command::Run(args) => {
            println!("Run command: {:?}", args);
        }
        Command::Translate(args) => {
            println!("Translate command: {:?}", args);
        }
        Command::Doctor(args) => {
            println!("Doctor command: {:?}", args);
        }
        Command::Init(args) => {
            println!("Init command: {:?}", args);
        }
        Command::Convert(args) => {
            println!("Convert command: {:?}", args);
        }
        _ => {}
    }
}

pub fn entry() {
    let cli = Cli::parse();
    let system = system();
    match cli.command {
        None => {
            if let Err(e) = tui::launch_tui(system) {
                eprintln!("Failed to launch TUI: {e}");
                eprintln!("\nThis may be due to:");
                eprintln!("  - Terminal not supporting TUI mode");
                eprintln!("  - Terminal size too small");
                eprintln!("  - Missing required terminal capabilities");
            }
        }
        Some(cmd) => run_cli(system, cmd),
    }
}
