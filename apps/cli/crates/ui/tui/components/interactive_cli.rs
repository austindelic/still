use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Text},
    widgets::{Block, Borders, Paragraph, Widget},
};
use std::process::{Command, Stdio};
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::thread;

/// State of the interactive CLI component
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractiveMode {
    View,        // Viewing output only
    Interactive, // Interacting with the CLI tool
}

/// Generic interactive CLI component that can embed any CLI tool
#[derive(Debug)]
pub struct InteractiveCli {
    tool_name: String,
    mode: InteractiveMode,
    output: Arc<Mutex<Vec<String>>>,
    process: Option<std::process::Child>,
    last_update: std::time::Instant,
}

impl InteractiveCli {
    /// Create a new interactive CLI component with the specified tool name
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

    /// Toggle interactive mode
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

    /// Send input to the interactive process
    pub fn send_input(&mut self, input: &str) {
        if let Some(ref mut child) = self.process {
            if let Some(ref mut stdin) = child.stdin {
                let _ = stdin.write_all(input.as_bytes());
                let _ = stdin.flush();
            }
        }
    }

    /// Send a key event to the process
    pub fn send_key(&mut self, key: u8) {
        if let Some(ref mut child) = self.process {
            if let Some(ref mut stdin) = child.stdin {
                let _ = stdin.write_all(&[key]);
                let _ = stdin.flush();
            }
        }
    }

    /// Render the component
    pub fn render(&mut self, area: Rect, buf: &mut Buffer) {
        // Update output periodically in view mode
        if self.mode == InteractiveMode::View && self.last_update.elapsed().as_millis() > 500 {
            self.update_view_output();
            self.last_update = std::time::Instant::now();
        }

        let mode_indicator = match self.mode {
            InteractiveMode::View => format!(" {} (Press 'i' to interact) ", self.tool_name),
            InteractiveMode::Interactive => format!(" {} (Interactive - Press 'i' to exit) ", self.tool_name),
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(if self.mode == InteractiveMode::Interactive {
                Style::default().fg(Color::Yellow).add_modifier(ratatui::style::Modifier::BOLD)
            } else {
                Style::default().fg(Color::Cyan)
            })
            .title(mode_indicator);

        let output = self.output.lock().unwrap();
        let content = if output.is_empty() {
            Text::from(vec![
                Line::from(format!("Loading {}...", self.tool_name).fg(Color::DarkGray)),
                Line::from(""),
                Line::from(format!("Press 'i' to interact with {}", self.tool_name).fg(Color::DarkGray)),
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

        Paragraph::new(content)
            .block(block)
            .render(area, buf);
    }

    /// Update output in view mode
    fn update_view_output(&mut self) {
        // In view mode, we can show a snapshot or help text
        let mut output = self.output.lock().unwrap();
        if output.is_empty() {
            *output = vec![
                format!("{} Interactive CLI", self.tool_name),
                "".to_string(),
                format!("Press 'i' to start {} in interactive mode", self.tool_name),
                "".to_string(),
                "In interactive mode, all keyboard input will be forwarded to the tool.".to_string(),
                "".to_string(),
                "Press 'i' again to exit interactive mode.".to_string(),
            ];
        }
    }

    /// Read process output in a background thread
    fn read_process_output(output: Arc<Mutex<Vec<String>>>, tool_name: String) {
        // This is a simplified version - in a real implementation,
        // we'd need proper PTY support for full terminal emulation
        let mut buffer = vec![0u8; 4096];
        
        // For now, we'll just update periodically
        // In a full implementation, we'd read from the process stdout/stderr
        let mut lines = vec![
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

    /// Check if currently in interactive mode
    pub fn is_interactive(&self) -> bool {
        self.mode == InteractiveMode::Interactive
    }

    /// Get the tool name
    pub fn tool_name(&self) -> &str {
        &self.tool_name
    }
}

impl Drop for InteractiveCli {
    fn drop(&mut self) {
        self.exit_interactive_mode();
    }
}

