#[path = "../core/mod.rs"]
mod core;

#[path = "../frontend/mod.rs"]
mod frontend;

#[path = "../sources/mod.rs"]
mod sources;

#[path = "../util/mod.rs"]
mod util;

fn main() {
    frontend::cli::entry()
}
