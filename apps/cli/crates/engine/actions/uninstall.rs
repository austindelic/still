//! Engine uninstall action traits.

use crate::system::MacOS;

/// Platform-specific uninstall operations.
pub trait UninstallOps {}

// macOS currently uses the marker trait until uninstall behavior is implemented.
impl UninstallOps for MacOS {}
