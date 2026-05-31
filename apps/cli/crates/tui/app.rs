//! Main TUI application state, rendering, and input handling.

use crate::components::action_menu::{Action, ActionMenu, ActionMenuState};
use crate::tabs::config::ConfigTab;
use crate::tabs::formula::{FormulaTab, NavigationDirection, PackageKind, PackageRow};
use crate::tabs::logs::LogsTab;
use crate::tabs::resources::ResourcesTab;
use crate::tabs::tasks::TasksTab;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use engine::actions::install::{InstallAndRecordRequest, InstallItemRequest, InstallRequest};
use engine::actions::uninstall::{UninstallRequest, UninstallTarget};
use engine::specs::item::{ItemKind, ItemSpec};
use ratatui::{
    DefaultTerminal, Frame,
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Widget},
};
use std::io;
use std::path::PathBuf;

/// Top-level tab identifiers for the TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Packages,
    Tasks,
    Config,
    Logs,
    Resources,
}

impl Tab {
    /// Returns the static label rendered in the tab bar for this tab.
    ///
    /// The label is presentation text only; changing it does not change keyboard
    /// routing or the enum variant used by application state.
    pub fn as_str(&self) -> &'static str {
        match self {
            Tab::Packages => "Packages",
            Tab::Tasks => "Tasks",
            Tab::Config => "Config",
            Tab::Logs => "Logs",
            Tab::Resources => "Resources",
        }
    }

    /// Returns every tab in display order.
    ///
    /// Navigation and rendering use this slice as the canonical tab ordering, so
    /// new tabs should be added here at the same time they are added to `Tab`.
    pub fn all() -> &'static [Tab] {
        &[
            Tab::Packages,
            Tab::Tasks,
            Tab::Config,
            Tab::Logs,
            Tab::Resources,
        ]
    }

    /// Returns the next tab, wrapping from the last tab back to the first.
    ///
    /// `self` is matched by value against `Tab::all`; unknown ordering falls back
    /// to the first tab through the `position(...).unwrap_or(0)` default.
    pub fn next(&self) -> Tab {
        let tabs = Tab::all();
        let current_idx = tabs.iter().position(|t| t == self).unwrap_or(0);
        let next_idx = (current_idx + 1) % tabs.len();
        tabs[next_idx]
    }

    /// Returns the previous tab, wrapping from the first tab to the last.
    ///
    /// `self` is matched by value against `Tab::all`; unknown ordering falls back
    /// to the first tab before stepping backward.
    pub fn previous(&self) -> Tab {
        let tabs = Tab::all();
        let current_idx = tabs.iter().position(|t| t == self).unwrap_or(0);
        let prev_idx = if current_idx == 0 {
            tabs.len() - 1
        } else {
            current_idx - 1
        };
        tabs[prev_idx]
    }
}

impl Default for Tab {
    fn default() -> Self {
        Tab::Packages
    }
}

/// Main application state machine for the terminal UI.
///
/// Owns global UI state such as active tab, search focus, action-menu state, and
/// tab-specific child state. Callers should construct this with `App::new`, then
/// pass a terminal to `run` so the app can handle drawing and input until exit.
#[derive(Debug)]
pub struct App {
    current_tab: Tab,
    search_query: String,
    exit: bool,
    search_focused: bool,
    action_menu: ActionMenuState,
    // Tab-specific state
    formula_tab: FormulaTab,
    tasks_tab: TasksTab,
    config_tab: ConfigTab,
    logs_tab: LogsTab,
    resources_tab: ResourcesTab,
}

impl App {
    /// Creates a new application state with tab-specific state initialized.
    ///
    /// Formula data is loaded from the local cache during construction. If that
    /// load fails, the app keeps running with an empty formula tab and prints the
    /// error to stderr; this keeps TUI startup resilient while registry caching is
    /// still evolving.
    pub fn new() -> Self {
        let formula_tab = FormulaTab::new().unwrap_or_else(|e| {
            eprintln!("Warning: Failed to initialize formula tab: {}", e);
            FormulaTab::default()
        });

        Self {
            current_tab: Tab::default(),
            search_query: String::new(),
            exit: false,
            search_focused: false,
            action_menu: ActionMenuState::Closed,
            formula_tab,
            tasks_tab: TasksTab,
            config_tab: ConfigTab,
            logs_tab: LogsTab,
            resources_tab: ResourcesTab::new(),
        }
    }
}

impl Default for App {
    fn default() -> Self {
        // This is only used for tests - use App::new() in production
        Self::new()
    }
}

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Split the area into: tabs, search bar, and content
        let vertical = Layout::vertical([
            Constraint::Length(3), // Tabs
            Constraint::Length(3), // Search bar
            Constraint::Min(0),    // Content
        ])
        .split(area);

        // Render tabs
        self.render_tabs(vertical[0], buf);

        // Render search bar
        self.render_search_bar(vertical[1], buf);

        // Render content based on current tab
        self.render_content(vertical[2], buf);

        // Render action menu modal if open
        if let ActionMenuState::Open { .. } = self.action_menu {
            self.render_action_menu(area, buf);
        }
    }
}

impl App {
    fn render_tabs(&mut self, area: Rect, buf: &mut Buffer) {
        let tabs: Vec<Span> = Tab::all()
            .iter()
            .enumerate()
            .flat_map(|(idx, tab)| {
                let is_active = *tab == self.current_tab;
                let style = if is_active {
                    Style::default()
                        .fg(Color::Cyan)
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Gray)
                };

                let mut spans = vec![Span::styled(format!(" {} ", tab.as_str()), style)];

                // Add separator between tabs (except after last)
                if idx < Tab::all().len() - 1 {
                    spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
                }

                spans
            })
            .collect();

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue))
            .title(" Tabs ");

        Paragraph::new(Line::from(tabs))
            .block(block)
            .render(area, buf);
    }

    fn render_search_bar(&mut self, area: Rect, buf: &mut Buffer) {
        let search_text = if self.search_query.is_empty() {
            Text::from(vec![Line::from(vec![
                Span::styled("🔍 ", Style::default().fg(Color::Yellow)),
                Span::styled("Search...", Style::default().fg(Color::DarkGray)),
            ])])
        } else {
            Text::from(vec![Line::from(vec![
                Span::styled("🔍 ", Style::default().fg(Color::Yellow)),
                Span::styled(self.search_query.clone(), Style::default().fg(Color::White)),
            ])])
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(if self.search_focused {
                " Search (typing...) "
            } else {
                " Search (press '/' to focus) "
            })
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
        // Split into table and preview
        let horizontal = Layout::horizontal([
            Constraint::Percentage(60), // Table
            Constraint::Percentage(40), // Preview
        ])
        .split(area);

        // Delegate rendering to the current tab mode
        match self.current_tab {
            Tab::Packages => {
                self.formula_tab
                    .render_table(horizontal[0], buf, &self.search_query);
                self.formula_tab
                    .render_preview(horizontal[1], buf, &self.search_query);
            }
            Tab::Resources => {
                // Resources tab uses full area for btop
                self.render_resources(area, buf);
            }
            Tab::Tasks => {
                self.tasks_tab.render(area, buf);
            }
            Tab::Config => {
                self.config_tab.render(area, buf);
            }
            Tab::Logs => {
                self.logs_tab.render(area, buf);
            }
        }
    }

    fn render_resources(&mut self, area: Rect, buf: &mut Buffer) {
        self.resources_tab.render(area, buf);
    }

    /// Runs the application's main loop until the user quits.
    ///
    /// `terminal` must already be initialized for ratatui drawing. The method
    /// mutates `self` in response to keyboard events, redraws each frame, and
    /// returns any terminal or event-read I/O error to the caller.
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
        // Check if we're in interactive CLI mode (Resources tab)
        let resources_interactive =
            self.current_tab == Tab::Resources && self.resources_tab.is_interactive();

        // If in interactive mode, forward keys to the CLI tool
        if resources_interactive {
            // Esc exits interactive mode
            if key_event.code == KeyCode::Esc {
                self.resources_tab.toggle_interactive();
                return;
            }

            // Forward all keys to the interactive CLI
            // Convert key code to bytes that can be sent to the process
            match key_event.code {
                KeyCode::Char(c) => {
                    self.resources_tab.send_key(c as u8);
                }
                KeyCode::Enter => {
                    self.resources_tab.send_key(b'\n');
                }
                KeyCode::Backspace => {
                    self.resources_tab.send_key(0x7f); // DEL character
                }
                KeyCode::Tab => {
                    self.resources_tab.send_key(b'\t');
                }
                KeyCode::Up => {
                    // Send ANSI escape sequence for up arrow
                    self.resources_tab.send_input("\x1b[A");
                }
                KeyCode::Down => {
                    self.resources_tab.send_input("\x1b[B");
                }
                KeyCode::Left => {
                    self.resources_tab.send_input("\x1b[D");
                }
                KeyCode::Right => {
                    self.resources_tab.send_input("\x1b[C");
                }
                _ => {
                    // For other keys, try to send as-is if possible
                    // Most special keys won't work without proper terminal emulation
                }
            }
            // Don't process other keys when in interactive mode
            return;
        }

        // Check if action menu is open - if so, only handle menu-specific keys
        let menu_open = matches!(self.action_menu, ActionMenuState::Open { .. });

        // Handle Ctrl+C to exit
        if key_event.modifiers.contains(KeyModifiers::CONTROL)
            && key_event.code == KeyCode::Char('c')
        {
            self.exit();
            return;
        }

        match key_event.code {
            KeyCode::Char('q') => self.exit(),

            // Handle action menu navigation first (when menu is open)
            KeyCode::Up | KeyCode::Char('k') if menu_open => {
                if let ActionMenuState::Open {
                    ref mut selected_action,
                } = self.action_menu
                {
                    if *selected_action > 0 {
                        *selected_action -= 1;
                    } else {
                        *selected_action = Action::all().len() - 1;
                    }
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

            // Select action in menu
            KeyCode::Enter if menu_open => {
                if let ActionMenuState::Open { selected_action } = self.action_menu {
                    self.handle_action_selection(selected_action);
                    self.action_menu = ActionMenuState::Closed;
                }
            }

            // Close menu with Esc
            KeyCode::Esc if menu_open => {
                self.action_menu = ActionMenuState::Closed;
            }

            // If menu is open, ignore all other keys
            _ if menu_open => {}

            // Focus search bar (only when menu is closed)
            KeyCode::Char('/') => {
                self.search_focused = true;
            }

            // Row navigation (only when search is not focused and menu is closed)
            KeyCode::Up | KeyCode::Char('k') if !self.search_focused => {
                self.handle_navigation(NavigationDirection::Up);
            }
            KeyCode::Down | KeyCode::Char('j') if !self.search_focused => {
                self.handle_navigation(NavigationDirection::Down);
            }

            // Tab navigation (only when search is not focused and menu is closed)
            KeyCode::Tab if !self.search_focused => {
                self.switch_tab(self.current_tab.next());
            }
            KeyCode::BackTab if !self.search_focused => {
                self.switch_tab(self.current_tab.previous());
            }
            KeyCode::Left if !self.search_focused => {
                self.switch_tab(self.current_tab.previous());
            }
            KeyCode::Right if !self.search_focused => {
                self.switch_tab(self.current_tab.next());
            }

            // Number keys for direct tab selection (only when search is not focused and menu is closed)
            KeyCode::Char('1') if !self.search_focused => {
                self.switch_tab(Tab::Packages);
            }
            KeyCode::Char('2') if !self.search_focused => {
                self.switch_tab(Tab::Tasks);
            }
            KeyCode::Char('3') if !self.search_focused => {
                self.switch_tab(Tab::Config);
            }
            KeyCode::Char('4') if !self.search_focused => {
                self.switch_tab(Tab::Logs);
            }
            KeyCode::Char('5') if !self.search_focused => {
                self.switch_tab(Tab::Resources);
            }

            // Package filters (only when search is not focused and menu is closed)
            KeyCode::Char('t') if !self.search_focused => {
                if self.current_tab == Tab::Packages {
                    self.formula_tab.cycle_kind_filter();
                }
            }
            KeyCode::Char('i') if !self.search_focused => {
                if self.current_tab == Tab::Packages {
                    self.formula_tab.cycle_install_filter();
                } else if self.current_tab == Tab::Resources {
                    // Toggle interactive mode for Resources tab
                    self.resources_tab.toggle_interactive();
                }
            }
            KeyCode::Char('r') if !self.search_focused => {
                if self.current_tab == Tab::Packages {
                    self.formula_tab.reset_filters();
                }
            }

            // Search bar input (when focused and menu is closed)
            KeyCode::Char(c) if self.search_focused => {
                self.search_query.push(c);
                self.reset_selection();
            }
            KeyCode::Backspace if self.search_focused => {
                self.search_query.pop();
                self.reset_selection();
            }
            KeyCode::Esc => {
                // Clear search if menu is closed
                self.search_query.clear();
                self.search_focused = false;
                self.reset_selection();
            }
            KeyCode::Enter if self.search_focused => {
                self.search_focused = false;
            }

            // Open action menu with Enter or 'l' (when not in search and menu is closed)
            KeyCode::Enter | KeyCode::Char('l') if !self.search_focused => {
                if self.current_tab == Tab::Packages {
                    self.action_menu = ActionMenuState::Open { selected_action: 0 };
                }
            }
            _ => {}
        }
    }

    fn handle_navigation(&mut self, direction: NavigationDirection) {
        match self.current_tab {
            Tab::Packages => {
                self.formula_tab
                    .handle_navigation(direction, &self.search_query);
            }
            _ => {
                // Other tabs don't support navigation yet
            }
        }
    }

    fn switch_tab(&mut self, new_tab: Tab) {
        self.current_tab = new_tab;
        self.reset_selection();
    }

    fn reset_selection(&mut self) {
        match self.current_tab {
            Tab::Packages => {
                self.formula_tab.reset_selection();
            }
            _ => {
                // Other tabs don't have selection yet
            }
        }
    }

    fn exit(&mut self) {
        self.exit = true;
    }

    fn render_action_menu(&mut self, area: Rect, buf: &mut Buffer) {
        // Get selected formula name for display
        let selected_formula = match self.current_tab {
            Tab::Packages => {
                let filtered = self.formula_tab.filter(&self.search_query);
                if let Some(row) = filtered.get(
                    self.formula_tab
                        .selected_index
                        .min(filtered.len().saturating_sub(1)),
                ) {
                    Some(row.name.clone())
                } else {
                    None
                }
            }
            _ => None,
        };

        let title = if let Some(name) = selected_formula {
            format!(" Actions: {} ", name)
        } else {
            " Actions ".to_string()
        };

        let action_menu = ActionMenu::new(self.action_menu, title);
        action_menu.render(area, buf);
    }

    fn handle_action_selection(&mut self, action_idx: usize) {
        let action = Action::all().get(action_idx).copied();

        if let Some(action) = action {
            match action {
                Action::Install => {
                    if let Some(row) = self.selected_package_row() {
                        match install_package_from_tui(&row) {
                            Ok(()) => eprintln!("Installed: {}", row.name),
                            Err(err) => eprintln!("Install failed for {}: {err}", row.name),
                        }
                    }
                }
                Action::Uninstall => {
                    if let Some(row) = self.selected_package_row() {
                        match uninstall_package_from_tui(&row) {
                            Ok(()) => eprintln!("Uninstalled: {}", row.name),
                            Err(err) => eprintln!("Uninstall failed for {}: {err}", row.name),
                        }
                    }
                }
                Action::Info => {
                    if let Some(row) = self.selected_package_row() {
                        eprintln!("Info for: {}", row.name);
                        eprintln!("Selected package version: {}", row.version);
                    }
                }
                Action::Cancel => {
                    // Just close the menu (already handled)
                }
            }
        }
    }

    fn selected_package_row(&self) -> Option<PackageRow> {
        if self.current_tab != Tab::Packages {
            return None;
        }
        let filtered = self.formula_tab.filter(&self.search_query);
        filtered
            .get(
                self.formula_tab
                    .selected_index
                    .min(filtered.len().saturating_sub(1)),
            )
            .map(|row| (*row).clone())
    }
}

fn install_package_from_tui(row: &PackageRow) -> std::io::Result<()> {
    let item = match row.kind {
        PackageKind::Formula => InstallItemRequest {
            kind: ItemKind::Package,
            spec: row.name.parse::<ItemSpec>().map_err(io_error)?,
            tool: Default::default(),
        },
        PackageKind::Cask => InstallItemRequest {
            kind: ItemKind::App,
            spec: format!("{}@latest@homebrew-cask", row.name)
                .parse::<ItemSpec>()
                .map_err(io_error)?,
            tool: Default::default(),
        },
    };
    let request = InstallAndRecordRequest {
        start_dir: std::env::current_dir()?,
        home_dir: home_dir()?,
        global: false,
        force: false,
        install: InstallRequest { items: vec![item] },
    };
    let runtime = tokio::runtime::Runtime::new()?;
    runtime
        .block_on(engine::actions::install::run_and_record(request))
        .map(|_| ())
        .map_err(io_error)
}

fn uninstall_package_from_tui(row: &PackageRow) -> std::io::Result<()> {
    let target = match row.kind {
        PackageKind::Formula => UninstallTarget {
            kind: Some(ItemKind::Package),
            name: row.name.clone(),
            version: "latest".to_string(),
            backend: None,
            exact: false,
        },
        PackageKind::Cask => UninstallTarget {
            kind: Some(ItemKind::App),
            name: row.name.clone(),
            version: "latest".to_string(),
            backend: Some("homebrew-cask".parse().map_err(io_error)?),
            exact: false,
        },
    };
    let request = UninstallRequest {
        start_dir: std::env::current_dir()?,
        home_dir: home_dir()?,
        global: false,
        target,
    };
    let runtime = tokio::runtime::Runtime::new()?;
    runtime
        .block_on(engine::actions::uninstall::run(request))
        .map(|_| ())
        .map_err(io_error)
}

fn home_dir() -> std::io::Result<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "home directory not found")
        })
}

fn io_error(error: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::other(error.to_string())
}

/// Initializes terminal mode, runs the TUI application, and restores the terminal.
///
/// This function owns the terminal lifecycle: it enables raw mode through
/// `ratatui::init`, constructs `App`, runs the event loop, and restores the
/// terminal before returning. The result is `Ok(())` when the user exits normally
/// and an `io::Error` when terminal setup, drawing, or event handling fails.
pub fn launch_tui() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let app_result = App::new().run(&mut terminal);
    ratatui::restore();
    app_result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn create_key_event(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::empty())
    }

    #[test]
    fn tab_navigation() {
        let mut app = App::default();
        assert_eq!(app.current_tab, Tab::Packages);

        app.handle_key_event(create_key_event(KeyCode::Right));
        assert_eq!(app.current_tab, Tab::Tasks);

        app.handle_key_event(create_key_event(KeyCode::Left));
        assert_eq!(app.current_tab, Tab::Packages);

        app.handle_key_event(create_key_event(KeyCode::Char('2')));
        assert_eq!(app.current_tab, Tab::Tasks);

        app.handle_key_event(create_key_event(KeyCode::Char('3')));
        assert_eq!(app.current_tab, Tab::Config);
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
    }

    #[test]
    fn exit_functionality() {
        let mut app = App::default();
        assert!(!app.exit);

        app.handle_key_event(create_key_event(KeyCode::Char('q')));
        assert!(app.exit);
    }
}
