//! Legacy engine module root kept for compatibility with earlier module layouts.

/// Engine actions that mutate or inspect Still-managed state.
pub mod actions;
/// Platform abstractions from older engine layout.
pub mod platform;
/// Typed spec models.
pub mod specs;
/// Shared utility modules.
pub mod utils;
