use crate::utils::install::InstallUtil;
use crate::utils::paths::PathUtil;
use crate::utils::uninstall::UninstallUtil;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacOS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Linux;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Windows;

pub trait SystemUtil: InstallUtil + UninstallUtil + PathUtil {}
impl SystemUtil for MacOS {}
impl SystemUtil for Linux {}
impl SystemUtil for Windows {}

#[cfg(target_os = "macos")]
pub type System = MacOS;

#[cfg(target_os = "linux")]
pub type System = Linux;

#[cfg(target_os = "windows")]
pub type System = Windows;

pub fn init_system() -> System {
    System {}
}
