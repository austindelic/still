//! Package browser backed by cached Still registry metadata.

use engine::infra::paths::PathOps;
use engine::platform::System;
use engine::resolve::registries::specs::brew::{CaskSpec, FormulaSpec};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Widget},
};
use std::fs;

/// State for the TUI package browser.
#[derive(Debug, Default)]
pub struct PackageBrowser {
    packages: Vec<PackageRow>,
    selected_index: usize,
    scroll_offset: usize,
}

/// Item kind rendered by the package browser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageKind {
    Package,
    App,
}

impl PackageKind {
    fn as_str(&self) -> &'static str {
        match self {
            PackageKind::Package => "Package",
            PackageKind::App => "App",
        }
    }
}

/// Renderable package row built from cached registry metadata.
#[derive(Debug, Clone)]
pub struct PackageRow {
    pub kind: PackageKind,
    pub name: String,
    pub version: String,
    pub state: String,
}

impl PackageBrowser {
    #[cfg(test)]
    pub(crate) fn with_rows(packages: Vec<PackageRow>) -> Self {
        Self {
            packages,
            selected_index: 0,
            scroll_offset: 0,
        }
    }

    /// Loads packages and apps from Still's local cache.
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            packages: Self::load_packages_from_cache()?,
            selected_index: 0,
            scroll_offset: 0,
        })
    }

    fn load_packages_from_cache() -> Result<Vec<PackageRow>, Box<dyn std::error::Error>> {
        let mut rows = Vec::new();
        rows.extend(Self::load_formulas_from_cache()?);
        rows.extend(Self::load_casks_from_cache()?);
        Ok(rows)
    }

    fn load_formulas_from_cache() -> Result<Vec<PackageRow>, Box<dyn std::error::Error>> {
        let formula_path = System::cache_dir().join("still").join("formula.json");
        if !formula_path.exists() {
            return Ok(vec![]);
        }

        let json_content = fs::read_to_string(&formula_path)?;
        let json_array: serde_json::Value = serde_json::from_str(&json_content)
            .map_err(|err| format!("failed to parse formula cache: {err}"))?;
        let array = json_array
            .as_array()
            .ok_or("formula cache is not an array")?;

        let mut rows = Vec::new();
        for formula_value in array {
            let Ok(formula) = serde_json::from_value::<FormulaSpec>(formula_value.clone()) else {
                continue;
            };
            let installed = !formula.installed.is_empty();
            rows.push(PackageRow {
                kind: PackageKind::Package,
                name: formula.name,
                version: formula.versions.stable,
                state: state_label(installed).to_string(),
            });
        }

        Ok(rows)
    }

    fn load_casks_from_cache() -> Result<Vec<PackageRow>, Box<dyn std::error::Error>> {
        let cask_path = System::cache_dir().join("still").join("cask.json");
        if !cask_path.exists() {
            return Ok(vec![]);
        }

        let json_content = fs::read_to_string(&cask_path)?;
        let json_array: serde_json::Value = serde_json::from_str(&json_content)
            .map_err(|err| format!("failed to parse app cache: {err}"))?;
        let array = json_array.as_array().ok_or("app cache is not an array")?;

        let mut rows = Vec::new();
        for cask_value in array {
            let Ok(cask) = serde_json::from_value::<CaskSpec>(cask_value.clone()) else {
                continue;
            };
            let installed = !cask.installed.is_empty();
            rows.push(PackageRow {
                kind: PackageKind::App,
                name: cask.token,
                version: if cask.version.is_empty() {
                    "-".to_string()
                } else {
                    cask.version
                },
                state: state_label(installed).to_string(),
            });
        }

        Ok(rows)
    }

    fn filtered_rows(&self, query: &str) -> Vec<&PackageRow> {
        if query.is_empty() {
            return self.packages.iter().collect();
        }

        let matcher = SkimMatcherV2::default();
        let query_lower = query.to_lowercase();
        let mut scored: Vec<(i64, &PackageRow)> = self
            .packages
            .iter()
            .filter_map(|row| {
                let name_score = matcher.fuzzy_match(&row.name.to_lowercase(), &query_lower);
                let searchable_text = format!(
                    "{} {} {} {}",
                    row.kind.as_str().to_lowercase(),
                    row.name.to_lowercase(),
                    row.version.to_lowercase(),
                    row.state.to_lowercase()
                );
                let text_score = matcher.fuzzy_match(&searchable_text, &query_lower);
                name_score.or(text_score).map(|score| (score as i64, row))
            })
            .collect();

        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored.into_iter().map(|(_, row)| row).collect()
    }

    /// Renders the item table for the current search.
    pub fn render_table(&mut self, area: Rect, buf: &mut Buffer, search_query: &str) {
        let filtered_count = self.item_count(search_query);
        let selected_idx = self.selected_index.min(filtered_count.saturating_sub(1));
        let viewport_height = area.height.saturating_sub(3) as usize;

        if selected_idx < self.scroll_offset {
            self.scroll_offset = selected_idx;
        } else if selected_idx >= self.scroll_offset + viewport_height {
            self.scroll_offset = selected_idx.saturating_sub(viewport_height.saturating_sub(1));
        }
        self.scroll_offset = self
            .scroll_offset
            .min(filtered_count.saturating_sub(viewport_height));

        let visible_rows: Vec<&PackageRow> = self
            .filtered_rows(search_query)
            .into_iter()
            .skip(self.scroll_offset)
            .take(viewport_height)
            .collect();

        let table_rows = visible_rows
            .iter()
            .enumerate()
            .map(|(display_idx, row)| {
                let is_selected = self.scroll_offset + display_idx == selected_idx;
                let style = if is_selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                Row::new(vec![
                    Cell::from(row.kind.as_str()),
                    Cell::from(row.name.as_str()),
                    Cell::from(row.version.as_str()),
                    Cell::from(row.state.as_str()),
                ])
                .style(style)
            })
            .collect::<Vec<_>>();

        let header = Row::new(["Kind", "Name", "Version", "State"])
            .style(Style::default().fg(Color::White).bg(Color::Blue).bold());

        let table = Table::new(
            table_rows,
            [
                Constraint::Percentage(14),
                Constraint::Percentage(40),
                Constraint::Percentage(26),
                Constraint::Percentage(20),
            ],
        )
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue))
                .title(format!(" Items - {filtered_count} ")),
        )
        .column_spacing(2);

        table.render(area, buf);
    }

    /// Renders details for the selected item.
    pub fn render_details(&self, area: Rect, buf: &mut Buffer, search_query: &str) {
        let rows = self.filtered_rows(search_query);
        let content = if rows.is_empty() {
            let empty = if search_query.is_empty() {
                "No cached items found"
            } else {
                "No matching items"
            };
            Text::from(vec![Line::from(empty.dark_gray())])
        } else {
            let selected_idx = self.selected_index.min(rows.len().saturating_sub(1));
            let selected = rows[selected_idx];
            Text::from(vec![
                Line::from(vec![
                    Span::styled(
                        selected.name.clone(),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!(" ({}/{})", selected_idx + 1, rows.len()),
                        Style::default().fg(Color::Gray),
                    ),
                ]),
                Line::from(""),
                label_value("Kind", selected.kind.as_str()),
                label_value("Version", &selected.version),
                label_value("State", &selected.state),
                Line::from(""),
                Line::from("q Quit  / Search  Enter Actions  Esc Clear".dark_gray()),
            ])
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" Details ");

        Paragraph::new(content)
            .block(block)
            .wrap(ratatui::widgets::Wrap { trim: true })
            .render(area, buf);
    }

    /// Returns the selected item for the current search.
    pub fn selected_row(&self, search_query: &str) -> Option<PackageRow> {
        let rows = self.filtered_rows(search_query);
        rows.get(self.selected_index.min(rows.len().saturating_sub(1)))
            .map(|row| (*row).clone())
    }

    /// Moves the selected row within the current search results.
    pub fn handle_navigation(&mut self, direction: NavigationDirection, search_query: &str) {
        let item_count = self.item_count(search_query);
        if item_count == 0 {
            return;
        }

        match direction {
            NavigationDirection::Up => {
                self.selected_index = self.selected_index.checked_sub(1).unwrap_or(item_count - 1);
            }
            NavigationDirection::Down => {
                self.selected_index = (self.selected_index + 1) % item_count;
            }
        }
    }

    /// Resets the selected row and scroll offset.
    pub fn reset_selection(&mut self) {
        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    fn item_count(&self, search_query: &str) -> usize {
        self.filtered_rows(search_query).len()
    }
}

fn state_label(installed: bool) -> &'static str {
    if installed { "Installed" } else { "Available" }
}

fn label_value(label: &'static str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label}: "), Style::default().fg(Color::Yellow)),
        Span::styled(value.to_string(), Style::default().fg(Color::White)),
    ])
}

/// Direction for row navigation.
#[derive(Debug, Clone, Copy)]
pub enum NavigationDirection {
    Up,
    Down,
}
