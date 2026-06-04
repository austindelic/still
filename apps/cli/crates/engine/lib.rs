//! Core Still engine APIs shared by the CLI and optional TUI.

#![allow(async_fn_in_trait)]

/// Install, uninstall, and other state-changing actions.
pub mod actions;
/// Public request and result DTOs shared by CLI and TUI callers.
pub mod api;
/// Config discovery, parsing, mutation, and persistence.
pub mod config;
/// Normalized desired state produced from config.
pub mod desired;
/// Typed engine errors.
pub mod error;
/// Filesystem, network, archive, hashing, and process traits/adapters.
pub mod infra;
/// Install orchestration, layout, rollback, and receipt helpers.
pub mod install;
/// Installed-item inventory and receipt discovery.
pub mod inventory;
/// Lockfile rendering helpers.
pub mod lockfile;
/// Side-effect-free planning.
pub mod planning;
/// Platform identifiers, filters, and compile-time selected host behavior.
pub mod platform;
/// Source selection, source registries, and item-kind inference.
pub mod resolve;
/// Synchronous runtime facade for frontend callers.
pub mod runtime;
/// Typed models for external and Still-owned specs.
pub mod specs;
/// Project trust marker verification.
pub mod trust;
