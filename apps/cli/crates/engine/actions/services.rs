//! Engine action for inspecting and running configured services.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

use crate::config::{ConfigScope, ConfigSelection, resolve_config_path};
use crate::error::EngineError;
use crate::specs::toml::{ExpandedService, ServiceAction, ServiceEntry, parse_still_toml};

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
    let services = select_services(config.services, request.name.as_deref())?;
    let reports = services
        .into_iter()
        .map(|(name, service)| service_report(name, service, request.operation, &working_dir))
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

    let ActionCommand::Command(command) = action else {
        return Ok(ServiceReport {
            name,
            status: ServiceStatus::Skipped,
            detail: "task-backed service actions are listed but not executed by services yet"
                .to_string(),
            execution: None,
        });
    };

    let output = shell_command(&command)
        .current_dir(working_dir)
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
    let start = service
        .start
        .map(action_command)
        .or_else(|| service.task.map(ActionCommand::Task));
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
        stop: service.stop.map(action_command),
        check: service.check.map(action_command),
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
}
