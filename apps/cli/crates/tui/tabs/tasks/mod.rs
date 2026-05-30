//! Tasks tab for configured Still task summaries.

#![allow(dead_code)]

use std::{env, fs};

use engine::config::{ConfigScope, ConfigSelection, resolve_config_path};
use engine::specs::toml::{TaskEntry, parse_still_toml};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Widget},
};

/// TUI tab for listing configured Still tasks.
#[derive(Debug)]
pub struct TasksTab;

impl Default for TasksTab {
    fn default() -> Self {
        Self
    }
}

impl TasksTab {
    /// Renders configured task names and command summaries.
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        let lines = task_lines();
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue))
            .title(" Tasks ");
        Paragraph::new(Text::from(lines))
            .block(block)
            .render(area, buf);
    }
}

fn task_lines() -> Vec<Line<'static>> {
    let Ok(start_dir) = env::current_dir() else {
        return vec![Line::from("Unable to read current directory".red())];
    };
    let home_dir = dirs_home();
    let Ok(resolved) = resolve_config_path(
        &start_dir,
        &home_dir,
        ConfigSelection {
            scope: ConfigScope::Project,
            for_write: false,
        },
    ) else {
        return vec![Line::from(
            "No still.toml found for this directory".dark_gray(),
        )];
    };
    let Ok(content) = fs::read_to_string(&resolved.path) else {
        return vec![Line::from(
            format!("Unable to read {}", resolved.path.display()).red(),
        )];
    };
    let Ok(config) = parse_still_toml(&content) else {
        return vec![Line::from("still.toml could not be parsed".red())];
    };
    if config.tasks.is_empty() {
        return vec![Line::from("No tasks configured".dark_gray())];
    }

    config
        .tasks
        .into_iter()
        .map(|(name, entry)| {
            Line::from(vec![
                Span::styled(name, Style::default().fg(Color::Cyan)),
                Span::raw("  "),
                Span::styled(task_summary(entry), Style::default().fg(Color::Gray)),
            ])
        })
        .collect()
}

fn task_summary(entry: TaskEntry) -> String {
    match entry {
        TaskEntry::Command(command) => command,
        TaskEntry::Expanded(task) => task
            .description
            .or_else(|| match task.run {
                engine::specs::toml::TaskRun::Command(command) => Some(command),
                engine::specs::toml::TaskRun::Commands(commands) => commands.first().cloned(),
                engine::specs::toml::TaskRun::None => None,
            })
            .unwrap_or_else(|| "configured".to_string()),
    }
}

fn dirs_home() -> std::path::PathBuf {
    env::var_os("HOME")
        .map(Into::into)
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| ".".into()))
}
