# Still CLI Feature Spec

This is the working product spec for the Rust CLI. It is intentionally ahead of
the implementation so the command shape, config structure, source model, and
feature boundaries can settle before more code is built.

Source material for this version:

- `apps/cli/examples/still.toml`
- `apps/cli/examples/still.schema.json`
- current CLI product notes in `apps/cli/README.md`

## Product Model

Still manages a project environment from `still.toml`.

- `tools`: language runtimes, toolchains, and command-line developer tools.
- `packages`: system packages, libraries, and command-line packages needed by the project.
- `apps`: desktop or platform apps.
- `env`: environment variables and env files.
- `services`: long-running dependencies required while the project is active.
- `tasks`: named commands and task graphs.
- `agents`: AI-agent instructions, targets, and skills for project-aware assistance.

The main workflow is:

```bash
still init
still install --tool cargo:ripgrep@14.1.1 --package homebrew:openssl --app flatpak:org.mozilla.firefox
still run cargo test
still task lint
```

`still init` creates `still.toml` and should trust that newly created file as
part of the init flow. Install and other config-changing commands should
automatically reconcile requested state, so users should not need to run
`still sync` in the common path. `sync` remains available for explicit full
reconciliation.

## Principles

- Multi-OS first: macOS, Linux, and Windows are first-class targets.
- Config should be portable across machines. Platform-specific differences
  belong in platform filters, platform names, and source selection.
- CLI and TUI code parse, format, and dispatch. Engine code owns source
  selection, install planning, filesystem behavior, service behavior, and source
  execution.
- Source crates read raw ecosystem metadata and install manually through Still,
  rather than primarily shelling out to package managers.
- Desired state should be typed before planning.
- No floating installs are allowed. Every installed tool, package, or app must be
  attached to either a project config or the global config.

## Config Resolution

`still.toml` is the desired-state file. Commands that mutate desired state should
write to config explicitly and predictably.

- Project config is the nearest `still.toml` found by walking upward from the current directory.
- Global config lives at `~/.config/still/config.toml`.
- `-g` / `--global` forces commands that read or write desired state to use the global config.
- If no project config exists, commands that add desired state may fall back to the global config.
- Project config overrides global config for active project state.
- `install` installs immediately and records successful requested items in config.
- `sync` reconciles installed state from config.
- Config-changing install and uninstall flows refresh the lockfile next to the selected config.
- Config writes should preserve unrelated sections and existing user formatting as much as practical.
- `still.toml` is user-authored TOML and should preserve comments/order during
  edits. Generated files such as lockfiles, trust markers, install markers, and
  agent metadata are Still-owned typed TOML.
- Duplicate entries should not be added.
- Creating a local project config is explicit through `still init`; install-style add commands do not silently create `still.toml`.

## Platform Model

Still supports these platform identifiers:

- `macos`
- `linux`
- `windows`

The active host platform is detected by the engine. Config entries decide whether
they apply to the host through `platforms`, `ignore`, `only`, and
platform-specific `names`.

```toml
[packages.fd.names]
macos = "fd"
linux = "fd-find"
windows = "fd"
```

Rules:

- `platforms` is an allow-list for supported platforms.
- `ignore` excludes a platform.
- `only` restricts an entry to one platform.
- `only` replaces the allow-list when present, then `ignore` is applied as a deny-list.
- `names` maps a portable Still name to platform/source-specific names.
- `sources` maps platform names to platform-specific source choices, with scalar `source` as the fallback.
- Platform-specific install paths, executable linking, app registration, service management, and shell activation belong in the engine/platform layer.
- Still uses one multi-platform-capable `still.lock.toml` next to the selected config. Each lockfile item records its resolved platform.

## Source Model

The user-facing type (`tool`, `package`, `app`) is not the same as the source.

- Type answers what role the item plays in the project.
- Source answers where Still resolves metadata and artifacts from.
- `auto` is source selection, not a source implementation.
- Sources are type-specific by capability: a source may support tools, packages,
  apps, or a combination.
- Explicit source selection is exact and never falls back.
- Missing source or `auto` uses ordered source candidates filtered by compiled
  features, current OS, and item kind.

Initial source families:

- `homebrew`: Homebrew formulae and casks from raw Homebrew metadata.
- `cargo`: Rust crates and crate metadata.
- `npm`: npm-compatible package metadata.
- `pipx`: Python CLI package metadata and venv-style installs.
- `go`: Go module metadata and binary installs.
- `aqua`: Aqua registry/package metadata.
- `apt`, `dnf`, `pacman`: Linux package metadata, phased after simpler manual installers.
- `winget`: Windows package metadata, phased after simpler manual installers.
- `flatpak`: Flatpak app metadata.

Initial auto source candidates:

- Tools:
  - macOS: `aqua`, `homebrew`, `cargo`, `npm`, `pipx`, `go`
  - Linux: `aqua`, `cargo`, `npm`, `pipx`, `go`
  - Windows: `aqua`, `cargo`, `npm`, `pipx`, `go`
- Packages:
  - macOS: `homebrew`
  - Linux: `apt`, `dnf`, `pacman`
  - Windows: `winget`
- Apps:
  - macOS: `homebrew`
  - Linux: `flatpak`
  - Windows: `winget`

These names are product direction, not a promise that every source is
implemented today.

## Install Syntax

`install` supports explicit type groups:

```bash
still install --tool cargo:ripgrep@14.1.1 aqua:fd --package homebrew:openssl --app flatpak:org.mozilla.firefox
```

Request syntax:

- `source:name` means `source:name@latest`.
- `source:name@version` pins a version for that source.
- `name` means source auto-selection and `latest`.
- `name@version` means source auto-selection with a pinned version.
- Empty versions normalize to `latest`.

Examples:

```bash
still install --tool cargo:ripgrep@14.1.1
still install --tool cargo:ripgrep
still install --tool ripgrep@14.1.1
still install --package homebrew:openssl
still install --app flatpak:org.mozilla.firefox
```

Source flags select a source for following specs:

```bash
still install --tool --cargo ripgrep@14.1.1 cargo-nextest
still install --package --apt openssl curl
```

Rules:

- Explicit `--tool`, `--package`, and `--app` flags classify following values until another kind flag appears.
- Source flags such as `--cargo`, `--homebrew`, and `--apt` select the source for following values until another source flag appears.
- Explicit `source:name@version` overrides the active source flag for that item.
- Source flags do not change item kind.
- Unclassified positional installs are accepted only when Still can infer the type from an obvious type-specific source or context.
- If inference is ambiguous, return an error suggesting `--tool`, `--package`, or `--app`.
- Install all requested items first; write config once after every install succeeds.
- If any install fails, leave config unchanged and report which item failed.
- Partial successful installs are not recorded by default; there is no `--partial` behavior in the v0.1 contract.

Config write rules:

- Latest tools write as `name = "latest"` under `[tools]` when no explicit source is selected.
- Source-specific or pinned tools write as `[tools.name]` with `source`.
- Latest packages/apps can append to `[packages].latest` and `[apps].latest` when no explicit source is selected.
- Source-specific or pinned packages/apps write as keyed entries.
- Repeated entries should be a no-op unless the requested version/source changes.
- Changed existing entries require `--force`.
- Install refreshes the lockfile after successful config writes.

## Config Surface

The config supports these top-level sections:

```toml
[tools]
[env]
[packages]
[apps]
[services]
[tasks]
[agents]
```

### Tools

Tools are language runtimes, toolchains, and developer tools.

```toml
[tools]
node = "22"
go = "latest"

[tools.rust]
version = "stable"
source = "rustup"
sources = { macos = "rustup", linux = "aqua", windows = "rustup" }
components = ["rustfmt", "clippy"]
targets = ["wasm32-unknown-unknown"]
```

Rules:

- A string value is shorthand for `{ version = "<value>" }`.
- Expanded tools must include `version`.
- `source` selects a source such as `cargo`, `npm`, `aqua`, `homebrew`, or `auto`.
- `sources` overrides `source` for specific platforms.
- `components` and `targets` are toolchain-specific extras.

### Packages

Packages are system packages, libraries, or command-line packages used by the
project.

```toml
[packages]
latest = ["jq", "ffmpeg"]
postgresql = { version = "16", source = "auto" }

[packages.llvm]
version = "18"
source = "auto"
sources = { macos = "homebrew", linux = "apt", windows = "winget" }

[packages.fd.names]
macos = "fd"
linux = "fd-find"
windows = "fd"
```

Rules:

- `latest` is an array of package names that should track the latest available version.
- A keyed package may specify `version`, `source`, `sources`, `names`, `platforms`, `ignore`, and `only`.
- `sources` overrides `source` for specific platforms.
- `names` maps Still's logical package name to source/platform-specific names.
- Platform filter fields are normalized before planning.
- Native package ecosystems are phased: Still should read raw metadata and
  install manually where practical, but may use receipts and planned support
  while dependency/script semantics mature.

### Apps

Apps are desktop or platform applications.

```toml
[apps]
latest = ["zed"]
firefox = { source = "flatpak" }
```

Rules:

- Apps use the same metadata shape as packages: `version`, `source`, `sources`, `names`, `platforms`, `ignore`, and `only`.
- Apps are separate from packages because installation, launchers, registration, and uninstall behavior differ.
- Apps participate in install, sync, list, and uninstall.
- Activation is limited to shell/PATH environment setup and does not launch apps in v0.1.

### Environment

```toml
[env]
RUST_LOG = "debug"
NODE_ENV = "development"
files = [".env", ".env.local"]
```

Rules:

- Scalar keys under `[env]` are environment variables.
- `files` lists env files loaded in order.
- Later values override earlier values when the same variable appears multiple times.
- Loading env files is trust-sensitive.
- Missing env files are errors.
- Env values are loaded literally after simple quote stripping.

### Services

```toml
[services]
docker-compose = "docker compose up"
dev-server = { task = "dev-server" }

[services.docker]
start = { task = "docker:start" }
stop = { task = "docker:stop" }
check = { task = "docker:check" }
```

Rules:

- A string service is shorthand for a start command.
- A `{ task = "name" }` service action runs a task.
- A `{ command = "..." }` service action runs a command directly.
- A service must define at least `preset`, `task`, `start`, or `check`.
- Services are explicit in v0.1; Still does not auto-watch or restart stale services by default.
- `sync` prepares tools, packages, apps, agents, and lockfile state only; it does not start services.

### Tasks

```toml
[tasks]
test = "cargo test"
fmt = "cargo fmt"

[tasks.ci]
description = "Run full CI"
depends = ["fmt", "test"]
run = "echo CI passed"
```

Rules:

- A string task is shorthand for `{ run = "<command>" }`.
- Expanded tasks must include `run`.
- `run` may be a string or list of strings.
- `depends` names tasks that must run before the current task.
- `requires` names services that must be configured before the task can run.
- Task execution output is captured per command and returned for CLI/TUI formatting.

### Agents

```toml
[agents]
targets = ["claude", "codex"]
instructions = "AGENTS.md"
skills = ["rust-review", "repo-auditor"]

[agents.skills]
rust-review = { source = "rust-review", auto = true, tools = ["cargo:cargo-nextest@0.9.99"], packages = ["llvm"] }
```

Rules:

- `targets` lists agent integrations Still should prepare.
- `instructions` points to the shared project instruction file.
- `skills` can be either an array or an `[agents.skills]` table.
- Bare skill names resolve as official Still skill names.
- `owner/name` skill names resolve from a GitHub-owned skill source.
- URL strings resolve external skills.
- `auto = true` allows Still to add missing tool/package/app dependencies declared by skill metadata or inline skill config.
- Skill-linked dependencies must become normal desired-state entries.
- Still-managed skills materialize under `.agents/skills/<skill-name>` in v0.1.
- Still must not overwrite unmarked custom skill directories.

Skill dependency specs use the same install syntax:

```toml
[dependencies]
tools = ["cargo:cargo-nextest@0.9.99"]
packages = ["homebrew:llvm"]
apps = ["flatpak:org.mozilla.firefox"]
```

## Command Catalog

- `init`: create a starter `still.toml`, infer project defaults, and trust the newly created file.
- `install`: install requested tools/packages/apps immediately and add them to config.
- `sync`: read config, resolve desired state, update the lockfile, install missing items, and report drift.
- `list`: show active tools/packages/apps and where each version came from.
- `list --all`: show known installed and configured items, including inactive project/global entries.
- `uninstall`: remove an item from desired state, remove Still-managed artifacts and links when present, and refresh the lockfile.
- `run`: run a command with Still-managed PATH/env and return the child exit code.
- `task`: run a named task from config; no name lists configured tasks.
- `services`: inspect, start, stop, and check configured services.
- `agents`: inspect, sync, and validate configured agent instructions and skills.
- `config check`: validate config through typed TOML parser and engine validators.
- `doctor`: diagnose machine, cache, config, permissions, source availability, and platform health.
- `env`: print resolved environment/debug information.
- `activate`: print shell-specific activation code or instructions.
- `trust`: mark project-defined executable behavior as trusted.

Experimental commands outside this list are not v0.1 priorities unless they get
a concrete product role and documented behavior.

## Future-Proof Implementation Shape

- Config parsing produces typed desired state for tools, packages, apps, env, services, tasks, and agents.
- Resolution maps desired state to platform-specific install plans.
- Source adapters implement metadata resolution, artifact selection, install, uninstall, status, and drift behavior behind typed traits.
- Platform adapters own filesystem paths, executable linking, app registration, shell activation, and service process behavior.
- CLI and TUI layers should not know source internals.
- Lockfiles should record resolved source, platform, version, artifact identity, checksums, outputs, and linked executables.
- Tests should cover parser/planner/source behavior without requiring real package managers whenever possible.

## Trust And Safety

Still should read config by default, but project-defined executable behavior
should require trust.

Trust should gate:

- tasks
- services
- agents and external skills
- env files
- project-defined commands that can execute arbitrary shell code

Installing public artifacts from configured sources is not the same as running
project-defined shell code, but source install scripts and external agent skills
may need trust policy.

- Trust is stored in a project-local `.still/trust.toml` marker scoped to the config path and content fingerprint.
- `still init` writes a Still-managed trust marker for the newly created config.
- Still does not define or execute project-authored package post-install scripts in v0.1.
- CLI and TUI entry points must route trust-sensitive behavior through engine actions so policy stays consistent.
