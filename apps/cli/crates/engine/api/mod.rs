//! Public request and result DTOs shared by frontends.

pub mod install {
    //! Install request and result DTOs.

    pub use crate::actions::install::{
        InstallAndRecordRequest, InstallItemRequest, InstallRequest, InstallResult,
        InstalledItemResult, ToolInstallOptions,
    };
}

pub mod list {
    //! List request and result DTOs.

    pub use crate::actions::list::{ListItem, ListRequest, ListResult, ListSection};
}

pub mod sync {
    //! Sync request and result DTOs.

    pub use crate::actions::sync::{SyncDrift, SyncRequest, SyncResult};
}

pub mod uninstall {
    //! Uninstall request and result DTOs.

    pub use crate::actions::uninstall::{UninstallRequest, UninstallResult, UninstallTarget};
}
