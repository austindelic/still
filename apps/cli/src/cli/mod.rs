//! Root CLI facade and feature-gated no-argument behavior.

pub use engine::cli::{args, commands, output, run_parsed, run_parsed_with_no_command};

/// Parses process arguments, dispatches commands, and exits on failure.
pub fn entry() {
    #[cfg(not(feature = "tui"))]
    {
        engine::cli::entry();
    }

    #[cfg(feature = "tui")]
    {
        engine::cli::entry_with_no_command(run_without_command);
    }
}

#[cfg(feature = "tui")]
fn run_without_command<O: output::Output>(output: &mut O) -> i32 {
    launch_tui_or_report(output, still_tui::launch_tui)
}

/// Launches the TUI and maps terminal startup failures into CLI output.
#[cfg(feature = "tui")]
pub fn launch_tui_or_report<O, F>(output: &mut O, launch: F) -> i32
where
    O: output::Output,
    F: FnOnce() -> std::io::Result<()>,
{
    match launch() {
        Ok(()) => 0,
        Err(e) => {
            output.error(&format!("Failed to launch TUI: {e}"));
            output.error("");
            output.error("This may be due to:");
            output.error("  - Terminal not supporting TUI mode");
            output.error("  - Terminal size too small");
            output.error("  - Missing required terminal capabilities");
            1
        }
    }
}

#[cfg(all(test, feature = "tui"))]
mod tests {
    use super::*;

    #[derive(Default)]
    struct TestOutput {
        stdout: String,
        stderr: String,
    }

    impl output::Output for TestOutput {
        fn info(&mut self, msg: &str) {
            self.stdout.push_str(msg);
            self.stdout.push('\n');
        }

        fn error(&mut self, msg: &str) {
            self.stderr.push_str(msg);
            self.stderr.push('\n');
        }

        fn success(&mut self, msg: &str) {
            self.info(&format!("✓ {msg}"));
        }

        fn warning(&mut self, msg: &str) {
            self.error(&format!("⚠ {msg}"));
        }
    }

    #[test]
    fn launch_tui_failure_reports_terminal_guidance() {
        let mut output = TestOutput::default();

        let code = launch_tui_or_report(&mut output, || {
            Err(std::io::Error::other("terminal unavailable"))
        });

        assert_eq!(code, 1);
        assert!(output.stderr.contains("Failed to launch TUI"));
        assert!(output.stderr.contains("Terminal not supporting TUI mode"));
        assert_eq!(output.stdout, "");
    }
}
