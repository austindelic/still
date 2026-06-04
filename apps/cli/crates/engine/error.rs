//! Typed engine errors used before CLI/TUI formatting.

use std::{fmt, path::PathBuf};

/// Shared result type for deterministic engine validation and planning.
pub type EngineResult<T> = Result<T, EngineError>;
/// Shared result type used by engine modules.
pub type Result<T, E = EngineError> = std::result::Result<T, E>;

/// Typed errors returned by engine validation, resolution, and planning code.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("no still.toml found; run `still init` in this project or pass `--global`")]
    MissingProjectConfig,
    #[error("{} already exists; pass --force to overwrite it", path.display())]
    ConfigAlreadyExists { path: PathBuf },
    #[error("unknown platform \"{platform}\"")]
    UnknownPlatform { platform: String },
    #[error("unknown item kind \"{kind}\"")]
    UnknownItemKind { kind: String },
    #[error("unknown source \"{id}\"")]
    UnknownSource { id: String },
    #[error("source \"{id}\" does not support {kind} items")]
    UnsupportedSourceForKind { id: String, kind: String },
    #[error("source \"{id}\" does not support {platform}")]
    UnsupportedSourceForPlatform { id: String, platform: String },
    #[error("no source candidates are available for {kind} items on {platform}")]
    NoSourceCandidates { kind: String, platform: String },
    #[error("source install planning for {kind} {name}@{version} from {source_id} is not implemented yet")]
    SourceInstallNotImplemented {
        source_id: String,
        kind: String,
        name: String,
        version: String,
    },
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
    #[error("{feature} is not supported on {platform}")]
    UnsupportedPlatform { feature: String, platform: String },
    #[error("{message}")]
    Conflict { message: String },
    #[error("{message}")]
    Message { message: String },
    #[error("{context}: {source}")]
    Context {
        context: String,
        #[source]
        source: Box<EngineError>,
    },
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    TomlDe(#[from] toml_edit::de::Error),
    #[error("{0}")]
    TomlSer(#[from] toml_edit::ser::Error),
    #[error("{0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("{0}")]
    Semver(#[from] semver::Error),
    #[error("{0}")]
    Archive(String),
}

impl EngineError {
    pub fn message(message: impl Into<String>) -> Self {
        Self::Message {
            message: message.into(),
        }
    }

    pub fn context(self, context: impl Into<String>) -> Self {
        Self::Context {
            context: context.into(),
            source: Box::new(self),
        }
    }
}

impl From<String> for EngineError {
    fn from(message: String) -> Self {
        Self::message(message)
    }
}

impl From<&str> for EngineError {
    fn from(message: &str) -> Self {
        Self::message(message)
    }
}

impl From<Box<dyn std::error::Error>> for EngineError {
    fn from(error: Box<dyn std::error::Error>) -> Self {
        Self::message(error.to_string())
    }
}

impl From<Box<dyn std::error::Error + Send + Sync>> for EngineError {
    fn from(error: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self::message(error.to_string())
    }
}

pub trait EngineContext<T> {
    fn context(self, context: impl fmt::Display) -> EngineResult<T>;
    fn with_context<C, F>(self, context: F) -> EngineResult<T>
    where
        C: fmt::Display,
        F: FnOnce() -> C;
}

impl<T, E> EngineContext<T> for std::result::Result<T, E>
where
    E: Into<EngineError>,
{
    fn context(self, context: impl fmt::Display) -> EngineResult<T> {
        self.map_err(|err| err.into().context(context.to_string()))
    }

    fn with_context<C, F>(self, context: F) -> EngineResult<T>
    where
        C: fmt::Display,
        F: FnOnce() -> C,
    {
        self.map_err(|err| err.into().context(context().to_string()))
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

    #[test]
    fn engine_errors_can_add_display_context() {
        let err = EngineError::message("inner").context("outer");

        assert_eq!(err.to_string(), "outer: inner");
    }
}
