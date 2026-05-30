//! Modal action menu used by package rows.

use crate::components::popup::popup_area;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

/// Action menu modal state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionMenuState {
    Closed,
    Open { selected_action: usize },
}

/// Actions available from the package action menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Install,
    Uninstall,
    Info,
    Cancel,
}

impl Action {
    /// Returns actions in display order.
    ///
    /// The returned slice is the canonical order for rendering and keyboard
    /// selection. `selected_action` indexes from `ActionMenuState::Open` are
    /// interpreted against this order.
    pub fn all() -> &'static [Action] {
        &[
            Action::Install,
            Action::Uninstall,
            Action::Info,
            Action::Cancel,
        ]
    }

    /// Returns the label rendered for this action.
    ///
    /// The label is presentation text only; command execution should continue to
    /// match on the `Action` value so display wording can change safely.
    pub fn as_str(&self) -> &'static str {
        match self {
            Action::Install => "Install",
            Action::Uninstall => "Uninstall",
            Action::Info => "Info",
            Action::Cancel => "Cancel",
        }
    }
}

/// Renderable action menu modal.
///
/// The menu stores its current open/closed state and title. It does not mutate
/// selection itself; callers update `ActionMenuState` in their event handling and
/// pass the new state into `ActionMenu::new` for rendering.
pub struct ActionMenu {
    state: ActionMenuState,
    title: String,
}

impl ActionMenu {
    /// Creates a menu for a state and title.
    ///
    /// `state` controls whether the menu renders and which row is highlighted.
    /// `title` is drawn in the modal border and is owned by the menu so callers
    /// can build it from selected package data.
    pub fn new(state: ActionMenuState, title: String) -> Self {
        Self { state, title }
    }

    /// Renders the menu if it is open.
    ///
    /// `area` is the parent region used to center the popup. `buf` is the ratatui
    /// frame buffer that receives the clear layer, border, and action rows. When
    /// the state is `Closed`, this method returns without mutating the buffer.
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

        let mut lines = vec![Line::from("")];

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
            .border_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .style(Style::default().bg(Color::DarkGray)) // Background for the modal itself
            .title(self.title.clone());

        Paragraph::new(Text::from(lines))
            .block(block)
            .render(modal_area, buf);
    }
}
