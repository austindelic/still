//! Tasks tab placeholder.

#![allow(dead_code)]

/// TUI tab for listing and running Still tasks.
///
/// This placeholder owns no state yet; future task definitions, selection, and
/// execution status should live here.
pub struct TasksTab;

impl Default for TasksTab {
    fn default() -> Self {
        Self
    }
}

impl TasksTab {
    /// Renders the tasks tab.
    ///
    /// `_area` is the parent layout region and `_buf` is the ratatui frame buffer.
    /// They are intentionally unused until the tab has real content.
    pub fn render(&self, _area: ratatui::layout::Rect, _buf: &mut ratatui::buffer::Buffer) {
        // TODO: Implement tasks tab
    }
}
