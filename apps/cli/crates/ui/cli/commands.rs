use crate::cli::args::InstallArgs;
use crate::cli::args::{Cli, Command};
use crate::tui;
use clap::Parser;
use engine::actions::install::{InstallRequest, run};

pub fn install(args: InstallArgs) {
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    let install_request = InstallRequest { tool: args.tool };
    let result = rt.block_on(run(install_request));
    match result {
        Ok(res) => {
            if let Some(binary_path) = &res.binary_path {
                println!("Binary installed at: {}", binary_path.display());
            } else {
                println!(
                    "Warning: Could not find binary in {}",
                    res.install_path.display()
                );
            }
            println!(
                "Successfully installed {}@{} to {}",
                res.tool_name,
                res.version,
                res.install_path.display()
            );
        }
        Err(e) => {
            eprintln!("install failed: {e}");
            std::process::exit(1);
        }
    }
}

pub fn run_cli(cmd: Command) {
    match cmd {
        Command::Install(args) => {
            install(args);
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
