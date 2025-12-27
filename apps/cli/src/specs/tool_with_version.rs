use std::{fmt, str::FromStr};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolWithVersionSpec {
    pub tool: String,
    pub version: String,
}

#[derive(Debug, Clone)]
enum ParseToolWithVersionSpecError {
    EmptyInput,
    TooManyAts {
        input: String,
    },
    EmptyTool {
        input: String,
    },
    InvalidToolFormat {
        tool: String,
        reason: String,
    },
    InvalidVersion {
        tool: String,
        version: String,
        reason: String,
    },
}

impl fmt::Display for ParseToolWithVersionSpecError {
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
            Self::InvalidToolFormat { tool, reason } => write!(
                f,
                "Invalid tool name \"{}\": {}. Tool names must match: [a-zA-Z][a-zA-Z0-9_-]*",
                tool, reason
            ),
            Self::InvalidVersion {
                tool,
                version,
                reason,
            } => write!(
                f,
                "Invalid version \"{}\" for tool \"{}\": {}. Version must be SemVer (e.g. 1.2.3) or \"latest\". {}",
                version, tool, reason, examples
            ),
        }
    }
}

impl From<ParseToolWithVersionSpecError> for String {
    fn from(e: ParseToolWithVersionSpecError) -> Self {
        e.to_string()
    }
}

fn is_valid_tool_format(tool: &str) -> Result<(), String> {
    // Simple, dependency-free validator:
    // - first char letter
    // - rest [A-Za-z0-9_-]
    let mut chars = tool.chars();

    let Some(first) = chars.next() else {
        return Err("tool name is empty".into());
    };
    if !first.is_ascii_alphabetic() {
        return Err("must start with a letter".into());
    }
    for c in chars {
        if !(c.is_ascii_alphanumeric() || c == '_' || c == '-') {
            return Err(format!("contains invalid character '{}'", c));
        }
    }
    Ok(())
}

fn validate_version(tool: &str, version: &str) -> Result<(), ParseToolWithVersionSpecError> {
    if version.eq_ignore_ascii_case("latest") {
        return Ok(());
    }

    match semver::Version::parse(version) {
        Ok(_) => Ok(()),
        Err(e) => Err(ParseToolWithVersionSpecError::InvalidVersion {
            tool: tool.to_string(),
            version: version.to_string(),
            reason: e.to_string(),
        }),
    }
}

impl FromStr for ToolWithVersionSpec {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let s = input.trim();
        if s.is_empty() {
            return Err(ParseToolWithVersionSpecError::EmptyInput.into());
        }

        // At most one '@'
        if s.matches('@').count() > 1 {
            return Err(ParseToolWithVersionSpecError::TooManyAts {
                input: s.to_string(),
            }
            .into());
        }

        // Auto-expand:
        // - "tool" => "tool@latest"
        // - "tool@" => version defaults to "latest"
        let (tool, version) = match s.split_once('@') {
            None => (s, "latest"),
            Some((t, v)) if v.is_empty() => (t, "latest"),
            Some((t, v)) => (t, v),
        };

        if tool.is_empty() {
            return Err(ParseToolWithVersionSpecError::EmptyTool {
                input: s.to_string(),
            }
            .into());
        }

        // Tool format validation
        if let Err(reason) = is_valid_tool_format(tool) {
            return Err(ParseToolWithVersionSpecError::InvalidToolFormat {
                tool: tool.to_string(),
                reason,
            }
            .into());
        }

        // Version validation
        validate_version(tool, version).map_err(String::from)?;

        Ok(Self {
            tool: tool.to_string(),
            version: version.to_string(), // keep original (or normalise if you want)
        })
    }
}
