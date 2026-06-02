//! Configuration tab for the active Still config.

#![allow(dead_code)]

use std::{env, fs};

use engine::config::{ConfigScope, ConfigSelection, resolve_config_path};
use engine::specs::toml::parse_still_toml;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Widget},
};

/// TUI tab for viewing the active Still config summary.
#[derive(Debug)]
pub struct ConfigTab;

impl Default for ConfigTab {
    fn default() -> Self {
        Self
    }
}

impl ConfigTab {
    /// Renders the active config path and section counts.
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        let lines = config_lines();
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue))
            .title(" Config ");
        Paragraph::new(Text::from(lines))
            .block(block)
            .render(area, buf);
    }
}

fn config_lines() -> Vec<Line<'static>> {
    let Ok(start_dir) = env::current_dir() else {
        return vec![Line::from("Unable to read current directory".red())];
    };
    let home_dir = env::var_os("HOME")
        .map(Into::into)
        .unwrap_or_else(|| start_dir.clone());
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

    vec![
        label_value("Path", resolved.path.display().to_string()),
        label_value("Tools", config.tools.len().to_string()),
        label_value(
            "Packages",
            (config.packages.latest.len() + config.packages.entries.len()).to_string(),
        ),
        label_value(
            "Apps",
            (config.apps.latest.len() + config.apps.entries.len()).to_string(),
        ),
        label_value("Tasks", config.tasks.len().to_string()),
        label_value("Services", config.services.len().to_string()),
        label_value(
            "Agent skills",
            config
                .agents
                .and_then(|agents| agents.skills)
                .map(|skills| match skills {
                    engine::specs::toml::AgentSkills::List(items) => items.len(),
                    engine::specs::toml::AgentSkills::Table(items) => items.len(),
                })
                .unwrap_or(0)
                .to_string(),
        ),
    ]
}

fn label_value(label: &'static str, value: String) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label}: "), Style::default().fg(Color::Cyan)),
        Span::styled(value, Style::default().fg(Color::White)),
    ])
}
