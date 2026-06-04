//! Typed spec models used by config parsing and registry integrations.

/// Agent skill normalization models.
pub mod agents;
/// Homebrew formula and cask JSON models.
pub mod brew;
/// Shared item request models.
pub mod item;
/// Still TOML config model.
pub mod toml;
/// User-facing tool spec parser.
pub mod tool;

/// Compatibility path for source defaulting and kind inference during migration.
pub use crate::resolve::infer as backend;
/// Compatibility path for source selection models during migration.
pub use crate::resolve::source;
