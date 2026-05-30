//! Resource monitor tab backed by an embedded interactive CLI.

use crate::components::interactive_cli::InteractiveCli;
use ratatui::{buffer::Buffer, layout::Rect};

/// Resources tab that displays `btop` output using the interactive CLI component.
///
/// The tab is a thin owner around `InteractiveCli`: it fixes the executable to
/// `btop`, forwards render/input calls, and keeps process state scoped to the
/// resources view.
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
    /// Creates a resources tab using the default resource monitor command.
    ///
    /// Construction does not start `btop`; the embedded component begins in view
    /// mode and only spawns the process when interactive mode is toggled on.
    pub fn new() -> Self {
        Self::default()
    }

    /// Renders the embedded resource monitor.
    ///
    /// `area` is provided by the parent TUI layout and `buf` receives the panel.
    /// Rendering delegates to `InteractiveCli` and may update placeholder output.
    pub fn render(&mut self, area: Rect, buf: &mut Buffer) {
        self.cli.render(area, buf);
    }

    /// Toggles interactive mode for the embedded `btop` process.
    ///
    /// Entering interactive mode attempts to spawn `btop`; leaving it kills the
    /// child process if one is running. Errors are shown in the panel output.
    pub fn toggle_interactive(&mut self) {
        self.cli.toggle_interactive();
    }

    /// Returns whether the embedded CLI is in interactive mode.
    pub fn is_interactive(&self) -> bool {
        self.cli.is_interactive()
    }

    /// Sends text input to the embedded CLI tool.
    ///
    /// `input` is forwarded verbatim to `btop` stdin only when interactive mode
    /// is active and a child stdin handle is available.
    pub fn send_input(&mut self, input: &str) {
        self.cli.send_input(input);
    }

    /// Sends one key byte to the embedded CLI tool.
    ///
    /// `key` is written as a raw byte and is intended for simple keyboard events.
    pub fn send_key(&mut self, key: u8) {
        self.cli.send_key(key);
    }
}
