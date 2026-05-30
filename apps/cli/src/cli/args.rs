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
    #[command(about = "Install a package/app into the current environment")]
    Install(InstallArgs),
    #[command(about = "Remove a package/app from the current environment")]
    Uninstall(UninstallArgs),
    #[command(about = "Switch to a specific runtime or toolchain version")]
    Use(UseArgs),
    #[command(about = "Diagnose the environment and suggest fixes or updates")]
    Doctor(DoctorArgs),
    #[command(about = "Run a command within the managed environment")]
    Run(RunArgs),
    #[command(about = "Translate project definitions between supported formats")]
    Translate(TranslateArgs),
    #[command(about = "Initialize configuration for a new project")]
    Init(InitArgs),
    #[command(about = "Convert configuration or lockfiles to another supported format")]
    Convert(ConvertArgs),
    #[command(about = "Display environment information required for debugging")]
    Env,
    #[command(about = "Open or run the web-based management dashboard")]
    Web,
    #[command(about = "Activate a workspace or profile for the current shell session")]
    Activate,
    #[command(about = "Synchronize the workspace state with configured sources")]
    Sync,
    #[command(about = "Run or manage tasks defined in config")]
    Task,
    #[command(about = "Inspect, validate, or edit Still configuration")]
    Config,
    #[command(about = "Run post-install behavior")]
    PostInstall,
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

/// Arguments for selecting a tool entry in project configuration.
///
/// `tool_name` is currently a plain string because the final config mutation
/// flow is still being designed; validate or normalize it before writing config.
#[derive(clap::Args, Debug, Clone)]
pub struct UseArgs {
    /// Tool name to select or update.
    #[arg(short, long, value_name = "TOOL")]
    pub tool_name: String,
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

/// Arguments for translating project definitions between formats.
///
/// Empty for now while the supported input/output formats are still being
/// narrowed into an explicit contract.
#[derive(clap::Args, Debug, Clone)]
pub struct TranslateArgs {}

/// Arguments for initializing a project config.
///
/// Empty for now; future flags should describe template choice, overwrite
/// behavior, and schema version rather than hiding those choices in prompts.
#[derive(clap::Args, Debug, Clone)]
pub struct InitArgs {}

/// Arguments for converting Still-owned config or lockfiles.
///
/// Empty for now while conversion targets and write policy are still undefined.
#[derive(clap::Args, Debug, Clone)]
pub struct ConvertArgs {}

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
