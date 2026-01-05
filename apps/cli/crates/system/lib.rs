pub mod platform;
pub mod system;
pub mod utils;

pub use platform::Platform;
pub use system::{System, init_system, SystemUtil, MacOS, Linux, Windows};
pub use utils::{fs, hashing, net, install, paths, uninstall};

