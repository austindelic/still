//! CLI command dispatch and help rendering.

/// Install command handler.
pub mod install;

use crate::cli::args::{AgentsCommand, Cli, Command, ConfigCommand};
use crate::cli::output::Output;
use crate::cli::runtime::CliRuntime;
use clap::CommandFactory;

/// Dispatches one parsed subcommand to its CLI handler.
///
/// `cmd` contains the command-specific parsed input from Clap. `runtime` is the
/// mutable engine boundary used by handlers that perform real work. `output`
/// receives all user-facing text. The return value is a process-style exit code:
/// `0` for success and nonzero for command failure.
pub fn run_cli<R, O>(cmd: Command, runtime: &mut R, output: &mut O) -> i32
where
    R: CliRuntime,
    O: Output,
{
    match cmd {
        Command::Init(args) => match runtime.init(args.force) {
            Ok(result) => {
                output.success(&format!("Created {}", result.path.display()));
                0
            }
            Err(e) => {
                output.error(&format!("init failed: {e}"));
                1
            }
        },
        Command::Trust(args) => {
            output.info(&format!("Trust command: {:?}", args));
            0
        }
        Command::Install(args) => install::run(args, runtime, output),
        Command::Sync(args) => {
            output.info(&format!("Sync command: {:?}", args));
            0
        }
        Command::List(args) => match runtime.list(args.all) {
            Ok(result) => {
                output.info(&format!("Config: {}", result.path.display()));
                for section in result.sections {
                    output.info(match section.kind {
                        engine::specs::item::ItemKind::Tool => "Tools:",
                        engine::specs::item::ItemKind::Package => "Packages:",
                        engine::specs::item::ItemKind::App => "Apps:",
                    });
                    if section.items.is_empty() {
                        output.info("  (none)");
                    }
                    for item in section.items {
                        let backend = item
                            .backend
                            .map(|backend| format!("@{backend}"))
                            .unwrap_or_default();
                        output.info(&format!("  {}@{}{}", item.name, item.version, backend));
                    }
                }
                0
            }
            Err(e) => {
                output.error(&format!("list failed: {e}"));
                1
            }
        },
        Command::Uninstall(args) => {
            output.info(&format!("Uninstall command: {:?}", args));
            0
        }
        Command::Run(args) => {
            output.info(&format!("Run command: {:?}", args));
            0
        }
        Command::Task(args) => {
            output.info(&format!("Task command: {:?}", args));
            0
        }
        Command::Services(args) => {
            output.info(&format!("Services command: {:?}", args));
            0
        }
        Command::Agents(args) => {
            let operation = match args.command.unwrap_or(AgentsCommand::List) {
                AgentsCommand::List => engine::actions::agents::AgentsOperation::List,
                AgentsCommand::Check => engine::actions::agents::AgentsOperation::Check,
                AgentsCommand::Sync => engine::actions::agents::AgentsOperation::Sync,
            };
            match runtime.agents(operation) {
                Ok(result) => {
                    output.info(&format!("Config: {}", result.path.display()));
                    if result.agents.targets.is_empty() {
                        output.info("Targets: (none)");
                    } else {
                        output.info(&format!("Targets: {}", result.agents.targets.join(", ")));
                    }
                    if let Some(instructions) = result.agents.instructions {
                        output.info(&format!("Instructions: {instructions}"));
                    }
                    output.info("Skills:");
                    if result.agents.skills.is_empty() {
                        output.info("  (none)");
                    }
                    for skill in result.agents.skills {
                        output.info(&format!("  {}", skill.name));
                    }
                    if let Some(path) = result.gitignore_path {
                        output.success(&format!("Updated {}", path.display()));
                    }
                    0
                }
                Err(e) => {
                    output.error(&format!("agents failed: {e}"));
                    1
                }
            }
        }
        Command::Config(args) => match args.command {
            ConfigCommand::Check => match runtime.config_check(args.global) {
                Ok(result) => {
                    output.success(&format!("Config OK: {}", result.path.display()));
                    0
                }
                Err(e) => {
                    output.error(&format!("config check failed: {e}"));
                    1
                }
            },
        },
        Command::Doctor(args) => {
            output.info(&format!("Doctor command: {:?}", args));
            0
        }
        Command::Env(args) => match runtime.env(args.global) {
            Ok(result) => {
                output.info(&format!("Config: {}", result.path.display()));
                for file in result.files {
                    output.info(&format!("env file: {file}"));
                }
                for (key, value) in result.vars {
                    output.info(&format!("{key}={value}"));
                }
                0
            }
            Err(e) => {
                output.error(&format!("env failed: {e}"));
                1
            }
        },
        Command::Activate(args) => {
            output.info(&format!("Activate command: {:?}", args));
            0
        }
    }
}

/// Dispatches a parsed top-level CLI value using command-module help behavior.
///
/// `cli` is parsed input that may or may not contain a subcommand. `runtime` and
/// `output` are passed through to command handlers unchanged. With no subcommand,
/// this function writes generated help and returns the help-render result.
pub fn run_parsed<R, O>(cli: Cli, runtime: &mut R, output: &mut O) -> i32
where
    R: CliRuntime,
    O: Output,
{
    match cli.command {
        Some(cmd) => run_cli(cmd, runtime, output),
        None => run_without_command(output),
    }
}

fn run_without_command<O: Output>(output: &mut O) -> i32 {
    print_help(output)
}

/// Renders and writes top-level help text to an output sink.
///
/// `output` receives the generated help on stdout when rendering succeeds, or a
/// render error on stderr when Clap fails to write help. Returns `0` for rendered
/// help and `1` for rendering failure.
pub fn print_help<O: Output>(output: &mut O) -> i32 {
    match help_text() {
        Ok(help) => {
            output.info(&help);
            0
        }
        Err(e) => {
            output.error(&format!("failed to render help: {e}"));
            1
        }
    }
}

/// Builds the top-level Clap help text used by no-command behavior and tests.
///
/// The returned string always includes a trailing newline so command handlers
/// can pass it directly to `Output::info`. Errors are I/O failures from Clap's
/// help writer.
pub fn help_text() -> std::io::Result<String> {
    let mut command = Cli::command();
    let mut bytes = Vec::new();
    command.write_help(&mut bytes)?;
    bytes.push(b'\n');
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use anyhow::anyhow;
    use engine::actions::{
        agents::{AgentsOperation, AgentsResult},
        config::CheckConfigResult,
        env::EnvResult,
        init::InitResult,
        install::InstallResult,
        list::{ListItem, ListResult, ListSection},
    };
    use engine::specs::agents::{NormalizedAgents, NormalizedSkill, NormalizedSkillSource};
    use engine::specs::item::ItemKind;
    use engine::specs::toml::StillConfig;

    use super::*;
    use crate::cli::args::{ConfigArgs, ConfigCommand};
    use crate::cli::output::BufferedOutput;

    struct FakeRuntime {
        config_check_result: Option<anyhow::Result<CheckConfigResult>>,
        config_check_globals: Vec<bool>,
        init_result: Option<anyhow::Result<InitResult>>,
        init_forces: Vec<bool>,
        env_result: Option<anyhow::Result<EnvResult>>,
        env_globals: Vec<bool>,
        list_result: Option<anyhow::Result<ListResult>>,
        list_alls: Vec<bool>,
        agents_result: Option<anyhow::Result<AgentsResult>>,
        agents_operations: Vec<AgentsOperation>,
    }

    impl CliRuntime for FakeRuntime {
        fn install(
            &mut self,
            _request: crate::cli::runtime::InstallCommandRequest,
        ) -> anyhow::Result<InstallResult> {
            panic!("install should not run in config tests");
        }

        fn config_check(&mut self, global: bool) -> anyhow::Result<CheckConfigResult> {
            self.config_check_globals.push(global);
            self.config_check_result
                .take()
                .expect("test runtime config_check result was not configured")
        }

        fn init(&mut self, force: bool) -> anyhow::Result<InitResult> {
            self.init_forces.push(force);
            self.init_result
                .take()
                .expect("test runtime init result was not configured")
        }

        fn env(&mut self, global: bool) -> anyhow::Result<EnvResult> {
            self.env_globals.push(global);
            self.env_result
                .take()
                .expect("test runtime env result was not configured")
        }

        fn list(&mut self, all: bool) -> anyhow::Result<ListResult> {
            self.list_alls.push(all);
            self.list_result
                .take()
                .expect("test runtime list result was not configured")
        }

        fn agents(&mut self, operation: AgentsOperation) -> anyhow::Result<AgentsResult> {
            self.agents_operations.push(operation);
            self.agents_result
                .take()
                .expect("test runtime agents result was not configured")
        }
    }

    #[test]
    fn config_check_formats_success() {
        let mut runtime = FakeRuntime {
            config_check_result: Some(Ok(CheckConfigResult {
                path: PathBuf::from("/repo/still.toml"),
                config: StillConfig::default(),
            })),
            config_check_globals: Vec::new(),
            init_result: None,
            init_forces: Vec::new(),
            env_result: None,
            env_globals: Vec::new(),
            list_result: None,
            list_alls: Vec::new(),
            agents_result: None,
            agents_operations: Vec::new(),
        };
        let mut output = BufferedOutput::default();

        let code = run_cli(
            Command::Config(ConfigArgs {
                global: false,
                command: ConfigCommand::Check,
            }),
            &mut runtime,
            &mut output,
        );

        assert_eq!(code, 0);
        assert_eq!(runtime.config_check_globals, [false]);
        insta::assert_snapshot!(output.stdout, @r###"
✓ Config OK: /repo/still.toml
"###);
        assert_eq!(output.stderr, "");
    }

    #[test]
    fn config_check_formats_error() {
        let mut runtime = FakeRuntime {
            config_check_result: Some(Err(anyhow!("failed to parse still.toml"))),
            config_check_globals: Vec::new(),
            init_result: None,
            init_forces: Vec::new(),
            env_result: None,
            env_globals: Vec::new(),
            list_result: None,
            list_alls: Vec::new(),
            agents_result: None,
            agents_operations: Vec::new(),
        };
        let mut output = BufferedOutput::default();

        let code = run_cli(
            Command::Config(ConfigArgs {
                global: true,
                command: ConfigCommand::Check,
            }),
            &mut runtime,
            &mut output,
        );

        assert_eq!(code, 1);
        assert_eq!(runtime.config_check_globals, [true]);
        assert_eq!(output.stdout, "");
        insta::assert_snapshot!(output.stderr, @r###"
config check failed: failed to parse still.toml
"###);
    }

    #[test]
    fn init_formats_success() {
        let mut runtime = FakeRuntime {
            config_check_result: None,
            config_check_globals: Vec::new(),
            init_result: Some(Ok(InitResult {
                path: PathBuf::from("/repo/still.toml"),
            })),
            init_forces: Vec::new(),
            env_result: None,
            env_globals: Vec::new(),
            list_result: None,
            list_alls: Vec::new(),
            agents_result: None,
            agents_operations: Vec::new(),
        };
        let mut output = BufferedOutput::default();

        let code = run_cli(
            Command::Init(crate::cli::args::InitArgs { force: true }),
            &mut runtime,
            &mut output,
        );

        assert_eq!(code, 0);
        assert_eq!(runtime.init_forces, [true]);
        insta::assert_snapshot!(output.stdout, @r###"
✓ Created /repo/still.toml
"###);
        assert_eq!(output.stderr, "");
    }

    #[test]
    fn init_formats_error() {
        let mut runtime = FakeRuntime {
            config_check_result: None,
            config_check_globals: Vec::new(),
            init_result: Some(Err(anyhow!("still.toml already exists"))),
            init_forces: Vec::new(),
            env_result: None,
            env_globals: Vec::new(),
            list_result: None,
            list_alls: Vec::new(),
            agents_result: None,
            agents_operations: Vec::new(),
        };
        let mut output = BufferedOutput::default();

        let code = run_cli(
            Command::Init(crate::cli::args::InitArgs { force: false }),
            &mut runtime,
            &mut output,
        );

        assert_eq!(code, 1);
        assert_eq!(runtime.init_forces, [false]);
        assert_eq!(output.stdout, "");
        insta::assert_snapshot!(output.stderr, @r###"
init failed: still.toml already exists
"###);
    }

    #[test]
    fn env_formats_values_and_files() {
        let mut runtime = FakeRuntime {
            config_check_result: None,
            config_check_globals: Vec::new(),
            init_result: None,
            init_forces: Vec::new(),
            env_result: Some(Ok(EnvResult {
                path: PathBuf::from("/repo/still.toml"),
                files: vec![".env".to_string()],
                vars: vec![("RUST_LOG".to_string(), "debug".to_string())],
            })),
            env_globals: Vec::new(),
            list_result: None,
            list_alls: Vec::new(),
            agents_result: None,
            agents_operations: Vec::new(),
        };
        let mut output = BufferedOutput::default();

        let code = run_cli(
            Command::Env(crate::cli::args::EnvArgs { global: true }),
            &mut runtime,
            &mut output,
        );

        assert_eq!(code, 0);
        assert_eq!(runtime.env_globals, [true]);
        insta::assert_snapshot!(output.stdout, @r###"
Config: /repo/still.toml
env file: .env
RUST_LOG=debug
"###);
        assert_eq!(output.stderr, "");
    }

    #[test]
    fn env_formats_error() {
        let mut runtime = FakeRuntime {
            config_check_result: None,
            config_check_globals: Vec::new(),
            init_result: None,
            init_forces: Vec::new(),
            env_result: Some(Err(anyhow!("failed to read still.toml"))),
            env_globals: Vec::new(),
            list_result: None,
            list_alls: Vec::new(),
            agents_result: None,
            agents_operations: Vec::new(),
        };
        let mut output = BufferedOutput::default();

        let code = run_cli(
            Command::Env(crate::cli::args::EnvArgs { global: false }),
            &mut runtime,
            &mut output,
        );

        assert_eq!(code, 1);
        assert_eq!(runtime.env_globals, [false]);
        assert_eq!(output.stdout, "");
        insta::assert_snapshot!(output.stderr, @r###"
env failed: failed to read still.toml
"###);
    }

    #[test]
    fn list_formats_configured_items() {
        let mut runtime = FakeRuntime {
            config_check_result: None,
            config_check_globals: Vec::new(),
            init_result: None,
            init_forces: Vec::new(),
            env_result: None,
            env_globals: Vec::new(),
            list_result: Some(Ok(ListResult {
                path: PathBuf::from("/repo/still.toml"),
                sections: vec![
                    section(ItemKind::Tool, [item("jq", "latest", None)]),
                    section(
                        ItemKind::Package,
                        [
                            item("llvm", "18", Some("homebrew")),
                            item("openssl", "latest", None),
                        ],
                    ),
                    section(ItemKind::App, [item("firefox", "latest", None)]),
                ],
            })),
            list_alls: Vec::new(),
            agents_result: None,
            agents_operations: Vec::new(),
        };
        let mut output = BufferedOutput::default();

        let code = run_cli(
            Command::List(crate::cli::args::ListArgs { all: true }),
            &mut runtime,
            &mut output,
        );

        assert_eq!(code, 0);
        assert_eq!(runtime.list_alls, [true]);
        insta::assert_snapshot!(output.stdout, @r###"
Config: /repo/still.toml
Tools:
  jq@latest
Packages:
  llvm@18@homebrew
  openssl@latest
Apps:
  firefox@latest
"###);
        assert_eq!(output.stderr, "");
    }

    #[test]
    fn list_formats_errors() {
        let mut runtime = FakeRuntime {
            config_check_result: None,
            config_check_globals: Vec::new(),
            init_result: None,
            init_forces: Vec::new(),
            env_result: None,
            env_globals: Vec::new(),
            list_result: Some(Err(anyhow!("failed to read still.toml"))),
            list_alls: Vec::new(),
            agents_result: None,
            agents_operations: Vec::new(),
        };
        let mut output = BufferedOutput::default();

        let code = run_cli(
            Command::List(crate::cli::args::ListArgs { all: false }),
            &mut runtime,
            &mut output,
        );

        assert_eq!(code, 1);
        assert_eq!(runtime.list_alls, [false]);
        assert_eq!(output.stdout, "");
        insta::assert_snapshot!(output.stderr, @r###"
list failed: failed to read still.toml
"###);
    }

    #[test]
    fn agents_sync_formats_config_and_written_gitignore() {
        let mut runtime = FakeRuntime {
            config_check_result: None,
            config_check_globals: Vec::new(),
            init_result: None,
            init_forces: Vec::new(),
            env_result: None,
            env_globals: Vec::new(),
            list_result: None,
            list_alls: Vec::new(),
            agents_result: Some(Ok(AgentsResult {
                path: PathBuf::from("/repo/still.toml"),
                agents: NormalizedAgents {
                    targets: vec!["claude".to_string(), "codex".to_string()],
                    instructions: Some("AGENTS.md".to_string()),
                    skills: vec![normalized_skill("rust-review")],
                },
                gitignore: "# still-managed skills\n/rust-review/\n".to_string(),
                gitignore_path: Some(PathBuf::from("/repo/.agents/skills/.gitignore")),
            })),
            agents_operations: Vec::new(),
        };
        let mut output = BufferedOutput::default();

        let code = run_cli(
            Command::Agents(crate::cli::args::AgentsArgs {
                command: Some(AgentsCommand::Sync),
            }),
            &mut runtime,
            &mut output,
        );

        assert_eq!(code, 0);
        assert_eq!(runtime.agents_operations, [AgentsOperation::Sync]);
        insta::assert_snapshot!(output.stdout, @r###"
Config: /repo/still.toml
Targets: claude, codex
Instructions: AGENTS.md
Skills:
  rust-review
✓ Updated /repo/.agents/skills/.gitignore
"###);
        assert_eq!(output.stderr, "");
    }

    fn section<const N: usize>(kind: ItemKind, items: [ListItem; N]) -> ListSection {
        ListSection {
            kind,
            items: items.into(),
        }
    }

    fn item(name: &str, version: &str, backend: Option<&str>) -> ListItem {
        ListItem {
            name: name.to_string(),
            version: version.to_string(),
            backend: backend.map(str::to_string),
        }
    }

    fn normalized_skill(name: &str) -> NormalizedSkill {
        NormalizedSkill {
            name: name.to_string(),
            source: NormalizedSkillSource::Official {
                name: name.to_string(),
            },
            auto: false,
            tools: Vec::new(),
            packages: Vec::new(),
            apps: Vec::new(),
        }
    }
}
