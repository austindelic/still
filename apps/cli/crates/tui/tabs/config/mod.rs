//! Configuration tab placeholder.

#![allow(dead_code)]

/// TUI tab for viewing or editing Still configuration.
///
/// This placeholder owns no state yet; future config UI state should live here
/// rather than in the top-level `App`.
pub struct ConfigTab;

impl Default for ConfigTab {
    fn default() -> Self {
        Self
    }
}

impl ConfigTab {
    /// Renders the configuration tab.
    ///
    /// `_area` is the parent layout region and `_buf` is the ratatui frame buffer.
    /// They are intentionally unused until the tab has real content.
    pub fn render(&self, _area: ratatui::layout::Rect, _buf: &mut ratatui::buffer::Buffer) {
        // TODO: Implement config tab
    }
}
