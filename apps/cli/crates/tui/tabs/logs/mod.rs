//! Logs tab for local Still runtime messages.

#![allow(dead_code)]

use std::{env, fs};

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Text},
    widgets::{Block, Borders, Paragraph, Widget},
};

/// TUI tab for displaying local Still log lines.
#[derive(Debug)]
pub struct LogsTab;

impl Default for LogsTab {
    fn default() -> Self {
        Self
    }
}

impl LogsTab {
    /// Renders the newest local log lines when a Still log file exists.
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        let lines = log_lines();
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue))
            .title(" Logs ");
        Paragraph::new(Text::from(lines))
            .block(block)
            .render(area, buf);
    }
}

fn log_lines() -> Vec<Line<'static>> {
    let path = env::var_os("STILL_LOG").map(Into::into).or_else(|| {
        env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .map(|home| home.join(".cache/still/still.log"))
    });
    let Some(path) = path else {
        return vec![Line::from("No log file configured".dark_gray())];
    };
    let Ok(content) = fs::read_to_string(&path) else {
        return vec![Line::from(
            format!("No logs found at {}", path.display()).dark_gray(),
        )];
    };
    let mut lines = content
        .lines()
        .rev()
        .take(100)
        .map(|line| Line::from(line.to_string()))
        .collect::<Vec<_>>();
    lines.reverse();
    if lines.is_empty() {
        vec![Line::from("Log file is empty".dark_gray())]
    } else {
        lines
    }
}
