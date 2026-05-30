//! Host platform selection and shared system trait composition.

use crate::actions::install::InstallOps;
use crate::actions::uninstall::UninstallOps;
use crate::utils::paths::PathOps;

/// macOS host marker used to select platform-specific engine implementations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacOS;

/// Linux host marker used to select platform-specific engine implementations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Linux;

/// Windows host marker used to select platform-specific engine implementations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Windows;

/// Trait bundle for behavior required by the selected host platform.
///
/// Engine actions can bound generic platform code on this trait when they need
/// install, uninstall, and path behavior together.
pub trait SystemOps: InstallOps + UninstallOps + PathOps {}
impl SystemOps for MacOS {}
impl SystemOps for Linux {}
impl SystemOps for Windows {}

/// Platform type alias for macOS builds.
#[cfg(target_os = "macos")]
pub type System = MacOS;

/// Platform type alias for Linux builds.
#[cfg(target_os = "linux")]
pub type System = Linux;

/// Platform type alias for Windows builds.
#[cfg(target_os = "windows")]
pub type System = Windows;

/// Constructs the current platform marker.
///
/// The return type is the compile-time `System` alias for the target OS. The
/// marker carries no runtime state; it exists to select trait implementations.
pub fn init_system() -> System {
    System {}
}
