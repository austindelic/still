# Still CLI

`apps/cli` contains the Rust implementation of the `still` binary, the install engine, and the optional TUI library.

Still is a config-first, multi-OS project environment manager. The CLI should make local state match `still.toml`, then run commands and tasks inside that managed environment.

See `SPEC.md` for the working CLI feature spec and open product decisions. See
`DESIGN.md` for the intended Rust code structure and architecture boundaries.

## Product Model

`still.toml` describes project state:

- `tools`: language runtimes and toolchains such as Node, Python, Go, and Rust.
- `packages`: command-line/system packages.
- `apps`: desktop or platform apps.
- `env`: environment variables and env files.
- `services`: long-running dependencies needed while the project is active.
- `tasks`: named commands and task graphs.
- `agents`: AI-agent instructions, targets, and skills.

The core loop is:

```bash
still init
still install --tool jq ripgrep fd --package openssl llvm --app zed firefox
still run cargo test
still task lint
```

## Command Goals

- `init`: create a starter `still.toml`, infer project defaults, and trust the newly created config.
- `trust`: mark the current project/config as trusted before executing project-defined behavior.
- `sync`: read `still.toml`, resolve desired state, update the lockfile, install missing items, and report drift.
- `install`: install requested tools/packages/apps now, add them to config, and refresh the lockfile.
- `uninstall`: remove a tool, package, or app from desired state and remove related Still-managed artifacts.
- `run <COMMAND...>`: run an arbitrary command with Still-managed PATH/env and return the child exit code.
- `task <NAME>`: run a named task from config; no name should list available tasks.
- `config check`: validate `still.toml` through the typed TOML parser and engine config validators.
- `doctor`: diagnose machine, cache, config, permissions, and platform health.
- `env`: print resolved environment/debug information.
- `activate`: print shell-specific activation code or instructions.
- `list`: list active tools/packages/apps and show which config selected each version.
- `list --all`: list all known installed and configured items, including inactive project/global entries.

## Trust Model

Still should read config by default, but it should not execute project-defined behavior until the project is trusted.

Trust should gate:

- tasks
- services
- agents and external skills
- env files
- project-defined commands that can execute arbitrary shell code

A future setting may relax this, but it should be strongly discouraged.

## Config Direction

Use `examples/still.toml` and `examples/still.schema.json` as concept references while the engine parser and validators remain the authoritative config check path.

Important decisions:

- `auto` is backend selection, not a backend.
- Config mutation should be explicit through commands like `install`, `use`, `add`, or `config set`.
- `install` installs immediately and records requested items in config; `sync` reconciles from config.
- `config check` validates config; `doctor` diagnoses the environment.

## V1 Backends

V1 backend/provider families:

- `core` / `native`: Still-managed first-party installers and built-in platform behavior.
- `github`: GitHub releases, tags, and release assets.
- `http`: Direct URL downloads with checksum verification.
- `cargo`: Rust crates installed through Cargo.
- `go`: Go tools/modules installed through the Go toolchain.
- `npm`: Node packages installed through npm-compatible package metadata.
- `pipx`: Python CLI tools installed through pipx.
- `asdf`: Existing asdf plugin ecosystem.
- `aqua`: Aqua registry/package ecosystem.

Backend-specific fetching, resolution, and install details should live behind common engine interfaces. Config parsing should produce typed requests first; backend selection happens after parsing and before install planning.

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
