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
  init       Initialize configuration for a new project
  trust      Trust project-defined executable behavior
  install    Install requested tools, packages, or apps
  sync       Synchronize installed state with config
  list       List configured and installed items
  uninstall  Remove a Still-managed install
  run        Run a command within the managed environment
  task       Run or list tasks defined in config
  services   Inspect, start, stop, or check services
  agents     Inspect, sync, or validate agent instructions and skills
  config     Inspect, validate, or edit Still configuration
  doctor     Diagnose machine, cache, config, and platform health
  env        Display resolved environment information
  activate   Print shell activation code
  help       Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
"###);
}

#[test]
fn default_build_does_not_expose_tui_command() {
    let help = clap_help();

    assert!(!help.contains("  tui"));
}

#[test]
fn help_does_not_expose_non_priority_commands() {
    let help = clap_help();

    assert!(!help.contains("translate"));
    assert!(!help.contains("convert"));
    assert!(!help.contains("post-install"));
    assert!(!help.contains("  web"));
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
