use std::{fmt, str::FromStr};

use anyhow::{Context, Result, bail};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolSpec {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone)]
enum ParseToolSpecError {
    EmptyInput,
    TooManyAts {
        input: String,
    },
    EmptyTool {
        input: String,
    },
    InvalidToolFormat {
        name: String,
        reason: String,
    },
    InvalidVersion {
        name: String,
        version: String,
        reason: String,
    },
}

impl std::error::Error for ParseToolSpecError {}

impl fmt::Display for ParseToolSpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let examples =
            "Examples: bun@1.3.5, bun@latest, bun (defaults to latest), bun@ (defaults to latest)";
        match self {
            Self::EmptyInput => write!(f, "Tool spec cannot be empty. {}", examples),
            Self::TooManyAts { input } => write!(
                f,
                "Invalid tool spec \"{}\": expected at most one '@'. {}",
                input, examples
            ),
            Self::EmptyTool { input } => write!(
                f,
                "Invalid tool spec \"{}\": tool name cannot be empty (before '@'). {}",
                input, examples
            ),
            Self::InvalidToolFormat { name, reason } => write!(
                f,
                "Invalid tool name \"{}\": {}. Tool names must match: [a-zA-Z][a-zA-Z0-9_-]*",
                name, reason
            ),
            Self::InvalidVersion {
                name,
                version,
                reason,
            } => write!(
                f,
                "Invalid version \"{}\" for tool \"{}\": {}. Version must be SemVer (e.g. 1.2.3) or \"latest\". {}",
                version, name, reason, examples
            ),
        }
    }
}

fn is_valid_tool_format(tool: &str) -> Result<()> {
    let mut chars = tool.chars();

    let Some(first) = chars.next() else {
        bail!("tool name is empty");
    };

    if !first.is_ascii_alphabetic() {
        bail!("tool name must start with a letter");
    }

    for c in chars {
        if !(c.is_ascii_alphanumeric() || c == '_' || c == '-') {
            bail!("tool name contains invalid character '{}'", c);
        }
    }

    Ok(())
}

fn validate_version(tool: &str, version: &str) -> Result<()> {
    if version.eq_ignore_ascii_case("latest") {
        return Ok(());
    }

    semver::Version::parse(version).map(|_| ()).map_err(|e| {
        anyhow::anyhow!(ParseToolSpecError::InvalidVersion {
            name: tool.to_string(),
            version: version.to_string(),
            reason: e.to_string(),
        })
    })
}

impl FromStr for ToolSpec {
    type Err = anyhow::Error;

    fn from_str(input: &str) -> Result<Self> {
        let s = input.trim();

        if s.is_empty() {
            bail!(ParseToolSpecError::EmptyInput);
        }

        if s.matches('@').count() > 1 {
            bail!(ParseToolSpecError::TooManyAts {
                input: s.to_string(),
            });
        }

        let (tool, version) = match s.split_once('@') {
            None => (s, "latest"),
            Some((t, v)) if v.is_empty() => (t, "latest"),
            Some((t, v)) => (t, v),
        };

        if tool.is_empty() {
            bail!(ParseToolSpecError::EmptyTool {
                input: s.to_string(),
            });
        }

        // Tool format validation
        if let Err(reason) = is_valid_tool_format(tool) {
            bail!(ParseToolSpecError::InvalidToolFormat {
                name: tool.to_string(),
                reason: format!("{reason:#}"), // preserve anyhow message nicely
            });
        }

        // Version validation (+ context)
        validate_version(tool, version)
            .with_context(|| format!("while validating version for tool \"{}\"", tool))?;

        Ok(Self {
            name: tool.to_string(),
            version: version.to_string(),
        })
    }
}
