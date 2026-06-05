# Still
             .    o8o  oooo  oooo
           .o8    `"'  `888  `888
 .oooo.o .o888oo oooo   888   888
d88(  "8   888   `888   888   888
`"Y88b.    888    888   888   888
o.  )88b   888 .  888   888   888
8""888P'   "888" o888o o888o o888o

**Still is a config-first project environment manager for the whole machine
state your project actually depends on.**

Tools, system packages, desktop apps, env files, services, tasks, and agent
setup all belong in one reviewed project file, not scattered through shell
history and setup notes.

```bash
still init
still install --tool cargo:ripgrep@14.1.1 --package homebrew:openssl --app flatpak:org.mozilla.firefox
still sync
still run cargo test
still task lint
```

Still is pre-1.0. The command surface, config model, source architecture,
lockfiles, trust model, optional TUI, docs, and companion apps are being shaped
in public in this repo.

Start here:

- [CLI spec](apps/cli/SPEC.md): product behavior, config, commands, and source syntax.
- [CLI design](apps/cli/DESIGN.md): Rust architecture and crate boundaries.
- [Docs content](apps/docs/content/docs): Fumadocs source for the public docs app.
- [Example config](apps/cli/examples/still.toml): complete `still.toml` shape.

## Why Still

Modern project setup is bigger than a language version file.

- A Rust service might need `cargo-nextest`, `openssl`, Docker, env files, and a
  local task graph.
- A desktop project might need native packages, platform apps, shell activation,
  and agent instructions.
- A team might need the same setup on macOS, Linux, and Windows without turning
  onboarding into a ritual.

Still makes that setup explicit:

- `still.toml` is desired state.
- `still.lock.toml` records resolved state.
- source crates resolve metadata and artifacts.
- engine code plans and executes behavior.
- CLI and TUI code stay thin: parse input, call the engine, present results.

No floating installs. No mystery machine state. No "works on my laptop" as the
deployment strategy.

## The Shape

`still.toml` is meant to be plain enough to review in a pull request:

```toml
[tools]
node = "22"
python = "3.12"
go = "latest"

[tools.rust]
version = "stable"
source = "rustup"
components = ["rustfmt", "clippy"]

[packages]
latest = ["ripgrep", "jq", "ffmpeg"]
postgresql = { version = "16", source = "auto" }

[packages.fd.names]
macos = "fd"
linux = "fd-find"
windows = "fd"

[apps]
latest = ["zed", "firefox"]
visual-studio-code = { source = "auto" }

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
stop = { task = "docker:stop" }
check = { task = "docker:check" }

[agents]
targets = ["claude", "codex"]
instructions = "AGENTS.md"
```

## Source Syntax

Still uses **sources** for resolution and install planning. A source knows how
to read ecosystem metadata and, where implemented, install through Still-owned
layouts instead of blindly wrapping package manager commands.

Install specs use:

```text
source:name@version
```

Examples:

```bash
still install --tool cargo:ripgrep@14.1.1
still install --tool --cargo ripgrep@14.1.1 cargo-nextest
still install --package homebrew:openssl
still install --app flatpak:org.mozilla.firefox
```

Rules:

- `source:name` means `source:name@latest`.
- `name` means source auto-selection and `latest`.
- empty or omitted versions normalize to `latest`.
- explicit sources never silently fall back.
- `--tool`, `--package`, and `--app` classify item role.
- `--cargo`, `--homebrew`, `--apt`, and similar flags choose the source for
  following specs.

## Command Map

| Command              | Purpose                                                                               |
| -------------------- | ------------------------------------------------------------------------------------- |
| `still init`         | Create a starter `still.toml` and trust the generated config.                         |
| `still install`      | Install requested tools/packages/apps and record successful requests in config.       |
| `still sync`         | Reconcile installed state from config and refresh the lockfile.                       |
| `still list`         | Show configured and Still-managed installed items.                                    |
| `still uninstall`    | Remove desired state and Still-managed artifacts.                                     |
| `still run`          | Run a child command with Still-managed `PATH` and env.                                |
| `still task`         | Run or list configured tasks.                                                         |
| `still services`     | Inspect, start, stop, or check configured services.                                   |
| `still agents`       | Inspect, sync, or validate agent instructions and skills.                             |
| `still config check` | Validate config through typed parser and engine validators.                           |
| `still doctor`       | Diagnose machine, cache, config, permissions, source availability, and platform state. |
| `still env`          | Print resolved environment information.                                               |
| `still activate`     | Print shell activation code.                                                          |
| `still trust`        | Trust project-defined executable behavior after review.                               |

The default build prints help when run with no command. With `--features tui`,
the same binary includes the optional terminal UI and opens it when no command
is supplied.

## Trust Model

Still can read config by default, but project-defined executable behavior is
trust-sensitive.

Trust gates:

- env files
- tasks
- services
- external agent skills
- project-defined commands that can execute shell code

Trust is stored in `.still/trust.toml` and scoped to the config path and content
fingerprint. If `still.toml` changes, review it and run:

```bash
still trust
```

## Architecture

```text
apps/cli
  src/cli          Clap, route_command, EngineSession, presentation, terminal UI
  crates/engine   config, desired state, resolve, planning, install, inventory
  crates/source-kit
                  shared source installer machinery
  crates/sources  source-specific metadata, capabilities, and install rules
  crates/tui      optional terminal UI library

apps/docs         Next/Fumadocs documentation app
apps/web          SvelteKit companion web app
packages/*        shared workspace packages
```

The engine owns product behavior. CLI and TUI code own input/output. Source
crates own ecosystem-specific knowledge. Platform-specific behavior is selected
with `cfg(target_os)` so future builds can compile out code for other operating
systems.

## Development

Run JavaScript and TypeScript workspace commands from the repo root:

```bash
bun install
bun run build
bun run check
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
cargo run -p still --features tui --bin still
```

Run docs commands from the repo root:

```bash
bun run --filter docs check
bun run --filter docs types:check
bun run --filter docs build
```

## Status

Still is intentionally early. Some source crates are scaffolds while the engine
pipeline, source-kit boundary, and manual installer model settle. The docs keep
the intended contract visible without pretending every source implementation is
finished.

The north star is simple: clone a project, review `still.toml`, trust it, and
let Still make the machine match the repo.
