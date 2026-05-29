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

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct BufferedOutput {
    pub stdout: String,
    pub stderr: String,
}

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

fn push_line(buffer: &mut String, msg: &str) {
    buffer.push_str(msg);
    buffer.push('\n');
}
