# Agent Instructions

## App Overview

`apps/cli` contains the Rust workspace for the Still CLI. The binary is `still_s`, with shared behavior split into the `engine` and `ui` crates.

## Layout

- `src/main.rs`: CLI binary entrypoint.
- `crates/engine`: Core install, uninstall, use, registry, spec, archive, filesystem, network, hashing, and path logic.
- `crates/ui`: CLI argument parsing, command dispatch, output, and TUI components.
- `examples`: Example Still config and script files.

## Commands

Run these from `apps/cli`:

- `cargo fmt`: Format Rust code.
- `cargo check`: Check the workspace.
- `cargo test`: Run Rust tests.
- `cargo build`: Build the CLI workspace.
- `cargo run --bin still_s -- --help`: Run the CLI help locally.

## Guidance

- Keep engine behavior independent from UI concerns when possible.
- Put command parsing and presentation details in `crates/ui`.
- Put filesystem, registry, package spec, install, and toolchain behavior in `crates/engine`.
- Preserve deterministic install behavior and avoid introducing hidden global state.
- Prefer typed Rust APIs and structured parsing over ad hoc string handling.
- Add focused tests near changed behavior when modifying shared engine logic.

## V1 Backends

Still v1 should support these backend/provider families:

- `core` / `native`: Still-managed first-party installers and built-in platform behavior.
- `github`: GitHub releases, tags, and release assets.
- `http`: Direct URL downloads with checksum verification.
- `cargo`: Rust crates installed through Cargo.
- `go`: Go tools/modules installed through the Go toolchain.
- `npm`: Node packages installed through npm-compatible package metadata.
- `pipx`: Python CLI tools installed through pipx.
- `asdf`: Existing asdf plugin ecosystem.
- `aqua`: Aqua registry/package ecosystem.

When adding backend code, keep backend-specific fetching, resolution, and install details isolated behind a common engine interface. Config parsing should produce typed package/tool requests first; backend selection should happen after parsing and before install planning.
