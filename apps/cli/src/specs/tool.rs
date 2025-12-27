use std::{fmt, str::FromStr};

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

impl From<ParseToolSpecError> for String {
    fn from(e: ParseToolSpecError) -> Self {
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

fn validate_version(tool: &str, version: &str) -> Result<(), ParseToolSpecError> {
    if version.eq_ignore_ascii_case("latest") {
        return Ok(());
    }

    match semver::Version::parse(version) {
        Ok(_) => Ok(()),
        Err(e) => Err(ParseToolSpecError::InvalidVersion {
            name: tool.to_string(),
            version: version.to_string(),
            reason: e.to_string(),
        }),
    }
}

impl FromStr for ToolSpec {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let s = input.trim();
        if s.is_empty() {
            return Err(ParseToolSpecError::EmptyInput.into());
        }

        // At most one '@'
        if s.matches('@').count() > 1 {
            return Err(ParseToolSpecError::TooManyAts {
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
            return Err(ParseToolSpecError::EmptyTool {
                input: s.to_string(),
            }
            .into());
        }

        // Tool format validation
        if let Err(reason) = is_valid_tool_format(tool) {
            return Err(ParseToolSpecError::InvalidToolFormat {
                name: tool.to_string(),
                reason,
            }
            .into());
        }

        // Version validation
        validate_version(tool, version).map_err(String::from)?;

        Ok(Self {
            name: tool.to_string(),
            version: version.to_string(), // keep original (or normalise if you want)
        })
    }
}
