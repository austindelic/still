use std::{future::Future, io, path::PathBuf};

use crate::tabs::packages::{PackageKind, PackageRow};
use engine::{
    api::{
        install::{InstallAndRecordRequest, InstallItemRequest, InstallRequest},
        uninstall::{UninstallRequest, UninstallTarget},
    },
    error::Result,
    specs::item::{ItemKind, ItemSpec},
};

/// TUI-owned session for calling engine actions through one Tokio runtime.
#[derive(Debug)]
pub struct EngineSession {
    context: TuiContext,
    tokio: tokio::runtime::Runtime,
}

impl EngineSession {
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            context: TuiContext::load()?,
            tokio: tokio::runtime::Runtime::new()?,
        })
    }

    pub fn install_package(&mut self, row: &PackageRow) -> io::Result<()> {
        let item = install_item(row)?;
        let context = self.context.clone();
        let request = InstallAndRecordRequest {
            start_dir: context.start_dir,
            home_dir: context.home_dir,
            global: false,
            force: false,
            install: InstallRequest { items: vec![item] },
        };
        self.block_on(engine::actions::install::run_and_record(request))
            .map(|_| ())
            .map_err(io_error)
    }

    pub fn uninstall_package(&mut self, row: &PackageRow) -> io::Result<()> {
        let request = UninstallRequest {
            start_dir: self.context.start_dir.clone(),
            home_dir: self.context.home_dir.clone(),
            global: false,
            target: uninstall_target(row),
        };
        self.block_on(engine::actions::uninstall::run(request))
            .map(|_| ())
            .map_err(io_error)
    }

    fn block_on<T>(&self, future: impl Future<Output = Result<T>>) -> Result<T> {
        self.tokio.block_on(future)
    }
}

#[derive(Debug, Clone)]
struct TuiContext {
    start_dir: PathBuf,
    home_dir: PathBuf,
}

impl TuiContext {
    fn load() -> io::Result<Self> {
        Ok(Self {
            start_dir: std::env::current_dir()?,
            home_dir: home_dir()?,
        })
    }
}

fn install_item(row: &PackageRow) -> io::Result<InstallItemRequest> {
    Ok(InstallItemRequest {
        kind: Some(item_kind(row.kind)),
        spec: row.name.parse::<ItemSpec>().map_err(io_error)?,
        tool: Default::default(),
    })
}

fn uninstall_target(row: &PackageRow) -> UninstallTarget {
    UninstallTarget {
        kind: Some(item_kind(row.kind)),
        name: row.name.clone(),
        version: "latest".to_string(),
        backend: None,
        exact: false,
    }
}

fn item_kind(kind: PackageKind) -> ItemKind {
    match kind {
        PackageKind::Package => ItemKind::Package,
        PackageKind::App => ItemKind::App,
    }
}

fn home_dir() -> io::Result<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "home directory not found"))
}

fn io_error(error: impl std::fmt::Display) -> io::Error {
    io::Error::other(error.to_string())
}
