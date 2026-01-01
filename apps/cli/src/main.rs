mod archive;
mod cli;
mod commands;
mod ghcr;
mod homebrew;
mod install_utils;
mod platform;
mod specs;
mod tui;

fn main() {
    cli::entry()
}
