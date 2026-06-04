pub mod install;

#[cfg(not(feature = "tui"))]
use clap::CommandFactory;
use engine::{
    actions::{
        agents::AgentsOperation,
        doctor::DoctorStatus,
        install::InstallItemRequest,
        services::{ServiceReport, ServiceStatus, ServicesOperation},
        sync::SyncDrift,
        uninstall::UninstallTarget,
    },
    specs::item::ItemKind,
};

#[cfg(not(feature = "tui"))]
use crate::cli::args::Cli;
use crate::cli::{
    args::{AgentsCommand, Command, ConfigCommand, ServicesCommand, UninstallSpec},
    output::Output,
    runtime::CliRuntime,
};

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
            Err(e) => fail(output, "init", e),
        },
        Command::Trust(_args) => match runtime.trust() {
            Ok(result) => {
                output.success(&format!("Trusted {}", result.config_path.display()));
                output.info(&format!("Marker: {}", result.trust_path.display()));
                0
            }
            Err(e) => fail(output, "trust", e),
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
                    let source = item
                        .spec
                        .backend
                        .as_ref()
                        .map(|source| format!(" from {source}"))
                        .unwrap_or_default();
                    output.info(&format!(
                        "  {} {}@{}{}",
                        item.kind, item.spec.name, item.spec.version, source
                    ));
                }
                0
            }
            Err(e) => fail(output, "sync", e),
        },
        Command::List(args) => match runtime.list(args.all, args.global) {
            Ok(result) => {
                output.info(&format!("Config: {}", result.path.display()));
                for section in result.sections {
                    output.info(match section.kind {
                        ItemKind::Tool => "Tools:",
                        ItemKind::Package => "Packages:",
                        ItemKind::App => "Apps:",
                    });
                    if section.items.is_empty() {
                        output.info("  (none)");
                    }
                    for item in section.items {
                        let source = item
                            .backend
                            .map(|source| format!(" from {source}"))
                            .unwrap_or_default();
                        let status = match (item.project, item.global, item.installed) {
                            (true, true, true) => " (project, global, installed)",
                            (true, true, false) => " (project, global)",
                            (true, false, true) => " (project, installed)",
                            (true, false, false) => " (project)",
                            (false, true, true) => " (global, installed)",
                            (false, true, false) => " (global)",
                            (false, false, true) => " (installed)",
                            (false, false, false) => "",
                        };
                        output.info(&format!(
                            "  {}@{}{}{}",
                            item.name, item.version, source, status
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
            Err(e) => fail(output, "list", e),
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
                Err(e) => fail(output, "uninstall", e),
            }
        }
        Command::Run(args) => {
            match runtime.run_command(normalize_child_command(args.command), args.global) {
                Ok(result) => {
                    write_child_output(output, &result.stdout, false);
                    write_child_output(output, &result.stderr, true);
                    result.status
                }
                Err(e) => fail(output, "run", e),
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
                                output.info(&format!("  {} - {}", task.name, description))
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
            Err(e) => fail(output, "task", e),
        },
        Command::Services(args) => {
            let (operation, name) = match args
                .command
                .unwrap_or(ServicesCommand::Status { name: None })
            {
                ServicesCommand::Status { name } => (ServicesOperation::Status, name),
                ServicesCommand::Start { name } => (ServicesOperation::Start, name),
                ServicesCommand::Stop { name } => (ServicesOperation::Stop, name),
                ServicesCommand::Check { name } => (ServicesOperation::Check, name),
            };
            match runtime.services(operation, name, args.global) {
                Ok(result) => {
                    output.info(&format!("Config: {}", result.path.display()));
                    let exit_code = services_exit_code(&result.services);
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
                    exit_code
                }
                Err(e) => fail(output, "services", e),
            }
        }
        Command::Agents(args) => {
            let operation = match args.command.unwrap_or(AgentsCommand::List) {
                AgentsCommand::List => AgentsOperation::List,
                AgentsCommand::Check => AgentsOperation::Check,
                AgentsCommand::Sync {
                    accept_auto_deps: false,
                } => AgentsOperation::Sync,
                AgentsCommand::Sync {
                    accept_auto_deps: true,
                } => AgentsOperation::SyncAcceptAutoDependencies,
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
                        AgentsOperation::Sync | AgentsOperation::SyncAcceptAutoDependencies
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
                Err(e) => fail(output, "agents", e),
            }
        }
        Command::Config(args) => match args.command {
            ConfigCommand::Check => match runtime.config_check(args.global) {
                Ok(result) => {
                    output.success(&format!("Config OK: {}", result.path.display()));
                    0
                }
                Err(e) => fail(output, "config check", e),
            },
        },
        Command::Doctor(_args) => match runtime.doctor() {
            Ok(result) => {
                for check in result.checks {
                    let status = match check.status {
                        DoctorStatus::Ok => "ok",
                        DoctorStatus::Warning => "warn",
                        DoctorStatus::Error => "error",
                    };
                    output.info(&format!("[{status}] {}: {}", check.name, check.detail));
                }
                0
            }
            Err(e) => fail(output, "doctor", e),
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
            Err(e) => fail(output, "env", e),
        },
        Command::Activate(args) => match runtime.activate(args.shell, args.global) {
            Ok(result) => {
                output.info(&result.code);
                0
            }
            Err(e) => fail(output, "activate", e),
        },
    }
}

#[cfg(not(feature = "tui"))]
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

#[cfg(not(feature = "tui"))]
pub fn help_text() -> std::io::Result<String> {
    let mut command = Cli::command();
    let mut bytes = Vec::new();
    command.write_help(&mut bytes)?;
    bytes.push(b'\n');
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn fail<O: Output>(output: &mut O, command: &str, err: engine::error::EngineError) -> i32 {
    output.error(&format!("{command} failed: {err}"));
    1
}

fn uninstall_target(kind: Option<ItemKind>, spec: &UninstallSpec) -> UninstallTarget {
    UninstallTarget {
        kind,
        name: spec.spec.name.clone(),
        version: spec.spec.version.to_string(),
        backend: spec.spec.backend.clone(),
        exact: spec.exact,
    }
}

fn service_status_label(status: ServiceStatus) -> &'static str {
    match status {
        ServiceStatus::Configured => "configured",
        ServiceStatus::Skipped => "skipped",
        ServiceStatus::Ok => "ok",
        ServiceStatus::Failed => "failed",
    }
}

fn services_exit_code(services: &[ServiceReport]) -> i32 {
    if services
        .iter()
        .any(|service| service.status == ServiceStatus::Failed)
    {
        1
    } else {
        0
    }
}

fn install_item_label(item: &InstallItemRequest) -> String {
    let source = item
        .spec
        .backend
        .as_ref()
        .map(|source| format!(" from {source}"))
        .unwrap_or_default();
    format!(
        "{} {}@{}{}",
        item.kind, item.spec.name, item.spec.version, source
    )
}

fn sync_drift_label(drift: SyncDrift) -> &'static str {
    match drift {
        SyncDrift::LockfileMissing => "lockfile missing",
        SyncDrift::LockfileOutdated => "lockfile outdated",
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
