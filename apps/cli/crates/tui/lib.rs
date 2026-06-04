//! Optional text UI library for the `still` binary.

/// Main application state and terminal loop.
mod app;
/// Shared TUI components.
mod components;
/// Terminal launch and restore boundary.
mod launch;
/// TUI-owned engine session.
mod session;
/// TUI tab implementations.
mod tabs;

/// Launches the TUI terminal event loop.
pub use launch::launch_tui;
