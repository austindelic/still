//! Clap definitions for the public `still` command-line contract.

use clap::{Parser, Subcommand};
use engine::registries::specs::tool::ToolSpec;

/// Top-level CLI parser for the `still` binary.
///
/// This type is the source of truth for command spelling, positional arguments,
/// option names, and generated help text. `Cli::parse()` reads process argv;
/// tests and higher-level routing code should prefer `Cli::try_parse_from(...)`
/// or direct construction when they need deterministic input.
#[derive(Parser)]
#[command(
    name = "Still",
    about = "Universal Package Manager + Version Manager",
    long_about = None
)]
pub struct Cli {
    /// Optional subcommand; absence is handled by build-specific no-command behavior.
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Public subcommands exposed by the `still` binary.
///
/// Keep this enum focused on the input contract: command names, aliases, and
/// argument structs. Command behavior belongs in `cli::commands`, and engine
/// side effects belong behind `CliRuntime`.
#[derive(Subcommand)]
pub enum Command {
    #[command(about = "Initialize configuration for a new project")]
    Init(InitArgs),
    #[command(about = "Trust project-defined executable behavior")]
    Trust(TrustArgs),
    #[command(about = "Install requested tools, packages, or apps")]
    Install(InstallArgs),
    #[command(about = "Synchronize installed state with config")]
    Sync(SyncArgs),
    #[command(about = "List configured and installed items")]
    List(ListArgs),
    #[command(about = "Remove a Still-managed install")]
    Uninstall(UninstallArgs),
    #[command(about = "Run a command within the managed environment")]
    Run(RunArgs),
    #[command(about = "Run or list tasks defined in config")]
    Task(TaskArgs),
    #[command(about = "Inspect, start, stop, or check services")]
    Services(ServicesArgs),
    #[command(about = "Inspect, sync, or validate agent instructions and skills")]
    Agents(AgentsArgs),
    #[command(about = "Inspect, validate, or edit Still configuration")]
    Config(ConfigArgs),
    #[command(about = "Diagnose machine, cache, config, and platform health")]
    Doctor(DoctorArgs),
    #[command(about = "Display resolved environment information")]
    Env(EnvArgs),
    #[command(about = "Print shell activation code")]
    Activate(ActivateArgs),
}

/// Arguments for installing requested tool/package/app specs.
///
/// Group flags classify following values until another flag appears. Each value
/// is parsed before command dispatch, so invalid names, versions, or backend
/// syntax fail as Clap input errors instead of reaching the engine.
#[derive(clap::Args, Debug, Clone)]
pub struct InstallArgs {
    /// Requested tools in `name`, `name@version`, or `name@version@backend` form.
    #[arg(short = 't', long = "tool", value_name = "TOOL", num_args = 1..)]
    pub tools: Vec<ToolSpec>,
    /// Requested packages in `name`, `name@version`, or `name@version@backend` form.
    #[arg(short = 'p', long = "package", value_name = "PACKAGE", num_args = 1..)]
    pub packages: Vec<ToolSpec>,
    /// Requested apps in `name`, `name@version`, or `name@version@backend` form.
    #[arg(short = 'a', long = "app", value_name = "APP", num_args = 1..)]
    pub apps: Vec<ToolSpec>,
}

/// Arguments for uninstalling one requested tool/package/app spec.
///
/// `tool` uses the same syntax as `install` so the remove path can target a
/// specific version later without changing the CLI contract.
#[derive(clap::Args, Debug, Clone)]
pub struct UninstallArgs {
    /// Requested item in `name`, `name@latest`, or `name@version` form.
    #[arg(value_name = "TOOL@VERSION")]
    pub tool: ToolSpec,
}

/// Arguments for project trust.
#[derive(clap::Args, Debug, Clone)]
pub struct TrustArgs {}

/// Arguments for syncing installed state.
#[derive(clap::Args, Debug, Clone)]
pub struct SyncArgs {}

/// Arguments for listing configured and installed state.
#[derive(clap::Args, Debug, Clone)]
pub struct ListArgs {
    /// Include inactive global/project entries and known installed items.
    #[arg(long)]
    pub all: bool,
}

/// Arguments for environment diagnostics.
///
/// This is intentionally empty in v0.1 so `doctor` can gain scoped checks later
/// without changing command routing.
#[derive(clap::Args, Debug, Clone)]
pub struct DoctorArgs {}

/// Arguments for running an arbitrary command inside the managed environment.
///
/// `command` stores the executable name followed by all remaining arguments.
/// The eventual runner should preserve ordering and avoid shell expansion unless
/// the user explicitly asks for shell execution.
#[derive(clap::Args, Debug, Clone)]
pub struct RunArgs {
    /// Child command and arguments to execute.
    pub command: Vec<String>,
}

/// Arguments for task execution.
#[derive(clap::Args, Debug, Clone)]
pub struct TaskArgs {
    /// Task name. When omitted, Still lists configured tasks.
    pub name: Option<String>,
}

/// Arguments for service operations.
#[derive(clap::Args, Debug, Clone)]
pub struct ServicesArgs {
    #[command(subcommand)]
    pub command: Option<ServicesCommand>,
}

/// Service subcommands.
#[derive(Subcommand, Debug, Clone)]
pub enum ServicesCommand {
    Status { name: Option<String> },
    Start { name: Option<String> },
    Stop { name: Option<String> },
    Check { name: Option<String> },
}

/// Arguments for agent operations.
#[derive(clap::Args, Debug, Clone)]
pub struct AgentsArgs {
    #[command(subcommand)]
    pub command: Option<AgentsCommand>,
}

/// Agent subcommands.
#[derive(Subcommand, Debug, Clone)]
pub enum AgentsCommand {
    List,
    Sync,
    Check,
}

/// Arguments for initializing a project config.
///
/// Empty for now; future flags should describe template choice, overwrite
/// behavior, and schema version rather than hiding those choices in prompts.
#[derive(clap::Args, Debug, Clone)]
pub struct InitArgs {}

/// Arguments for config operations.
#[derive(clap::Args, Debug, Clone)]
pub struct ConfigArgs {
    /// Use the global Still config.
    #[arg(short, long)]
    pub global: bool,
    #[command(subcommand)]
    pub command: ConfigCommand,
}

/// Config subcommands.
#[derive(Subcommand, Debug, Clone)]
pub enum ConfigCommand {
    Check,
}

/// Arguments for resolved environment output.
#[derive(clap::Args, Debug, Clone)]
pub struct EnvArgs {}

/// Arguments for shell activation.
#[derive(clap::Args, Debug, Clone)]
pub struct ActivateArgs {
    /// Shell to generate activation code for.
    #[arg(long)]
    pub shell: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_install_tool_spec_fails_during_parse() {
        let err = match Cli::try_parse_from(["still", "install", "--tool", "bad/tool"]) {
            Ok(_) => panic!("invalid tool spec should fail to parse"),
            Err(err) => err,
        };

        insta::assert_snapshot!(err.to_string(), @r###"
error: invalid value 'bad/tool' for '--tool <TOOL>...': Invalid tool spec: item name contains invalid character '/'. Examples: bun@1.3.5, bun@latest, bun@latest@aqua, rust@stable@rustup, bun

For more information, try '--help'.
"###);
    }

    #[test]
    fn install_accepts_grouped_item_flags() {
        let cli = Cli::try_parse_from([
            "still",
            "install",
            "--tool",
            "jq",
            "ripgrep",
            "fd",
            "--package",
            "openssl",
            "llvm",
            "--app",
            "zed",
            "firefox",
        ])
        .expect("grouped install args should parse");

        let Some(Command::Install(args)) = cli.command else {
            panic!("expected install command");
        };

        assert_eq!(names(&args.tools), ["jq", "ripgrep", "fd"]);
        assert_eq!(names(&args.packages), ["openssl", "llvm"]);
        assert_eq!(names(&args.apps), ["zed", "firefox"]);
    }

    #[test]
    fn install_accepts_short_group_flags_and_backend_specs() {
        let cli = Cli::try_parse_from([
            "still",
            "install",
            "-t",
            "rust@stable@rustup",
            "-p",
            "ripgrep@1.0.0@homebrew",
            "-a",
            "firefox@latest@homebrew-cask",
        ])
        .expect("short grouped install args should parse");

        let Some(Command::Install(args)) = cli.command else {
            panic!("expected install command");
        };

        assert_eq!(args.tools[0].name, "rust");
        assert_eq!(args.tools[0].version, "stable");
        assert_eq!(args.tools[0].backend.as_ref().unwrap().as_str(), "rustup");
        assert_eq!(
            args.packages[0].backend.as_ref().unwrap().as_str(),
            "homebrew"
        );
        assert_eq!(
            args.apps[0].backend.as_ref().unwrap().as_str(),
            "homebrew-cask"
        );
    }

    fn names(items: &[ToolSpec]) -> Vec<&str> {
        items.iter().map(|item| item.name.as_str()).collect()
    }
}
