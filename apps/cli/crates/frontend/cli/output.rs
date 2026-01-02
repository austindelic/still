/// Output formatting utilities
pub struct Output;

impl Output {
    /// Print an info message
    pub fn info(msg: &str) {
        println!("{}", msg);
    }

    /// Print an error message
    pub fn error(msg: &str) {
        eprintln!("{}", msg);
    }

    /// Print a success message
    pub fn success(msg: &str) {
        println!("✓ {}", msg);
    }

    /// Print a warning message
    pub fn warning(msg: &str) {
        eprintln!("⚠ {}", msg);
    }
}

