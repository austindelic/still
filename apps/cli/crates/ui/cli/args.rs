use clap::{Parser, Subcommand};
use engine::registries::specs::tool::ToolSpec;

#[derive(Parser)]
#[command(name = "Still", about = "Universal Package Manager + Version Manager")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    Install(InstallArgs),     // Install a package/app into the current environment.
    Uninstall(UninstallArgs), // Remove a package/app from the current environment.
    Use(UseArgs),             // Switch to a specific runtime or toolchain version.
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
    Task,
    Config,
    PostInstall,
}

// Command argument structs
#[derive(clap::Args, Debug, Clone)]
pub struct InstallArgs {
    #[arg(value_name = "TOOL@VERSION")]
    pub tool: ToolSpec,
}

#[derive(clap::Args, Debug, Clone)]
pub struct UninstallArgs {
    #[arg(value_name = "TOOL@VERSION")]
    pub tool: ToolSpec,
}

#[derive(clap::Args, Debug, Clone)]
pub struct UseArgs {
    #[arg(short, long, value_name = "TOOL")]
    pub tool_name: String,
}

#[derive(clap::Args, Debug, Clone)]
pub struct DoctorArgs {}

#[derive(clap::Args, Debug, Clone)]
pub struct RunArgs {
    pub command: Vec<String>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct TranslateArgs {}

#[derive(clap::Args, Debug, Clone)]
pub struct InitArgs {}

#[derive(clap::Args, Debug, Clone)]
pub struct ConvertArgs {}
