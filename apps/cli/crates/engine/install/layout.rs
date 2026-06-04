//! Install layout paths owned by Still.

use std::path::PathBuf;

use crate::actions::install::InstallItemRequest;
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

/// Returns the final install directory for a command-backed tool.
pub(crate) fn tool_install_path(item: &InstallItemRequest) -> PathBuf {
    System::tool_dir()
        .join(&item.spec.name)
        .join(item.spec.version.as_str())
}

/// Returns the staging directory used before promoting a tool install.
pub(crate) fn tool_staging_path(item: &InstallItemRequest) -> PathBuf {
    System::tool_dir()
        .join(&item.spec.name)
        .join(format!(".{}.staging", item.spec.version.as_str()))
}

/// Returns the temporary backup path used while replacing a tool install.
pub(crate) fn tool_backup_path(item: &InstallItemRequest) -> PathBuf {
    System::tool_dir()
        .join(&item.spec.name)
        .join(format!(".{}.previous", item.spec.version.as_str()))
}

/// Returns the receipt path used for native package/app installs.
pub(crate) fn native_receipt_path(item: &InstallItemRequest) -> PathBuf {
    native_receipt_root(item.kind)
        .join(&item.spec.name)
        .join(item.spec.version.as_str())
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
