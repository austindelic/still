use crate::commands::{
    AddArgs, AddCommand, ConvertArgs, ConvertCommand, DoctorArgs, DoctorCommand, InitArgs,
    InitCommand, InstallArgs, InstallCommand, RemoveArgs, RemoveCommand, RunArgs, RunCommand,
    TranslateArgs, TranslateCommand, UninstallArgs, UninstallCommand, UseArgs, UseCommand,
};
use crate::tui;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "Still", about = "Universal Package Manager + Version Manager")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    Install(InstallArgs),     // Install a package/app into the current environment.
    Uninstall(UninstallArgs), // Remove a package/app from the current environment.
    Use(UseArgs),             // Switch to a specific runtime or toolchain version.
    Add(AddArgs),             // Add a dependency or entry to the project manifest.
    Remove(RemoveArgs),       // Remove a dependency or entry from the project manifest.
    Doctor(DoctorArgs),       // Diagnose the environment and suggest fixes or updates.
    Run(RunArgs),             // Run a command within the managed environment.
    Translate(TranslateArgs), // Translate project definitions between supported formats.
    Init(InitArgs),           // Initialize configuration for a new project.
    Convert(ConvertArgs),     // Convert configuration or lockfiles to another supported format.
    Env,                      // Display environment information required for debugging.
    Tui,                      // Launch the text-based user interface.
    Web,                      // Open or run the web-based management dashboard.
    Activate,                 // Activate a workspace or profile for the current shell session.
    Sync,                     // Synchronize the workspace state with configured sources.
}
fn run_cli(cmd: &Commands) {
    match cmd {
        Commands::Install(args) => InstallCommand::from(args.clone()).run(),
        Commands::Uninstall(args) => UninstallCommand::from(args.clone()).run(),
        Commands::Use(args) => UseCommand::from(args.clone()).run(),
        Commands::Add(args) => AddCommand::from(args.clone()).run(),
        Commands::Remove(args) => RemoveCommand::from(args.clone()).run(),
        Commands::Run(args) => RunCommand::from(args.clone()).run(),
        Commands::Translate(args) => TranslateCommand::from(args.clone()).run(),
        Commands::Doctor(args) => DoctorCommand::from(args.clone()).run(),
        Commands::Init(args) => InitCommand::from(args.clone()).run(),
        Commands::Convert(args) => ConvertCommand::from(args.clone()).run(),

        _ => {}
    }
}

pub fn entry() {
    let cli = Cli::parse();

    match &cli.command {
        None => tui::launch_tui().expect("tui broke :/"),
        Some(cmd) => run_cli(cmd),
    }
}
