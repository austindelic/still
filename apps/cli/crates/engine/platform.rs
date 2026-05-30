//! Platform identifiers and filters used before planning.

use std::{fmt, str::FromStr};

use crate::error::{EngineError, EngineResult};

/// Supported host platform identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PlatformId {
    Macos,
    Linux,
    Windows,
}

pub fn current_platform() -> PlatformId {
    if cfg!(target_os = "macos") {
        PlatformId::Macos
    } else if cfg!(target_os = "windows") {
        PlatformId::Windows
    } else {
        PlatformId::Linux
    }
}

impl fmt::Display for PlatformId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Macos => f.write_str("macos"),
            Self::Linux => f.write_str("linux"),
            Self::Windows => f.write_str("windows"),
        }
    }
}

impl FromStr for PlatformId {
    type Err = EngineError;

    fn from_str(input: &str) -> EngineResult<Self> {
        match input {
            "macos" | "darwin" => Ok(Self::Macos),
            "linux" => Ok(Self::Linux),
            "windows" | "win32" => Ok(Self::Windows),
            value => Err(EngineError::UnknownPlatform {
                platform: value.to_string(),
            }
            .into()),
        }
    }
}

/// Normalized platform applicability for one config entry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlatformFilter {
    allow: Vec<PlatformId>,
    deny: Vec<PlatformId>,
}

impl PlatformFilter {
    /// Creates a filter from raw config fields.
    /// # Errors
    /// Fails when any supplied platform string is unknown.
    pub fn from_config(
        platforms: &[String],
        ignore: Option<&str>,
        only: Option<&str>,
    ) -> EngineResult<Self> {
        let allow = if let Some(only) = only {
            vec![parse_platform(only)?]
        } else {
            parse_platforms(platforms)?
        };

        let deny = match ignore {
            Some(platform) => vec![parse_platform(platform)?],
            None => Vec::new(),
        };

        Ok(Self { allow, deny })
    }

    /// Returns whether this entry applies to `platform`.
    pub fn matches(&self, platform: PlatformId) -> bool {
        let allowed = self.allow.is_empty() || self.allow.contains(&platform);
        let denied = self.deny.contains(&platform);
        allowed && !denied
    }
}

fn parse_platforms(values: &[String]) -> EngineResult<Vec<PlatformId>> {
    values.iter().map(|value| value.parse()).collect()
}

fn parse_platform(value: &str) -> EngineResult<PlatformId> {
    value.parse()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_platform_aliases() {
        assert_eq!("macos".parse::<PlatformId>().unwrap(), PlatformId::Macos);
        assert_eq!("darwin".parse::<PlatformId>().unwrap(), PlatformId::Macos);
        assert_eq!("win32".parse::<PlatformId>().unwrap(), PlatformId::Windows);
    }

    #[test]
    fn empty_filter_matches_all_platforms() {
        let filter = PlatformFilter::default();

        assert!(filter.matches(PlatformId::Macos));
        assert!(filter.matches(PlatformId::Linux));
        assert!(filter.matches(PlatformId::Windows));
    }

    #[test]
    fn platforms_allow_list_limits_matches() {
        let filter =
            PlatformFilter::from_config(&["macos".to_string(), "linux".to_string()], None, None)
                .unwrap();

        assert!(filter.matches(PlatformId::Macos));
        assert!(filter.matches(PlatformId::Linux));
        assert!(!filter.matches(PlatformId::Windows));
    }

    #[test]
    fn ignore_excludes_platform_after_allow_list() {
        let filter = PlatformFilter::from_config(
            &["macos".to_string(), "linux".to_string()],
            Some("linux"),
            None,
        )
        .unwrap();

        assert!(filter.matches(PlatformId::Macos));
        assert!(!filter.matches(PlatformId::Linux));
        assert!(!filter.matches(PlatformId::Windows));
    }

    #[test]
    fn only_overrides_platforms_allow_list() {
        let filter =
            PlatformFilter::from_config(&["macos".to_string()], None, Some("windows")).unwrap();

        assert!(!filter.matches(PlatformId::Macos));
        assert!(filter.matches(PlatformId::Windows));
    }

    #[test]
    fn ignore_excludes_after_only_filter() {
        let filter =
            PlatformFilter::from_config(&["macos".to_string()], Some("windows"), Some("windows"))
                .unwrap();

        assert!(!filter.matches(PlatformId::Macos));
        assert!(!filter.matches(PlatformId::Windows));
    }

    #[test]
    fn unknown_platform_is_an_error() {
        let err = PlatformFilter::from_config(&["freebsd".to_string()], None, None).unwrap_err();

        assert_eq!(
            err,
            EngineError::UnknownPlatform {
                platform: "freebsd".to_string()
            }
        );
    }
}
