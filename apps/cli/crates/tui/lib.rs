//! Optional text UI library for the `still` binary.

/// Main application state and terminal loop.
mod app;
/// Shared TUI components.
mod components;
/// TUI tab implementations.
mod tabs;

/// Launches the TUI terminal event loop.
pub use app::launch_tui;
