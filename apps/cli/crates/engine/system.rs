//! Host platform selection and shared system trait composition.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

/// Platform type alias for macOS builds.
#[cfg(target_os = "macos")]
pub type System = macos::MacOS;

/// Platform type alias for Linux builds.
#[cfg(target_os = "linux")]
pub type System = linux::Linux;

/// Platform type alias for Windows builds.
#[cfg(target_os = "windows")]
pub type System = windows::Windows;

/// Constructs the current platform marker.
///
/// The return type is the compile-time `System` alias for the target OS. The
/// marker carries no runtime state; it exists to select trait implementations.
pub fn init_system() -> System {
    System {}
}
