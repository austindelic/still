# Agent Instructions

## App Overview

`apps/cli` contains the Rust workspace for the `still` binary, reusable engine,
source crates, and optional TUI.

Read these first when making non-trivial changes:

- `README.md`: product overview and user-facing direction.
- `SPEC.md`: product behavior, command contract, config shape, and source syntax.
- `DESIGN.md`: code architecture, crate boundaries, and implementation rules.

Keep this file focused on workflow guidance for agents. Public product goals
belong in `README.md` or `SPEC.md`; implementation architecture belongs in
`DESIGN.md`.

Every non-trivial code change must update `README.md`, `SPEC.md`, and/or
`DESIGN.md` when it changes product behavior, command syntax, crate boundaries,
architecture, dependency policy, generated files, or test expectations.

## Layout

- `src/main.rs`: binary entrypoint. Keep it thin; it should declare the root
  `cli` module and delegate to `cli::entry()`.
- `src/cli`: Clap args, CLI context/session, command routing, output
  formatting, Tokio runtime boundary, miette diagnostics, indicatif progress,
  and CLI command handlers.
- `crates/engine`: core config, desired state, source selection, planning,
  lockfile, inventory, trust, platform traits, and typed errors/results.
- `crates/source-kit`: shared source installer machinery such as download,
  cache, checksums, archive extraction, staging, receipts, and executable
  discovery.
- `crates/sources/*`: source-specific metadata parsing, resolution, artifact
  selection, dependency interpretation, and install rules.
- `crates/tui`: optional TUI library compiled through the root `tui` feature.
- `examples`: concept config and schema files.

## Architecture Rules

- Start command spelling/input changes in `src/cli/args.rs`.
- Keep CLI runtime code in the requested shape: `context.rs` has `CliContext`,
  `session.rs` has `EngineSession`, `route.rs` has `route_command(...)`,
  `present.rs` formats typed results, and `ui.rs` is the terminal facade.
- Use `mod.rs` only for small module glue and re-exports. Do not let it become
  a dumping ground for behavior.
- Put CLI presentation, stdout/stderr formatting, Clap behavior, miette
  diagnostics, indicatif progress, and future anstream/anstyle styling in
  `src/cli`.
- Put real install, source selection, resolve, cache, filesystem, lockfile,
  inventory, trust, and platform behavior in `crates/engine`,
  `crates/source-kit`, or `crates/sources/*`.
- Keep engine modules free of Clap, TUI state, progress bar types, and
  user-facing formatting.
- Keep the root `src/cli` responsible for feature-gated TUI launch so `engine`
  does not depend on `still_tui`.
- Keep `crates/tui` free of CLI parsing and command dispatch.
- The default `still` build is CLI-only. With `--features tui`, the same binary
  includes TUI dependencies and opens the TUI when no subcommand is provided.
- Do not add a separate `still_tui` binary unless the product direction changes.
- Use `source` terminology for new design/code. Older source-adapter naming is
  migration debt unless touching compatibility code.
- Use `toml` plus `serde` for owned/generated TOML files such as lockfiles,
  trust markers, install markers, and generated metadata. Use `toml_edit` only
  when editing existing user-authored TOML and preserving comments/order.
- Keep direct external dependency versions in `[workspace.dependencies]` when
  possible. Use the latest crates.io versions compatible with the workspace
  Rust version, and document any MSRV exception.

## Current CLI Decisions

- No subcommand prints help in the default build.
- No subcommand opens the TUI in `--features tui` builds.
- CLI subcommands should behave the same with or without the TUI feature.
- `install` installs immediately and records requested items in config.
- Config mutation should be explicit. In v0.1, `install` and `uninstall` are the
  supported mutation flows.
- `sync` reconciles local installed state from `still.toml`.
- `config check` is the typed config validation path.
- `doctor` diagnoses local machine and project health.
- Project-defined executable behavior should be gated by trust.
- `auto` is source selection, not a source.

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

- Put Rust unit tests in the same file as the code being tested using
  `#[cfg(test)] mod tests`.
- Put CLI contract tests beside the CLI modules they exercise unless a future
  integration test folder is intentionally reintroduced.
- For engine/source behavior, prefer focused tests that avoid terminal/UI
  concerns and real package managers.
- Platform planner tests should inject fake platforms instead of relying on the
  developer's host OS.

## Comments

- Before adding, rewriting, or auditing comments, read and follow the project
  skill at `.agents/skills/code-comments/SKILL.md`.
- Rustdoc powers IDE hover and `cargo doc`; keep public API docs compact,
  caller-oriented, and synchronized with Clap help where doc text affects
  generated CLI output.

## Sources

Initial source families are:

- `homebrew`
- `cargo`
- `npm`
- `pipx`
- `go`
- `aqua`
- `apt`
- `dnf`
- `pacman`
- `winget`
- `flatpak`

Source-specific fetching, metadata parsing, resolution, artifact selection, and
install details should be isolated behind common engine/source-kit interfaces.
Config parsing should produce typed requests first; source selection should
happen after parsing and before install planning.
