//! Shared item models for tools, packages, and apps.

use std::{fmt, str::FromStr};

use crate::error::Result;

use crate::error::EngineError;
use crate::resolve::source::{SourceId, SourceIntent};

/// User-facing role an item plays in a Still project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ItemKind {
    Tool,
    Package,
    App,
}

impl fmt::Display for ItemKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tool => f.write_str("tool"),
            Self::Package => f.write_str("package"),
            Self::App => f.write_str("app"),
        }
    }
}

impl FromStr for ItemKind {
    type Err = EngineError;

    fn from_str(input: &str) -> std::result::Result<Self, Self::Err> {
        match input {
            "tool" | "tools" => Ok(Self::Tool),
            "package" | "packages" | "pkg" | "pkgs" => Ok(Self::Package),
            "app" | "apps" => Ok(Self::App),
            value => Err(Self::Err::UnknownItemKind {
                kind: value.to_string(),
            }),
        }
    }
}

pub type BackendId = SourceId;

/// Requested version string from CLI or config.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VersionReq(String);

impl VersionReq {
    /// Creates a version request, defaulting blank values to `latest`.
    pub fn new(value: impl AsRef<str>) -> Result<Self> {
        let raw = value.as_ref().trim();
        let value = if raw.is_empty() { "latest" } else { raw };
        validate_version_req(value)?;
        Ok(Self(value.to_string()))
    }

    /// Returns the requested version exactly as normalized by Still.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns whether the request tracks the selected source's latest version.
    pub fn is_latest(&self) -> bool {
        self.0.eq_ignore_ascii_case("latest")
    }
}

impl Default for VersionReq {
    fn default() -> Self {
        Self("latest".to_string())
    }
}

impl fmt::Display for VersionReq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for VersionReq {
    type Err = EngineError;

    fn from_str(input: &str) -> Result<Self> {
        Self::new(input)
    }
}

/// Parsed item request before source resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemSpec {
    pub name: String,
    pub version: VersionReq,
    pub backend: Option<BackendId>,
}

impl ItemSpec {
    /// Parses `name`, `name@version`, `source:name`, or `source:name@version`.
    pub fn parse(input: &str) -> Result<Self> {
        let value = input.trim();
        if value.is_empty() {
            return Err(invalid_item_spec("item spec cannot be empty"));
        }

        let (source_prefix, body) = split_source_prefix(value)?;
        let parts: Vec<&str> = body.split('@').collect();
        if parts.len() > 3 || (source_prefix.is_some() && parts.len() > 2) {
            return Err(invalid_item_spec(format!(
                "invalid item spec \"{value}\": expected name, name@version, source:name, or source:name@version"
            )));
        }

        let name = parts[0].trim();
        validate_identifier("item", name, false)?;

        let version = VersionReq::new(parts.get(1).copied().unwrap_or("latest"))?;
        let legacy_source = match parts.get(2).map(|value| value.trim()) {
            Some("") => {
                return Err(invalid_item_spec(format!(
                    "invalid item spec \"{value}\": source cannot be empty"
                )));
            }
            Some(value) => Some(SourceId::new(value)?),
            None => None,
        };
        let backend = source_prefix.or(legacy_source);

        Ok(Self {
            name: name.to_string(),
            version,
            backend,
        })
    }

    /// Returns the source intent represented by this parsed spec.
    pub fn source_intent(&self) -> SourceIntent {
        match &self.backend {
            Some(source) if source.as_str() == "auto" => SourceIntent::Auto,
            Some(source) => SourceIntent::Explicit(source.clone()),
            None => SourceIntent::Auto,
        }
    }
}

impl FromStr for ItemSpec {
    type Err = EngineError;

    fn from_str(input: &str) -> Result<Self> {
        Self::parse(input)
    }
}

fn validate_identifier(label: &str, value: &str, allow_slash: bool) -> Result<()> {
    if value.is_empty() {
        return Err(invalid_item_spec(format!("{label} name cannot be empty")));
    }

    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return Err(invalid_item_spec(format!("{label} name cannot be empty")));
    };

    if !first.is_ascii_alphanumeric() {
        return Err(invalid_item_spec(format!(
            "{label} name must start with a letter or number"
        )));
    }

    for c in chars {
        if !(c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.') || (allow_slash && c == '/'))
        {
            return Err(invalid_item_spec(format!(
                "{label} name contains invalid character '{c}'"
            )));
        }
    }

    Ok(())
}

fn validate_version_req(value: &str) -> Result<()> {
    for c in value.chars() {
        if !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+' | '/')) {
            return Err(invalid_item_spec(format!(
                "version contains invalid character '{c}'"
            )));
        }
    }
    Ok(())
}

fn split_source_prefix(value: &str) -> Result<(Option<SourceId>, &str)> {
    let Some((source, body)) = value.split_once(':') else {
        return Ok((None, value));
    };
    if source.trim().is_empty() {
        return Err(invalid_item_spec(format!(
            "invalid item spec \"{value}\": source cannot be empty"
        )));
    }
    if body.trim().is_empty() {
        return Err(invalid_item_spec(format!(
            "invalid item spec \"{value}\": item name cannot be empty"
        )));
    }
    Ok((Some(SourceId::new(source.trim())?), body))
}

fn invalid_item_spec(reason: impl Into<String>) -> EngineError {
    EngineError::InvalidItemSpec {
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_name_as_latest_auto_source() {
        let spec: ItemSpec = "ripgrep".parse().unwrap();

        assert_eq!(spec.name, "ripgrep");
        assert_eq!(spec.version.as_str(), "latest");
        assert_eq!(spec.backend, None);
        assert_eq!(spec.source_intent(), SourceIntent::Auto);
    }

    #[test]
    fn parses_source_prefix_and_version() {
        let spec: ItemSpec = "cargo:ripgrep@14.1.1".parse().unwrap();

        assert_eq!(spec.name, "ripgrep");
        assert_eq!(spec.version.as_str(), "14.1.1");
        assert_eq!(spec.backend.unwrap().as_str(), "cargo");
    }

    #[test]
    fn parses_legacy_version_and_source_suffix() {
        let spec: ItemSpec = "rust@stable@rustup".parse().unwrap();

        assert_eq!(spec.name, "rust");
        assert_eq!(spec.version.as_str(), "stable");
        assert_eq!(spec.backend.unwrap().as_str(), "rustup");
    }

    #[test]
    fn parses_source_names_with_hyphens() {
        let spec: ItemSpec = "firefox@latest@homebrew-cask".parse().unwrap();

        assert_eq!(spec.name, "firefox");
        assert_eq!(spec.version.as_str(), "latest");
        assert_eq!(spec.backend.unwrap().as_str(), "homebrew-cask");
    }

    #[test]
    fn rejects_empty_source_suffix() {
        let err = ItemSpec::parse("ripgrep@latest@").unwrap_err();

        assert!(err.to_string().contains("source cannot be empty"));
    }

    #[test]
    fn rejects_empty_source_prefix() {
        let err = ItemSpec::parse(":ripgrep").unwrap_err();

        assert!(err.to_string().contains("source cannot be empty"));
    }

    #[test]
    fn unknown_item_kind_is_typed() {
        let err = "service".parse::<ItemKind>().unwrap_err();

        assert!(matches!(
            err,
            EngineError::UnknownItemKind { kind } if kind == "service"
        ));
    }
}
