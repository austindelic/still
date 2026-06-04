//! Platform identities and compile-time selected host adapters.

pub mod host;
pub mod system;

pub use host::*;
pub use system::{System, init_system};
