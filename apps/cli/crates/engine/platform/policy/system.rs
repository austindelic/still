use crate::platform::policy::{
    install::InstallPolicy, paths::PathPolicy, uninstall::UninstallPolicy,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacOS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Linux;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Windows;

pub trait SystemPolicy: InstallPolicy + UninstallPolicy + PathPolicy {}
impl SystemPolicy for MacOS {}
impl SystemPolicy for Linux {}
impl SystemPolicy for Windows {}

#[cfg(target_os = "macos")]
pub type System = MacOS;

#[cfg(target_os = "linux")]
pub type System = Linux;

#[cfg(target_os = "windows")]
pub type System = Windows;

pub fn system() -> System {
    System {}
}
