use crate::commands::{self, AddArgs, AddCommand};
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
    Install {
        tool_name: String,
    },
    Uninstall {
        tool_name: String,
    },
    Use {
        #[arg(short, long)]
        tool_name: String,
    },
    Add(AddArgs),
    Remove {
        tool_name_with_version_number: String,
    },
    Doctor,
    Run {
        task_name: String,
    },
    Translate,
    Init,
    Convert,
}

fn run_cli(cmd: &Commands) {
    match cmd {
        Commands::Install { tool_name } => {
            commands::install(tool_name);
        }
        Commands::Uninstall { tool_name: _ } => {}
        Commands::Use { tool_name: _ } => {}
        Commands::Add(args) => AddCommand::from(args.clone()).run(),
        Commands::Remove {
            tool_name_with_version_number,
        } => commands::remove(tool_name_with_version_number),
        Commands::Run { task_name: _ } => {}
        Commands::Translate => {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime should build");
            if let Err(err) = runtime.block_on(commands::translslte()) {
                eprintln!("translate failed: {err}");
            }
        }
        Commands::Doctor => {}
        Commands::Init => {}
        Commands::Convert => {}
    }
}

pub fn entry() {
    let cli = Cli::parse();

    match &cli.command {
        None => tui::launch_tui().expect("tui broke :/"),
        Some(cmd) => run_cli(cmd),
    }
}
