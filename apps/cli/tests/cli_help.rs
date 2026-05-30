use clap::CommandFactory;
use still::cli::args::Cli;

#[rustfmt::skip]
#[test]
fn default_help_is_generated_by_clap() {
    let help = clap_help();

    insta::assert_snapshot!(help, @r###"
Universal Package Manager + Version Manager

Usage: Still [COMMAND]

Commands:
  install       Install a package/app into the current environment
  uninstall     Remove a package/app from the current environment
  use           Switch to a specific runtime or toolchain version
  doctor        Diagnose the environment and suggest fixes or updates
  run           Run a command within the managed environment
  translate     Translate project definitions between supported formats
  init          Initialize configuration for a new project
  convert       Convert configuration or lockfiles to another supported format
  env           Display environment information required for debugging
  web           Open or run the web-based management dashboard
  activate      Activate a workspace or profile for the current shell session
  sync          Synchronize the workspace state with configured sources
  task          Run or manage tasks defined in config
  config        Inspect, validate, or edit Still configuration
  post-install  Run post-install behavior
  help          Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
"###);
}

#[test]
fn default_build_does_not_expose_tui_command() {
    let help = clap_help();

    assert!(!help.contains("  tui"));
}

fn clap_help() -> String {
    let mut command = Cli::command();
    let mut bytes = Vec::new();
    command
        .write_help(&mut bytes)
        .expect("help output should render");
    bytes.push(b'\n');
    String::from_utf8(bytes).expect("help output should be utf8")
}
