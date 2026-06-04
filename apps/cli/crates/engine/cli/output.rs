//! Output abstraction for production CLI writes and command tests.

/// Output sink used by command handlers.
///
/// Handlers write through this trait instead of calling stdout/stderr directly so
/// integration-style unit tests can assert command output without spawning the
/// binary. Each method receives already formatted message text without a trailing
/// newline; implementations decide the final stream and decoration.
pub trait Output {
    /// Writes informational stdout content.
    ///
    /// `msg` is emitted as normal command output.
    fn info(&mut self, msg: &str);
    /// Writes error stderr content.
    ///
    /// `msg` should describe a failure path and is emitted on stderr.
    fn error(&mut self, msg: &str);
    /// Writes success stdout content.
    ///
    /// `msg` is emitted as successful command output; implementations may add a
    /// visual success marker.
    fn success(&mut self, msg: &str);
    /// Writes warning stderr content.
    ///
    /// `msg` is emitted on stderr for recoverable or partial-success conditions.
    fn warning(&mut self, msg: &str);
}

/// Production output sink backed by stdout and stderr.
///
/// Use this only at the binary boundary. Command logic should remain generic over
/// `Output` so tests can use `BufferedOutput`.
#[derive(Debug, Default)]
pub struct StdOutput;

impl Output for StdOutput {
    fn info(&mut self, msg: &str) {
        println!("{msg}");
    }

    fn error(&mut self, msg: &str) {
        eprintln!("{msg}");
    }

    fn success(&mut self, msg: &str) {
        println!("✓ {msg}");
    }

    fn warning(&mut self, msg: &str) {
        eprintln!("⚠ {msg}");
    }
}

/// Test output sink that records stdout and stderr separately.
///
/// Each write appends a newline to the selected buffer, matching the production
/// line-oriented output behavior closely enough for snapshot and string asserts.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg(test)]
pub struct BufferedOutput {
    /// Captured stdout lines.
    pub stdout: String,
    /// Captured stderr lines.
    pub stderr: String,
}

#[cfg(test)]
impl Output for BufferedOutput {
    fn info(&mut self, msg: &str) {
        push_line(&mut self.stdout, msg);
    }

    fn error(&mut self, msg: &str) {
        push_line(&mut self.stderr, msg);
    }

    fn success(&mut self, msg: &str) {
        push_line(&mut self.stdout, &format!("✓ {msg}"));
    }

    fn warning(&mut self, msg: &str) {
        push_line(&mut self.stderr, &format!("⚠ {msg}"));
    }
}

#[cfg(test)]
fn push_line(buffer: &mut String, msg: &str) {
    buffer.push_str(msg);
    buffer.push('\n');
}
