use crate::system::MacOS;
use std::path::{Path, PathBuf};

pub trait UninstallOps {}

// MacOS implementation
impl UninstallOps for MacOS {}
