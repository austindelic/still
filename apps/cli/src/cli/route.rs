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
    session::{EngineSession, InstallCommandRequest},
    ui::Ui,
};

pub fn route_command<U>(cmd: Command, session: &mut EngineSession, ui: &mut U) -> i32
where
    U: Ui,
{
    match cmd {
        Command::Init(args) => match session.init(args.force) {
            Ok(result) => present::init_created(result, ui),
            Err(e) => present::fail(ui, "init", e),
        },
        Command::Trust(_args) => match session.trust() {
            Ok(result) => present::trusted(result, ui),
            Err(e) => present::fail(ui, "trust", e),
        },
        Command::Install(args) => install(args, session, ui),
        Command::Sync(args) => match session.sync(args.global) {
            Ok(result) => present::sync_result(result, ui),
            Err(e) => present::fail(ui, "sync", e),
        },
        Command::List(args) => match session.list(args.all, args.global) {
            Ok(result) => present::list_result(result, ui),
            Err(e) => present::fail(ui, "list", e),
        },
        Command::Uninstall(args) => {
            let Some((kind, spec)) = args.target() else {
                ui.error("uninstall failed: expected a tool, package, app, or item target");
                return 1;
            };
            match session.uninstall(uninstall_target(kind, spec), args.global) {
                Ok(result) => present::uninstall_result(result, ui),
                Err(e) => present::fail(ui, "uninstall", e),
            }
        }
        Command::Run(args) => {
            let global = args.global;
            match session.run_command(args.into_command(), global) {
                Ok(result) => present::run_result(result, ui),
                Err(e) => present::fail(ui, "run", e),
            }
        }
        Command::Task(args) => match session.task(args.name, args.global) {
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
            match session.services(operation, name, args.global) {
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
            match session.agents(operation, args.global) {
                Ok(result) => present::agents_result(result, operation, ui),
                Err(e) => present::fail(ui, "agents", e),
            }
        }
        Command::Config(args) => match args.command {
            ConfigCommand::Check => match session.config_check(args.global) {
                Ok(result) => present::config_ok(result, ui),
                Err(e) => present::fail(ui, "config check", e),
            },
        },
        Command::Doctor(_args) => match session.doctor() {
            Ok(result) => present::doctor_result(result, ui),
            Err(e) => present::fail(ui, "doctor", e),
        },
        Command::Env(args) => match session.env(args.global) {
            Ok(result) => present::env_result(result, ui),
            Err(e) => present::fail(ui, "env", e),
        },
        Command::Activate(args) => match session.activate(args.shell, args.global) {
            Ok(result) => present::activate_result(result.code, ui),
            Err(e) => present::fail(ui, "activate", e),
        },
    }
}

fn install<U>(args: InstallArgs, session: &mut EngineSession, ui: &mut U) -> i32
where
    U: Ui,
{
    let global = args.global;
    let force = args.force;
    let items = install_items(args);
    let request_items = items.clone();
    let install_request = InstallCommandRequest {
        global,
        force,
        install: InstallRequest { items },
    };

    let spinner = Spinner::start("Installing requested items");
    let result = session.install_and_record(install_request);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::args::InstallItemArg;

    #[test]
    fn install_items_builds_engine_requests() {
        let args = InstallArgs {
            global: true,
            force: true,
            items: vec![InstallItemArg {
                kind: Some(ItemKind::Tool),
                spec: "cargo:ripgrep@14.1.1"
                    .parse()
                    .expect("install spec should parse"),
            }],
        };

        let items = install_items(args);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, Some(ItemKind::Tool));
        assert_eq!(items[0].spec.name, "ripgrep");
        assert_eq!(items[0].spec.version.as_str(), "14.1.1");
        assert_eq!(items[0].spec.backend.as_ref().unwrap().as_str(), "cargo");
        assert_eq!(items[0].tool, ToolInstallOptions::default());
    }

    #[test]
    fn uninstall_target_builds_engine_request() {
        let spec = "cargo:ripgrep@14.1.1"
            .parse::<UninstallSpec>()
            .expect("uninstall spec should parse");

        let target = uninstall_target(Some(ItemKind::Tool), &spec);

        assert_eq!(target.kind, Some(ItemKind::Tool));
        assert_eq!(target.name, "ripgrep");
        assert_eq!(target.version, "14.1.1");
        assert_eq!(target.backend.as_ref().unwrap().as_str(), "cargo");
        assert!(target.exact);
    }
}
