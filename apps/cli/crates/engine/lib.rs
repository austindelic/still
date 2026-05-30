//! Core Still engine APIs shared by the CLI and optional TUI.

#![allow(async_fn_in_trait)]

/// Install, uninstall, and other state-changing actions.
pub mod actions;
/// Config discovery and filesystem locations.
pub mod config;
/// Desired-state config mutation helpers.
pub mod config_edit;
/// Typed engine errors.
pub mod error;
/// Lockfile rendering helpers.
pub mod lockfile;
/// Side-effect-free planning models.
pub mod planner;
/// Platform identifiers and filters.
pub mod platform;
/// Package registry integrations and re-exports.
pub mod registries;
/// Typed models for external and Still-owned specs.
pub mod specs;
/// Platform abstraction for host-specific behavior.
pub mod system;
/// Filesystem, network, archive, hashing, and path utilities.
pub mod utils;
