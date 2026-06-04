//! Platform-specific Still path conventions.

use std::path::PathBuf;

/// Path locations required by engine actions on a host platform.
///
/// Implementors should return absolute paths for Still-owned storage and config
/// locations. These methods do not create directories; callers that write to the
/// returned paths must create parents explicitly.
pub trait PathOps {
    /// Root directory for Still-managed files.
    fn root_dir() -> PathBuf;
    /// Cache directory for downloaded registry and archive data.
    fn cache_dir() -> PathBuf;
    /// Directory containing linked executables.
    fn bin_dir() -> PathBuf;
    /// User configuration directory.
    fn config_dir() -> PathBuf;
    /// User configuration file path.
    fn config_file() -> PathBuf;
    /// Directory for Still-managed apps.
    fn apps_dir() -> PathBuf;
    /// Current user's home directory.
    fn home_dir() -> PathBuf;
    /// Directory for Still-managed tools.
    fn tool_dir() -> PathBuf;
}
