//! Component for embedding a child CLI process inside a TUI panel.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Text},
    widgets::{Block, Borders, Paragraph, Widget},
};
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

/// Interaction mode for the embedded CLI component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractiveMode {
    View,
    Interactive,
}

/// Generic interactive CLI component that can embed a child CLI tool.
///
/// The component owns process state, captured output, and whether keyboard input
/// is currently being forwarded. It is intentionally process-oriented and does
/// not parse Still commands; callers choose the executable name when constructing
/// the component.
#[derive(Debug)]
pub struct InteractiveCli {
    tool_name: String,
    mode: InteractiveMode,
    output: Arc<Mutex<Vec<String>>>,
    process: Option<std::process::Child>,
    last_update: std::time::Instant,
}

impl InteractiveCli {
    /// Creates a new interactive CLI component for a tool name.
    ///
    /// `tool_name` is the executable to find on PATH when interactive mode starts
    /// and the label rendered in passive view mode. Construction does not spawn
    /// the process; the component starts in `InteractiveMode::View`.
    pub fn new(tool_name: impl Into<String>) -> Self {
        let tool_name = tool_name.into();
        Self {
            tool_name: tool_name.clone(),
            mode: InteractiveMode::View,
            output: Arc::new(Mutex::new(Vec::new())),
            process: None,
            last_update: std::time::Instant::now(),
        }
    }

    /// Toggles between passive view mode and interactive process mode.
    ///
    /// When entering interactive mode, the component attempts to locate and spawn
    /// the configured tool. When leaving, it kills the child process if one is
    /// running. Errors are shown inside the component output rather than returned.
    pub fn toggle_interactive(&mut self) {
        match self.mode {
            InteractiveMode::View => {
                self.enter_interactive_mode();
            }
            InteractiveMode::Interactive => {
                self.exit_interactive_mode();
            }
        }
    }

    /// Enter interactive mode - spawn the CLI tool
    fn enter_interactive_mode(&mut self) {
        if self.process.is_some() {
            return; // Already in interactive mode
        }

        // Find the tool
        if let Some(tool_path) = Self::find_tool(&self.tool_name) {
            // Spawn the process
            match Command::new(&tool_path)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
            {
                Ok(child) => {
                    self.process = Some(child);
                    self.mode = InteractiveMode::Interactive;

                    // Start a thread to read output
                    let output = Arc::clone(&self.output);
                    let tool_name = self.tool_name.clone();

                    thread::spawn(move || {
                        Self::read_process_output(output, tool_name);
                    });
                }
                Err(e) => {
                    // Failed to spawn, show error
                    let mut output = self.output.lock().unwrap();
                    *output = vec![
                        format!("Failed to start {}: {}", self.tool_name, e),
                        "".to_string(),
                        "Make sure the tool is installed and available in PATH.".to_string(),
                    ];
                }
            }
        } else {
            let mut output = self.output.lock().unwrap();
            *output = vec![
                format!("{} not found in PATH", self.tool_name),
                "".to_string(),
                format!("Press 'i' to try starting {} interactively", self.tool_name),
            ];
        }
    }

    /// Exit interactive mode - kill the process
    fn exit_interactive_mode(&mut self) {
        if let Some(mut child) = self.process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.mode = InteractiveMode::View;
    }

    /// Sends string input to the interactive process.
    ///
    /// `input` is written verbatim to the child process stdin when interactive
    /// mode is active and the child exposes a writable stdin handle. The method is
    /// a no-op in view mode or after the process exits.
    pub fn send_input(&mut self, input: &str) {
        if let Some(ref mut child) = self.process {
            if let Some(ref mut stdin) = child.stdin {
                let _ = stdin.write_all(input.as_bytes());
                let _ = stdin.flush();
            }
        }
    }

    /// Sends one key byte to the interactive process.
    ///
    /// `key` is forwarded as a single raw byte. This is for simple key events and
    /// should be replaced with PTY-aware terminal input before supporting complex
    /// interactive applications.
    pub fn send_key(&mut self, key: u8) {
        if let Some(ref mut child) = self.process {
            if let Some(ref mut stdin) = child.stdin {
                let _ = stdin.write_all(&[key]);
                let _ = stdin.flush();
            }
        }
    }

    /// Renders the embedded CLI panel.
    ///
    /// `area` is the region allocated by the parent tab and `buf` is the ratatui
    /// frame buffer to draw into. Rendering may update cached output in view
    /// mode, but it does not spawn or stop the child process.
    pub fn render(&mut self, area: Rect, buf: &mut Buffer) {
        // Update output periodically in view mode
        if self.mode == InteractiveMode::View && self.last_update.elapsed().as_millis() > 500 {
            self.update_view_output();
            self.last_update = std::time::Instant::now();
        }

        let mode_indicator = match self.mode {
            InteractiveMode::View => format!(" {} (Press 'i' to interact) ", self.tool_name),
            InteractiveMode::Interactive => {
                format!(" {} (Interactive - Press 'i' to exit) ", self.tool_name)
            }
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(if self.mode == InteractiveMode::Interactive {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(ratatui::style::Modifier::BOLD)
            } else {
                Style::default().fg(Color::Cyan)
            })
            .title(mode_indicator);

        let output = self.output.lock().unwrap();
        let content = if output.is_empty() {
            Text::from(vec![
                Line::from(format!("Loading {}...", self.tool_name).fg(Color::DarkGray)),
                Line::from(""),
                Line::from(
                    format!("Press 'i' to interact with {}", self.tool_name).fg(Color::DarkGray),
                ),
            ])
        } else {
            // Limit output to fit in the area
            let max_lines = (area.height.saturating_sub(2)) as usize; // Subtract borders
            let lines: Vec<Line> = output
                .iter()
                .take(max_lines)
                .map(|line| Line::from(line.as_str()))
                .collect();
            Text::from(lines)
        };

        Paragraph::new(content).block(block).render(area, buf);
    }

    /// Update output in view mode
    fn update_view_output(&mut self) {
        // In view mode, we can show a snapshot or help text
        let mut output = self.output.lock().unwrap();
        if output.is_empty() {
            let tool_name = self.tool_name();
            *output = vec![
                format!("{} Interactive CLI", tool_name),
                "".to_string(),
                format!("Press 'i' to start {} in interactive mode", tool_name),
                "".to_string(),
                "In interactive mode, all keyboard input will be forwarded to the tool."
                    .to_string(),
                "".to_string(),
                "Press 'i' again to exit interactive mode.".to_string(),
            ];
        }
    }

    /// Read process output in a background thread
    fn read_process_output(output: Arc<Mutex<Vec<String>>>, tool_name: String) {
        // This is a simplified version - in a real implementation,
        // we'd need proper PTY support for full terminal emulation
        // For now, we'll just update periodically
        // In a full implementation, we'd read from the process stdout/stderr
        let lines = vec![
            format!("{} is running in interactive mode", tool_name),
            "".to_string(),
            "All keyboard input is forwarded to the process.".to_string(),
            "".to_string(),
            "Note: Full terminal emulation requires PTY support.".to_string(),
        ];

        let mut output_guard = output.lock().unwrap();
        *output_guard = lines;
    }

    /// Find a tool in PATH
    fn find_tool(tool_name: &str) -> Option<String> {
        #[cfg(unix)]
        {
            if let Ok(output) = Command::new("which").arg(tool_name).output() {
                if output.status.success() {
                    if let Ok(path) = String::from_utf8(output.stdout) {
                        let path = path.trim().to_string();
                        if !path.is_empty() {
                            return Some(path);
                        }
                    }
                }
            }
        }

        #[cfg(target_os = "windows")]
        {
            if let Ok(output) = Command::new("where").arg(tool_name).output() {
                if output.status.success() {
                    if let Ok(path) = String::from_utf8(output.stdout) {
                        let path = path.lines().next().unwrap_or("").trim().to_string();
                        if !path.is_empty() {
                            return Some(path);
                        }
                    }
                }
            }
        }

        None
    }

    /// Returns whether keyboard input is currently forwarded to the child process.
    pub fn is_interactive(&self) -> bool {
        self.mode == InteractiveMode::Interactive
    }

    /// Returns the configured tool name.
    pub fn tool_name(&self) -> &str {
        &self.tool_name
    }
}

impl Drop for InteractiveCli {
    fn drop(&mut self) {
        self.exit_interactive_mode();
    }
}
