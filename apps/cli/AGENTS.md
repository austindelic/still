# Agent Instructions

## App Overview

`apps/cli` contains the Rust workspace for the Still CLI. The binary is `still_s`, with shared behavior split into the `engine` and `ui` crates.

## Layout

- `src/main.rs`: CLI binary entrypoint.
- `crates/engine`: Core install, uninstall, use, registry, spec, archive, filesystem, network, hashing, and path logic.
- `crates/ui`: CLI argument parsing, command dispatch, output, and TUI components.
- `examples`: Example Still config and script files.

## CLI Architecture

- `src/main.rs`: Starts the binary and delegates immediately to `ui::cli::entry()`. Keep this file thin.
- `crates/ui/cli/args.rs`: Defines the public CLI contract with Clap. Edit command names, aliases, help text, flags, positional args, and input structs here.
- `crates/ui/cli/commands/mod.rs`: Dispatches parsed commands to command handlers. It owns no-argument behavior, exit codes, and feature-gated TUI dispatch.
- `crates/ui/cli/commands/install.rs`: UI-layer install command handler. It converts `InstallArgs` into an engine request, calls the runtime, and formats user-facing output.
- `crates/ui/cli/runtime.rs`: Boundary between CLI handlers and engine actions. Use `RealRuntime` in production and fake implementations in unit tests.
- `crates/ui/cli/output.rs`: Output abstraction for stdout/stderr. Use `StdOutput` for real CLI output and `BufferedOutput` in tests.
- `crates/engine/actions/*.rs`: Engine actions that perform real work. Keep these free of Clap-specific input and user-interface formatting.
- `crates/ui/tui`: Optional TUI code compiled only with the `tui` Cargo feature. It should consume engine/UI APIs instead of duplicating command behavior.
- `apps/cli/tests`: App-level integration and CLI contract tests, such as help text and feature-gated command visibility.
- `crates/*/tests`: Crate-level integration tests for public behavior spanning multiple modules in that crate.

When changing command spelling or accepted inputs, start in `crates/ui/cli/args.rs`, then update the matching command handler and tests. When changing what a command does after parsing, start in `crates/ui/cli/commands/<command>.rs` or the relevant `crates/engine/actions/*.rs` file.

## CLI Command Spec

This section is the editable product spec for the CLI. Use it to settle command names, inputs, and behavior before changing `crates/ui/cli/args.rs`.

### No Command

Purpose: Default behavior when the user runs `still` with no subcommand.

Current input:

- No args.

Intended behavior:

- CLI-only build: print help.
- TUI-enabled build: open the TUI.

Output and side effects:

- CLI-only build writes help text to stdout.
- TUI-enabled build enters an interactive terminal UI.

Open questions:

- Should no-command behavior always print help, even when TUI is compiled in?
- Should there be a config option that controls default behavior?

### `install <TOOL@VERSION>`

Purpose: Install a tool, package, or app into the current Still-managed environment.

Current input:

- `tool: ToolSpec`
- Format: `name`, `name@latest`, or `name@1.2.3`.

Intended behavior:

- Resolve the requested item.
- Select the backend/provider.
- Install into Still-managed storage.
- Link exposed binaries.

Output and side effects:

- Writes install progress and result output.
- Downloads artifacts as needed.
- Writes files under Still-managed install/cache paths.
- May create or replace symlinks in Still-managed bin paths.

Open questions:

- Should `install` accept multiple tools in one invocation?
- Should `install` infer tools/packages/apps, or should those be separate commands?
- Should version syntax support ranges, channels, or backend-qualified specs?

### `uninstall <TOOL@VERSION>`

Purpose: Remove a tool, package, or app from the current Still-managed environment.

Current input:

- `tool: ToolSpec`
- Format: `name`, `name@latest`, or `name@1.2.3`.

Intended behavior:

- Resolve the installed item.
- Remove its Still-managed install directory.
- Remove related symlinks if they point to the removed install.

Output and side effects:

- Writes uninstall status.
- Deletes Still-managed files and links.

Open questions:

- Should uninstall require an exact version, or default to all installed versions?
- Should uninstall update `still.toml` or only affect local installed state?

### `use --tool-name <TOOL>`

Purpose: Select or activate a runtime/tool version for the current project.

Current input:

- `--tool-name <TOOL>`
- Current struct only accepts a tool name, not a version.

Intended behavior:

- Add or update a tool entry in project config.
- Resolve the selected version during sync.

Output and side effects:

- Should write project config when implemented.
- Should report the selected tool and version.

Open questions:

- Should this accept `TOOL@VERSION` instead of `--tool-name <TOOL>`?
- Should this command be renamed to `add`, `pin`, or `tool use`?
- Should it install immediately or only update config?

### `doctor`

Purpose: Diagnose the local Still environment and report actionable fixes.

Current input:

- No args.

Intended behavior:

- Check config paths, cache paths, install paths, permissions, platform support, and required external tools.
- Report missing or inconsistent state.

Output and side effects:

- Writes diagnostic output.
- Should not mutate by default.

Open questions:

- Should there be `--fix` for safe repairs?
- Should diagnostics output support JSON?

### `run <COMMAND...>`

Purpose: Run a command inside the Still-managed project environment.

Current input:

- `command: Vec<String>`
- Example shape: `still run cargo test`.

Intended behavior:

- Build the project environment from config/lockfile.
- Run the command with Still-managed PATH and env vars.
- Return the child command exit code.

Output and side effects:

- Streams child stdout/stderr.
- May trigger environment resolution if state is stale.

Open questions:

- Should `run` require `--` before the child command?
- Should it auto-sync before running?

### `translate`

Purpose: Translate project definitions between supported formats.

Current input:

- No args.

Intended behavior:

- Convert between Still config and other ecosystem config formats.

Output and side effects:

- TBD.

Open questions:

- Is this distinct from `convert`?
- Which source and destination formats matter first?

### `init`

Purpose: Initialize Still configuration for a project.

Current input:

- No args.

Intended behavior:

- Create a starter `still.toml`.
- Optionally detect common project tools and packages.

Output and side effects:

- Writes project config files.
- Should not overwrite existing config without confirmation or force.

Open questions:

- Should `init` be interactive by default?
- Should it accept templates, such as `--template rust` or `--template node`?

### `convert`

Purpose: Convert configuration or lockfiles to another supported format.

Current input:

- No args.

Intended behavior:

- Convert Still-owned config or lockfile data between versions or formats.

Output and side effects:

- TBD.

Open questions:

- Should this be merged with `translate`?
- Is this for Still schema migrations, external formats, or both?

### `env`

Purpose: Display environment information required for debugging or shell integration.

Current input:

- No args.

Intended behavior:

- Print resolved paths, platform info, active config, cache path, install path, and shell export data.

Output and side effects:

- Writes environment/debug info.
- Should not mutate state.

Open questions:

- Should `env` print human output by default and use `--json` for machines?
- Should shell activation output live here or under `activate`?

### `tui`

Purpose: Launch the text-based user interface.

Current input:

- No args.
- Only compiled when the `tui` Cargo feature is enabled.

Intended behavior:

- Open the interactive TUI.

Output and side effects:

- Enters terminal UI mode.
- May read package cache and project state.

Open questions:

- Should `tui` exist as a subcommand if no-command launches TUI in TUI builds?
- Should TUI be packaged as the same binary feature or separate artifact?

### `web`

Purpose: Open or run a web-based management dashboard.

Current input:

- No args.

Intended behavior:

- TBD.

Output and side effects:

- TBD.

Open questions:

- Is this a local web server, browser opener, or separate app?
- Should this command exist in v1?

### `activate`

Purpose: Activate a workspace or profile for the current shell session.

Current input:

- No args.

Intended behavior:

- Print shell-specific activation code or instructions.
- Configure PATH and environment variables for the current project.

Output and side effects:

- Usually writes shell code to stdout.
- Should not directly mutate the parent shell.

Open questions:

- Should activation be shell-specific via `--shell zsh|bash|fish`?
- Should this overlap with `env`?

### `sync`

Purpose: Synchronize local installed state with configured project state.

Current input:

- No args.

Intended behavior:

- Read `still.toml`.
- Resolve tools, packages, apps, services, and tasks.
- Update lockfile and install missing items.
- Remove or report drift depending on policy.

Output and side effects:

- Reads and writes config/lock/install/cache state.
- Downloads and installs artifacts as needed.

Open questions:

- Should `sync` install everything by default or require explicit groups?
- Should services be started by `sync` or only checked?
- Should destructive changes require confirmation?

### `task`

Purpose: Run or manage tasks defined in `still.toml`.

Current input:

- No args.

Intended behavior:

- List tasks when no task name is provided.
- Run a named task when provided.
- Respect task dependencies and service requirements.

Output and side effects:

- Writes task output.
- Runs configured commands.
- May start/check required services.

Open questions:

- Should task invocation be `still task <name>` or `still run <task>`?
- Should tasks stream raw command output or wrap it with Still status lines?

### `config`

Purpose: Inspect, validate, or edit Still configuration.

Current input:

- No args.

Intended behavior:

- Validate `still.toml`.
- Show resolved config.
- Potentially get/set config values.

Output and side effects:

- Reads project and user config.
- May write config for set/edit subcommands.

Open questions:

- Should validation be `still config check`, `still check`, or `still doctor`?
- Should Taplo/schema validation be exposed here?

### `post-install`

Purpose: Run post-install behavior after an install operation.

Current input:

- No args.

Intended behavior:

- TBD.

Output and side effects:

- TBD.

Open questions:

- Should this be a public command or an internal hook?
- Should post-install behavior be modeled as tasks/hooks instead?

## Commands

Run these from `apps/cli`:

- `cargo fmt`: Format Rust code.
- `cargo check`: Check the default CLI-only workspace.
- `cargo check --features tui`: Check the TUI-enabled workspace.
- `cargo test --workspace`: Run default CLI-only workspace tests.
- `cargo test --workspace --features tui`: Run TUI-enabled workspace tests.
- `cargo build`: Build the CLI workspace.
- `cargo build --features tui`: Build the TUI-enabled CLI workspace.
- `cargo run --bin still_s -- --help`: Run the CLI help locally.
- `cargo run --features tui --bin still_s -- --help`: Confirm the TUI-enabled CLI includes the `tui` command.

## Guidance

- Keep engine behavior independent from UI concerns when possible.
- Put command parsing and presentation details in `crates/ui`.
- Put filesystem, registry, package spec, install, and toolchain behavior in `crates/engine`.
- Preserve deterministic install behavior and avoid introducing hidden global state.
- Prefer typed Rust APIs and structured parsing over ad hoc string handling.
- Add focused tests near changed behavior when modifying shared engine logic.
- The default build is CLI-only. TUI code and dependencies are compiled only with the `tui` Cargo feature.
- Put Rust unit tests in the same file as the code being tested using `#[cfg(test)] mod tests`.
- Put integration tests in the nearest `tests/*.rs` folder when they test public behavior from outside the crate or span multiple modules.

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
