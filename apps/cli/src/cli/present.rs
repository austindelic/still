use engine::{
    actions::{
        agents::{AgentsOperation, AgentsResult},
        config::CheckConfigResult,
        doctor::{DoctorResult, DoctorStatus},
        env::EnvResult,
        init::InitResult,
        install::{InstallItemRequest, InstallResult},
        list::{ListResult, ListSection},
        run::RunResult,
        services::{ServiceReport, ServiceStatus, ServicesResult},
        sync::{SyncDrift, SyncResult},
        task::TaskResult,
        trust::TrustResult,
        uninstall::UninstallResult,
    },
    specs::item::ItemKind,
};

use crate::cli::ui::Ui;

pub fn fail<U: Ui>(ui: &mut U, command: &str, err: engine::error::EngineError) -> i32 {
    ui.error(&format!("{command} failed: {err}"));
    1
}

pub fn init_created<U: Ui>(result: InitResult, ui: &mut U) -> i32 {
    ui.success(&format!("Created {}", result.path.display()));
    0
}

pub fn trusted<U: Ui>(result: TrustResult, ui: &mut U) -> i32 {
    ui.success(&format!("Trusted {}", result.config_path.display()));
    ui.info(&format!("Marker: {}", result.trust_path.display()));
    0
}

pub fn install_result<U: Ui>(
    request_items: &[InstallItemRequest],
    result: &InstallResult,
    ui: &mut U,
) -> i32 {
    write_grouped_request(request_items, ui);
    write_install_result(result, ui);
    0
}

pub fn sync_result<U: Ui>(result: SyncResult, ui: &mut U) -> i32 {
    ui.info(&format!("Config: {}", result.path.display()));
    ui.info(&format!("Lockfile: {}", result.lockfile_path.display()));
    if result.drift.is_empty() {
        ui.info("Drift: none");
    } else {
        ui.info("Drift:");
        for drift in result.drift {
            ui.info(&format!("  {}", sync_drift_label(drift)));
        }
    }
    if result.missing.is_empty() {
        ui.info("Missing: none");
    } else {
        ui.info("Missing:");
        for item in &result.missing {
            ui.info(&format!(
                "  {} {}@{}",
                item.kind, item.spec.name, item.spec.version
            ));
        }
    }
    if !result.installed.is_empty() {
        ui.info("Installed:");
        for item in &result.installed {
            ui.info(&format!(
                "  {} {}@{}",
                item.kind, item.spec.name, item.spec.version
            ));
        }
    }
    ui.info("Sync plan:");
    if result.items.is_empty() {
        ui.info("  (nothing to sync)");
    }
    for item in result.items {
        let source = item
            .spec
            .backend
            .as_ref()
            .map(|source| format!(" from {source}"))
            .unwrap_or_default();
        ui.info(&format!(
            "  {} {}@{}{}",
            item.kind, item.spec.name, item.spec.version, source
        ));
    }
    0
}

pub fn list_result<U: Ui>(result: ListResult, ui: &mut U) -> i32 {
    ui.info(&format!("Config: {}", result.path.display()));
    for section in result.sections {
        write_list_section(section, ui);
    }
    0
}

pub fn uninstall_result<U: Ui>(result: UninstallResult, ui: &mut U) -> i32 {
    ui.success(&format!(
        "Removed {} {} from {}",
        result.kind,
        result.name,
        result.path.display()
    ));
    for path in result.removed_paths {
        ui.info(&format!("Removed artifact {}", path.display()));
    }
    0
}

pub fn run_result<U: Ui>(result: RunResult, ui: &mut U) -> i32 {
    write_child_output(ui, &result.stdout, false);
    write_child_output(ui, &result.stderr, true);
    result.status
}

pub fn task_result<U: Ui>(result: TaskResult, ui: &mut U) -> i32 {
    if result.executions.is_empty() {
        ui.info(&format!("Config: {}", result.path.display()));
        ui.info("Tasks:");
        if result.tasks.is_empty() {
            ui.info("  (none)");
        }
        for task in result.tasks {
            match task.description {
                Some(description) => ui.info(&format!("  {} - {}", task.name, description)),
                None => ui.info(&format!("  {}", task.name)),
            }
        }
        0
    } else {
        for execution in result.executions {
            ui.info(&format!("task {}: {}", execution.task, execution.command));
            write_child_output(ui, &execution.stdout, false);
            write_child_output(ui, &execution.stderr, true);
        }
        result.status
    }
}

pub fn services_result<U: Ui>(result: ServicesResult, ui: &mut U) -> i32 {
    ui.info(&format!("Config: {}", result.path.display()));
    let exit_code = services_exit_code(&result.services);
    for service in result.services {
        write_service(service, ui);
    }
    exit_code
}

pub fn agents_result<U: Ui>(result: AgentsResult, operation: AgentsOperation, ui: &mut U) -> i32 {
    ui.info(&format!("Config: {}", result.path.display()));
    if result.agents.targets.is_empty() {
        ui.info("Targets: (none)");
    } else {
        ui.info(&format!("Targets: {}", result.agents.targets.join(", ")));
    }
    if let Some(instructions) = result.agents.instructions {
        ui.info(&format!("Instructions: {instructions}"));
    }
    ui.info("Skills:");
    if result.agents.skills.is_empty() {
        ui.info("  (none)");
    }
    for skill in result.agents.skills {
        ui.info(&format!("  {}", skill.name));
    }
    if matches!(
        operation,
        AgentsOperation::Sync | AgentsOperation::SyncAcceptAutoDependencies
    ) && !result.auto_added.is_empty()
    {
        ui.info("Auto-added dependencies:");
        for item in result.auto_added {
            ui.info(&format!("  {}", install_item_label(&item)));
        }
    } else if !result.pending_auto_dependencies.is_empty() {
        ui.info("Auto dependencies to add on sync:");
        for item in result.pending_auto_dependencies {
            ui.info(&format!("  {}", install_item_label(&item)));
        }
    }
    if !result.missing_dependencies.is_empty() {
        ui.info("Missing skill dependencies:");
        for item in result.missing_dependencies {
            ui.info(&format!("  {}", install_item_label(&item)));
        }
    }
    if let Some(path) = result.gitignore_path {
        ui.success(&format!("Updated {}", path.display()));
    }
    for path in result.target_manifests {
        ui.success(&format!("Updated {}", path.display()));
    }
    if result.review_required {
        ui.error("agents sync needs --accept-auto-deps before writing auto dependencies");
        return 1;
    }
    0
}

pub fn config_ok<U: Ui>(result: CheckConfigResult, ui: &mut U) -> i32 {
    ui.success(&format!("Config OK: {}", result.path.display()));
    0
}

pub fn doctor_result<U: Ui>(result: DoctorResult, ui: &mut U) -> i32 {
    for check in result.checks {
        let status = match check.status {
            DoctorStatus::Ok => "ok",
            DoctorStatus::Warning => "warn",
            DoctorStatus::Error => "error",
        };
        ui.info(&format!("[{status}] {}: {}", check.name, check.detail));
    }
    0
}

pub fn env_result<U: Ui>(result: EnvResult, ui: &mut U) -> i32 {
    ui.info(&format!("Config: {}", result.path.display()));
    for file in result.files {
        ui.info(&format!("env file: {file}"));
    }
    for (key, value) in result.vars {
        ui.info(&format!("{key}={value}"));
    }
    0
}

pub fn activate_result<U: Ui>(code: String, ui: &mut U) -> i32 {
    ui.info(&code);
    0
}

fn write_grouped_request<U: Ui>(items: &[InstallItemRequest], ui: &mut U) {
    for (kind, heading) in [
        (Some(ItemKind::Tool), "Tools"),
        (Some(ItemKind::Package), "Packages"),
        (Some(ItemKind::App), "Apps"),
        (None, "Items"),
    ] {
        let names = items
            .iter()
            .filter(|item| item.kind == kind)
            .map(|item| item.spec.name.as_str())
            .collect::<Vec<_>>();
        if names.is_empty() {
            continue;
        }
        ui.info(&format!("{heading}:"));
        for name in names {
            ui.info(&format!("  {name}"));
        }
    }
}

fn write_install_result<U: Ui>(res: &InstallResult, ui: &mut U) {
    if res.installed.is_empty() {
        write_one_install(
            &res.tool_name,
            &res.version,
            &res.install_path,
            res.binary_path.as_deref(),
            ui,
        );
        return;
    }

    for item in &res.installed {
        write_one_install(
            &item.name,
            &item.version,
            &item.install_path,
            item.binary_path.as_deref(),
            ui,
        );
    }
}

fn write_one_install<U: Ui>(
    name: &str,
    version: &str,
    install_path: &std::path::Path,
    binary_path: Option<&std::path::Path>,
    ui: &mut U,
) {
    if let Some(binary_path) = binary_path {
        ui.info(&format!("Binary installed at: {}", binary_path.display()));
    } else {
        ui.warning(&format!(
            "Could not find binary in {}",
            install_path.display()
        ));
    }

    ui.success(&format!(
        "Successfully installed {name}@{version} to {}",
        install_path.display()
    ));
}

fn write_list_section<U: Ui>(section: ListSection, ui: &mut U) {
    ui.info(match section.kind {
        ItemKind::Tool => "Tools:",
        ItemKind::Package => "Packages:",
        ItemKind::App => "Apps:",
    });
    if section.items.is_empty() {
        ui.info("  (none)");
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
        ui.info(&format!(
            "  {}@{}{}{}",
            item.name, item.version, source, status
        ));
        for output_path in item.outputs {
            ui.info(&format!("    output: {output_path}"));
        }
        for linked in item.linked_executables {
            ui.info(&format!("    linked: {linked}"));
        }
    }
}

fn write_service<U: Ui>(service: ServiceReport, ui: &mut U) {
    ui.info(&format!(
        "{}: {} - {}",
        service.name,
        service_status_label(service.status),
        service.detail
    ));
    if let Some(execution) = service.execution {
        ui.info(&format!("command: {}", execution.command));
        write_child_output(ui, &execution.stdout, false);
        write_child_output(ui, &execution.stderr, true);
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
    let kind = item
        .kind
        .map(|kind| kind.to_string())
        .unwrap_or_else(|| "item".to_string());
    format!(
        "{} {}@{}{}",
        kind, item.spec.name, item.spec.version, source
    )
}

fn sync_drift_label(drift: SyncDrift) -> &'static str {
    match drift {
        SyncDrift::LockfileMissing => "lockfile missing",
        SyncDrift::LockfileOutdated => "lockfile outdated",
    }
}

fn write_child_output<U: Ui>(ui: &mut U, content: &str, stderr: bool) {
    for line in content.lines() {
        if stderr {
            ui.error(line);
        } else {
            ui.info(line);
        }
    }
}
