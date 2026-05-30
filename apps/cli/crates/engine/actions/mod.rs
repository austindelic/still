//! Engine actions that mutate or inspect Still-managed state.

/// Shell activation action implementation.
pub mod activate;
/// Agent action implementation.
pub mod agents;
/// Config validation actions.
pub mod config;
/// Doctor diagnostic actions.
pub mod doctor;
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
/// Services action implementation.
pub mod services;
/// Sync action implementation.
pub mod sync;
/// Task action implementation.
pub mod task;
/// Trust action implementation.
pub mod trust;
/// Uninstall action implementation.
pub mod uninstall;
