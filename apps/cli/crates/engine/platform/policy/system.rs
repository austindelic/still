use crate::platform::policy::{install::InstallPolicy, uninstall::UninstallPolicy};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacOS;
pub struct Linux;
pub struct Windows;

trait SystemPolicy: InstallPolicy + UninstallPolicy {}
impl SystemPolicy for MacOS {}
impl SystemPolicy for Linux {}
impl SystemPolicy for Windows {}

pub fn get_system() -> Box<dyn SystemPolicy> {
    #[cfg(target_os = "macos")]
    return Box::new(MacOS);

    #[cfg(target_os = "linux")]
    return Box::new(Linux);

    #[cfg(target_os = "windows")]
    return Box::new(Windows);
}
