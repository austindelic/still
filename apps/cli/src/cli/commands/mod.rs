//! CLI command dispatch and help rendering.

/// Install command handler.
pub mod install;

use crate::cli::args::{AgentsCommand, Cli, Command, ConfigCommand, ServicesCommand};
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
        Command::Trust(_args) => match runtime.trust() {
            Ok(result) => {
                output.success(&format!("Trusted {}", result.config_path.display()));
                output.info(&format!("Marker: {}", result.trust_path.display()));
                0
            }
            Err(e) => {
                output.error(&format!("trust failed: {e}"));
                1
            }
        },
        Command::Install(args) => install::run(args, runtime, output),
        Command::Sync(args) => match runtime.sync(args.global) {
            Ok(result) => {
                output.info(&format!("Config: {}", result.path.display()));
                output.info(&format!("Lockfile: {}", result.lockfile_path.display()));
                if result.drift.is_empty() {
                    output.info("Drift: none");
                } else {
                    output.info("Drift:");
                    for drift in result.drift {
                        output.info(&format!("  {}", sync_drift_label(drift)));
                    }
                }
                if result.missing.is_empty() {
                    output.info("Missing: none");
                } else {
                    output.info("Missing:");
                    for item in &result.missing {
                        output.info(&format!(
                            "  {} {}@{}",
                            item.kind, item.spec.name, item.spec.version
                        ));
                    }
                }
                if !result.installed.is_empty() {
                    output.info("Installed:");
                    for item in &result.installed {
                        output.info(&format!(
                            "  {} {}@{}",
                            item.kind, item.spec.name, item.spec.version
                        ));
                    }
                }
                output.info("Sync plan:");
                if result.items.is_empty() {
                    output.info("  (nothing to sync)");
                }
                for item in result.items {
                    let backend = item
                        .spec
                        .backend
                        .as_ref()
                        .map(|backend| format!("@{backend}"))
                        .unwrap_or_default();
                    output.info(&format!(
                        "  {} {}@{}{}",
                        item.kind, item.spec.name, item.spec.version, backend
                    ));
                }
                0
            }
            Err(e) => {
                output.error(&format!("sync failed: {e}"));
                1
            }
        },
        Command::List(args) => match runtime.list(args.all, args.global) {
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
                        let status = match (item.project, item.global, item.installed) {
                            (true, true, true) => " (project, global, installed)",
                            (true, true, false) => " (project, global)",
                            (true, false, true) => " (configured, installed)",
                            (true, false, false) => "",
                            (false, true, true) => " (global, installed)",
                            (false, true, false) => " (global)",
                            (false, false, true) => " (installed)",
                            (false, false, false) => "",
                        };
                        output.info(&format!(
                            "  {}@{}{}{}",
                            item.name, item.version, backend, status
                        ));
                        for output_path in item.outputs {
                            output.info(&format!("    output: {output_path}"));
                        }
                        for linked in item.linked_executables {
                            output.info(&format!("    linked: {linked}"));
                        }
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
            let Some((kind, spec)) = args.target() else {
                output.error("uninstall failed: expected a tool, package, app, or item target");
                return 1;
            };
            match runtime.uninstall(uninstall_target(kind, spec), args.global) {
                Ok(result) => {
                    output.success(&format!(
                        "Removed {} {} from {}",
                        result.kind,
                        result.name,
                        result.path.display()
                    ));
                    for path in result.removed_paths {
                        output.info(&format!("Removed artifact {}", path.display()));
                    }
                    0
                }
                Err(e) => {
                    output.error(&format!("uninstall failed: {e}"));
                    1
                }
            }
        }
        Command::Run(args) => {
            match runtime.run_command(normalize_child_command(args.command), args.global) {
                Ok(result) => {
                    write_child_output(output, &result.stdout, false);
                    write_child_output(output, &result.stderr, true);
                    result.status
                }
                Err(e) => {
                    output.error(&format!("run failed: {e}"));
                    1
                }
            }
        }
        Command::Task(args) => match runtime.task(args.name, args.global) {
            Ok(result) => {
                if result.executions.is_empty() {
                    output.info(&format!("Config: {}", result.path.display()));
                    output.info("Tasks:");
                    if result.tasks.is_empty() {
                        output.info("  (none)");
                    }
                    for task in result.tasks {
                        match task.description {
                            Some(description) => {
                                output.info(&format!("  {} - {}", task.name, description));
                            }
                            None => output.info(&format!("  {}", task.name)),
                        }
                    }
                    0
                } else {
                    for execution in result.executions {
                        output.info(&format!("task {}: {}", execution.task, execution.command));
                        write_child_output(output, &execution.stdout, false);
                        write_child_output(output, &execution.stderr, true);
                    }
                    result.status
                }
            }
            Err(e) => {
                output.error(&format!("task failed: {e}"));
                1
            }
        },
        Command::Services(args) => {
            let (operation, name) = match args
                .command
                .unwrap_or(ServicesCommand::Status { name: None })
            {
                ServicesCommand::Status { name } => {
                    (engine::actions::services::ServicesOperation::Status, name)
                }
                ServicesCommand::Start { name } => {
                    (engine::actions::services::ServicesOperation::Start, name)
                }
                ServicesCommand::Stop { name } => {
                    (engine::actions::services::ServicesOperation::Stop, name)
                }
                ServicesCommand::Check { name } => {
                    (engine::actions::services::ServicesOperation::Check, name)
                }
            };
            match runtime.services(operation, name, args.global) {
                Ok(result) => {
                    output.info(&format!("Config: {}", result.path.display()));
                    for service in result.services {
                        output.info(&format!(
                            "{}: {} - {}",
                            service.name,
                            service_status_label(service.status),
                            service.detail
                        ));
                        if let Some(execution) = service.execution {
                            output.info(&format!("command: {}", execution.command));
                            write_child_output(output, &execution.stdout, false);
                            write_child_output(output, &execution.stderr, true);
                        }
                    }
                    0
                }
                Err(e) => {
                    output.error(&format!("services failed: {e}"));
                    1
                }
            }
        }
        Command::Agents(args) => {
            let operation = match args.command.unwrap_or(AgentsCommand::List) {
                AgentsCommand::List => engine::actions::agents::AgentsOperation::List,
                AgentsCommand::Check => engine::actions::agents::AgentsOperation::Check,
                AgentsCommand::Sync {
                    accept_auto_deps: false,
                } => engine::actions::agents::AgentsOperation::Sync,
                AgentsCommand::Sync {
                    accept_auto_deps: true,
                } => engine::actions::agents::AgentsOperation::SyncAcceptAutoDependencies,
            };
            match runtime.agents(operation, args.global) {
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
                    if matches!(
                        operation,
                        engine::actions::agents::AgentsOperation::Sync
                            | engine::actions::agents::AgentsOperation::SyncAcceptAutoDependencies
                    ) && !result.auto_added.is_empty()
                    {
                        output.info("Auto-added dependencies:");
                        for item in result.auto_added {
                            output.info(&format!("  {}", install_item_label(&item)));
                        }
                    } else if !result.pending_auto_dependencies.is_empty() {
                        output.info("Auto dependencies to add on sync:");
                        for item in result.pending_auto_dependencies {
                            output.info(&format!("  {}", install_item_label(&item)));
                        }
                    }
                    if !result.missing_dependencies.is_empty() {
                        output.info("Missing skill dependencies:");
                        for item in result.missing_dependencies {
                            output.info(&format!("  {}", install_item_label(&item)));
                        }
                    }
                    if let Some(path) = result.gitignore_path {
                        output.success(&format!("Updated {}", path.display()));
                    }
                    for path in result.target_manifests {
                        output.success(&format!("Updated {}", path.display()));
                    }
                    if result.review_required {
                        output.error(
                            "agents sync needs --accept-auto-deps before writing auto dependencies",
                        );
                        return 1;
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
        Command::Doctor(_args) => match runtime.doctor() {
            Ok(result) => {
                for check in result.checks {
                    let status = match check.status {
                        engine::actions::doctor::DoctorStatus::Ok => "ok",
                        engine::actions::doctor::DoctorStatus::Warning => "warn",
                        engine::actions::doctor::DoctorStatus::Error => "error",
                    };
                    output.info(&format!("[{status}] {}: {}", check.name, check.detail));
                }
                0
            }
            Err(e) => {
                output.error(&format!("doctor failed: {e}"));
                1
            }
        },
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
        Command::Activate(args) => match runtime.activate(args.shell, args.global) {
            Ok(result) => {
                output.info(&result.code);
                0
            }
            Err(e) => {
                output.error(&format!("activate failed: {e}"));
                1
            }
        },
    }
}

fn uninstall_target(
    kind: Option<engine::specs::item::ItemKind>,
    spec: &crate::cli::args::UninstallSpec,
) -> engine::actions::uninstall::UninstallTarget {
    engine::actions::uninstall::UninstallTarget {
        kind,
        name: spec.spec.name.clone(),
        version: spec.spec.version.clone(),
        backend: spec.spec.backend.clone(),
        exact: spec.exact,
    }
}

fn service_status_label(status: engine::actions::services::ServiceStatus) -> &'static str {
    match status {
        engine::actions::services::ServiceStatus::Configured => "configured",
        engine::actions::services::ServiceStatus::Skipped => "skipped",
        engine::actions::services::ServiceStatus::Ok => "ok",
        engine::actions::services::ServiceStatus::Failed => "failed",
    }
}

fn install_item_label(item: &engine::actions::install::InstallItemRequest) -> String {
    let backend = item
        .spec
        .backend
        .as_ref()
        .map(|backend| format!("@{backend}"))
        .unwrap_or_default();
    format!(
        "{} {}@{}{}",
        item.kind, item.spec.name, item.spec.version, backend
    )
}

fn sync_drift_label(drift: engine::actions::sync::SyncDrift) -> &'static str {
    match drift {
        engine::actions::sync::SyncDrift::LockfileMissing => "lockfile missing",
        engine::actions::sync::SyncDrift::LockfileOutdated => "lockfile outdated",
    }
}

fn write_child_output<O: Output>(output: &mut O, content: &str, stderr: bool) {
    for line in content.lines() {
        if stderr {
            output.error(line);
        } else {
            output.info(line);
        }
    }
}

fn normalize_child_command(command: Vec<String>) -> Vec<String> {
    let mut removed_separator = false;
    command
        .into_iter()
        .filter(|arg| {
            if !removed_separator && arg == "--" {
                removed_separator = true;
                return false;
            }
            true
        })
        .collect()
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
        activate::{ActivateResult, ShellKind},
        agents::{AgentsOperation, AgentsResult},
        config::CheckConfigResult,
        doctor::{DoctorCheck, DoctorResult, DoctorStatus},
        env::EnvResult,
        init::InitResult,
        install::{InstallItemRequest, InstallResult},
        list::{ListItem, ListResult, ListSection},
        run::RunResult,
        services::{ServiceReport, ServiceStatus, ServicesOperation, ServicesResult},
        sync::{SyncDrift, SyncItem, SyncResult},
        task::{TaskExecution, TaskResult, TaskSummary},
        trust::TrustResult,
        uninstall::{UninstallResult, UninstallTarget},
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
        agents_globals: Vec<bool>,
        run_result: Option<anyhow::Result<RunResult>>,
        run_commands: Vec<Vec<String>>,
        run_globals: Vec<bool>,
        task_result: Option<anyhow::Result<TaskResult>>,
        task_names: Vec<Option<String>>,
        task_globals: Vec<bool>,
        activate_result: Option<anyhow::Result<ActivateResult>>,
        activate_shells: Vec<Option<String>>,
        activate_globals: Vec<bool>,
        doctor_result: Option<anyhow::Result<DoctorResult>>,
        sync_result: Option<anyhow::Result<SyncResult>>,
        services_result: Option<anyhow::Result<ServicesResult>>,
        services_requests: Vec<(ServicesOperation, Option<String>, bool)>,
        trust_result: Option<anyhow::Result<TrustResult>>,
        uninstall_result: Option<anyhow::Result<UninstallResult>>,
        uninstall_targets: Vec<UninstallTarget>,
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

        fn list(&mut self, all: bool, _global: bool) -> anyhow::Result<ListResult> {
            self.list_alls.push(all);
            self.list_result
                .take()
                .expect("test runtime list result was not configured")
        }

        fn agents(
            &mut self,
            operation: AgentsOperation,
            global: bool,
        ) -> anyhow::Result<AgentsResult> {
            self.agents_operations.push(operation);
            self.agents_globals.push(global);
            self.agents_result
                .take()
                .expect("test runtime agents result was not configured")
        }

        fn run_command(&mut self, command: Vec<String>, global: bool) -> anyhow::Result<RunResult> {
            self.run_commands.push(command);
            self.run_globals.push(global);
            self.run_result
                .take()
                .expect("test runtime run result was not configured")
        }

        fn task(&mut self, name: Option<String>, global: bool) -> anyhow::Result<TaskResult> {
            self.task_names.push(name);
            self.task_globals.push(global);
            self.task_result
                .take()
                .expect("test runtime task result was not configured")
        }

        fn activate(
            &mut self,
            shell: Option<String>,
            global: bool,
        ) -> anyhow::Result<ActivateResult> {
            self.activate_shells.push(shell);
            self.activate_globals.push(global);
            self.activate_result
                .take()
                .expect("test runtime activate result was not configured")
        }

        fn doctor(&mut self) -> anyhow::Result<DoctorResult> {
            self.doctor_result
                .take()
                .expect("test runtime doctor result was not configured")
        }

        fn sync(&mut self, _global: bool) -> anyhow::Result<SyncResult> {
            self.sync_result
                .take()
                .expect("test runtime sync result was not configured")
        }

        fn services(
            &mut self,
            operation: ServicesOperation,
            name: Option<String>,
            global: bool,
        ) -> anyhow::Result<ServicesResult> {
            self.services_requests.push((operation, name, global));
            self.services_result
                .take()
                .expect("test runtime services result was not configured")
        }

        fn trust(&mut self) -> anyhow::Result<TrustResult> {
            self.trust_result
                .take()
                .expect("test runtime trust result was not configured")
        }

        fn uninstall(
            &mut self,
            target: UninstallTarget,
            _global: bool,
        ) -> anyhow::Result<UninstallResult> {
            self.uninstall_targets.push(target);
            self.uninstall_result
                .take()
                .expect("test runtime uninstall result was not configured")
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
            agents_globals: Vec::new(),
            run_result: None,
            run_commands: Vec::new(),
            run_globals: Vec::new(),
            task_result: None,
            task_names: Vec::new(),
            task_globals: Vec::new(),
            activate_result: None,
            activate_shells: Vec::new(),
            activate_globals: Vec::new(),
            doctor_result: None,
            sync_result: None,
            services_result: None,
            services_requests: Vec::new(),
            trust_result: None,
            uninstall_result: None,
            uninstall_targets: Vec::new(),
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
            agents_globals: Vec::new(),
            run_result: None,
            run_commands: Vec::new(),
            run_globals: Vec::new(),
            task_result: None,
            task_names: Vec::new(),
            task_globals: Vec::new(),
            activate_result: None,
            activate_shells: Vec::new(),
            activate_globals: Vec::new(),
            doctor_result: None,
            sync_result: None,
            services_result: None,
            services_requests: Vec::new(),
            trust_result: None,
            uninstall_result: None,
            uninstall_targets: Vec::new(),
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
            agents_globals: Vec::new(),
            run_result: None,
            run_commands: Vec::new(),
            run_globals: Vec::new(),
            task_result: None,
            task_names: Vec::new(),
            task_globals: Vec::new(),
            activate_result: None,
            activate_shells: Vec::new(),
            activate_globals: Vec::new(),
            doctor_result: None,
            sync_result: None,
            services_result: None,
            services_requests: Vec::new(),
            trust_result: None,
            uninstall_result: None,
            uninstall_targets: Vec::new(),
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
            agents_globals: Vec::new(),
            run_result: None,
            run_commands: Vec::new(),
            run_globals: Vec::new(),
            task_result: None,
            task_names: Vec::new(),
            task_globals: Vec::new(),
            activate_result: None,
            activate_shells: Vec::new(),
            activate_globals: Vec::new(),
            doctor_result: None,
            sync_result: None,
            services_result: None,
            services_requests: Vec::new(),
            trust_result: None,
            uninstall_result: None,
            uninstall_targets: Vec::new(),
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
            agents_globals: Vec::new(),
            run_result: None,
            run_commands: Vec::new(),
            run_globals: Vec::new(),
            task_result: None,
            task_names: Vec::new(),
            task_globals: Vec::new(),
            activate_result: None,
            activate_shells: Vec::new(),
            activate_globals: Vec::new(),
            doctor_result: None,
            sync_result: None,
            services_result: None,
            services_requests: Vec::new(),
            trust_result: None,
            uninstall_result: None,
            uninstall_targets: Vec::new(),
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
            agents_globals: Vec::new(),
            run_result: None,
            run_commands: Vec::new(),
            run_globals: Vec::new(),
            task_result: None,
            task_names: Vec::new(),
            task_globals: Vec::new(),
            activate_result: None,
            activate_shells: Vec::new(),
            activate_globals: Vec::new(),
            doctor_result: None,
            sync_result: None,
            services_result: None,
            services_requests: Vec::new(),
            trust_result: None,
            uninstall_result: None,
            uninstall_targets: Vec::new(),
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
                    section(
                        ItemKind::Tool,
                        [item("jq", "latest", None), global_item("node", "22", None)],
                    ),
                    section(
                        ItemKind::Package,
                        [
                            item("llvm", "18", Some("homebrew")),
                            item("openssl", "latest", None),
                        ],
                    ),
                    section(
                        ItemKind::App,
                        [
                            item("firefox", "latest", None),
                            installed_item("zed", "latest", Some("homebrew-cask")),
                        ],
                    ),
                ],
            })),
            list_alls: Vec::new(),
            agents_result: None,
            agents_operations: Vec::new(),
            agents_globals: Vec::new(),
            run_result: None,
            run_commands: Vec::new(),
            run_globals: Vec::new(),
            task_result: None,
            task_names: Vec::new(),
            task_globals: Vec::new(),
            activate_result: None,
            activate_shells: Vec::new(),
            activate_globals: Vec::new(),
            doctor_result: None,
            sync_result: None,
            services_result: None,
            services_requests: Vec::new(),
            trust_result: None,
            uninstall_result: None,
            uninstall_targets: Vec::new(),
        };
        let mut output = BufferedOutput::default();

        let code = run_cli(
            Command::List(crate::cli::args::ListArgs {
                global: false,
                all: true,
            }),
            &mut runtime,
            &mut output,
        );

        assert_eq!(code, 0);
        assert_eq!(runtime.list_alls, [true]);
        insta::assert_snapshot!(output.stdout, @r###"
Config: /repo/still.toml
Tools:
  jq@latest
  node@22 (global)
Packages:
  llvm@18@homebrew
  openssl@latest
Apps:
  firefox@latest
  zed@latest@homebrew-cask (installed)
    output: /opt/still/apps/zed/latest
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
            agents_globals: Vec::new(),
            run_result: None,
            run_commands: Vec::new(),
            run_globals: Vec::new(),
            task_result: None,
            task_names: Vec::new(),
            task_globals: Vec::new(),
            activate_result: None,
            activate_shells: Vec::new(),
            activate_globals: Vec::new(),
            doctor_result: None,
            sync_result: None,
            services_result: None,
            services_requests: Vec::new(),
            trust_result: None,
            uninstall_result: None,
            uninstall_targets: Vec::new(),
        };
        let mut output = BufferedOutput::default();

        let code = run_cli(
            Command::List(crate::cli::args::ListArgs {
                global: false,
                all: false,
            }),
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
                target_manifests: vec![
                    PathBuf::from("/repo/.agents/targets/claude.toml"),
                    PathBuf::from("/repo/.agents/targets/codex.toml"),
                ],
                pending_auto_dependencies: Vec::new(),
                auto_added: Vec::new(),
                missing_dependencies: Vec::new(),
                review_required: false,
            })),
            agents_operations: Vec::new(),
            agents_globals: Vec::new(),
            run_result: None,
            run_commands: Vec::new(),
            run_globals: Vec::new(),
            task_result: None,
            task_names: Vec::new(),
            task_globals: Vec::new(),
            activate_result: None,
            activate_shells: Vec::new(),
            activate_globals: Vec::new(),
            doctor_result: None,
            sync_result: None,
            services_result: None,
            services_requests: Vec::new(),
            trust_result: None,
            uninstall_result: None,
            uninstall_targets: Vec::new(),
        };
        let mut output = BufferedOutput::default();

        let code = run_cli(
            Command::Agents(crate::cli::args::AgentsArgs {
                global: false,
                command: Some(AgentsCommand::Sync {
                    accept_auto_deps: false,
                }),
            }),
            &mut runtime,
            &mut output,
        );

        assert_eq!(code, 0);
        assert_eq!(runtime.agents_operations, [AgentsOperation::Sync]);
        assert_eq!(runtime.agents_globals, [false]);
        insta::assert_snapshot!(output.stdout, @r###"
Config: /repo/still.toml
Targets: claude, codex
Instructions: AGENTS.md
Skills:
  rust-review
✓ Updated /repo/.agents/skills/.gitignore
✓ Updated /repo/.agents/targets/claude.toml
✓ Updated /repo/.agents/targets/codex.toml
"###);
        assert_eq!(output.stderr, "");
    }

    #[test]
    fn agents_check_formats_pending_auto_dependencies() {
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
                    targets: vec!["codex".to_string()],
                    instructions: Some("AGENTS.md".to_string()),
                    skills: vec![normalized_skill("rust-review")],
                },
                gitignore: "# still-managed skills\n/rust-review/\n".to_string(),
                gitignore_path: None,
                target_manifests: Vec::new(),
                pending_auto_dependencies: vec![InstallItemRequest {
                    kind: ItemKind::Tool,
                    spec: "cargo-nextest@0.9.99@cargo".parse().unwrap(),
                    tool: Default::default(),
                }],
                auto_added: Vec::new(),
                missing_dependencies: Vec::new(),
                review_required: false,
            })),
            agents_operations: Vec::new(),
            agents_globals: Vec::new(),
            run_result: None,
            run_commands: Vec::new(),
            run_globals: Vec::new(),
            task_result: None,
            task_names: Vec::new(),
            task_globals: Vec::new(),
            activate_result: None,
            activate_shells: Vec::new(),
            activate_globals: Vec::new(),
            doctor_result: None,
            sync_result: None,
            services_result: None,
            services_requests: Vec::new(),
            trust_result: None,
            uninstall_result: None,
            uninstall_targets: Vec::new(),
        };
        let mut output = BufferedOutput::default();

        let code = run_cli(
            Command::Agents(crate::cli::args::AgentsArgs {
                global: true,
                command: Some(AgentsCommand::Check),
            }),
            &mut runtime,
            &mut output,
        );

        assert_eq!(code, 0);
        assert_eq!(runtime.agents_operations, [AgentsOperation::Check]);
        assert_eq!(runtime.agents_globals, [true]);
        insta::assert_snapshot!(output.stdout, @r###"
Config: /repo/still.toml
Targets: codex
Instructions: AGENTS.md
Skills:
  rust-review
Auto dependencies to add on sync:
  tool cargo-nextest@0.9.99@cargo
"###);
        assert_eq!(output.stderr, "");
    }

    #[test]
    fn agents_sync_requires_auto_dependency_review_acknowledgement() {
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
                    targets: vec!["codex".to_string()],
                    instructions: Some("AGENTS.md".to_string()),
                    skills: vec![normalized_skill("rust-review")],
                },
                gitignore: "# still-managed skills\n/rust-review/\n".to_string(),
                gitignore_path: None,
                target_manifests: Vec::new(),
                pending_auto_dependencies: vec![InstallItemRequest {
                    kind: ItemKind::Package,
                    spec: "llvm".parse().unwrap(),
                    tool: Default::default(),
                }],
                auto_added: Vec::new(),
                missing_dependencies: Vec::new(),
                review_required: true,
            })),
            agents_operations: Vec::new(),
            agents_globals: Vec::new(),
            run_result: None,
            run_commands: Vec::new(),
            run_globals: Vec::new(),
            task_result: None,
            task_names: Vec::new(),
            task_globals: Vec::new(),
            activate_result: None,
            activate_shells: Vec::new(),
            activate_globals: Vec::new(),
            doctor_result: None,
            sync_result: None,
            services_result: None,
            services_requests: Vec::new(),
            trust_result: None,
            uninstall_result: None,
            uninstall_targets: Vec::new(),
        };
        let mut output = BufferedOutput::default();

        let code = run_cli(
            Command::Agents(crate::cli::args::AgentsArgs {
                global: false,
                command: Some(AgentsCommand::Sync {
                    accept_auto_deps: false,
                }),
            }),
            &mut runtime,
            &mut output,
        );

        assert_eq!(code, 1);
        assert_eq!(runtime.agents_operations, [AgentsOperation::Sync]);
        assert_eq!(runtime.agents_globals, [false]);
        insta::assert_snapshot!(output.stdout, @r###"
Config: /repo/still.toml
Targets: codex
Instructions: AGENTS.md
Skills:
  rust-review
Auto dependencies to add on sync:
  package llvm@latest
"###);
        insta::assert_snapshot!(output.stderr, @r###"
agents sync needs --accept-auto-deps before writing auto dependencies
"###);
    }

    #[test]
    fn run_formats_child_output_and_returns_child_status() {
        let mut runtime = FakeRuntime {
            config_check_result: None,
            config_check_globals: Vec::new(),
            init_result: None,
            init_forces: Vec::new(),
            env_result: None,
            env_globals: Vec::new(),
            list_result: None,
            list_alls: Vec::new(),
            agents_result: None,
            agents_operations: Vec::new(),
            agents_globals: Vec::new(),
            run_result: Some(Ok(RunResult {
                status: 7,
                stdout: "hello\nworld\n".to_string(),
                stderr: "warn\n".to_string(),
            })),
            run_commands: Vec::new(),
            run_globals: Vec::new(),
            task_result: None,
            task_names: Vec::new(),
            task_globals: Vec::new(),
            activate_result: None,
            activate_shells: Vec::new(),
            activate_globals: Vec::new(),
            doctor_result: None,
            sync_result: None,
            services_result: None,
            services_requests: Vec::new(),
            trust_result: None,
            uninstall_result: None,
            uninstall_targets: Vec::new(),
        };
        let mut output = BufferedOutput::default();

        let code = run_cli(
            Command::Run(crate::cli::args::RunArgs {
                global: true,
                command: vec![
                    "cargo".to_string(),
                    "test".to_string(),
                    "--quiet".to_string(),
                ],
            }),
            &mut runtime,
            &mut output,
        );

        assert_eq!(code, 7);
        assert_eq!(
            runtime.run_commands,
            [vec![
                "cargo".to_string(),
                "test".to_string(),
                "--quiet".to_string()
            ]]
        );
        assert_eq!(runtime.run_globals, [true]);
        insta::assert_snapshot!(output.stdout, @r###"
hello
world
"###);
        insta::assert_snapshot!(output.stderr, @r###"
warn
"###);
    }

    #[test]
    fn run_formats_runtime_errors() {
        let mut runtime = FakeRuntime {
            config_check_result: None,
            config_check_globals: Vec::new(),
            init_result: None,
            init_forces: Vec::new(),
            env_result: None,
            env_globals: Vec::new(),
            list_result: None,
            list_alls: Vec::new(),
            agents_result: None,
            agents_operations: Vec::new(),
            agents_globals: Vec::new(),
            run_result: Some(Err(anyhow!("failed to run cargo"))),
            run_commands: Vec::new(),
            run_globals: Vec::new(),
            task_result: None,
            task_names: Vec::new(),
            task_globals: Vec::new(),
            activate_result: None,
            activate_shells: Vec::new(),
            activate_globals: Vec::new(),
            doctor_result: None,
            sync_result: None,
            services_result: None,
            services_requests: Vec::new(),
            trust_result: None,
            uninstall_result: None,
            uninstall_targets: Vec::new(),
        };
        let mut output = BufferedOutput::default();

        let code = run_cli(
            Command::Run(crate::cli::args::RunArgs {
                global: false,
                command: vec!["cargo".to_string()],
            }),
            &mut runtime,
            &mut output,
        );

        assert_eq!(code, 1);
        assert_eq!(output.stdout, "");
        insta::assert_snapshot!(output.stderr, @r###"
run failed: failed to run cargo
"###);
    }

    #[test]
    fn run_strips_argument_separator_before_runtime() {
        let mut runtime = FakeRuntime {
            config_check_result: None,
            config_check_globals: Vec::new(),
            init_result: None,
            init_forces: Vec::new(),
            env_result: None,
            env_globals: Vec::new(),
            list_result: None,
            list_alls: Vec::new(),
            agents_result: None,
            agents_operations: Vec::new(),
            agents_globals: Vec::new(),
            run_result: Some(Ok(RunResult {
                status: 0,
                stdout: String::new(),
                stderr: String::new(),
            })),
            run_commands: Vec::new(),
            run_globals: Vec::new(),
            task_result: None,
            task_names: Vec::new(),
            task_globals: Vec::new(),
            activate_result: None,
            activate_shells: Vec::new(),
            activate_globals: Vec::new(),
            doctor_result: None,
            sync_result: None,
            services_result: None,
            services_requests: Vec::new(),
            trust_result: None,
            uninstall_result: None,
            uninstall_targets: Vec::new(),
        };
        let mut output = BufferedOutput::default();

        let code = run_cli(
            Command::Run(crate::cli::args::RunArgs {
                global: false,
                command: vec![
                    "cargo".to_string(),
                    "test".to_string(),
                    "--".to_string(),
                    "--quiet".to_string(),
                ],
            }),
            &mut runtime,
            &mut output,
        );

        assert_eq!(code, 0);
        assert_eq!(
            runtime.run_commands,
            [vec![
                "cargo".to_string(),
                "test".to_string(),
                "--quiet".to_string()
            ]]
        );
    }

    #[test]
    fn task_without_name_lists_tasks() {
        let mut runtime = FakeRuntime {
            config_check_result: None,
            config_check_globals: Vec::new(),
            init_result: None,
            init_forces: Vec::new(),
            env_result: None,
            env_globals: Vec::new(),
            list_result: None,
            list_alls: Vec::new(),
            agents_result: None,
            agents_operations: Vec::new(),
            agents_globals: Vec::new(),
            run_result: None,
            run_commands: Vec::new(),
            run_globals: Vec::new(),
            task_result: Some(Ok(TaskResult {
                path: PathBuf::from("/repo/still.toml"),
                tasks: vec![
                    TaskSummary {
                        name: "lint".to_string(),
                        description: Some("Run lint".to_string()),
                    },
                    TaskSummary {
                        name: "test".to_string(),
                        description: None,
                    },
                ],
                executions: Vec::new(),
                status: 0,
            })),
            task_names: Vec::new(),
            task_globals: Vec::new(),
            activate_result: None,
            activate_shells: Vec::new(),
            activate_globals: Vec::new(),
            doctor_result: None,
            sync_result: None,
            services_result: None,
            services_requests: Vec::new(),
            trust_result: None,
            uninstall_result: None,
            uninstall_targets: Vec::new(),
        };
        let mut output = BufferedOutput::default();

        let code = run_cli(
            Command::Task(crate::cli::args::TaskArgs {
                global: true,
                name: None,
            }),
            &mut runtime,
            &mut output,
        );

        assert_eq!(code, 0);
        assert_eq!(runtime.task_names, [None]);
        assert_eq!(runtime.task_globals, [true]);
        insta::assert_snapshot!(output.stdout, @r###"
Config: /repo/still.toml
Tasks:
  lint - Run lint
  test
"###);
        assert_eq!(output.stderr, "");
    }

    #[test]
    fn task_with_name_formats_executions_and_status() {
        let mut runtime = FakeRuntime {
            config_check_result: None,
            config_check_globals: Vec::new(),
            init_result: None,
            init_forces: Vec::new(),
            env_result: None,
            env_globals: Vec::new(),
            list_result: None,
            list_alls: Vec::new(),
            agents_result: None,
            agents_operations: Vec::new(),
            agents_globals: Vec::new(),
            run_result: None,
            run_commands: Vec::new(),
            run_globals: Vec::new(),
            task_result: Some(Ok(TaskResult {
                path: PathBuf::from("/repo/still.toml"),
                tasks: Vec::new(),
                executions: vec![TaskExecution {
                    task: "test".to_string(),
                    command: "cargo test".to_string(),
                    status: 2,
                    stdout: "running tests\n".to_string(),
                    stderr: "failed\n".to_string(),
                }],
                status: 2,
            })),
            task_names: Vec::new(),
            task_globals: Vec::new(),
            activate_result: None,
            activate_shells: Vec::new(),
            activate_globals: Vec::new(),
            doctor_result: None,
            sync_result: None,
            services_result: None,
            services_requests: Vec::new(),
            trust_result: None,
            uninstall_result: None,
            uninstall_targets: Vec::new(),
        };
        let mut output = BufferedOutput::default();

        let code = run_cli(
            Command::Task(crate::cli::args::TaskArgs {
                global: false,
                name: Some("test".to_string()),
            }),
            &mut runtime,
            &mut output,
        );

        assert_eq!(code, 2);
        assert_eq!(runtime.task_names, [Some("test".to_string())]);
        insta::assert_snapshot!(output.stdout, @r###"
task test: cargo test
running tests
"###);
        insta::assert_snapshot!(output.stderr, @r###"
failed
"###);
    }

    #[test]
    fn activate_prints_shell_code() {
        let mut runtime = FakeRuntime {
            config_check_result: None,
            config_check_globals: Vec::new(),
            init_result: None,
            init_forces: Vec::new(),
            env_result: None,
            env_globals: Vec::new(),
            list_result: None,
            list_alls: Vec::new(),
            agents_result: None,
            agents_operations: Vec::new(),
            agents_globals: Vec::new(),
            run_result: None,
            run_commands: Vec::new(),
            run_globals: Vec::new(),
            task_result: None,
            task_names: Vec::new(),
            task_globals: Vec::new(),
            activate_result: Some(Ok(ActivateResult {
                shell: ShellKind::Posix,
                code: "export PATH=\"/opt/still/bin:$PATH\"".to_string(),
            })),
            activate_shells: Vec::new(),
            activate_globals: Vec::new(),
            doctor_result: None,
            sync_result: None,
            services_result: None,
            services_requests: Vec::new(),
            trust_result: None,
            uninstall_result: None,
            uninstall_targets: Vec::new(),
        };
        let mut output = BufferedOutput::default();

        let code = run_cli(
            Command::Activate(crate::cli::args::ActivateArgs {
                global: true,
                shell: Some("zsh".to_string()),
            }),
            &mut runtime,
            &mut output,
        );

        assert_eq!(code, 0);
        assert_eq!(runtime.activate_shells, [Some("zsh".to_string())]);
        assert_eq!(runtime.activate_globals, [true]);
        insta::assert_snapshot!(output.stdout, @r###"
export PATH="/opt/still/bin:$PATH"
"###);
        assert_eq!(output.stderr, "");
    }

    #[test]
    fn activate_formats_errors() {
        let mut runtime = FakeRuntime {
            config_check_result: None,
            config_check_globals: Vec::new(),
            init_result: None,
            init_forces: Vec::new(),
            env_result: None,
            env_globals: Vec::new(),
            list_result: None,
            list_alls: Vec::new(),
            agents_result: None,
            agents_operations: Vec::new(),
            agents_globals: Vec::new(),
            run_result: None,
            run_commands: Vec::new(),
            run_globals: Vec::new(),
            task_result: None,
            task_names: Vec::new(),
            task_globals: Vec::new(),
            activate_result: Some(Err(anyhow!("unsupported shell"))),
            activate_shells: Vec::new(),
            activate_globals: Vec::new(),
            doctor_result: None,
            sync_result: None,
            services_result: None,
            services_requests: Vec::new(),
            trust_result: None,
            uninstall_result: None,
            uninstall_targets: Vec::new(),
        };
        let mut output = BufferedOutput::default();

        let code = run_cli(
            Command::Activate(crate::cli::args::ActivateArgs {
                global: false,
                shell: Some("bad".to_string()),
            }),
            &mut runtime,
            &mut output,
        );

        assert_eq!(code, 1);
        assert_eq!(output.stdout, "");
        insta::assert_snapshot!(output.stderr, @r###"
activate failed: unsupported shell
"###);
    }

    #[test]
    fn doctor_formats_checks() {
        let mut runtime = FakeRuntime {
            config_check_result: None,
            config_check_globals: Vec::new(),
            init_result: None,
            init_forces: Vec::new(),
            env_result: None,
            env_globals: Vec::new(),
            list_result: None,
            list_alls: Vec::new(),
            agents_result: None,
            agents_operations: Vec::new(),
            agents_globals: Vec::new(),
            run_result: None,
            run_commands: Vec::new(),
            run_globals: Vec::new(),
            task_result: None,
            task_names: Vec::new(),
            task_globals: Vec::new(),
            activate_result: None,
            activate_shells: Vec::new(),
            activate_globals: Vec::new(),
            doctor_result: Some(Ok(DoctorResult {
                checks: vec![
                    DoctorCheck {
                        name: "platform".to_string(),
                        status: DoctorStatus::Ok,
                        detail: "detected macos".to_string(),
                    },
                    DoctorCheck {
                        name: "project config".to_string(),
                        status: DoctorStatus::Warning,
                        detail: "no still.toml found".to_string(),
                    },
                ],
            })),
            sync_result: None,
            services_result: None,
            services_requests: Vec::new(),
            trust_result: None,
            uninstall_result: None,
            uninstall_targets: Vec::new(),
        };
        let mut output = BufferedOutput::default();

        let code = run_cli(
            Command::Doctor(crate::cli::args::DoctorArgs {}),
            &mut runtime,
            &mut output,
        );

        assert_eq!(code, 0);
        insta::assert_snapshot!(output.stdout, @r###"
[ok] platform: detected macos
[warn] project config: no still.toml found
"###);
        assert_eq!(output.stderr, "");
    }

    #[test]
    fn doctor_formats_errors() {
        let mut runtime = FakeRuntime {
            config_check_result: None,
            config_check_globals: Vec::new(),
            init_result: None,
            init_forces: Vec::new(),
            env_result: None,
            env_globals: Vec::new(),
            list_result: None,
            list_alls: Vec::new(),
            agents_result: None,
            agents_operations: Vec::new(),
            agents_globals: Vec::new(),
            run_result: None,
            run_commands: Vec::new(),
            run_globals: Vec::new(),
            task_result: None,
            task_names: Vec::new(),
            task_globals: Vec::new(),
            activate_result: None,
            activate_shells: Vec::new(),
            activate_globals: Vec::new(),
            doctor_result: Some(Err(anyhow!("home directory missing"))),
            sync_result: None,
            services_result: None,
            services_requests: Vec::new(),
            trust_result: None,
            uninstall_result: None,
            uninstall_targets: Vec::new(),
        };
        let mut output = BufferedOutput::default();

        let code = run_cli(
            Command::Doctor(crate::cli::args::DoctorArgs {}),
            &mut runtime,
            &mut output,
        );

        assert_eq!(code, 1);
        assert_eq!(output.stdout, "");
        insta::assert_snapshot!(output.stderr, @r###"
doctor failed: home directory missing
"###);
    }

    #[test]
    fn sync_formats_desired_items() {
        let mut runtime = FakeRuntime {
            config_check_result: None,
            config_check_globals: Vec::new(),
            init_result: None,
            init_forces: Vec::new(),
            env_result: None,
            env_globals: Vec::new(),
            list_result: None,
            list_alls: Vec::new(),
            agents_result: None,
            agents_operations: Vec::new(),
            agents_globals: Vec::new(),
            run_result: None,
            run_commands: Vec::new(),
            run_globals: Vec::new(),
            task_result: None,
            task_names: Vec::new(),
            task_globals: Vec::new(),
            activate_result: None,
            activate_shells: Vec::new(),
            activate_globals: Vec::new(),
            doctor_result: None,
            sync_result: Some(Ok(SyncResult {
                path: PathBuf::from("/repo/still.toml"),
                lockfile_path: PathBuf::from("/repo/still.lock.toml"),
                drift: vec![SyncDrift::LockfileOutdated],
                items: vec![
                    sync_item(ItemKind::Tool, "rust@stable@rustup"),
                    sync_item(ItemKind::Package, "openssl"),
                    sync_item(ItemKind::App, "firefox@latest@homebrew-cask"),
                ],
                missing: vec![sync_item(ItemKind::Package, "openssl")],
                installed: vec![sync_item(ItemKind::Tool, "rust@stable@rustup")],
            })),
            services_result: None,
            services_requests: Vec::new(),
            trust_result: None,
            uninstall_result: None,
            uninstall_targets: Vec::new(),
        };
        let mut output = BufferedOutput::default();

        let code = run_cli(
            Command::Sync(crate::cli::args::SyncArgs { global: false }),
            &mut runtime,
            &mut output,
        );

        assert_eq!(code, 0);
        insta::assert_snapshot!(output.stdout, @r###"
Config: /repo/still.toml
Lockfile: /repo/still.lock.toml
Drift:
  lockfile outdated
Missing:
  package openssl@latest
Installed:
  tool rust@stable
Sync plan:
  tool rust@stable@rustup
  package openssl@latest
  app firefox@latest@homebrew-cask
"###);
        assert_eq!(output.stderr, "");
    }

    #[test]
    fn sync_formats_empty_plan() {
        let mut runtime = FakeRuntime {
            config_check_result: None,
            config_check_globals: Vec::new(),
            init_result: None,
            init_forces: Vec::new(),
            env_result: None,
            env_globals: Vec::new(),
            list_result: None,
            list_alls: Vec::new(),
            agents_result: None,
            agents_operations: Vec::new(),
            agents_globals: Vec::new(),
            run_result: None,
            run_commands: Vec::new(),
            run_globals: Vec::new(),
            task_result: None,
            task_names: Vec::new(),
            task_globals: Vec::new(),
            activate_result: None,
            activate_shells: Vec::new(),
            activate_globals: Vec::new(),
            doctor_result: None,
            sync_result: Some(Ok(SyncResult {
                path: PathBuf::from("/repo/still.toml"),
                lockfile_path: PathBuf::from("/repo/still.lock.toml"),
                drift: Vec::new(),
                items: Vec::new(),
                missing: Vec::new(),
                installed: Vec::new(),
            })),
            services_result: None,
            services_requests: Vec::new(),
            trust_result: None,
            uninstall_result: None,
            uninstall_targets: Vec::new(),
        };
        let mut output = BufferedOutput::default();

        let code = run_cli(
            Command::Sync(crate::cli::args::SyncArgs { global: false }),
            &mut runtime,
            &mut output,
        );

        assert_eq!(code, 0);
        insta::assert_snapshot!(output.stdout, @r###"
Config: /repo/still.toml
Lockfile: /repo/still.lock.toml
Drift: none
Missing: none
Sync plan:
  (nothing to sync)
"###);
    }

    #[test]
    fn services_status_formats_reports() {
        let mut runtime = FakeRuntime {
            config_check_result: None,
            config_check_globals: Vec::new(),
            init_result: None,
            init_forces: Vec::new(),
            env_result: None,
            env_globals: Vec::new(),
            list_result: None,
            list_alls: Vec::new(),
            agents_result: None,
            agents_operations: Vec::new(),
            agents_globals: Vec::new(),
            run_result: None,
            run_commands: Vec::new(),
            run_globals: Vec::new(),
            task_result: None,
            task_names: Vec::new(),
            task_globals: Vec::new(),
            activate_result: None,
            activate_shells: Vec::new(),
            activate_globals: Vec::new(),
            doctor_result: None,
            sync_result: None,
            services_result: Some(Ok(ServicesResult {
                path: PathBuf::from("/repo/still.toml"),
                services: vec![ServiceReport {
                    name: "web".to_string(),
                    status: ServiceStatus::Configured,
                    detail: "echo web".to_string(),
                    execution: None,
                }],
            })),
            services_requests: Vec::new(),
            trust_result: None,
            uninstall_result: None,
            uninstall_targets: Vec::new(),
        };
        let mut output = BufferedOutput::default();

        let code = run_cli(
            Command::Services(crate::cli::args::ServicesArgs {
                global: true,
                command: Some(ServicesCommand::Status {
                    name: Some("web".to_string()),
                }),
            }),
            &mut runtime,
            &mut output,
        );

        assert_eq!(code, 0);
        assert_eq!(
            runtime.services_requests,
            [(ServicesOperation::Status, Some("web".to_string()), true)]
        );
        insta::assert_snapshot!(output.stdout, @r###"
Config: /repo/still.toml
web: configured - echo web
"###);
        assert_eq!(output.stderr, "");
    }

    #[test]
    fn services_start_formats_execution_output() {
        let mut runtime = FakeRuntime {
            config_check_result: None,
            config_check_globals: Vec::new(),
            init_result: None,
            init_forces: Vec::new(),
            env_result: None,
            env_globals: Vec::new(),
            list_result: None,
            list_alls: Vec::new(),
            agents_result: None,
            agents_operations: Vec::new(),
            agents_globals: Vec::new(),
            run_result: None,
            run_commands: Vec::new(),
            run_globals: Vec::new(),
            task_result: None,
            task_names: Vec::new(),
            task_globals: Vec::new(),
            activate_result: None,
            activate_shells: Vec::new(),
            activate_globals: Vec::new(),
            doctor_result: None,
            sync_result: None,
            services_result: Some(Ok(ServicesResult {
                path: PathBuf::from("/repo/still.toml"),
                services: vec![ServiceReport {
                    name: "web".to_string(),
                    status: ServiceStatus::Ok,
                    detail: "start".to_string(),
                    execution: Some(engine::actions::services::ServiceExecution {
                        command: "echo web".to_string(),
                        status: 0,
                        stdout: "web\n".to_string(),
                        stderr: String::new(),
                    }),
                }],
            })),
            services_requests: Vec::new(),
            trust_result: None,
            uninstall_result: None,
            uninstall_targets: Vec::new(),
        };
        let mut output = BufferedOutput::default();

        let code = run_cli(
            Command::Services(crate::cli::args::ServicesArgs {
                global: false,
                command: Some(ServicesCommand::Start {
                    name: Some("web".to_string()),
                }),
            }),
            &mut runtime,
            &mut output,
        );

        assert_eq!(code, 0);
        assert_eq!(
            runtime.services_requests,
            [(ServicesOperation::Start, Some("web".to_string()), false)]
        );
        insta::assert_snapshot!(output.stdout, @r###"
Config: /repo/still.toml
web: ok - start
command: echo web
web
"###);
        assert_eq!(output.stderr, "");
    }

    #[test]
    fn trust_formats_success() {
        let mut runtime = FakeRuntime {
            config_check_result: None,
            config_check_globals: Vec::new(),
            init_result: None,
            init_forces: Vec::new(),
            env_result: None,
            env_globals: Vec::new(),
            list_result: None,
            list_alls: Vec::new(),
            agents_result: None,
            agents_operations: Vec::new(),
            agents_globals: Vec::new(),
            run_result: None,
            run_commands: Vec::new(),
            run_globals: Vec::new(),
            task_result: None,
            task_names: Vec::new(),
            task_globals: Vec::new(),
            activate_result: None,
            activate_shells: Vec::new(),
            activate_globals: Vec::new(),
            doctor_result: None,
            sync_result: None,
            services_result: None,
            services_requests: Vec::new(),
            trust_result: Some(Ok(TrustResult {
                config_path: PathBuf::from("/repo/still.toml"),
                trust_path: PathBuf::from("/repo/.still/trust.toml"),
                fingerprint: "abc".to_string(),
            })),
            uninstall_result: None,
            uninstall_targets: Vec::new(),
        };
        let mut output = BufferedOutput::default();

        let code = run_cli(
            Command::Trust(crate::cli::args::TrustArgs {}),
            &mut runtime,
            &mut output,
        );

        assert_eq!(code, 0);
        insta::assert_snapshot!(output.stdout, @r###"
✓ Trusted /repo/still.toml
Marker: /repo/.still/trust.toml
"###);
        assert_eq!(output.stderr, "");
    }

    #[test]
    fn uninstall_formats_success() {
        let mut runtime = FakeRuntime {
            config_check_result: None,
            config_check_globals: Vec::new(),
            init_result: None,
            init_forces: Vec::new(),
            env_result: None,
            env_globals: Vec::new(),
            list_result: None,
            list_alls: Vec::new(),
            agents_result: None,
            agents_operations: Vec::new(),
            agents_globals: Vec::new(),
            run_result: None,
            run_commands: Vec::new(),
            run_globals: Vec::new(),
            task_result: None,
            task_names: Vec::new(),
            task_globals: Vec::new(),
            activate_result: None,
            activate_shells: Vec::new(),
            activate_globals: Vec::new(),
            doctor_result: None,
            sync_result: None,
            services_result: None,
            services_requests: Vec::new(),
            trust_result: None,
            uninstall_result: Some(Ok(UninstallResult {
                path: PathBuf::from("/repo/still.toml"),
                kind: ItemKind::Package,
                name: "openssl".to_string(),
                removed_paths: vec![PathBuf::from("/opt/still/packages/openssl")],
            })),
            uninstall_targets: Vec::new(),
        };
        let mut output = BufferedOutput::default();

        let code = run_cli(
            Command::Uninstall(crate::cli::args::UninstallArgs {
                global: false,
                tool: None,
                package: Some("openssl".parse().unwrap()),
                app: None,
                item: None,
            }),
            &mut runtime,
            &mut output,
        );

        assert_eq!(code, 0);
        assert_eq!(runtime.uninstall_targets.len(), 1);
        assert_eq!(runtime.uninstall_targets[0].kind, Some(ItemKind::Package));
        assert_eq!(runtime.uninstall_targets[0].name, "openssl");
        assert_eq!(runtime.uninstall_targets[0].version, "latest");
        assert!(!runtime.uninstall_targets[0].exact);
        insta::assert_snapshot!(output.stdout, @r###"
✓ Removed package openssl from /repo/still.toml
Removed artifact /opt/still/packages/openssl
"###);
        assert_eq!(output.stderr, "");
    }

    #[test]
    fn uninstall_passes_versioned_target_to_runtime() {
        let mut runtime = FakeRuntime {
            uninstall_result: Some(Ok(UninstallResult {
                path: PathBuf::from("/repo/still.toml"),
                kind: ItemKind::Package,
                name: "openssl".to_string(),
                removed_paths: Vec::new(),
            })),
            config_check_result: None,
            config_check_globals: Vec::new(),
            init_result: None,
            init_forces: Vec::new(),
            env_result: None,
            env_globals: Vec::new(),
            list_result: None,
            list_alls: Vec::new(),
            agents_result: None,
            agents_operations: Vec::new(),
            agents_globals: Vec::new(),
            run_result: None,
            run_commands: Vec::new(),
            run_globals: Vec::new(),
            task_result: None,
            task_names: Vec::new(),
            task_globals: Vec::new(),
            activate_result: None,
            activate_shells: Vec::new(),
            activate_globals: Vec::new(),
            doctor_result: None,
            sync_result: None,
            services_result: None,
            services_requests: Vec::new(),
            trust_result: None,
            uninstall_targets: Vec::new(),
        };
        let mut output = BufferedOutput::default();

        let code = run_cli(
            Command::Uninstall(crate::cli::args::UninstallArgs {
                global: false,
                tool: None,
                package: Some("openssl@3@homebrew".parse().unwrap()),
                app: None,
                item: None,
            }),
            &mut runtime,
            &mut output,
        );

        assert_eq!(code, 0);
        assert_eq!(runtime.uninstall_targets.len(), 1);
        assert_eq!(runtime.uninstall_targets[0].kind, Some(ItemKind::Package));
        assert_eq!(runtime.uninstall_targets[0].name, "openssl");
        assert_eq!(runtime.uninstall_targets[0].version, "3");
        assert_eq!(
            runtime.uninstall_targets[0]
                .backend
                .as_ref()
                .map(|backend| backend.as_str()),
            Some("homebrew")
        );
        assert!(runtime.uninstall_targets[0].exact);
    }

    #[test]
    fn uninstall_passes_explicit_latest_as_exact_target() {
        let mut runtime = FakeRuntime {
            uninstall_result: Some(Ok(UninstallResult {
                path: PathBuf::from("/repo/still.toml"),
                kind: ItemKind::Package,
                name: "openssl".to_string(),
                removed_paths: Vec::new(),
            })),
            config_check_result: None,
            config_check_globals: Vec::new(),
            init_result: None,
            init_forces: Vec::new(),
            env_result: None,
            env_globals: Vec::new(),
            list_result: None,
            list_alls: Vec::new(),
            agents_result: None,
            agents_operations: Vec::new(),
            agents_globals: Vec::new(),
            run_result: None,
            run_commands: Vec::new(),
            run_globals: Vec::new(),
            task_result: None,
            task_names: Vec::new(),
            task_globals: Vec::new(),
            activate_result: None,
            activate_shells: Vec::new(),
            activate_globals: Vec::new(),
            doctor_result: None,
            sync_result: None,
            services_result: None,
            services_requests: Vec::new(),
            trust_result: None,
            uninstall_targets: Vec::new(),
        };
        let mut output = BufferedOutput::default();

        let code = run_cli(
            Command::Uninstall(crate::cli::args::UninstallArgs {
                global: false,
                tool: None,
                package: None,
                app: None,
                item: Some("openssl@latest".parse().unwrap()),
            }),
            &mut runtime,
            &mut output,
        );

        assert_eq!(code, 0);
        assert_eq!(runtime.uninstall_targets[0].name, "openssl");
        assert_eq!(runtime.uninstall_targets[0].kind, None);
        assert_eq!(runtime.uninstall_targets[0].version, "latest");
        assert!(runtime.uninstall_targets[0].exact);
    }

    fn section<const N: usize>(kind: ItemKind, items: [ListItem; N]) -> ListSection {
        ListSection {
            kind,
            items: items.into(),
        }
    }

    fn item(name: &str, version: &str, backend: Option<&str>) -> ListItem {
        ListItem {
            logical_name: name.to_string(),
            name: name.to_string(),
            version: version.to_string(),
            backend: backend.map(str::to_string),
            outputs: Vec::new(),
            linked_executables: Vec::new(),
            configured: true,
            project: true,
            global: false,
            installed: false,
        }
    }

    fn installed_item(name: &str, version: &str, backend: Option<&str>) -> ListItem {
        ListItem {
            logical_name: name.to_string(),
            name: name.to_string(),
            version: version.to_string(),
            backend: backend.map(str::to_string),
            outputs: vec![format!("/opt/still/apps/{name}/{version}")],
            linked_executables: Vec::new(),
            configured: false,
            project: false,
            global: false,
            installed: true,
        }
    }

    fn global_item(name: &str, version: &str, backend: Option<&str>) -> ListItem {
        ListItem {
            logical_name: name.to_string(),
            name: name.to_string(),
            version: version.to_string(),
            backend: backend.map(str::to_string),
            outputs: Vec::new(),
            linked_executables: Vec::new(),
            configured: true,
            project: false,
            global: true,
            installed: false,
        }
    }

    fn sync_item(kind: ItemKind, spec: &str) -> SyncItem {
        let parsed: engine::specs::item::ItemSpec = spec.parse().unwrap();
        SyncItem {
            kind,
            logical_name: parsed.name.clone(),
            spec: parsed,
            tool: Default::default(),
            desired_state: spec.to_string(),
        }
    }

    fn normalized_skill(name: &str) -> NormalizedSkill {
        NormalizedSkill {
            name: name.to_string(),
            source: NormalizedSkillSource::Official {
                name: name.to_string(),
                version: None,
            },
            auto: false,
            tools: Vec::new(),
            packages: Vec::new(),
            apps: Vec::new(),
        }
    }
}
