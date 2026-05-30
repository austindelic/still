//! Typed engine errors used before CLI/TUI formatting.

use std::path::PathBuf;

/// Shared result type for deterministic engine validation and planning.
pub type EngineResult<T> = Result<T, EngineError>;

/// Typed errors returned by engine validation, resolution, and planning code.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum EngineError {
    #[error("no still.toml found; run `still init` in this project or pass `--global`")]
    MissingProjectConfig,
    #[error("{} already exists; pass --force to overwrite it", path.display())]
    ConfigAlreadyExists { path: PathBuf },
    #[error("unknown platform \"{platform}\"")]
    UnknownPlatform { platform: String },
    #[error("unknown item kind \"{kind}\"")]
    UnknownItemKind { kind: String },
    #[error("invalid item spec: {reason}")]
    InvalidItemSpec { reason: String },
    #[error("invalid config: {reason}")]
    InvalidConfig { reason: String },
    #[error("install request must include at least one item")]
    EmptyInstallRequest,
    #[error(
        "project config is not trusted for {behavior}; review {config_path} then run `still trust`"
    )]
    UntrustedProjectConfig {
        config_path: PathBuf,
        behavior: String,
    },
    #[error("{feature} is not implemented yet")]
    NotImplemented { feature: String },
    #[error("{feature} is not supported on {platform}")]
    UnsupportedPlatform { feature: String, platform: String },
    #[error("{message}")]
    Conflict { message: String },
}

impl EngineError {
    /// Creates a not-yet-implemented error with a stable display shape.
    pub fn not_implemented(feature: impl Into<String>) -> Self {
        Self::NotImplemented {
            feature: feature.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_project_config_message_matches_cli_guidance() {
        let err = EngineError::MissingProjectConfig;

        assert_eq!(
            err.to_string(),
            "no still.toml found; run `still init` in this project or pass `--global`"
        );
    }

    #[test]
    fn unsupported_platform_names_feature_and_platform() {
        let err = EngineError::UnsupportedPlatform {
            feature: "homebrew-cask apps".to_string(),
            platform: "linux".to_string(),
        };

        assert_eq!(
            err.to_string(),
            "homebrew-cask apps is not supported on linux"
        );
    }
}
