//! Parser for user-facing tool request syntax.

use std::{fmt, str::FromStr};

use anyhow::Result;

use super::item::{BackendId, ItemSpec};

/// Parsed tool request from CLI/config input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolSpec {
    /// Tool or package name.
    pub name: String,
    /// Requested version, or `latest` when no version was provided.
    pub version: String,
    /// Requested backend when the user supplied `name@version@backend`.
    pub backend: Option<BackendId>,
}

#[derive(Debug, Clone)]
enum ParseToolSpecError {
    Invalid { reason: String },
}

impl std::error::Error for ParseToolSpecError {}

impl fmt::Display for ParseToolSpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let examples = "Examples: bun@1.3.5, bun@latest, bun@latest@aqua, rust@stable@rustup, bun";
        match self {
            Self::Invalid { reason } => write!(f, "Invalid tool spec: {reason}. {examples}"),
        }
    }
}

impl FromStr for ToolSpec {
    type Err = anyhow::Error;

    fn from_str(input: &str) -> Result<Self> {
        let item = ItemSpec::parse(input).map_err(|e| {
            let message = e.to_string();
            let reason = message
                .strip_prefix("invalid item spec: ")
                .unwrap_or(&message)
                .to_string();
            anyhow::anyhow!(ParseToolSpecError::Invalid { reason })
        })?;

        Ok(Self {
            name: item.name,
            version: item.version.to_string(),
            backend: item.backend,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_stable_toolchain_backend() {
        let spec: ToolSpec = "rust@stable@rustup".parse().unwrap();

        assert_eq!(spec.name, "rust");
        assert_eq!(spec.version, "stable");
        assert_eq!(spec.backend.unwrap().as_str(), "rustup");
    }

    #[test]
    fn keeps_latest_default() {
        let spec: ToolSpec = "ripgrep".parse().unwrap();

        assert_eq!(spec.name, "ripgrep");
        assert_eq!(spec.version, "latest");
        assert_eq!(spec.backend, None);
    }
}
