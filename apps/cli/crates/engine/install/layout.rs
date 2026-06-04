//! Install layout paths owned by Still.

use std::path::PathBuf;

use crate::infra::paths::PathOps;
use crate::platform::System;
use crate::specs::item::ItemKind;

/// Root directory for Still-managed install state.
pub fn root_dir() -> PathBuf {
    System::root_dir()
}

/// Directory that receives linked executables.
pub fn bin_dir() -> PathBuf {
    System::bin_dir()
}

/// Root directory for Still install receipts.
pub fn receipts_dir() -> PathBuf {
    root_dir().join("receipts")
}

/// Returns the receipt root for one Still item kind.
pub(crate) fn native_receipt_root(kind: ItemKind) -> PathBuf {
    let folder = match kind {
        ItemKind::Tool => "tools",
        ItemKind::Package => "packages",
        ItemKind::App => "apps",
    };
    receipts_dir().join(folder)
}
