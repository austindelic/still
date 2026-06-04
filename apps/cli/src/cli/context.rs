use std::path::PathBuf;

use engine::error::{EngineContext, EngineError, Result};

/// Host and workspace context needed by CLI-routed engine actions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliContext {
    pub start_dir: PathBuf,
    pub home_dir: PathBuf,
}

impl CliContext {
    /// Loads the current process context once for a CLI invocation.
    pub fn load() -> Result<Self> {
        Ok(Self {
            start_dir: current_dir()?,
            home_dir: home_dir()?,
        })
    }
}

fn current_dir() -> Result<PathBuf> {
    std::env::current_dir().context("failed to read current directory")
}

fn home_dir() -> Result<PathBuf> {
    dirs::home_dir().ok_or_else(|| EngineError::message("failed to find home directory"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_reads_process_context() {
        let context = CliContext::load().expect("context should load");

        assert_eq!(
            context.start_dir,
            std::env::current_dir().expect("current directory should exist")
        );
        assert!(context.home_dir.is_absolute());
    }
}
