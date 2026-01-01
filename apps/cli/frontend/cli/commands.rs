use crate::cli::args::{Cli, Commands};
use crate::cli::install;
use crate::tui;
use clap::Parser;

pub fn run_cli(cmd: Commands) {
    match cmd {
        Commands::Install(args) => {
            install::run(args);
        }
        Commands::Uninstall(args) => {
            println!("Uninstall command: {:?}", args);
        }
        Commands::Use(args) => {
            println!("Use command: {:?}", args);
        }
        Commands::Run(args) => {
            println!("Run command: {:?}", args);
        }
        Commands::Translate(args) => {
            println!("Translate command: {:?}", args);
        }
        Commands::Doctor(args) => {
            println!("Doctor command: {:?}", args);
        }
        Commands::Init(args) => {
            println!("Init command: {:?}", args);
        }
        Commands::Convert(args) => {
            println!("Convert command: {:?}", args);
        }
        _ => {}
    }
}

pub fn entry() {
    let cli = Cli::parse();

    match cli.command {
        None => {
            if let Err(e) = tui::launch_tui() {
                eprintln!("Failed to launch TUI: {e}");
                eprintln!("\nThis may be due to:");
                eprintln!("  - Terminal not supporting TUI mode");
                eprintln!("  - Terminal size too small");
                eprintln!("  - Missing required terminal capabilities");
            }
        }
        Some(cmd) => run_cli(cmd),
    }
}
