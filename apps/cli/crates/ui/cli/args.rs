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
    #[cfg(feature = "tui")]
    Tui, // Launch the text-based user interface.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_install_tool_spec_fails_during_parse() {
        let err = match Cli::try_parse_from(["still", "install", "bad/tool"]) {
            Ok(_) => panic!("invalid tool spec should fail to parse"),
            Err(err) => err,
        };

        insta::assert_snapshot!(err.to_string(), @r###"
error: invalid value 'bad/tool' for '<TOOL@VERSION>': Invalid tool name "bad/tool": tool name contains invalid character '/'. Tool names must match: [a-zA-Z][a-zA-Z0-9_-]*

For more information, try '--help'.
"###);
    }
}
