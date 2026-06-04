use engine::{
    actions::{
        agents::AgentsOperation,
        install::{InstallItemRequest, InstallRequest, ToolInstallOptions},
        services::ServicesOperation,
        uninstall::UninstallTarget,
    },
    specs::item::ItemKind,
};

use crate::cli::{
    args::{AgentsCommand, Command, ConfigCommand, InstallArgs, ServicesCommand, UninstallSpec},
    present,
    progress::Spinner,
    runtime::{CliRuntime, ScopedInstallRequest},
    ui::Ui,
};

pub fn dispatch<R, U>(cmd: Command, runtime: &mut R, ui: &mut U) -> i32
where
    R: CliRuntime,
    U: Ui,
{
    match cmd {
        Command::Init(args) => match runtime.init(args.force) {
            Ok(result) => present::init_created(result, ui),
            Err(e) => present::fail(ui, "init", e),
        },
        Command::Trust(_args) => match runtime.trust() {
            Ok(result) => present::trusted(result, ui),
            Err(e) => present::fail(ui, "trust", e),
        },
        Command::Install(args) => install(args, runtime, ui),
        Command::Sync(args) => match runtime.sync(args.global) {
            Ok(result) => present::sync_result(result, ui),
            Err(e) => present::fail(ui, "sync", e),
        },
        Command::List(args) => match runtime.list(args.all, args.global) {
            Ok(result) => present::list_result(result, ui),
            Err(e) => present::fail(ui, "list", e),
        },
        Command::Uninstall(args) => {
            let Some((kind, spec)) = args.target() else {
                ui.error("uninstall failed: expected a tool, package, app, or item target");
                return 1;
            };
            match runtime.uninstall(uninstall_target(kind, spec), args.global) {
                Ok(result) => present::uninstall_result(result, ui),
                Err(e) => present::fail(ui, "uninstall", e),
            }
        }
        Command::Run(args) => {
            let global = args.global;
            match runtime.run_command(args.into_command(), global) {
                Ok(result) => present::run_result(result, ui),
                Err(e) => present::fail(ui, "run", e),
            }
        }
        Command::Task(args) => match runtime.task(args.name, args.global) {
            Ok(result) => present::task_result(result, ui),
            Err(e) => present::fail(ui, "task", e),
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
                Ok(result) => present::services_result(result, ui),
                Err(e) => present::fail(ui, "services", e),
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
                Ok(result) => present::agents_result(result, operation, ui),
                Err(e) => present::fail(ui, "agents", e),
            }
        }
        Command::Config(args) => match args.command {
            ConfigCommand::Check => match runtime.config_check(args.global) {
                Ok(result) => present::config_ok(result, ui),
                Err(e) => present::fail(ui, "config check", e),
            },
        },
        Command::Doctor(_args) => match runtime.doctor() {
            Ok(result) => present::doctor_result(result, ui),
            Err(e) => present::fail(ui, "doctor", e),
        },
        Command::Env(args) => match runtime.env(args.global) {
            Ok(result) => present::env_result(result, ui),
            Err(e) => present::fail(ui, "env", e),
        },
        Command::Activate(args) => match runtime.activate(args.shell, args.global) {
            Ok(result) => present::activate_result(result.code, ui),
            Err(e) => present::fail(ui, "activate", e),
        },
    }
}

fn install<R, U>(args: InstallArgs, runtime: &mut R, ui: &mut U) -> i32
where
    R: CliRuntime,
    U: Ui,
{
    let global = args.global;
    let force = args.force;
    let items = install_items(args);
    let request_items = items.clone();
    let install_request = ScopedInstallRequest {
        global,
        force,
        install: InstallRequest { items },
    };

    let spinner = Spinner::start("Installing requested items");
    let result = runtime.install(install_request);
    spinner.finish();

    match result {
        Ok(result) => present::install_result(&request_items, &result, ui),
        Err(e) => {
            ui.error(&format!("install failed: {e}"));
            1
        }
    }
}

fn install_items(args: InstallArgs) -> Vec<InstallItemRequest> {
    args.items
        .into_iter()
        .map(|item| InstallItemRequest {
            kind: item.kind,
            spec: item.spec,
            tool: ToolInstallOptions::default(),
        })
        .collect()
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
