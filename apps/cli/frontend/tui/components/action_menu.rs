use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};
use crate::tui::components::popup::popup_area;

/// Action menu modal state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionMenuState {
    Closed,
    Open { selected_action: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Install,
    Uninstall,
    Info,
    Cancel,
}

impl Action {
    pub fn all() -> &'static [Action] {
        &[Action::Install, Action::Uninstall, Action::Info, Action::Cancel]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Action::Install => "Install",
            Action::Uninstall => "Uninstall",
            Action::Info => "Info",
            Action::Cancel => "Cancel",
        }
    }
}

pub struct ActionMenu {
    state: ActionMenuState,
    title: String,
}

impl ActionMenu {
    pub fn new(state: ActionMenuState, title: String) -> Self {
        Self { state, title }
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        if let ActionMenuState::Closed = self.state {
            return;
        }

        // Calculate modal size and position (centered)
        let modal_width = 40;
        let modal_height = Action::all().len() as u16 + 4; // +4 for borders and title
        
        // Use popup_area helper to center the popup
        let modal_area = popup_area(area, modal_width, modal_height);
        
        // Clear the background before rendering the popup
        Clear.render(modal_area, buf);
        
        let selected_idx = if let ActionMenuState::Open { selected_action } = self.state {
            selected_action
        } else {
            0
        };

        let mut lines = vec![
            Line::from(""),
        ];

        for (idx, action) in Action::all().iter().enumerate() {
            let is_selected = idx == selected_idx;
            let style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let prefix = if is_selected { "▶ " } else { "  " };
            lines.push(Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(action.as_str(), style),
            ]));
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            .style(Style::default().bg(Color::DarkGray)) // Background for the modal itself
            .title(self.title.clone());

        Paragraph::new(Text::from(lines))
            .block(block)
            .render(modal_area, buf);
    }
}

