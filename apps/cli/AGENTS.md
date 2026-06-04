# Agent Instructions

## App Overview

`apps/cli` contains the Rust workspace for the `still` binary. CLI parsing and presentation live in the root package, core behavior lives in `crates/engine`, and the optional TUI lives in `crates/tui`.

Public product goals for the CLI belong in `apps/cli/README.md`. Keep this file focused on implementation guidance for agents.

## Layout

- `src/main.rs`: Binary entrypoint. Keep it thin; it should delegate to `still::cli::entry()`.
- `src/lib.rs`: Exposes the root `cli` module.
- `src/cli`: Clap args, command routing, output formatting, runtime boundary, and CLI command handlers.
- `crates/engine`: Core install, registry, spec, archive, filesystem, network, hashing, path, and platform logic.
- `crates/tui`: Optional TUI library compiled through the root `tui` feature.
- `examples`: Concept config and schema files.
- `tests`: App-level CLI contract and integration tests.

## Architecture Rules

- Start command spelling/input changes in `src/cli/args.rs`.
- Put CLI presentation, stdout/stderr formatting, and Clap-specific behavior in `src/cli`.
- Put real install, resolve, cache, filesystem, backend, and platform behavior in `crates/engine`.
- Keep `crates/engine` free of Clap, TUI state, and user-facing formatting.
- Keep `crates/tui` free of CLI parsing and command dispatch.
- The default `still` build is CLI-only. With `--features tui`, the same binary includes TUI dependencies and opens the TUI when no subcommand is provided.
- Do not add a separate `still_tui` binary unless the product direction changes again.

## Current CLI Decisions

- No subcommand prints help in the default build.
- No subcommand opens the TUI in `--features tui` builds.
- CLI subcommands should behave the same with or without the TUI feature.
- `install` installs immediately and records requested items in config.
- Config mutation should be explicit. In v0.1, `install` and `uninstall` are the supported mutation flows.
- `sync` reconciles local installed state from `still.toml`.
- `config check` is the schema/Taplo validation path.
- `doctor` diagnoses local machine and project health.
- Project-defined executable behavior should be gated by trust.
- `auto` is backend selection, not a backend.

## Commands

Run these from `apps/cli`:

- `cargo fmt`: Format Rust code.
- `cargo check -p still`: Check the default CLI build.
- `cargo test -p still`: Test the default CLI build.
- `cargo check -p still --features tui`: Check the TUI-enabled build.
- `cargo test -p still --features tui`: Test the TUI-enabled build.
- `cargo test --workspace`: Run all Rust workspace tests.
- `cargo run -p still --bin still -- --help`: Run CLI help locally.
- `cargo run -p still --features tui --bin still -- --help`: Run TUI-enabled CLI help locally.
- `cargo run -p still --features tui --bin still`: Open the TUI.

## Tests

- Put Rust unit tests in the same file as the code being tested using `#[cfg(test)] mod tests`.
- Put integration tests in the nearest `tests/*.rs` folder when they test public behavior from outside the crate or span multiple modules.
- For CLI contract changes, update `tests/cli_help.rs` and any command-level tests.
- For engine behavior, prefer focused engine tests that avoid terminal/UI concerns.

## Comments

- Before adding, rewriting, or auditing comments, read and follow the project skill at `.agents/skills/code-comments/SKILL.md`.
- Rustdoc powers IDE hover and `cargo doc`; keep public API docs compact, caller-oriented, and synchronized with Clap help where doc text affects generated CLI output.

## Backends

V1 backend/provider families are:

- `core` / `native`
- `github`
- `http`
- `cargo`
- `go`
- `npm`
- `pipx`
- `asdf`
- `aqua`

Backend-specific fetching, resolution, and install details should be isolated behind common engine interfaces. Config parsing should produce typed package/tool requests first; backend selection should happen after parsing and before install planning.
