use crate::core::specs::brew::{CaskSpec, FormulaSpec};
use crate::util::fs::FsUtils;
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

/// State for the Formula/Packages tab
#[derive(Debug)]
pub struct FormulaTab {
    packages: Vec<PackageRow>,
    pub selected_index: usize,
    scroll_offset: usize,
    filters: PackageFilters,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageKind {
    Formula,
    Cask,
}

impl PackageKind {
    fn as_str(&self) -> &'static str {
        match self {
            PackageKind::Formula => "Formula",
            PackageKind::Cask => "Cask",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageKindFilter {
    All,
    Formula,
    Cask,
}

impl PackageKindFilter {
    fn as_str(&self) -> &'static str {
        match self {
            PackageKindFilter::All => "All Types",
            PackageKindFilter::Formula => "Formulas",
            PackageKindFilter::Cask => "Casks",
        }
    }

    fn next(&self) -> Self {
        match self {
            PackageKindFilter::All => PackageKindFilter::Formula,
            PackageKindFilter::Formula => PackageKindFilter::Cask,
            PackageKindFilter::Cask => PackageKindFilter::All,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallFilter {
    All,
    Installed,
    Available,
}

impl InstallFilter {
    fn as_str(&self) -> &'static str {
        match self {
            InstallFilter::All => "All",
            InstallFilter::Installed => "Installed",
            InstallFilter::Available => "Available",
        }
    }

    fn next(&self) -> Self {
        match self {
            InstallFilter::All => InstallFilter::Installed,
            InstallFilter::Installed => InstallFilter::Available,
            InstallFilter::Available => InstallFilter::All,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PackageFilters {
    kind: PackageKindFilter,
    install: InstallFilter,
}

impl Default for PackageFilters {
    fn default() -> Self {
        Self {
            kind: PackageKindFilter::All,
            install: InstallFilter::All,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PackageRow {
    pub kind: PackageKind,
    pub name: String,
    pub version: String,
    pub status: String,
    pub installed: bool,
}

impl Default for FormulaTab {
    fn default() -> Self {
        Self {
            packages: vec![],
            selected_index: 0,
            scroll_offset: 0,
            filters: PackageFilters::default(),
        }
    }
}

impl FormulaTab {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let packages = Self::load_packages_from_cache()?;
        Ok(Self {
            packages,
            selected_index: 0,
            scroll_offset: 0,
            filters: PackageFilters::default(),
        })
    }

    fn load_packages_from_cache() -> Result<Vec<PackageRow>, Box<dyn std::error::Error>> {
        let mut rows = Vec::new();
        rows.extend(Self::load_formulas_from_cache()?);
        rows.extend(Self::load_casks_from_cache()?);
        Ok(rows)
    }

    fn load_formulas_from_cache() -> Result<Vec<PackageRow>, Box<dyn std::error::Error>> {
        let cache_dir = FsUtils::still_cache_dir()?;
        let formula_path = cache_dir.join("formula.json");
        
        if !formula_path.exists() {
            return Ok(vec![]);
        }
        
        let json_content = fs::read_to_string(&formula_path)?;
        
        // Parse as a JSON array first, then parse each formula individually
        // This allows us to skip malformed formulas instead of failing entirely
        let json_array: serde_json::Value = serde_json::from_str(&json_content)
            .map_err(|e| format!("Failed to parse JSON array: {}", e))?;
        
        let array = json_array.as_array()
            .ok_or("Expected JSON array")?;
        
        let mut rows: Vec<PackageRow> = Vec::new();
        let mut skipped = 0;
        
        for (idx, formula_value) in array.iter().enumerate() {
            match serde_json::from_value::<FormulaSpec>(formula_value.clone()) {
                Ok(formula) => {
                    let installed = !formula.installed.is_empty();
                    let status = if installed { "Installed" } else { "Available" };
                    rows.push(PackageRow {
                        kind: PackageKind::Formula,
                        name: formula.name,
                        version: formula.versions.stable,
                        status: status.to_string(),
                        installed,
                    });
                }
                Err(e) => {
                    // Skip malformed formulas but log a warning for the first few
                    if skipped < 3 {
                        eprintln!("Warning: Skipping formula at index {}: {}", idx, e);
                    }
                    skipped += 1;
                }
            }
        }
        
        if skipped > 0 {
            eprintln!("Loaded {} formulas (skipped {} malformed entries)", rows.len(), skipped);
        }
        
        Ok(rows)
    }

    fn load_casks_from_cache() -> Result<Vec<PackageRow>, Box<dyn std::error::Error>> {
        let cache_dir = FsUtils::still_cache_dir()?;
        let cask_path = cache_dir.join("cask.json");

        if !cask_path.exists() {
            return Ok(vec![]);
        }

        let json_content = fs::read_to_string(&cask_path)?;

        let json_array: serde_json::Value = serde_json::from_str(&json_content)
            .map_err(|e| format!("Failed to parse JSON array: {}", e))?;

        let array = json_array.as_array()
            .ok_or("Expected JSON array")?;

        let mut rows: Vec<PackageRow> = Vec::new();
        let mut skipped = 0;

        for (idx, cask_value) in array.iter().enumerate() {
            match serde_json::from_value::<CaskSpec>(cask_value.clone()) {
                Ok(cask) => {
                    let installed = !cask.installed.is_empty();
                    let status = if installed { "Installed" } else { "Available" };
                    let version = if cask.version.is_empty() { "-" } else { cask.version.as_str() };
                    rows.push(PackageRow {
                        kind: PackageKind::Cask,
                        name: cask.token,
                        version: version.to_string(),
                        status: status.to_string(),
                        installed,
                    });
                }
                Err(e) => {
                    if skipped < 3 {
                        eprintln!("Warning: Skipping cask at index {}: {}", idx, e);
                    }
                    skipped += 1;
                }
            }
        }

        if skipped > 0 {
            eprintln!("Loaded {} casks (skipped {} malformed entries)", rows.len(), skipped);
        }

        Ok(rows)
    }

    fn matches_filters(&self, row: &PackageRow) -> bool {
        let kind_ok = match self.filters.kind {
            PackageKindFilter::All => true,
            PackageKindFilter::Formula => row.kind == PackageKind::Formula,
            PackageKindFilter::Cask => row.kind == PackageKind::Cask,
        };

        let install_ok = match self.filters.install {
            InstallFilter::All => true,
            InstallFilter::Installed => row.installed,
            InstallFilter::Available => !row.installed,
        };

        kind_ok && install_ok
    }

    pub(crate) fn filter(&self, query: &str) -> Vec<&PackageRow> {
        let candidates: Vec<&PackageRow> = self
            .packages
            .iter()
            .filter(|row| self.matches_filters(row))
            .collect();

        if query.is_empty() {
            return candidates;
        }

        // Use fzf-like fuzzy matching (SkimMatcherV2 uses the same algorithm as fzf)
        let matcher = SkimMatcherV2::default();
        let query_lower = query.to_lowercase();

        // Score and filter packages using fuzzy matching
        let mut scored: Vec<(i64, &PackageRow)> = candidates
            .into_iter()
            .filter_map(|row| {
                // Try matching against name first (highest priority)
                let name_score = matcher.fuzzy_match(&row.name.to_lowercase(), &query_lower);

                // Also try matching against full searchable text
                let searchable_text = format!(
                    "{} {} {} {}",
                    row.kind.as_str().to_lowercase(),
                    row.name.to_lowercase(),
                    row.version.to_lowercase(),
                    row.status.to_lowercase()
                );
                let text_score = matcher.fuzzy_match(&searchable_text, &query_lower);

                // Use the best score
                let best_score = name_score
                    .or(text_score)
                    .map(|s| s as i64)
                    .unwrap_or(0);

                if best_score > 0 {
                    Some((best_score, row))
                } else {
                    None
                }
            })
            .collect();

        // Sort by score (highest first) to get fzf-like ordering
        scored.sort_by(|a, b| b.0.cmp(&a.0));

        // Return just the rows, in order of relevance
        scored.into_iter().map(|(_, row)| row).collect()
    }

    pub fn render_table(&mut self, area: Rect, buf: &mut Buffer, search_query: &str) {
        // Calculate filtered count first
        let filtered_count = self.get_item_count(search_query);
        
        // Ensure selected_index is within bounds
        let max_index = if filtered_count == 0 {
            0
        } else {
            filtered_count - 1
        };
        let selected_idx = self.selected_index.min(max_index);

        // Calculate viewport: account for header (1 row) and borders (2 rows)
        let available_height = area.height.saturating_sub(2); // Subtract borders
        let viewport_height = (available_height.saturating_sub(1)) as usize; // Subtract header
        
        // Adjust scroll offset to keep selected row visible
        if selected_idx < self.scroll_offset {
            self.scroll_offset = selected_idx;
        } else if selected_idx >= self.scroll_offset + viewport_height {
            self.scroll_offset = selected_idx.saturating_sub(viewport_height.saturating_sub(1));
        }
        
        // Ensure scroll_offset doesn't go beyond available rows
        if self.scroll_offset > filtered_count.saturating_sub(viewport_height) {
            self.scroll_offset = filtered_count.saturating_sub(viewport_height);
        }
        if self.scroll_offset > filtered_count {
            self.scroll_offset = 0;
        }
        
        let scroll_offset = self.scroll_offset;

        // Now get filtered rows and visible rows
        let filtered_rows = self.filter(search_query);
        let visible_rows: Vec<&PackageRow> = filtered_rows
            .into_iter()
            .skip(scroll_offset)
            .take(viewport_height)
            .collect();

        // Create table rows with highlighting for selected row
        let table_rows: Vec<Row> = visible_rows
            .iter()
            .enumerate()
            .map(|(display_idx, row)| {
                let actual_idx = scroll_offset + display_idx;
                let is_selected = actual_idx == selected_idx;
                let row_style = if is_selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                Row::new(vec![
                    Cell::from(row.kind.as_str()).style(if is_selected {
                        Style::default()
                            .fg(Color::Black)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    }),
                    Cell::from(row.name.as_str()).style(if is_selected {
                        Style::default()
                            .fg(Color::Black)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    }),
                    Cell::from(row.version.as_str()).style(if is_selected {
                        Style::default()
                            .fg(Color::Black)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    }),
                    Cell::from(row.status.as_str()).style(if is_selected {
                        Style::default()
                            .fg(Color::Black)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    }),
                ])
                .style(row_style)
                .height(1)
            })
            .collect();

        let headers = vec!["Type", "Name", "Version", "Status"];
        let header = Row::new(
            headers
                .iter()
                .map(|h| Cell::from(*h).style(Style::default().add_modifier(Modifier::BOLD)))
                .collect::<Vec<_>>(),
        )
        .style(Style::default().fg(Color::White).bg(Color::Blue))
        .height(1);

        let widths = vec![
            Constraint::Percentage(12),
            Constraint::Percentage(38),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ];

        let title = format!(
            " Packages - {} items [{} | {}] ",
            filtered_count,
            self.filters.kind.as_str(),
            self.filters.install.as_str()
        );

        let table = Table::new(table_rows, widths)
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Blue))
                    .title(title),
            )
            .column_spacing(2);

        table.render(area, buf);
    }

    pub fn render_preview(&self, area: Rect, buf: &mut Buffer, search_query: &str) {
        let filtered_rows = self.filter(search_query);
        let filtered_count = filtered_rows.len();
        
        let max_index = if filtered_count == 0 {
            0
        } else {
            filtered_count - 1
        };
        let selected_idx = self.selected_index.min(max_index);

        let preview_content = if filtered_rows.is_empty() {
            Text::from(vec![
                Line::from("No formulas to display".fg(Color::DarkGray)),
            ])
        } else {
            let selected_row = filtered_rows[selected_idx];
            let lines = vec![
                Line::from(vec![
                    Span::styled("Selected Formula ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    Span::styled(
                        format!("({}/{})", selected_idx + 1, filtered_count),
                        Style::default().fg(Color::Gray),
                    ),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Name: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::styled(selected_row.name.clone(), Style::default().fg(Color::White)),
                ]),
                Line::from(vec![
                    Span::styled("Type: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::styled(selected_row.kind.as_str(), Style::default().fg(Color::White)),
                ]),
                Line::from(vec![
                    Span::styled("Version: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::styled(selected_row.version.clone(), Style::default().fg(Color::White)),
                ]),
                Line::from(vec![
                    Span::styled("Status: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::styled(selected_row.status.clone(), Style::default().fg(Color::White)),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Filters: ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        format!("{} | {}", self.filters.kind.as_str(), self.filters.install.as_str()),
                        Style::default().fg(Color::White),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("Navigation: ", Style::default().fg(Color::Gray)),
                ]),
                Line::from(vec![
                    Span::styled("  ↑/↓ or j/k ", Style::default().fg(Color::DarkGray)),
                    Span::styled("Move up/down", Style::default().fg(Color::White)),
                ]),
                Line::from(vec![
                    Span::styled("  t ", Style::default().fg(Color::DarkGray)),
                    Span::styled("Toggle type filter", Style::default().fg(Color::White)),
                ]),
                Line::from(vec![
                    Span::styled("  i ", Style::default().fg(Color::DarkGray)),
                    Span::styled("Toggle installed filter", Style::default().fg(Color::White)),
                ]),
                Line::from(vec![
                    Span::styled("  r ", Style::default().fg(Color::DarkGray)),
                    Span::styled("Reset filters", Style::default().fg(Color::White)),
                ]),
                Line::from(vec![
                    Span::styled("  / ", Style::default().fg(Color::DarkGray)),
                    Span::styled("Search", Style::default().fg(Color::White)),
                ]),
                Line::from(vec![
                    Span::styled("  q ", Style::default().fg(Color::DarkGray)),
                    Span::styled("Quit", Style::default().fg(Color::White)),
                ]),
            ];

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

    pub fn handle_navigation(&mut self, direction: NavigationDirection, search_query: &str) {
        let filtered_rows = self.filter(search_query);
        let filtered_count = filtered_rows.len();
        
        match direction {
            NavigationDirection::Up => {
                if filtered_count > 0 {
                    if self.selected_index > 0 {
                        self.selected_index -= 1;
                    } else {
                        self.selected_index = filtered_count - 1;
                    }
                }
            }
            NavigationDirection::Down => {
                if filtered_count > 0 {
                    self.selected_index = (self.selected_index + 1) % filtered_count;
                }
            }
        }
    }

    pub fn reset_selection(&mut self) {
        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    pub fn cycle_kind_filter(&mut self) {
        self.filters.kind = self.filters.kind.next();
        self.reset_selection();
    }

    pub fn cycle_install_filter(&mut self) {
        self.filters.install = self.filters.install.next();
        self.reset_selection();
    }

    pub fn reset_filters(&mut self) {
        self.filters = PackageFilters::default();
        self.reset_selection();
    }

    pub fn get_item_count(&self, search_query: &str) -> usize {
        self.filter(search_query).len()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum NavigationDirection {
    Up,
    Down,
}

