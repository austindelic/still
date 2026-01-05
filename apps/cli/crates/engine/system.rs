use crate::actions::install::InstallOps;
use crate::actions::uninstall::UninstallOps;
use crate::utils::paths::PathOps;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacOS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Linux;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Windows;

pub trait SystemOps: InstallOps + UninstallOps + PathOps {}
impl SystemOps for MacOS {}
impl SystemOps for Linux {}
impl SystemOps for Windows {}

#[cfg(target_os = "macos")]
pub type System = MacOS;

#[cfg(target_os = "linux")]
pub type System = Linux;

#[cfg(target_os = "windows")]
pub type System = Windows;

pub fn init_system() -> System {
    System {}
}
