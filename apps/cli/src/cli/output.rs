use std::io::Write;

pub trait Output {
    fn info(&mut self, msg: &str);
    fn error(&mut self, msg: &str);
    fn success(&mut self, msg: &str);
    fn warning(&mut self, msg: &str);
}

#[derive(Debug, Default)]
pub struct StdOutput;

impl Output for StdOutput {
    fn info(&mut self, msg: &str) {
        let mut stdout = anstream::stdout();
        let _ = writeln!(stdout, "{msg}");
    }

    fn error(&mut self, msg: &str) {
        let mut stderr = anstream::stderr();
        let _ = writeln!(stderr, "{msg}");
    }

    fn success(&mut self, msg: &str) {
        let mut stdout = anstream::stdout();
        let _ = writeln!(stdout, "✓ {msg}");
    }

    fn warning(&mut self, msg: &str) {
        let mut stderr = anstream::stderr();
        let _ = writeln!(stderr, "⚠ {msg}");
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg(test)]
#[allow(dead_code)]
pub struct BufferedOutput {
    pub stdout: String,
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
#[allow(dead_code)]
fn push_line(buffer: &mut String, msg: &str) {
    buffer.push_str(msg);
    buffer.push('\n');
}
