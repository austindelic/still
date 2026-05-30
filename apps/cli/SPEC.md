# Still CLI Feature Spec

This is the working product and engineering spec for the Rust CLI. It is intentionally ahead of the implementation so the command shape, config structure, and feature boundaries can settle before more code is built.

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
still install --tool jq ripgrep fd --package openssl llvm --app zed firefox
still run cargo test
still task lint
```

`still init` creates `still.toml` and should trust that newly created file as part of the init flow. Install and other config-changing commands should automatically reconcile the requested state, so users should not need to run `still sync` in the common path. `sync` remains available for explicit full reconciliation.

## Design Principles

- Multi-OS first: macOS, Linux, and Windows are first-class targets.
- Config should be portable across machines. Platform-specific differences belong in platform filters, platform names, and backend selection.
- CLI and TUI code should parse, format, and dispatch. Engine code should own resolution, install planning, filesystem behavior, service behavior, and backend execution.
- Desired state should be typed before planning. Avoid stringly command construction where a typed request or backend interface can express the behavior.
- Backends should be replaceable without changing the CLI contract or config shape.
- No floating installs are allowed. Every installed tool, package, or app must be attached to either a project config or the global config.

## Config Resolution

`still.toml` is the desired-state file. Commands that mutate desired state should write to config explicitly and predictably.

- Project config is the nearest `still.toml` found by walking upward from the current directory.
- Global config lives at `~/.config/still/config.toml`.
- `-g` / `--global` forces commands that read or write desired state to use the global config.
- If no project config exists, commands that add desired state may fall back to the global config.
- Project config should override global config for active project state.
- `sync` reconciles installed state from config.
- `install` installs immediately and records successful requested items in config.
- Config-changing install and uninstall flows refresh the lockfile next to the selected config.
- Config writes should preserve unrelated sections and existing user formatting as much as practical.
- Duplicate entries should not be added.
- If no project config exists, commands that write desired state use the global config unless the command explicitly requires project scope.
- When project and global config both define an item, the project config is the active source; `list --all` still shows global-only entries.
- No floating installs are allowed. Every installed tool, package, or app must be attached to either a project config or the global config.
- Creating a local project config is explicit through `still init`; install-style add commands do not silently create `still.toml`.

## Platform Model

Still should plan for these platform identifiers from the beginning:

- `macos`
- `linux`
- `windows`

The active host platform is detected by the engine. Config entries can then decide whether they apply to the host through `platforms`, `ignore`, `only`, and platform-specific `names`.

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
- Still normalizes `platforms`, `only`, and `ignore` into one platform filter before planning: `only` replaces the allow-list when present, then `ignore` is applied as a deny-list.
- `names` maps a portable Still name to platform/backend-specific names.
- `backends` maps platform names to platform-specific backend choices, with scalar `backend` as the fallback.
- Platform-specific install paths, executable linking, app registration, service management, and shell activation belong in the engine layer.
- Config should express intent; backends and platform adapters should translate that intent into host-specific operations.
- Still uses one multi-platform-capable `still.lock.toml` next to the selected config. Each lockfile item records its resolved platform.
- `sync` replaces entries for the active host platform and preserves entries for other platforms, allowing the lockfile to accumulate resolved state across macOS, Linux, and Windows runs.

## Config Surface

The concept config supports these top-level sections:

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

Tools are language runtimes, toolchains, and developer tools that should be available in the managed environment.

Shorthand:

```toml
[tools]
node = "22"
python = "3.12"
go = "latest"
```

Expanded:

```toml
[tools.rust]
version = "stable"
backend = "rustup"
backends = { macos = "rustup", linux = "mise", windows = "rustup" }
components = ["rustfmt", "clippy"]
targets = ["wasm32-unknown-unknown"]
```

Rules:

- A string value is shorthand for `{ version = "<value>" }`.
- Expanded tools must include `version`.
- `backend` selects a provider family such as `rustup`, `npm`, `asdf`, `aqua`, or `auto`.
- `backends` overrides `backend` for specific platforms.
- `components` and `targets` are toolchain-specific extras, primarily for runtimes such as Rust.

### Environment

Environment config defines variables and env files for `still run`, `still task`, services, and activation.

```toml
[env]
RUST_LOG = "debug"
NODE_ENV = "development"
files = [".env", ".env.local"]
```

Rules:

- Scalar keys under `[env]` are environment variables.
- `files` lists env files loaded in order.
- Later values should override earlier values when the same variable appears multiple times.
- Loading env files is trust-sensitive because it imports project-defined process state.
- Env file entries are required; missing files are errors.
- Env values are loaded literally after simple quote stripping. Still does not perform shell expansion inside env values.
- Future Pkl support may validate richer config and environment inputs before they are loaded.

### Packages

Packages are system packages, libraries, or command-line packages used by the project.

Latest shorthand:

```toml
[packages]
latest = ["pandoc", "ripgrep", "fd", "jq", "ffmpeg"]
```

Inline pinned package:

```toml
[packages]
postgresql = { version = "16", backend = "auto" }
```

Expanded package:

```toml
[packages.llvm]
version = "18"
backend = "auto"
backends = { macos = "homebrew", linux = "apt", windows = "winget" }
```

Platform names:

```toml
[packages.fd.names]
macos = "fd"
linux = "fd-find"
windows = "fd"
```

Platform filters:

```toml
[packages.watchman]
version = "latest"
platforms = ["macos", "linux"]
ignore = "windows"
```

Rules:

- `latest` is an array of package names that should track the latest available version.
- `latest` entries use the same portable name on every platform; use a keyed package entry with `names` when a backend needs a platform-specific package name.
- A keyed package may specify `version`, `backend`, `backends`, `names`, `platforms`, `ignore`, and `only`.
- `backends` overrides `backend` for specific platforms.
- `names` maps Still's logical package name to backend/platform-specific package names.
- `platforms` is an allow-list.
- `ignore` excludes one platform.
- `only` restricts an entry to one platform.
- These three fields are normalized into the same platform filter used by sync and list planning.
- Package and app install plans use the resolved platform/backend name for install storage under Still-managed roots until a backend adapter supplies a native platform path.

### Apps

Apps are desktop or platform applications.

Latest shorthand:

```toml
[apps]
latest = ["docker-desktop", "zed", "firefox"]
```

Expanded app:

```toml
[apps.visual-studio-code]
version = "latest"
backend = "auto"
```

Rules:

- Apps use the same metadata shape as packages: `version`, `backend`, `names`, `platforms`, `ignore`, and `only`.
- Apps should be modeled separately from packages because installation, linking, launchers, and uninstall behavior differ.
- Apps delegate installation to app-oriented platform backends, then Still records the managed install marker under `<still-root>/apps/<name>/<version>`.
- Default app backends are `homebrew-cask` on macOS, `flatpak` on Linux, and `winget` on Windows.
- Apps participate in install, sync, list, and uninstall. Activation is limited to shell/PATH environment setup and does not launch or expose apps in v0.1.

### Services

Services are long-running dependencies needed while the project is active. They can auto-start when a project opens or activates, or they can be started explicitly by command.

Shorthand command:

```toml
[services]
docker-compose = "docker compose up"
```

Task reference:

```toml
[services]
dev-server = { task = "dev-server" }
```

Expanded service:

```toml
[services.docker]
start = { task = "docker:start" }
stop = { task = "docker:stop" }
check = { task = "docker:check" }
```

Rules:

- A string service is shorthand for a start command.
- A `{ task = "name" }` service action runs a task.
- A `{ command = "..." }` service action runs a command directly.
- Expanded services may define `preset`, `start`, `stop`, and `check`.
- A service must define at least `preset`, `start`, or `check`.
- Built-in presets should cover common services so users do not need to write start/check/stop commands every time.

Open decisions:

- Whether services auto-watch and restart stale services by default.
- How Still stores service process state.
- Whether `sync` starts services or only prepares them.

### Tasks

Tasks are named commands or command graphs.

Shorthand:

```toml
[tasks]
test = "cargo test"
fmt = "cargo fmt"
dev = "cargo run"
```

Expanded:

```toml
[tasks.lint]
description = "Run lint checks"
run = [
  "cargo fmt --check",
  "cargo clippy --all-targets --all-features",
]
```

Task graph:

```toml
[tasks.ci]
description = "Run full CI"
depends = ["lint", "test"]
run = "echo CI passed"

[tasks.dev-server]
description = "Run development server"
requires = ["docker"]
run = "cargo run"
```

Rules:

- A string task is shorthand for `{ run = "<command>" }`.
- Expanded tasks must include `run`.
- `run` may be a string or list of strings.
- `depends` names other tasks that must run before the current task.
- `requires` names services that must be available for the task.
- `task` with no name should list available tasks.
- List-form `run` stops on the first failing command and returns that status.
- Dependency tasks run at most once per invocation, even when multiple graph paths require the same task.
- Task execution output is captured per command and returned as structured execution records for CLI/TUI formatting.

### Agents

Agents define AI-assistant setup for the project. They are desired state, like tools and packages: Still should be able to sync the same agent instructions and skills across machines without each developer hand-wiring them.

```toml
[agents]
targets = ["claude", "codex"]
instructions = "AGENTS.md"
skills = ["rust-review", "repo-auditor"]
```

For explicit external or pinned sources, use the table form instead of the array form:

```toml
[agents]
targets = ["claude", "codex"]
instructions = "AGENTS.md"

[agents.skills]
external-skill = { url = "https://example.com/skill", version = "1.2.0" }
pinned-skill = "https://example.com/skill:1.2.0"
latest-skill = "https://example.com/skill"
rust-review = { source = "rust-review", auto = true, tools = ["rust@stable@rustup"], packages = ["llvm"] }
```

Rules:

- `targets` lists agent integrations Still should prepare. Initial targets are `claude` and `codex`.
- `instructions` points to the shared project instruction file.
- `skills` lists skills by source shorthand.
- `skills` can be either an array or an `[agents.skills]` table, but not both in the same TOML document.
- Bare skill names such as `rust-review` resolve from the future official Still public skill source.
- `owner/name` skill names resolve from a GitHub-owned skill source.
- URL strings resolve external skills.
- URL strings may include a version suffix as `<URL>:<VERSION>`.
- Inline skill entries may use `{ source = "..." }` for official/GitHub shorthand or `{ url = "...", version = "..." }` for external sources.
- Keys under `[agents.skills]` must be unique because TOML cannot repeat keys.
- Target-specific output paths are platform/agent integration details; config should stay portable.
- Skill folders created by Still are generated/materialized state and may be git-ignored. `still.toml` and the lockfile should be enough to recreate them.
- `auto = true` allows Still to accept a skill's declared tool/package/app requirements and add missing entries to config during sync.
- Explicit `tools`, `packages`, and `apps` on a skill entry link that skill to required project dependencies.
- Skill-linked dependencies must still become normal desired state entries. They should not create floating installs.

Managed skill ignore policy:

- Still-managed skills should materialize under `.agents/skills/<skill-name>` unless a target integration requires a different output path.
- Still should generate or update `.agents/skills/.gitignore` with only Still-managed skill directory names.
- The generated ignore file should use anchored entries such as `/rust-review/` and `/repo-auditor/`.
- Still must not add `*`, `/*`, or `.agents/skills/*` rules that hide all skills.
- User-authored custom skills that are not listed in the generated ignore file remain commit-ready.
- Each Still-managed skill directory should include a marker file such as `.still-managed`.
- Still should refuse to overwrite an existing unmarked custom skill directory with the same name.
- Removing a skill from config removes it from the generated ignore file, but deleting the local directory should only happen for marked managed folders.

Example generated `.agents/skills/.gitignore`:

```gitignore
# still-managed skills
/rust-review/
/repo-auditor/
```

Auto dependency behavior:

- If `auto = true`, Still may add missing tool/package/app dependencies declared by the skill metadata or inline skill entry.
- If `auto` is absent or false, Still should report missing skill dependencies but not mutate config automatically.
- Auto-added dependencies use the same config write rules as `still install`: latest shorthand where possible, keyed entries for pinned versions or explicit backends.
- Auto-added dependencies should be visible in review output before writing.
- Removing a skill should not automatically remove tools/packages/apps unless a future garbage-collection command can prove nothing else needs them.

Open decisions:

- Where official Still public skills are hosted and how names are resolved.
- Whether skills are installed into `.agents/skills`, tool-specific directories, or generated target directories.
- Whether agent targets can have per-target settings later.
- Whether skill versions are semver, tags, commits, or opaque source versions.
- Whether auto-added dependencies should be annotated in config or only tracked in the lockfile.

## Install And Add

`install` should support explicit type groups:

```bash
still install --tool jq ripgrep fd --package openssl llvm --app zed firefox
```

The parsed request should be grouped as:

```text
Tools:
  jq
  ripgrep
  fd

Packages:
  openssl
  llvm

Apps:
  zed
  firefox
```

Request syntax:

- `name` means `name@latest` with backend `auto`.
- `name@version` pins a version.
- `name@version@backend` pins both version and backend.
- `name@latest@backend` tracks latest from a selected backend.

Examples:

```bash
still install --package ripgrep@1@brew
still install -p ripgrep@1@brew
still install --tool rust@stable@rustup
still install --app firefox@latest@brew-cask
```

Behavior:

- Explicit `--tool`, `--package`, and `--app` flags classify following values until another group flag appears.
- Short aliases should be available: `-t` for `--tool`, `-p` for `--package`, and `-a` for `--app`.
- Unclassified positional installs are accepted when Still can infer the type from an obvious type-specific backend.
- When an item is unclassified, Still should infer only when the answer is obvious.
- If inference is ambiguous, return an error that suggests using `--tool`, `--package`, or `--app`.
- Install all requested items first; write config once after every install succeeds.
- If any install fails, leave config unchanged and report which item failed.
- Partial successful installs are not recorded by default; there is no `--partial` behavior in the v0.1 contract.

Config write rules:

- Latest tools write as `name = "latest"` under `[tools]`.
- Pinned or backend-specific tools write as `[tools.name]`.
- Latest packages/apps from explicit multi-item flags can append to `[packages].latest` and `[apps].latest`.
- Pinned or backend-specific packages/apps write as keyed entries.
- Repeated entries should be a no-op unless the requested version/backend changes.
- Changed existing entries require `--force`.
- Install refreshes the lockfile after successful config writes.

## Command Catalog

- `init`: create a starter `still.toml`, infer project defaults, and trust the newly created file.
- `install`: install requested tools/packages/apps immediately and add them to config.
- `sync`: read config, resolve desired state, update the lockfile, install missing items, and report drift.
- `list`: show active tools/packages/apps and where each version came from.
- `list --all`: show known installed and configured items, including inactive project/global entries.
- `uninstall`: remove an item from desired state, remove Still-managed artifacts and links when present, and refresh the lockfile.
- `run`: run a command with Still-managed PATH/env and return the child exit code.
- `task`: run a named task from config; no name should list available tasks.
- `services`: inspect, start, stop, and check configured services.
- `agents`: inspect, sync, and validate configured agent instructions and skills.
- `config check`: validate `still.toml` through the schema/Taplo path.
- `doctor`: diagnose machine, cache, config, permissions, and platform health.
- `env`: print resolved environment/debug information.
- `activate`: print shell-specific activation code or instructions.
- `trust`: mark project-defined executable behavior as trusted.

Commands such as `translate`, `convert`, `web`, and `post-install` are not v0.1 priorities unless they get a concrete product role.

## Backend And Type Structure

The user-facing type (`tool`, `package`, `app`) is not the same as the backend/provider.

- Type answers what role the item plays in the project.
- Backend answers how Still resolves and installs it.
- `auto` is backend selection, not a backend.
- Backends are type-specific: a backend that makes sense for tools may not make sense for packages or apps.

V1 backend/provider families:

- `core` / `native`: Still-managed first-party installers and built-in platform behavior.
- `aqua`: Aqua registry/package ecosystem.
- `homebrew`: Homebrew formulae.
- `homebrew-cask`: Homebrew casks.

Backend direction:

- Tools should prefer version/toolchain backends such as mise, asdf, rustup, npm, aqua, and language-specific installers.
- Packages should prefer package-manager backends such as Homebrew formulae, apt, dnf, pacman, winget, Chocolatey, Scoop, Nix, and future system package managers.
- Apps should prefer app-oriented backends such as Homebrew cask, Mac App Store, Flatpak, winget, Chocolatey, and platform-specific app sources.
- Backend names in this spec are product direction, not a promise that every backend is implemented today.

## Future-Proof Implementation Shape

The implementation should keep the future multi-platform shape visible from the start:

- Config parsing produces typed desired state for tools, packages, apps, env, services, tasks, and agents.
- Resolution maps desired state to platform-specific install plans.
- Backend adapters implement fetch, install, link, uninstall, status, and metadata behavior behind typed traits.
- Platform adapters own filesystem paths, executable linking, app registration, shell activation, and service process behavior.
- CLI and TUI layers should not know backend internals.
- Lockfiles should be able to record resolved backend, platform, version, source, checksums, outputs, and linked executables.
- Current lockfiles record kind, name, platform, version, backend when selected, source identity, desired-state checksum, expected outputs, and linked executable placeholders.
- Tests should cover parser/planner behavior without requiring real package managers whenever possible.

## Trust And Safety

Still should read config by default, but project-defined executable behavior should require trust.

Trust should gate:

- tasks
- services
- agents and external skills
- env files
- project-defined commands that can execute arbitrary shell code

Installing public packages from configured backends is not the same as running project-defined shell code, but backend post-install behavior and external agent skills may need trust policy.

- Trust is stored in a project-local `.still/trust.toml` marker scoped to the config path and content fingerprint.
- `still init` writes a Still-managed trust marker for the newly created config.

Open decisions:

- Whether package post-install scripts are allowed by default.
- How TUI review/confirmation should work for trust-sensitive actions.
