//! Registry integration modules.

/// Homebrew registry integration.
pub mod homebrew;

/// Compatibility re-export for callers that access specs through registries.
pub use crate::specs;
