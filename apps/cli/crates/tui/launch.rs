use std::io;

use crate::app::App;

/// Initializes terminal mode, runs the TUI application, and restores the terminal.
pub fn launch_tui() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let app_result = App::new().run(&mut terminal);
    ratatui::restore();
    app_result
}
