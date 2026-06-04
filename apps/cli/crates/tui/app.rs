//! Main TUI application state, rendering, and input handling.

use crate::components::action_menu::{Action, ActionMenu, ActionMenuState};
use crate::session::EngineSession;
use crate::tabs::packages::{NavigationDirection, PackageBrowser};
#[cfg(test)]
use crate::tabs::packages::{PackageKind, PackageRow};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    DefaultTerminal, Frame,
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Text},
    widgets::{Block, Borders, Paragraph, Widget},
};
use std::io;

/// Main application state machine for the terminal UI.
#[derive(Debug)]
pub struct App {
    search_query: String,
    exit: bool,
    search_focused: bool,
    action_menu: ActionMenuState,
    status_message: Option<String>,
    package_browser: PackageBrowser,
    session: Option<EngineSession>,
}

impl App {
    /// Creates a package browser backed by Still's cached item metadata.
    pub fn new() -> Self {
        let (package_browser, mut status_message) = match PackageBrowser::new() {
            Ok(browser) => (browser, None),
            Err(err) => (
                PackageBrowser::default(),
                Some(format!("Item cache unavailable: {err}")),
            ),
        };
        let session = match EngineSession::new() {
            Ok(session) => Some(session),
            Err(err) => {
                status_message = Some(format!("Engine session unavailable: {err}"));
                None
            }
        };

        Self {
            search_query: String::new(),
            exit: false,
            search_focused: false,
            action_menu: ActionMenuState::Closed,
            status_message,
            package_browser,
            session,
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let vertical = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(area);

        self.render_search_bar(vertical[0], buf);
        self.render_content(vertical[1], buf);
        self.render_footer(vertical[2], buf);

        if let ActionMenuState::Open { .. } = self.action_menu {
            self.render_action_menu(area, buf);
        }
    }
}

impl App {
    fn render_search_bar(&mut self, area: Rect, buf: &mut Buffer) {
        let search_text = if self.search_query.is_empty() {
            Text::from(vec![Line::from("Type to search".dark_gray())])
        } else {
            Text::from(vec![Line::from(self.search_query.clone())])
        };

        let title = if self.search_focused {
            " Search "
        } else {
            " Search (/) "
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(if self.search_focused {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Blue)
            });

        Paragraph::new(search_text).block(block).render(area, buf);
    }

    fn render_content(&mut self, area: Rect, buf: &mut Buffer) {
        let horizontal =
            Layout::horizontal([Constraint::Percentage(62), Constraint::Percentage(38)])
                .split(area);

        self.package_browser
            .render_table(horizontal[0], buf, &self.search_query);
        self.package_browser
            .render_details(horizontal[1], buf, &self.search_query);
    }

    fn render_footer(&self, area: Rect, buf: &mut Buffer) {
        let status = self
            .status_message
            .as_deref()
            .unwrap_or("q Quit  / Search  Enter Actions  Esc Clear");

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue))
            .title(" Still ");

        Paragraph::new(Line::from(status.to_string()))
            .block(block)
            .render(area, buf);
    }

    /// Runs the application's main loop until the user quits.
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    fn handle_events(&mut self) -> io::Result<()> {
        match event::read()? {
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                self.handle_key_event(key_event)
            }
            _ => {}
        };
        Ok(())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if key_event.modifiers.contains(KeyModifiers::CONTROL)
            && key_event.code == KeyCode::Char('c')
        {
            self.exit();
            return;
        }

        let menu_open = matches!(self.action_menu, ActionMenuState::Open { .. });

        match key_event.code {
            KeyCode::Char('q') if !self.search_focused => self.exit(),

            KeyCode::Up | KeyCode::Char('k') if menu_open => {
                if let ActionMenuState::Open {
                    ref mut selected_action,
                } = self.action_menu
                {
                    *selected_action = selected_action
                        .checked_sub(1)
                        .unwrap_or_else(|| Action::all().len() - 1);
                }
            }
            KeyCode::Down | KeyCode::Char('j') if menu_open => {
                if let ActionMenuState::Open {
                    ref mut selected_action,
                } = self.action_menu
                {
                    *selected_action = (*selected_action + 1) % Action::all().len();
                }
            }
            KeyCode::Enter if menu_open => {
                if let ActionMenuState::Open { selected_action } = self.action_menu {
                    self.handle_action_selection(selected_action);
                    self.action_menu = ActionMenuState::Closed;
                }
            }
            KeyCode::Esc if menu_open => {
                self.action_menu = ActionMenuState::Closed;
            }
            _ if menu_open => {}

            KeyCode::Char('/') => {
                self.search_focused = true;
                self.status_message = None;
            }
            KeyCode::Up | KeyCode::Char('k') if !self.search_focused => {
                self.package_browser
                    .handle_navigation(NavigationDirection::Up, &self.search_query);
            }
            KeyCode::Down | KeyCode::Char('j') if !self.search_focused => {
                self.package_browser
                    .handle_navigation(NavigationDirection::Down, &self.search_query);
            }
            KeyCode::Char(c) if self.search_focused => {
                self.search_query.push(c);
                self.package_browser.reset_selection();
            }
            KeyCode::Backspace if self.search_focused => {
                self.search_query.pop();
                self.package_browser.reset_selection();
            }
            KeyCode::Esc => {
                self.search_query.clear();
                self.search_focused = false;
                self.package_browser.reset_selection();
                self.status_message = None;
            }
            KeyCode::Enter if self.search_focused => {
                self.search_focused = false;
            }
            KeyCode::Enter | KeyCode::Char('l') if !self.search_focused => {
                if self
                    .package_browser
                    .selected_row(&self.search_query)
                    .is_some()
                {
                    self.action_menu = ActionMenuState::Open { selected_action: 0 };
                }
            }
            _ => {}
        }
    }

    fn exit(&mut self) {
        self.exit = true;
    }

    fn render_action_menu(&mut self, area: Rect, buf: &mut Buffer) {
        let title = self
            .package_browser
            .selected_row(&self.search_query)
            .map(|row| format!(" Actions: {} ", row.name))
            .unwrap_or_else(|| " Actions ".to_string());

        ActionMenu::new(self.action_menu, title).render(area, buf);
    }

    fn handle_action_selection(&mut self, action_idx: usize) {
        let Some(row) = self.package_browser.selected_row(&self.search_query) else {
            return;
        };
        let Some(session) = self.session.as_mut() else {
            self.status_message = Some("Engine session unavailable".to_string());
            return;
        };

        match Action::all().get(action_idx).copied() {
            Some(Action::Install) => {
                self.status_message = Some(match session.install_package(&row) {
                    Ok(()) => format!("Installed {}", row.name),
                    Err(err) => format!("Install failed: {err}"),
                });
            }
            Some(Action::Uninstall) => {
                self.status_message = Some(match session.uninstall_package(&row) {
                    Ok(()) => format!("Uninstalled {}", row.name),
                    Err(err) => format!("Uninstall failed: {err}"),
                });
            }
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn create_key_event(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::empty())
    }

    #[test]
    fn search_functionality() {
        let mut app = App::default();
        assert_eq!(app.search_query, "");

        app.handle_key_event(create_key_event(KeyCode::Char('/')));
        app.handle_key_event(create_key_event(KeyCode::Char('t')));
        app.handle_key_event(create_key_event(KeyCode::Char('e')));
        app.handle_key_event(create_key_event(KeyCode::Char('s')));
        assert_eq!(app.search_query, "tes");

        app.handle_key_event(create_key_event(KeyCode::Backspace));
        assert_eq!(app.search_query, "te");

        app.handle_key_event(create_key_event(KeyCode::Esc));
        assert_eq!(app.search_query, "");
        assert!(!app.search_focused);
    }

    #[test]
    fn exit_functionality() {
        let mut app = App::default();
        assert!(!app.exit);

        app.handle_key_event(create_key_event(KeyCode::Char('q')));
        assert!(app.exit);
    }

    #[test]
    fn removed_navigation_keys_do_not_change_state() {
        let mut app = App::default();
        app.handle_key_event(create_key_event(KeyCode::Char('2')));
        app.handle_key_event(create_key_event(KeyCode::Tab));
        app.handle_key_event(create_key_event(KeyCode::Char('t')));
        app.handle_key_event(create_key_event(KeyCode::Char('i')));

        assert!(!app.exit);
        assert!(!app.search_focused);
        assert_eq!(app.search_query, "");
        assert_eq!(app.action_menu, ActionMenuState::Closed);
    }

    #[test]
    fn action_menu_does_not_open_without_selection() {
        let mut app = App::default();
        app.package_browser = PackageBrowser::default();
        app.handle_key_event(create_key_event(KeyCode::Enter));
        assert_eq!(app.action_menu, ActionMenuState::Closed);
    }

    #[test]
    fn action_menu_opens_navigates_and_closes_with_selection() {
        let mut app = App::default();
        app.package_browser = PackageBrowser::with_rows(vec![PackageRow {
            kind: PackageKind::Package,
            name: "ripgrep".to_string(),
            version: "14.1.1".to_string(),
            state: "Available".to_string(),
        }]);

        app.handle_key_event(create_key_event(KeyCode::Enter));
        assert_eq!(
            app.action_menu,
            ActionMenuState::Open { selected_action: 0 }
        );

        app.handle_key_event(create_key_event(KeyCode::Down));
        assert_eq!(
            app.action_menu,
            ActionMenuState::Open { selected_action: 1 }
        );

        app.handle_key_event(create_key_event(KeyCode::Esc));
        assert_eq!(app.action_menu, ActionMenuState::Closed);
    }
}
