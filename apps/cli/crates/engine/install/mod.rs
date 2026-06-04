//! Install orchestration, layout, and receipt helpers.

pub mod layout;

pub use crate::actions::install::{
    InstallAndRecordRequest, InstallItemRequest, InstallRequest, InstallResult,
    InstalledItemResult, ToolInstallOptions, rollback_installed_items, run, run_and_record,
};
pub use layout::*;
