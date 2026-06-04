# Still CLI

`apps/cli` contains the Rust implementation of the `still` binary, the reusable
engine, source crates, and the optional TUI library.

Still is a source-based, multi-OS project environment manager. It reads
`still.toml`, resolves tools/packages/apps from source metadata, installs into
Still-managed layouts where practical, and runs commands inside that managed
environment.

Use these docs together:

- `SPEC.md`: product behavior, command contract, config shape, and source syntax.
- `DESIGN.md`: Rust code architecture, crate boundaries, and implementation rules.
- `AGENTS.md`: workflow guidance for humans and agents working in this workspace.

## Product Model

`still.toml` describes project state:

- `tools`: language runtimes, toolchains, and command-line developer tools.
- `packages`: system packages, libraries, and command-line packages.
- `apps`: desktop or platform apps.
- `env`: environment variables and env files.
- `services`: long-running dependencies needed while the project is active.
- `tasks`: named commands and task graphs.
- `agents`: AI-agent instructions, targets, and skills.

The core loop is:

```bash
still init
still install --tool cargo:ripgrep@14.1.1 --package homebrew:openssl --app flatpak:org.mozilla.firefox
still run cargo test
still task lint
```

Source flags can select a source for several following specs:

```bash
still install --tool --cargo ripgrep@14.1.1 cargo-nextest
still install --package --apt openssl curl
```

## Source Syntax

Still uses sources rather than shelling out to package managers as the primary
model. A source knows how to read ecosystem metadata and install manually through
Still's shared installer machinery.

Install specs use:

```text
source:name@version
```

Rules:

- `source:name` means `source:name@latest`.
- `source:name@version` pins a version for that source.
- `name` means source auto-selection and `latest`.
- `name@version` means source auto-selection with a pinned version.
- Empty or omitted versions normalize to `latest`.
- Explicit sources never fall back to another source.
- Source flags such as `--cargo` or `--homebrew` select a source; item-kind flags
  such as `--tool`, `--package`, and `--app` still select the item role.

## Command Goals

- `init`: create a starter `still.toml`, infer project defaults, and trust the newly created config.
- `trust`: mark the current project/config as trusted before executing project-defined behavior.
- `install`: install requested tools/packages/apps now, add them to config, and refresh the lockfile.
- `sync`: read config, resolve desired state, update the lockfile, install missing items, and report drift.
- `uninstall`: remove an item from desired state and remove related Still-managed artifacts.
- `run <COMMAND...>`: run an arbitrary command with Still-managed PATH/env and return the child exit code.
- `task <NAME>`: run a named task from config; no name lists available tasks.
- `services`: inspect, start, stop, and check configured services.
- `agents`: inspect, sync, and validate configured agent instructions and skills.
- `config check`: validate `still.toml` through typed config validators.
- `doctor`: diagnose machine, cache, config, permissions, source availability, and platform health.
- `env`: print resolved environment/debug information.
- `activate`: print shell-specific activation code or instructions.
- `list`: list active tools/packages/apps and show which config selected each version.
- `list --all`: list known installed and configured items, including inactive project/global entries.
- `--global`: force desired-state commands to read or write the global Still config where supported.

## Source Crates

The intended source workspace shape is:

```text
crates/
  engine/
  source-kit/
  sources/
    homebrew/
    cargo/
    npm/
    pipx/
    go/
    aqua/
    apt/
    dnf/
    pacman/
    winget/
    flatpak/
```

Package names use:

- `still-source-kit`
- `still-source-homebrew`
- `still-source-cargo`
- `still-source-npm`
- and the same pattern for each source.

`still-source-kit` owns shared installer machinery: download/cache, checksums,
archive extraction, staging/promotion, receipts, executable discovery, and
dependency helpers. `still-source-*` crates own source-specific metadata parsing,
resolution, artifact selection, and install rules.

Sources should be feature-gated in the future so users can build lighter
binaries. OS-specific source/platform code should be removed at compile time with
`cfg(target_os)` where possible.

## Trust Model

Still should read config by default, but it should not execute project-defined
behavior until the project is trusted.

Trust should gate:

- tasks
- services
- agents and external skills
- env files
- project-defined commands that can execute arbitrary shell code

Source package metadata may run ecosystem-defined install behavior in future
manual installers. That policy belongs in the engine/source layer and must be
clear in diagnostics.

## Build And Test

Run from `apps/cli`:

```bash
cargo check -p still
cargo test -p still
cargo check -p still --features tui
cargo test -p still --features tui
cargo test --workspace
```

Run locally:

```bash
cargo run -p still --bin still -- --help
cargo run -p still --features tui --bin still -- --help
cargo run -p still --features tui --bin still
```
