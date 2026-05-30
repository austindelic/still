//! Logs tab placeholder.

#![allow(dead_code)]

/// TUI tab for displaying Still logs.
///
/// This placeholder owns no state yet; future log filters, scroll position, and
/// loaded log lines should live here.
pub struct LogsTab;

impl Default for LogsTab {
    fn default() -> Self {
        Self
    }
}

impl LogsTab {
    /// Renders the logs tab.
    ///
    /// `_area` is the parent layout region and `_buf` is the ratatui frame buffer.
    /// They are intentionally unused until the tab has real content.
    pub fn render(&self, _area: ratatui::layout::Rect, _buf: &mut ratatui::buffer::Buffer) {
        // TODO: Implement logs tab
    }
}
