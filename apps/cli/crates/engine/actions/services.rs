//! Engine action for inspecting and running configured services.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

use crate::actions::run::resolve_env;
use crate::config::{ConfigScope, ConfigSelection, resolve_config_path};
use crate::error::EngineError;
use crate::specs::toml::{
    ExpandedService, ServiceAction, ServiceEntry, TaskEntry, TaskRun, parse_still_toml,
};
use crate::trust::assert_config_trusted;

/// Service operation requested by a caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServicesOperation {
    Status,
    Start,
    Stop,
    Check,
}

/// Request to inspect or run services.
#[derive(Debug, Clone)]
pub struct ServicesRequest {
    pub start_dir: PathBuf,
    pub home_dir: PathBuf,
    pub operation: ServicesOperation,
    pub name: Option<String>,
}

/// Result of a service operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServicesResult {
    pub path: PathBuf,
    pub services: Vec<ServiceReport>,
}

/// One configured service and any command execution output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceReport {
    pub name: String,
    pub status: ServiceStatus,
    pub detail: String,
    pub execution: Option<ServiceExecution>,
}

/// Service operation status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceStatus {
    Configured,
    Skipped,
    Ok,
    Failed,
}

/// Captured command execution for a service action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceExecution {
    pub command: String,
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Inspects services or runs their configured start/stop/check action.
/// # Errors
/// Fails when config cannot be read, the service is unknown, or a child command
/// cannot be spawned.
pub async fn run(request: ServicesRequest) -> Result<ServicesResult> {
    let resolved = resolve_config_path(
        &request.start_dir,
        &request.home_dir,
        ConfigSelection {
            scope: ConfigScope::Project,
            for_write: false,
        },
    )?;
    let working_dir = resolved
        .path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let content = tokio::fs::read_to_string(&resolved.path)
        .await
        .with_context(|| format!("failed to read {}", resolved.path.display()))?;
    let config = parse_still_toml(&content)?;
    let tasks = config.tasks;
    let services = select_services(config.services, request.name.as_deref())?;
    if request.operation != ServicesOperation::Status {
        assert_config_trusted(&resolved.path, content.as_bytes(), "service actions").await?;
    }
    let resolved_env = if request.operation == ServicesOperation::Status {
        None
    } else {
        Some(resolve_env(&request.start_dir, &request.home_dir).await?)
    };
    let empty_env = BTreeMap::new();
    let reports = services
        .into_iter()
        .map(|(name, service)| {
            let command_dir = resolved_env
                .as_ref()
                .map(|env| env.working_dir.as_path())
                .unwrap_or(&working_dir);
            let command_env = resolved_env
                .as_ref()
                .map(|env| &env.vars)
                .unwrap_or(&empty_env);
            service_report(
                name,
                service,
                request.operation,
                command_dir,
                command_env,
                &tasks,
            )
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(ServicesResult {
        path: resolved.path,
        services: reports,
    })
}

fn select_services(
    services: BTreeMap<String, ServiceEntry>,
    name: Option<&str>,
) -> Result<Vec<(String, ServiceEntry)>> {
    match name {
        Some(name) => {
            let service = services.get(name).ok_or_else(|| EngineError::Conflict {
                message: format!("unknown service \"{name}\""),
            })?;
            Ok(vec![(name.to_string(), service.clone())])
        }
        None => Ok(services.into_iter().collect()),
    }
}

fn service_report(
    name: String,
    service: ServiceEntry,
    operation: ServicesOperation,
    working_dir: &Path,
    env: &BTreeMap<String, String>,
    tasks: &BTreeMap<String, TaskEntry>,
) -> Result<ServiceReport> {
    let normalized = normalize_service(service);
    if operation == ServicesOperation::Status {
        return Ok(ServiceReport {
            name,
            status: ServiceStatus::Configured,
            detail: normalized.detail,
            execution: None,
        });
    }

    let action = match operation {
        ServicesOperation::Start => normalized.start,
        ServicesOperation::Stop => normalized.stop,
        ServicesOperation::Check => normalized.check,
        ServicesOperation::Status => None,
    };
    let Some(action) = action else {
        return Ok(ServiceReport {
            name,
            status: ServiceStatus::Skipped,
            detail: format!("no {} action configured", operation_name(operation)),
            execution: None,
        });
    };

    let command = match action {
        ActionCommand::Command(command) => command,
        ActionCommand::Task(task) => task_command(tasks, &task)?,
    };

    let output = shell_command(&command)
        .current_dir(working_dir)
        .envs(env)
        .output()
        .with_context(|| format!("failed to run service {name}"))?;
    let status = output.status.code().unwrap_or(1);

    Ok(ServiceReport {
        name,
        status: if status == 0 {
            ServiceStatus::Ok
        } else {
            ServiceStatus::Failed
        },
        detail: operation_name(operation).to_string(),
        execution: Some(ServiceExecution {
            command,
            status,
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }),
    })
}

struct NormalizedService {
    detail: String,
    start: Option<ActionCommand>,
    stop: Option<ActionCommand>,
    check: Option<ActionCommand>,
}

#[derive(Debug, Clone)]
enum ActionCommand {
    Command(String),
    Task(String),
}

fn normalize_service(service: ServiceEntry) -> NormalizedService {
    match service {
        ServiceEntry::Command(command) => NormalizedService {
            detail: command.clone(),
            start: Some(ActionCommand::Command(command)),
            stop: None,
            check: None,
        },
        ServiceEntry::Expanded(service) => normalize_expanded_service(service),
    }
}

fn normalize_expanded_service(service: ExpandedService) -> NormalizedService {
    let preset = service.preset.as_deref().and_then(service_preset);
    let start = service
        .start
        .map(action_command)
        .or_else(|| service.task.map(ActionCommand::Task))
        .or_else(|| preset.as_ref().and_then(|preset| preset.start.clone()));
    let detail = service
        .preset
        .clone()
        .or_else(|| action_detail(start.as_ref()))
        .or_else(|| {
            service
                .check
                .as_ref()
                .and_then(|action| action_detail_ref(action))
        })
        .unwrap_or_else(|| "configured".to_string());

    NormalizedService {
        detail,
        start,
        stop: service
            .stop
            .map(action_command)
            .or_else(|| preset.as_ref().and_then(|preset| preset.stop.clone())),
        check: service
            .check
            .map(action_command)
            .or_else(|| preset.and_then(|preset| preset.check)),
    }
}

#[derive(Debug, Clone)]
struct ServicePreset {
    start: Option<ActionCommand>,
    stop: Option<ActionCommand>,
    check: Option<ActionCommand>,
}

fn service_preset(name: &str) -> Option<ServicePreset> {
    let command = |value: &str| Some(ActionCommand::Command(value.to_string()));
    match name {
        "docker" => Some(ServicePreset {
            start: None,
            stop: None,
            check: command("docker info"),
        }),
        "docker-compose" | "compose" => Some(ServicePreset {
            start: command("docker compose up -d"),
            stop: command("docker compose down"),
            check: command("docker compose ps"),
        }),
        "postgres" | "postgresql" => Some(ServicePreset {
            start: None,
            stop: None,
            check: command("pg_isready"),
        }),
        "redis" => Some(ServicePreset {
            start: None,
            stop: None,
            check: command("redis-cli ping"),
        }),
        _ => None,
    }
}

fn action_command(action: ServiceAction) -> ActionCommand {
    match action {
        ServiceAction::Task { task } => ActionCommand::Task(task),
        ServiceAction::Command { command } => ActionCommand::Command(command),
    }
}

fn action_detail(action: Option<&ActionCommand>) -> Option<String> {
    match action {
        Some(ActionCommand::Command(command)) => Some(command.clone()),
        Some(ActionCommand::Task(task)) => Some(format!("task:{task}")),
        None => None,
    }
}

fn action_detail_ref(action: &ServiceAction) -> Option<String> {
    match action {
        ServiceAction::Command { command } => Some(command.clone()),
        ServiceAction::Task { task } => Some(format!("task:{task}")),
    }
}

fn task_command(tasks: &BTreeMap<String, TaskEntry>, task: &str) -> Result<String> {
    let entry = tasks.get(task).ok_or_else(|| EngineError::Conflict {
        message: format!("unknown task \"{task}\""),
    })?;
    match entry {
        TaskEntry::Command(command) => Ok(command.clone()),
        TaskEntry::Expanded(task_entry) => match &task_entry.run {
            TaskRun::Command(command) => Ok(command.clone()),
            TaskRun::Commands(commands) => commands.first().cloned().ok_or_else(|| {
                EngineError::InvalidConfig {
                    reason: format!("task \"{task}\" must define run"),
                }
                .into()
            }),
            TaskRun::None => Err(EngineError::InvalidConfig {
                reason: format!("task \"{task}\" must define run"),
            }
            .into()),
        },
    }
}

fn operation_name(operation: ServicesOperation) -> &'static str {
    match operation {
        ServicesOperation::Status => "status",
        ServicesOperation::Start => "start",
        ServicesOperation::Stop => "stop",
        ServicesOperation::Check => "check",
    }
}

fn shell_command(command: &str) -> Command {
    #[cfg(windows)]
    {
        let mut child = Command::new("cmd");
        child.args(["/C", command]);
        child
    }
    #[cfg(not(windows))]
    {
        let mut child = Command::new("sh");
        child.args(["-c", command]);
        child
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::trust::{config_fingerprint, trust_marker_path};

    use super::*;

    #[tokio::test]
    async fn services_status_lists_configured_services() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("still.toml"),
            r#"
            [services]
            web = "echo web"
            db = { task = "db:start" }
            "#,
        )
        .unwrap();

        let result = run(ServicesRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            operation: ServicesOperation::Status,
            name: None,
        })
        .await
        .unwrap();

        assert_eq!(result.services.len(), 2);
        assert!(
            result
                .services
                .iter()
                .all(|service| { service.status == ServiceStatus::Configured })
        );
    }

    #[tokio::test]
    async fn services_reports_unknown_service() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("still.toml"),
            "[services]\nweb = \"echo web\"\n",
        )
        .unwrap();

        let err = run(ServicesRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            operation: ServicesOperation::Check,
            name: Some("missing".to_string()),
        })
        .await
        .unwrap_err();

        assert!(err.to_string().contains("unknown service"));
    }

    #[tokio::test]
    async fn services_start_runs_task_backed_action() {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("still.toml");
        let config = r#"
            [services]
            web = { task = "web:start" }

            [tasks]
            "web:start" = "echo web"
            "#;
        fs::write(&config_path, config).unwrap();
        write_trust_marker(&config_path, config.as_bytes());

        let result = run(ServicesRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            operation: ServicesOperation::Start,
            name: Some("web".to_string()),
        })
        .await
        .unwrap();

        let execution = result.services[0].execution.as_ref().unwrap();
        assert_eq!(execution.command, "echo web");
        assert_eq!(execution.stdout, "web\n");
    }

    #[tokio::test]
    async fn service_actions_use_configured_env() {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("still.toml");
        let config = format!(
            r#"
            [env]
            INLINE = "service-env"

            [services]
            web = "{}"
            "#,
            env_echo_command("INLINE")
        );
        fs::write(&config_path, &config).unwrap();
        write_trust_marker(&config_path, config.as_bytes());

        let result = run(ServicesRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            operation: ServicesOperation::Start,
            name: Some("web".to_string()),
        })
        .await
        .unwrap();

        let execution = result.services[0].execution.as_ref().unwrap();
        assert_eq!(execution.stdout.trim(), "service-env");
    }

    #[test]
    fn built_in_presets_supply_common_service_actions() {
        let normalized = normalize_expanded_service(ExpandedService {
            preset: Some("docker-compose".to_string()),
            ..ExpandedService::default()
        });

        assert!(matches!(
            normalized.start,
            Some(ActionCommand::Command(ref command)) if command == "docker compose up -d"
        ));
        assert!(matches!(
            normalized.stop,
            Some(ActionCommand::Command(ref command)) if command == "docker compose down"
        ));
        assert!(matches!(
            normalized.check,
            Some(ActionCommand::Command(ref command)) if command == "docker compose ps"
        ));
    }

    #[test]
    fn explicit_service_actions_override_presets() {
        let normalized = normalize_expanded_service(ExpandedService {
            preset: Some("docker-compose".to_string()),
            check: Some(ServiceAction::Command {
                command: "custom check".to_string(),
            }),
            ..ExpandedService::default()
        });

        assert!(matches!(
            normalized.check,
            Some(ActionCommand::Command(ref command)) if command == "custom check"
        ));
    }

    #[tokio::test]
    async fn service_actions_require_trust() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("still.toml"),
            "[services]\nweb = \"echo web\"\n",
        )
        .unwrap();

        let err = run(ServicesRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            operation: ServicesOperation::Start,
            name: Some("web".to_string()),
        })
        .await
        .unwrap_err();

        assert!(err.to_string().contains("not trusted"));
        assert!(err.to_string().contains("service actions"));
    }

    fn write_trust_marker(config_path: &Path, content: &[u8]) {
        let marker_path = trust_marker_path(config_path);
        fs::create_dir_all(marker_path.parent().unwrap()).unwrap();
        fs::write(
            marker_path,
            format!(
                "config = \"{}\"\nfingerprint = \"{}\"\n",
                config_path.display(),
                config_fingerprint(content)
            ),
        )
        .unwrap();
    }

    fn env_echo_command(name: &str) -> String {
        if cfg!(windows) {
            format!("echo %{name}%")
        } else {
            format!("printf \\\"${name}\\\"")
        }
    }
}
