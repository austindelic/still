use ratatui::{
    buffer::Buffer,
    layout::Rect,
};
use crate::frontend::tui::components::interactive_cli::InteractiveCli;

/// Resources tab that displays btop output using the interactive CLI component
#[derive(Debug)]
pub struct ResourcesTab {
    cli: InteractiveCli,
}

impl Default for ResourcesTab {
    fn default() -> Self {
        Self {
            cli: InteractiveCli::new("btop"),
        }
    }
}

impl ResourcesTab {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer) {
        self.cli.render(area, buf);
    }

    /// Toggle interactive mode
    pub fn toggle_interactive(&mut self) {
        self.cli.toggle_interactive();
    }

    /// Check if in interactive mode
    pub fn is_interactive(&self) -> bool {
        self.cli.is_interactive()
    }

    /// Send input to the CLI tool
    pub fn send_input(&mut self, input: &str) {
        self.cli.send_input(input);
    }

    /// Send a key to the CLI tool
    pub fn send_key(&mut self, key: u8) {
        self.cli.send_key(key);
    }
}

