//! Shared item models for tools, packages, and apps.

use std::{fmt, str::FromStr};

use anyhow::{Result, bail};

/// User-facing role an item plays in a Still project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    type Err = anyhow::Error;

    fn from_str(input: &str) -> Result<Self> {
        match input {
            "tool" | "tools" => Ok(Self::Tool),
            "package" | "packages" | "pkg" | "pkgs" => Ok(Self::Package),
            "app" | "apps" => Ok(Self::App),
            value => bail!("unknown item kind \"{value}\""),
        }
    }
}

/// Backend selected for resolution and install.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BackendId(String);

impl BackendId {
    /// Creates a backend id after validating command/config input.
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        validate_identifier("backend", &value, false)?;
        Ok(Self(value))
    }

    /// Returns the backend id as written after normalization.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for BackendId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for BackendId {
    type Err = anyhow::Error;

    fn from_str(input: &str) -> Result<Self> {
        Self::new(input)
    }
}

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

    /// Returns whether the request tracks the backend's latest version.
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
    type Err = anyhow::Error;

    fn from_str(input: &str) -> Result<Self> {
        Self::new(input)
    }
}

/// Parsed item request before backend resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemSpec {
    pub name: String,
    pub version: VersionReq,
    pub backend: Option<BackendId>,
}

impl ItemSpec {
    /// Parses `name`, `name@version`, or `name@version@backend`.
    pub fn parse(input: &str) -> Result<Self> {
        let value = input.trim();
        if value.is_empty() {
            bail!("item spec cannot be empty");
        }

        let parts: Vec<&str> = value.split('@').collect();
        if parts.len() > 3 {
            bail!(
                "invalid item spec \"{value}\": expected name, name@version, or name@version@backend"
            );
        }

        let name = parts[0].trim();
        validate_identifier("item", name, false)?;

        let version = VersionReq::new(parts.get(1).copied().unwrap_or("latest"))?;
        let backend = match parts.get(2).map(|value| value.trim()) {
            Some("") => bail!("invalid item spec \"{value}\": backend cannot be empty"),
            Some(value) => Some(BackendId::new(value)?),
            None => None,
        };

        Ok(Self {
            name: name.to_string(),
            version,
            backend,
        })
    }
}

impl FromStr for ItemSpec {
    type Err = anyhow::Error;

    fn from_str(input: &str) -> Result<Self> {
        Self::parse(input)
    }
}

fn validate_identifier(label: &str, value: &str, allow_slash: bool) -> Result<()> {
    if value.is_empty() {
        bail!("{label} name cannot be empty");
    }

    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        bail!("{label} name cannot be empty");
    };

    if !first.is_ascii_alphanumeric() {
        bail!("{label} name must start with a letter or number");
    }

    for c in chars {
        if !(c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.') || (allow_slash && c == '/'))
        {
            bail!("{label} name contains invalid character '{c}'");
        }
    }

    Ok(())
}

fn validate_version_req(value: &str) -> Result<()> {
    for c in value.chars() {
        if !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+' | '/')) {
            bail!("version contains invalid character '{c}'");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_name_as_latest_auto_backend() {
        let spec: ItemSpec = "ripgrep".parse().unwrap();

        assert_eq!(spec.name, "ripgrep");
        assert_eq!(spec.version.as_str(), "latest");
        assert_eq!(spec.backend, None);
    }

    #[test]
    fn parses_version_and_backend() {
        let spec: ItemSpec = "rust@stable@rustup".parse().unwrap();

        assert_eq!(spec.name, "rust");
        assert_eq!(spec.version.as_str(), "stable");
        assert_eq!(spec.backend.unwrap().as_str(), "rustup");
    }

    #[test]
    fn parses_backend_names_with_hyphens() {
        let spec: ItemSpec = "firefox@latest@homebrew-cask".parse().unwrap();

        assert_eq!(spec.name, "firefox");
        assert_eq!(spec.version.as_str(), "latest");
        assert_eq!(spec.backend.unwrap().as_str(), "homebrew-cask");
    }

    #[test]
    fn rejects_empty_backend() {
        let err = ItemSpec::parse("ripgrep@latest@").unwrap_err();

        assert!(err.to_string().contains("backend cannot be empty"));
    }
}
