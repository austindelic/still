use dirs::home_dir;

use crate::system::MacOS;
use std::path::PathBuf;

pub trait PathOps {
    fn root_dir() -> PathBuf;
    fn cache_dir() -> PathBuf;
    fn bin_dir() -> PathBuf;
    fn config_dir() -> PathBuf;
    fn config_file() -> PathBuf;
    fn apps_dir() -> PathBuf;
    fn home_dir() -> PathBuf;
    fn tool_dir() -> PathBuf;
}

impl PathOps for MacOS {
    fn root_dir() -> PathBuf {
        PathBuf::from("/opt").join("still")
    }

    fn cache_dir() -> PathBuf {
        dirs::cache_dir().unwrap()
    }

    fn bin_dir() -> PathBuf {
        Self::root_dir().join("bin")
    }
    fn tool_dir() -> PathBuf {
        Self::root_dir().join("tools")
    }

    fn apps_dir() -> PathBuf {
        Self::root_dir().join("apps")
    }

    fn config_dir() -> PathBuf {
        home_dir().unwrap().join(".config").join("still")
    }

    fn config_file() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    fn home_dir() -> PathBuf {
        dirs::home_dir().expect("error fetching home_dir with dirs::home_dir on macos")
    }
}
