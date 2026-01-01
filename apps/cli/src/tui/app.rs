use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    DefaultTerminal, Frame,
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Widget},
};
use std::io;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Packages,
    Tasks,
    Config,
    Logs,
}

impl Tab {
    pub fn as_str(&self) -> &'static str {
        match self {
            Tab::Packages => "Packages",
            Tab::Tasks => "Tasks",
            Tab::Config => "Config",
            Tab::Logs => "Logs",
        }
    }

    pub fn all() -> &'static [Tab] {
        &[Tab::Packages, Tab::Tasks, Tab::Config, Tab::Logs]
    }

    pub fn next(&self) -> Tab {
        let tabs = Tab::all();
        let current_idx = tabs.iter().position(|t| t == self).unwrap_or(0);
        let next_idx = (current_idx + 1) % tabs.len();
        tabs[next_idx]
    }

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

#[derive(Debug)]
pub struct App {
    current_tab: Tab,
    search_query: String,
    exit: bool,
    search_focused: bool,
    selected_row_index: usize,
    // Sample data for spreadsheet
    rows: Vec<Vec<String>>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            current_tab: Tab::default(),
            search_query: String::new(),
            exit: false,
            search_focused: false,
            selected_row_index: 0,
            rows: vec![
                vec!["Package 1".to_string(), "v1.0.0".to_string(), "Installed".to_string()],
                vec!["Package 2".to_string(), "v2.1.0".to_string(), "Available".to_string()],
                vec!["Package 3".to_string(), "v1.5.0".to_string(), "Installed".to_string()],
                vec!["Package 4".to_string(), "v3.0.0".to_string(), "Available".to_string()],
            ],
        }
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Split the area into: tabs, search bar, and content
        let vertical = Layout::vertical([
            Constraint::Length(3), // Tabs
            Constraint::Length(3), // Search bar
            Constraint::Min(0),    // Spreadsheet content
        ])
        .split(area);

        // Render tabs
        self.render_tabs(vertical[0], buf);

        // Render search bar
        self.render_search_bar(vertical[1], buf);

        // Render spreadsheet and preview
        self.render_spreadsheet(vertical[2], buf);
    }
}

impl App {
    fn render_tabs(&self, area: Rect, buf: &mut Buffer) {
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

                let mut spans = vec![
                    Span::styled(
                        format!(" {} ", tab.as_str()),
                        style,
                    ),
                ];

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

    fn render_search_bar(&self, area: Rect, buf: &mut Buffer) {
        let search_text = if self.search_query.is_empty() {
            Text::from(vec![
                Line::from(vec![
                    Span::styled("🔍 ", Style::default().fg(Color::Yellow)),
                    Span::styled("Search...", Style::default().fg(Color::DarkGray)),
                ])
            ])
        } else {
            Text::from(vec![
                Line::from(vec![
                    Span::styled("🔍 ", Style::default().fg(Color::Yellow)),
                    Span::styled(self.search_query.clone(), Style::default().fg(Color::White)),
                ])
            ])
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(if self.search_focused {
                " Search (typing...) "
            } else {
                " Search (press '/' to focus) "
            })
            .border_style(if self.search_focused {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Blue)
            });

        Paragraph::new(search_text)
            .block(block)
            .render(area, buf);
    }

    fn render_spreadsheet(&self, area: Rect, buf: &mut Buffer) {
        // Split into table and preview
        let horizontal = Layout::horizontal([
            Constraint::Percentage(60), // Table
            Constraint::Percentage(40), // Preview
        ])
        .split(area);

        self.render_table(horizontal[0], buf);
        self.render_preview(horizontal[1], buf);
    }

    fn render_table(&self, area: Rect, buf: &mut Buffer) {
        // Filter rows based on search query
        let filtered_rows: Vec<&Vec<String>> = if self.search_query.is_empty() {
            self.rows.iter().collect()
        } else {
            self.rows
                .iter()
                .filter(|row| {
                    row.iter()
                        .any(|cell| cell.to_lowercase().contains(&self.search_query.to_lowercase()))
                })
                .collect()
        };

        // Ensure selected_row_index is within bounds
        let max_index = if filtered_rows.is_empty() {
            0
        } else {
            filtered_rows.len() - 1
        };
        let selected_idx = self.selected_row_index.min(max_index);

        // Create table rows with highlighting for selected row
        let table_rows: Vec<Row> = filtered_rows
            .iter()
            .enumerate()
            .map(|(idx, row)| {
                let is_selected = idx == selected_idx;
                let row_style = if is_selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else if idx % 2 == 0 {
                    Style::default().bg(Color::DarkGray)
                } else {
                    Style::default()
                };

                Row::new(
                    row.iter()
                        .map(|cell| {
                            Cell::from(cell.as_str()).style(if is_selected {
                                Style::default()
                                    .fg(Color::Black)
                                    .add_modifier(Modifier::BOLD)
                            } else {
                                Style::default()
                            })
                        })
                        .collect::<Vec<_>>(),
                )
                .style(row_style)
                .height(1)
            })
            .collect();

        // Define table columns based on current tab
        let headers = match self.current_tab {
            Tab::Packages => vec!["Name", "Version", "Status"],
            Tab::Tasks => vec!["Task", "Status", "Last Run"],
            Tab::Config => vec!["Key", "Value", "Type"],
            Tab::Logs => vec!["Time", "Level", "Message"],
        };

        let header = Row::new(
            headers
                .iter()
                .map(|h| Cell::from(*h).style(Style::default().add_modifier(Modifier::BOLD)))
                .collect::<Vec<_>>(),
        )
        .style(Style::default().fg(Color::White).bg(Color::Blue))
        .height(1);

        let widths = match self.current_tab {
            Tab::Packages => vec![Constraint::Percentage(40), Constraint::Percentage(30), Constraint::Percentage(30)],
            Tab::Tasks => vec![Constraint::Percentage(40), Constraint::Percentage(30), Constraint::Percentage(30)],
            Tab::Config => vec![Constraint::Percentage(30), Constraint::Percentage(50), Constraint::Percentage(20)],
            Tab::Logs => vec![Constraint::Percentage(20), Constraint::Percentage(20), Constraint::Percentage(60)],
        };

        let table = Table::new(table_rows, widths)
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Blue))
                    .title(format!(" {} - {} items ", self.current_tab.as_str(), filtered_rows.len())),
            )
            .column_spacing(2);

        table.render(area, buf);
    }

    fn render_preview(&self, area: Rect, buf: &mut Buffer) {
        // Filter rows based on search query
        let filtered_rows: Vec<&Vec<String>> = if self.search_query.is_empty() {
            self.rows.iter().collect()
        } else {
            self.rows
                .iter()
                .filter(|row| {
                    row.iter()
                        .any(|cell| cell.to_lowercase().contains(&self.search_query.to_lowercase()))
                })
                .collect()
        };

        // Get the selected row
        let max_index = if filtered_rows.is_empty() {
            0
        } else {
            filtered_rows.len() - 1
        };
        let selected_idx = self.selected_row_index.min(max_index);

        let preview_content = if filtered_rows.is_empty() {
            Text::from(vec![
                Line::from("No items to display".fg(Color::DarkGray)),
            ])
        } else {
            let selected_row = filtered_rows[selected_idx];
            let headers = match self.current_tab {
                Tab::Packages => vec!["Name", "Version", "Status"],
                Tab::Tasks => vec!["Task", "Status", "Last Run"],
                Tab::Config => vec!["Key", "Value", "Type"],
                Tab::Logs => vec!["Time", "Level", "Message"],
            };

            let mut lines = vec![
                Line::from(vec![
                    Span::styled("Selected Item ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    Span::styled(
                        format!("({}/{})", selected_idx + 1, filtered_rows.len()),
                        Style::default().fg(Color::Gray),
                    ),
                ]),
                Line::from(""),
            ];

            for (idx, header) in headers.iter().enumerate() {
                if idx < selected_row.len() {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("{}: ", header),
                            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            selected_row[idx].clone(),
                            Style::default().fg(Color::White),
                        ),
                    ]));
                }
            }

            // Add some additional details based on tab
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("Navigation: ", Style::default().fg(Color::Gray)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("  ↑/↓ or j/k ", Style::default().fg(Color::DarkGray)),
                Span::styled("Move up/down", Style::default().fg(Color::White)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("  / ", Style::default().fg(Color::DarkGray)),
                Span::styled("Search", Style::default().fg(Color::White)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("  q ", Style::default().fg(Color::DarkGray)),
                Span::styled("Quit", Style::default().fg(Color::White)),
            ]));

            Text::from(lines)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" Preview ");

        Paragraph::new(preview_content)
            .block(block)
            .wrap(ratatui::widgets::Wrap { trim: true })
            .render(area, buf);
    }

    /// runs the application's main loop until the user quits
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    fn handle_events(&mut self) -> io::Result<()> {
        match event::read()? {
            // it's important to check that the event is a key press event as
            // crossterm also emits key release and repeat events on Windows.
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                self.handle_key_event(key_event)
            }
            _ => {}
        };
        Ok(())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        // Get filtered rows count for bounds checking
        let filtered_count = if self.search_query.is_empty() {
            self.rows.len()
        } else {
            self.rows
                .iter()
                .filter(|row| {
                    row.iter()
                        .any(|cell| cell.to_lowercase().contains(&self.search_query.to_lowercase()))
                })
                .count()
        };

        match key_event.code {
            KeyCode::Char('q') => self.exit(),
            // Focus search bar
            KeyCode::Char('/') => {
                self.search_focused = true;
            }
            // Row navigation (only when search is not focused)
            KeyCode::Up | KeyCode::Char('k') if !self.search_focused => {
                if filtered_count > 0 {
                    if self.selected_row_index > 0 {
                        self.selected_row_index -= 1;
                    } else {
                        self.selected_row_index = filtered_count - 1;
                    }
                }
            }
            KeyCode::Down | KeyCode::Char('j') if !self.search_focused => {
                if filtered_count > 0 {
                    self.selected_row_index = (self.selected_row_index + 1) % filtered_count;
                }
            }
            // Tab navigation (only when search is not focused)
            KeyCode::Tab if !self.search_focused => {
                self.current_tab = self.current_tab.next();
                self.selected_row_index = 0; // Reset selection when switching tabs
            }
            KeyCode::BackTab if !self.search_focused => {
                self.current_tab = self.current_tab.previous();
                self.selected_row_index = 0;
            }
            KeyCode::Left if !self.search_focused => {
                self.current_tab = self.current_tab.previous();
                self.selected_row_index = 0;
            }
            KeyCode::Right if !self.search_focused => {
                self.current_tab = self.current_tab.next();
                self.selected_row_index = 0;
            }
            // Number keys for direct tab selection (only when search is not focused)
            KeyCode::Char('1') if !self.search_focused => {
                self.current_tab = Tab::Packages;
                self.selected_row_index = 0;
            }
            KeyCode::Char('2') if !self.search_focused => {
                self.current_tab = Tab::Tasks;
                self.selected_row_index = 0;
            }
            KeyCode::Char('3') if !self.search_focused => {
                self.current_tab = Tab::Config;
                self.selected_row_index = 0;
            }
            KeyCode::Char('4') if !self.search_focused => {
                self.current_tab = Tab::Logs;
                self.selected_row_index = 0;
            }
            // Search bar input (when focused)
            KeyCode::Char(c) if self.search_focused => {
                self.search_query.push(c);
                self.selected_row_index = 0; // Reset selection when searching
            }
            KeyCode::Backspace if self.search_focused => {
                self.search_query.pop();
                self.selected_row_index = 0; // Reset selection when searching
            }
            KeyCode::Esc => {
                self.search_query.clear();
                self.search_focused = false;
            }
            KeyCode::Enter if self.search_focused => {
                self.search_focused = false;
            }
            _ => {}
        }
    }

    fn exit(&mut self) {
        self.exit = true;
    }
}
pub fn launch_tui() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let app_result = App::default().run(&mut terminal);
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
