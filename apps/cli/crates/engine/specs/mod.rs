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
