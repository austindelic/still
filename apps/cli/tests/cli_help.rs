use clap::CommandFactory;
use ui::cli::args::Cli;

#[cfg(not(feature = "tui"))]
#[rustfmt::skip]
#[test]
fn default_help_is_generated_by_clap() {
    let help = clap_help();

    insta::assert_snapshot!(help, @r###"
Universal Package Manager + Version Manager

Usage: Still [COMMAND]

Commands:
  install       
  uninstall     
  use           
  doctor        
  run           
  translate     
  init          
  convert       
  env           
  web           
  activate      
  sync          
  task          
  config        
  post-install  
  help          Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
"###);
}

#[cfg(not(feature = "tui"))]
#[test]
fn default_build_does_not_expose_tui_command() {
    let help = clap_help();

    assert!(!help.contains("  tui"));
}

#[cfg(feature = "tui")]
#[test]
fn tui_feature_exposes_tui_command() {
    let help = clap_help();

    assert!(help.contains("  tui"));
}

#[cfg(feature = "tui")]
#[rustfmt::skip]
#[test]
fn tui_feature_help_is_generated_by_clap() {
    let help = clap_help();

    insta::assert_snapshot!(help, @r###"
Universal Package Manager + Version Manager

Usage: Still [COMMAND]

Commands:
  install       
  uninstall     
  use           
  doctor        
  run           
  translate     
  init          
  convert       
  env           
  tui           
  web           
  activate      
  sync          
  task          
  config        
  post-install  
  help          Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
"###);
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
