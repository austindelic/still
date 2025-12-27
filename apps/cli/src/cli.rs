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
    Install(InstallArgs),
    Uninstall(UninstallArgs),
    Use(UseArgs),
    Add(AddArgs),
    Remove(RemoveArgs),
    Doctor(DoctorArgs),
    Run(RunArgs),
    Translate(TranslateArgs),
    Init(InitArgs),
    Convert(ConvertArgs),
    Env,
    Tui,
    Web,
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
