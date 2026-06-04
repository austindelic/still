//! Typed spec models used by config parsing and registry integrations.

/// Agent skill normalization models.
pub mod agents;
/// Backend defaulting and normalization models.
pub mod backend;
/// Homebrew formula and cask JSON models.
pub mod brew;
/// Shared item request models.
pub mod item;
/// Source selection and candidate resolution models.
pub mod source;
/// Still TOML config model.
pub mod toml;
/// User-facing tool spec parser.
pub mod tool;
