//! Engine action for listing and running configured tasks.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

use crate::config::{ConfigScope, ConfigSelection, resolve_config_path};
use crate::error::EngineError;
use crate::specs::toml::{ExpandedTask, ServiceEntry, TaskEntry, TaskRun, parse_still_toml};
use crate::trust::assert_config_trusted;

/// Request to list or run configured tasks.
#[derive(Debug, Clone)]
pub struct TaskRequest {
    pub start_dir: PathBuf,
    pub home_dir: PathBuf,
    pub name: Option<String>,
}

/// Result from listing or running tasks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskResult {
    pub path: PathBuf,
    pub tasks: Vec<TaskSummary>,
    pub executions: Vec<TaskExecution>,
    pub status: i32,
}

/// One task available in config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskSummary {
    pub name: String,
    pub description: Option<String>,
}

/// Captured output for one executed task command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskExecution {
    pub task: String,
    pub command: String,
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Lists tasks when `name` is absent, otherwise runs the named task graph.
/// # Errors
/// Fails when config cannot be read, the task is unknown, or a child command
/// cannot be spawned.
pub async fn run(request: TaskRequest) -> Result<TaskResult> {
    let resolved = resolve_config_path(
        &request.start_dir,
        &request.home_dir,
        ConfigSelection {
            scope: ConfigScope::Project,
            for_write: false,
        },
    )?;
    let content = tokio::fs::read_to_string(&resolved.path)
        .await
        .with_context(|| format!("failed to read {}", resolved.path.display()))?;
    let config = parse_still_toml(&content)?;
    let working_dir = resolved
        .path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let tasks = config.tasks;
    let services = config.services;
    let summaries = task_summaries(&tasks);

    let Some(name) = request.name else {
        return Ok(TaskResult {
            path: resolved.path,
            tasks: summaries,
            executions: Vec::new(),
            status: 0,
        });
    };
    if !tasks.contains_key(&name) {
        return Err(EngineError::Conflict {
            message: format!("unknown task \"{name}\""),
        }
        .into());
    }
    assert_config_trusted(&resolved.path, content.as_bytes(), "task execution").await?;

    let mut executions = Vec::new();
    let mut visited = BTreeSet::new();
    run_task_graph(
        &tasks,
        &services,
        &name,
        &working_dir,
        &mut visited,
        &mut executions,
    )?;
    let status = executions
        .last()
        .map(|execution| execution.status)
        .unwrap_or(0);

    Ok(TaskResult {
        path: resolved.path,
        tasks: summaries,
        executions,
        status,
    })
}

fn task_summaries(tasks: &BTreeMap<String, TaskEntry>) -> Vec<TaskSummary> {
    tasks
        .iter()
        .map(|(name, entry)| TaskSummary {
            name: name.clone(),
            description: match entry {
                TaskEntry::Command(_) => None,
                TaskEntry::Expanded(task) => task.description.clone(),
            },
        })
        .collect()
}

fn run_task_graph(
    tasks: &BTreeMap<String, TaskEntry>,
    services: &BTreeMap<String, ServiceEntry>,
    name: &str,
    working_dir: &Path,
    visited: &mut BTreeSet<String>,
    executions: &mut Vec<TaskExecution>,
) -> Result<()> {
    if !visited.insert(name.to_string()) {
        return Ok(());
    }

    let task = tasks.get(name).ok_or_else(|| EngineError::Conflict {
        message: format!("unknown task \"{name}\""),
    })?;
    let normalized = normalize_task(name, task)?;
    for dependency in normalized.depends {
        run_task_graph(
            tasks,
            services,
            &dependency,
            working_dir,
            visited,
            executions,
        )?;
        if executions
            .last()
            .is_some_and(|execution| execution.status != 0)
        {
            return Ok(());
        }
    }

    for service in normalized.requires {
        if !services.contains_key(&service) {
            return Err(EngineError::Conflict {
                message: format!("task \"{name}\" requires unknown service \"{service}\""),
            }
            .into());
        }
    }

    for command in normalized.commands {
        let output = shell_command(&command)
            .current_dir(working_dir)
            .output()
            .with_context(|| format!("failed to run task {name}"))?;
        let status = output.status.code().unwrap_or(1);
        executions.push(TaskExecution {
            task: name.to_string(),
            command,
            status,
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
        if status != 0 {
            break;
        }
    }

    Ok(())
}

struct NormalizedTask {
    depends: Vec<String>,
    requires: Vec<String>,
    commands: Vec<String>,
}

fn normalize_task(name: &str, entry: &TaskEntry) -> Result<NormalizedTask> {
    match entry {
        TaskEntry::Command(command) => Ok(NormalizedTask {
            depends: Vec::new(),
            requires: Vec::new(),
            commands: vec![command.clone()],
        }),
        TaskEntry::Expanded(task) => normalize_expanded_task(name, task),
    }
}

fn normalize_expanded_task(name: &str, task: &ExpandedTask) -> Result<NormalizedTask> {
    let commands = match &task.run {
        TaskRun::None => {
            return Err(EngineError::InvalidConfig {
                reason: format!("task \"{name}\" must define run"),
            }
            .into());
        }
        TaskRun::Command(command) => vec![command.clone()],
        TaskRun::Commands(commands) => commands.clone(),
    };

    Ok(NormalizedTask {
        depends: task.depends.clone(),
        requires: task.requires.clone(),
        commands,
    })
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
    async fn task_without_name_lists_configured_tasks() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("still.toml"),
            r#"
            [tasks]
            test = "cargo test"
            lint = { description = "Run lint", run = "cargo fmt --check" }
            "#,
        )
        .unwrap();

        let result = run(TaskRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            name: None,
        })
        .await
        .unwrap();

        assert_eq!(
            result.tasks,
            [
                TaskSummary {
                    name: "lint".to_string(),
                    description: Some("Run lint".to_string()),
                },
                TaskSummary {
                    name: "test".to_string(),
                    description: None,
                },
            ]
        );
        assert_eq!(result.executions, []);
    }

    #[tokio::test]
    async fn task_reports_unknown_task() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("still.toml"),
            "[tasks]\ntest = \"echo ok\"\n",
        )
        .unwrap();

        let err = run(TaskRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            name: Some("missing".to_string()),
        })
        .await
        .unwrap_err();

        assert!(err.to_string().contains("unknown task"));
    }

    #[tokio::test]
    async fn task_execution_requires_trust() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("still.toml"),
            "[tasks]\ntest = \"echo ok\"\n",
        )
        .unwrap();

        let err = run(TaskRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            name: Some("test".to_string()),
        })
        .await
        .unwrap_err();

        assert!(err.to_string().contains("not trusted"));
        assert!(err.to_string().contains("task execution"));
    }

    #[tokio::test]
    async fn trusted_task_executes_command() {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("still.toml");
        let config = "[tasks]\ntest = \"echo ok\"\n";
        fs::write(&config_path, config).unwrap();
        write_trust_marker(&config_path, config.as_bytes());

        let result = run(TaskRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            name: Some("test".to_string()),
        })
        .await
        .unwrap();

        assert_eq!(result.status, 0);
        assert_eq!(result.executions[0].stdout, "ok\n");
    }

    #[tokio::test]
    async fn list_form_task_stops_on_first_failure() {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("still.toml");
        let config = r#"
            [tasks.test]
            run = ["echo before", "exit 7", "echo after"]
        "#;
        fs::write(&config_path, config).unwrap();
        write_trust_marker(&config_path, config.as_bytes());

        let result = run(TaskRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            name: Some("test".to_string()),
        })
        .await
        .unwrap();

        assert_eq!(result.status, 7);
        assert_eq!(result.executions.len(), 2);
        assert_eq!(result.executions[0].command, "echo before");
        assert_eq!(result.executions[1].command, "exit 7");
    }

    #[tokio::test]
    async fn dependency_tasks_run_once_per_invocation() {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("still.toml");
        let config = r#"
            [tasks]
            setup = "echo setup"

            [tasks.build]
            depends = ["setup"]
            run = "echo build"

            [tasks.ci]
            depends = ["setup", "build"]
            run = "echo ci"
        "#;
        fs::write(&config_path, config).unwrap();
        write_trust_marker(&config_path, config.as_bytes());

        let result = run(TaskRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            name: Some("ci".to_string()),
        })
        .await
        .unwrap();

        let executed: Vec<_> = result
            .executions
            .iter()
            .map(|execution| execution.task.as_str())
            .collect();
        assert_eq!(executed, ["setup", "build", "ci"]);
    }

    #[tokio::test]
    async fn task_requires_configured_services() {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("still.toml");
        let config = r#"
            [tasks.test]
            requires = ["db"]
            run = "echo test"
        "#;
        fs::write(&config_path, config).unwrap();
        write_trust_marker(&config_path, config.as_bytes());

        let err = run(TaskRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            name: Some("test".to_string()),
        })
        .await
        .unwrap_err();

        assert!(
            err.to_string()
                .contains("task \"test\" requires unknown service \"db\"")
        );
    }

    #[tokio::test]
    async fn task_runs_when_required_services_are_configured() {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("still.toml");
        let config = r#"
            [services]
            db = "echo db"

            [tasks.test]
            requires = ["db"]
            run = "echo test"
        "#;
        fs::write(&config_path, config).unwrap();
        write_trust_marker(&config_path, config.as_bytes());

        let result = run(TaskRequest {
            start_dir: temp.path().to_path_buf(),
            home_dir: temp.path().to_path_buf(),
            name: Some("test".to_string()),
        })
        .await
        .unwrap();

        assert_eq!(result.status, 0);
        assert_eq!(result.executions[0].stdout, "test\n");
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
}
