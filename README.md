<p align="center">
  <img src="apps/docs/public/brand/still-mark.svg" width="96" height="96" alt="Still logo" />
</p>

<h1 align="center">Still</h1>

<p align="center">
  A config-first project environment manager for tools, packages, apps, environment variables, services, tasks, and agent setup.
</p>

<p align="center">
  <a href="apps/docs/content/docs/getting-started.mdx">Getting started</a>
  ·
  <a href="apps/docs/content/docs/configuration.mdx">Configuration</a>
  ·
  <a href="apps/docs/content/docs/commands.mdx">Commands</a>
  ·
  <a href="apps/cli/SPEC.md">CLI spec</a>
</p>

```
             .    o8o  oooo  oooo
           .o8    `"'  `888  `888
 .oooo.o .o888oo oooo   888   888
d88(  "8   888   `888   888   888
`"Y88b.    888    888   888   888
o.  )88b   888 .  888   888   888
8""888P'   "888" o888o o888o o888o
```

Still turns project setup into committed desired state. Instead of scattering
runtime versions, system packages, desktop app dependencies, env files, task
graphs, services, and assistant setup across READMEs and local shell history,
Still puts the project environment in `still.toml` and reconciles the machine
from there.

```bash
still init
still install --tool rust@stable@rustup node@22 --package ripgrep jq --app zed
still sync
still task lint
```

Still is early. This repository is actively shaping the Rust CLI, engine,
config model, lockfile behavior, optional TUI, docs, and companion apps. The
product direction is intentionally documented in `apps/cli/SPEC.md`.

## Why Still

Modern projects depend on more than language packages:

- runtimes and toolchains such as Rust, Node, Python, and Go
- system packages and native libraries
- desktop or platform apps
- env files and process environment
- services that must be available while developing
- named tasks and task graphs
- AI-agent instructions and skills

Still models those pieces together and keeps them attached to a project config
or the global Still config. No floating installs. No mystery machine state.

## The Model

`still.toml` is desired state:

```toml
[tools]
node = "22"
python = "3.12"

[tools.rust]
version = "stable"
backend = "rustup"
components = ["rustfmt", "clippy"]

[packages]
latest = ["ripgrep", "jq", "ffmpeg"]

[apps]
latest = ["zed", "firefox"]

[env]
RUST_LOG = "debug"
files = [".env", ".env.local"]

[tasks.lint]
description = "Run lint checks"
run = [
  "cargo fmt --check",
  "cargo clippy --all-targets --all-features",
]

[services.docker]
start = { task = "docker:start" }
check = { task = "docker:check" }

[agents]
targets = ["claude", "codex"]
instructions = "AGENTS.md"
skills = ["rust-review", "repo-auditor"]
```

`still.lock.toml` records resolved state next to the selected config:

- item kind, name, version, backend, and platform
- source identity and desired-state checksum
- expected outputs and linked executables
- active-host entries while preserving other platform entries

## Core Workflow

Initialize a project:

```bash
still init
```

Add desired state and install immediately:

```bash
still install --tool jq ripgrep fd --package openssl llvm --app zed firefox
```

Reconcile from config:

```bash
still sync
```

Run inside the managed environment:

```bash
still run cargo test
still task lint
```

Inspect state and health:

```bash
still list --all
still config check
still doctor
```

## Command Map

| Command              | Purpose                                                                               |
| -------------------- | ------------------------------------------------------------------------------------- |
| `still init`         | Create a starter `still.toml` and trust the newly created config.                     |
| `still install`      | Install tools/packages/apps and add successful requests to config.                    |
| `still sync`         | Resolve desired state, refresh the lockfile, install missing items, and report drift. |
| `still list`         | Show active tools/packages/apps and where each version came from.                     |
| `still uninstall`    | Remove desired state and Still-managed artifacts.                                     |
| `still run`          | Run a child command with Still-managed `PATH` and env.                                |
| `still task`         | Run or list tasks from config.                                                        |
| `still services`     | Inspect, start, stop, or check configured services.                                   |
| `still agents`       | Inspect, sync, or validate agent instructions and skills.                             |
| `still config check` | Validate config through typed parser and engine validators.                           |
| `still doctor`       | Diagnose machine, cache, config, permissions, trust, and platform health.             |
| `still env`          | Print resolved environment information.                                               |
| `still activate`     | Print shell activation code.                                                          |
| `still trust`        | Mark project-defined executable behavior as trusted after review.                     |

## Trust Model

Still can read config by default, but project-defined executable behavior is
trust-sensitive.

Trust gates:

- tasks
- services
- env files
- agents and external skills
- project-defined commands that execute shell code

Trust is stored in `.still/trust.toml` and scoped to the config path and content
fingerprint. If `still.toml` changes, review it and run:

```bash
still trust
```

## Multi-OS By Design

Still plans for `macos`, `linux`, and `windows` from the start. Config can
express platform-specific package names, backend choices, and filters while
keeping the project-level intent portable.

```toml
[packages.fd.names]
macos = "fd"
linux = "fd-find"
windows = "fd"

[packages.llvm]
version = "18"
backend = "auto"
backends = { macos = "homebrew", linux = "apt", windows = "winget" }
```

## Repository Layout

| Path                         | Purpose                                                                      |
| ---------------------------- | ---------------------------------------------------------------------------- |
| `apps/cli`                   | Rust workspace for the `still` binary, engine crate, and optional TUI crate. |
| `apps/docs`                  | Next/Fumadocs documentation site.                                            |
| `apps/web`                   | Companion SvelteKit web app.                                                 |
| `packages/ui`                | Shared React UI package.                                                     |
| `packages/eslint-config`     | Shared ESLint config.                                                        |
| `packages/typescript-config` | Shared TypeScript config.                                                    |

## Development

Run JavaScript/TypeScript workspace commands from the repo root:

```bash
bun install
bun run build
bun run check
```

Run docs commands from `apps/docs`:

```bash
bun run dev
bun run check
bun run types:check
bun run build
```

Run Rust commands from `apps/cli`:

```bash
cargo fmt
cargo check -p still
cargo test -p still
cargo check -p still --features tui
cargo test --workspace
```

Run the CLI locally:

```bash
cd apps/cli
cargo run -p still --bin still -- --help
cargo run -p still --features tui --bin still -- --help
```

## Status

Still is pre-1.0 and the spec is ahead of some implementation details. The
public shape is being made explicit now so the engine, CLI, TUI, docs, and
future backends can converge on the same contract.

Useful starting points:

- `apps/cli/SPEC.md` for the product and command contract
- `apps/cli/examples/still.toml` for a complete example config
- `apps/cli/examples/still.schema.json` for the concept JSON Schema
- `apps/docs/content/docs` for the Fumadocs documentation source
