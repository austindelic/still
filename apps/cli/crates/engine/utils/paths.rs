use crate::system::{Linux, MacOS, Windows};
use std::path::PathBuf;

/// Ops for all paths that still will use across different operating systems
pub trait PathOps {
    /// Root installation directory where still stores its data
    fn root_dir(&self) -> PathBuf;

    /// Cache directory for downloaded files and temporary data
    fn cache_dir(&self) -> PathBuf;

    /// Directory where installed tools are stored
    fn tools_dir(&self) -> PathBuf;

    /// Directory where binaries are installed or symlinked
    fn bin_dir(&self) -> PathBuf;

    /// Configuration directory for still configuration files
    fn config_dir(&self) -> PathBuf;

    /// Data directory for still's persistent data
    fn data_dir(&self) -> PathBuf;
}

// macOS implementation
impl PathOps for MacOS {
    fn root_dir(&self) -> PathBuf {
        PathBuf::from("/opt/still")
    }

    fn cache_dir(&self) -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .join(".cache")
            .join("still")
    }

    fn tools_dir(&self) -> PathBuf {
        PathBuf::from("/opt/still/tools")
    }

    fn bin_dir(&self) -> PathBuf {
        PathBuf::from("/opt/still/bin")
    }

    fn config_dir(&self) -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("~"))
                    .join(".config")
            })
            .join("still")
    }

    fn data_dir(&self) -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("~"))
                    .join(".local")
                    .join("share")
            })
            .join("still")
    }
}

// Linux implementation
impl PathOps for Linux {
    fn root_dir(&self) -> PathBuf {
        PathBuf::from("/opt/still")
    }

    fn cache_dir(&self) -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .join(".cache")
            .join("still")
    }

    fn tools_dir(&self) -> PathBuf {
        PathBuf::from("/opt/still/tools")
    }

    fn bin_dir(&self) -> PathBuf {
        PathBuf::from("/opt/still/bin")
    }

    fn config_dir(&self) -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("~"))
                    .join(".config")
            })
            .join("still")
    }

    fn data_dir(&self) -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("~"))
                    .join(".local")
                    .join("share")
            })
            .join("still")
    }
}

// Windows implementation
impl PathOps for Windows {
    fn root_dir(&self) -> PathBuf {
        PathBuf::from(r"C:\still")
    }

    fn cache_dir(&self) -> PathBuf {
        dirs::cache_dir()
            .unwrap_or_else(|| {
                // Fallback to LOCALAPPDATA if cache_dir is not available
                std::env::var("LOCALAPPDATA")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| PathBuf::from(r"C:\Users\Public"))
            })
            .join("still")
    }

    fn tools_dir(&self) -> PathBuf {
        PathBuf::from(r"C:\still\tools")
    }

    fn bin_dir(&self) -> PathBuf {
        PathBuf::from(r"C:\still\bin")
    }

    fn config_dir(&self) -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| {
                // Fallback to APPDATA if config_dir is not available
                std::env::var("APPDATA")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| PathBuf::from(r"C:\Users\Public"))
            })
            .join("still")
    }

    fn data_dir(&self) -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| {
                // Fallback to LOCALAPPDATA if data_dir is not available
                std::env::var("LOCALAPPDATA")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| PathBuf::from(r"C:\Users\Public"))
            })
            .join("still")
    }
}
