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

#[cfg(target_os = "macos")]
pub const CURRENT_PLATFORM: PlatformId = PlatformId::Macos;
#[cfg(target_os = "linux")]
pub const CURRENT_PLATFORM: PlatformId = PlatformId::Linux;
#[cfg(target_os = "windows")]
pub const CURRENT_PLATFORM: PlatformId = PlatformId::Windows;

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
compile_error!("still supports macOS, Linux, and Windows targets");

pub const fn current_platform() -> PlatformId {
    CURRENT_PLATFORM
}

/// CPU architecture used when resolving source artifacts for a host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Architecture {
    X86_64,
    Aarch64,
    Arm,
    Unknown,
}

impl Architecture {
    /// Detects the compile-time target architecture for the current build.
    pub const fn detect() -> Self {
        #[cfg(target_arch = "x86_64")]
        {
            Self::X86_64
        }
        #[cfg(target_arch = "aarch64")]
        {
            Self::Aarch64
        }
        #[cfg(target_arch = "arm")]
        {
            Self::Arm
        }
        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "arm")))]
        {
            Self::Unknown
        }
    }
}

impl fmt::Display for Architecture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::X86_64 => f.write_str("x86_64"),
            Self::Aarch64 => f.write_str("aarch64"),
            Self::Arm => f.write_str("arm"),
            Self::Unknown => f.write_str("unknown"),
        }
    }
}

/// Host platform data supplied to source and planner resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HostPlatform {
    id: PlatformId,
    arch: Architecture,
}

impl HostPlatform {
    /// Detects the host represented by this compiled binary.
    pub const fn detect() -> Self {
        Self {
            id: current_platform(),
            arch: Architecture::detect(),
        }
    }

    /// Builds platform data for deterministic tests and dry planning.
    pub const fn new(id: PlatformId, arch: Architecture) -> Self {
        Self { id, arch }
    }

    /// Returns the operating system identifier used by config filters.
    pub const fn id(&self) -> PlatformId {
        self.id
    }

    /// Returns the architecture used by source artifact selectors.
    pub const fn arch(&self) -> Architecture {
        self.arch
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

        assert!(matches!(
            err,
            EngineError::UnknownPlatform { platform } if platform == "freebsd"
        ));
    }

    #[test]
    fn host_platform_can_be_injected_for_tests() {
        let platform = HostPlatform::new(PlatformId::Windows, Architecture::Aarch64);

        assert_eq!(platform.id(), PlatformId::Windows);
        assert_eq!(platform.arch(), Architecture::Aarch64);
    }

    #[test]
    fn detected_host_uses_compile_time_target_os() {
        assert_eq!(HostPlatform::detect().id(), current_platform());
    }
}
