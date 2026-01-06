use dirs::home_dir;

use crate::system::MacOS;
use std::path::PathBuf;

pub trait PathOps {
    fn root_dir(&self) -> PathBuf;
    fn cache_dir(&self) -> PathBuf;
    fn bin_dir(&self) -> PathBuf;
    fn config_dir(&self) -> PathBuf;
    fn config_file(&self) -> PathBuf;
    fn apps_dir(&self) -> PathBuf;
    fn data_dir(&self) -> PathBuf;
    fn home_dir(&self) -> PathBuf;
}

impl PathOps for MacOS {
    fn root_dir(&self) -> PathBuf {
        PathBuf::from("/opt").join("still")
    }

    fn cache_dir(&self) -> PathBuf {
        dirs::cache_dir().unwrap()
    }

    fn bin_dir(&self) -> PathBuf {
        self.root_dir().join("bin")
    }

    fn apps_dir(&self) -> PathBuf {
        self.root_dir().join("apps")
    }

    fn config_dir(&self) -> PathBuf {
        home_dir().unwrap().join(".config")
    }

    fn config_file(&self) -> PathBuf {
        self.config_dir().join("config.toml")
    }

    fn home_dir(&self) -> PathBuf {
        dirs::home_dir().expect("error fetching home_dir with dirs::home_dir on macos")
    }

    fn data_dir(&self) -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| {
                dirs::home_dir()
                    .expect("$HOME not set")
                    .join(".local")
                    .join("share")
            })
            .join("still")
    }
}
