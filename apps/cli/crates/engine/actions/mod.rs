//! Engine actions that mutate or inspect Still-managed state.

/// Agent action implementation.
pub mod agents;
/// Config validation actions.
pub mod config;
/// Environment inspection actions.
pub mod env;
/// Init action implementation.
pub mod init;
/// Install action implementation.
pub mod install;
/// List action implementation.
pub mod list;
/// Run action implementation.
pub mod run;
/// Task action implementation.
pub mod task;
/// Uninstall action implementation.
pub mod uninstall;
