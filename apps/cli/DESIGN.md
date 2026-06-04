# Still CLI Code Design

This document is the implementation architecture guide for the Rust CLI
workspace. `SPEC.md` owns product behavior and config shape. `DESIGN.md` owns
code structure, boundaries, data flow, and implementation patterns. `AGENTS.md`
owns contributor and agent workflow guidance.

Still should stay simple at the edges and rigorous in the core: parse user
intent, normalize it into typed desired state, create a side-effect-free plan,
then execute that plan through backend and platform adapters. That shape should
hold for every command, every supported OS, and every future UI.

## Design Goals

- Multi-OS from the start: macOS, Linux, and Windows are first-class platforms.
- Multi-backend without leaking backend details into command handlers.
- Typed data before side effects; no stringly install flows when a model can
  represent the behavior.
- Thin CLI and TUI layers; reusable behavior belongs in the engine.
- Plans before execution so users and tests can inspect what Still will do.
- Simple modules with clear ownership instead of broad catch-all abstractions.
- Deterministic tests that do not require the developer's real package managers.

Current code is transitional. Some install behavior still lives close to
Homebrew/macOS assumptions and some engine code still prints directly. Treat
that as migration debt: new work should move toward the boundaries in this doc.

## Crate Shape

The workspace ships one binary and uses two library layers:

- `still`: root package and `still` binary. Owns Clap parsing, command routing,
  output formatting, exit codes, and the feature-gated TUI entrypoint.
- `engine`: reusable core. Owns config loading and mutation, desired state,
  resolution, planning, lockfiles, installs, filesystem behavior, registries,
  backends, platform behavior, trust, and typed errors/results.
- `still-tui`: optional terminal UI library. Owns terminal lifecycle, input
  state, rendering, tabs, menus, and TUI-specific errors.

The default build is CLI-only. The `tui` feature compiles `still-tui` into the
same `still` binary and opens it when no command is supplied. Do not add a
separate `still_tui` binary unless the product direction changes.

## Command Lifecycle

Every command should follow the same lifecycle:

```text
CLI args or TUI event
  -> typed command request
  -> config discovery and trust context
  -> desired-state load or mutation
  -> platform filtering and name normalization
  -> backend and version resolution
  -> lockfile and installed-state comparison
  -> side-effect-free plan
  -> executor applies plan
  -> typed result, diagnostics, and warnings
  -> CLI or TUI formatting
```

Rules:

- CLI parsing starts in `src/cli/args.rs`.
- CLI handlers translate parsed input into engine requests; they do not perform
  install, config, lockfile, backend, or platform work directly.
- TUI events translate UI intent into the same engine requests the CLI uses.
- Engine actions return typed results and diagnostics. CLI/TUI code decide how
  to print, group, color, or render them.
- Command handlers may choose exit codes, but the reason for failure should come
  from a typed engine error or validation error.
- Side effects should happen only after a plan exists.

## Layer Ownership

### CLI Layer

The root `still` package owns:

- command names, aliases, flags, positional arguments, and generated help
- command dispatch and exit-code mapping
- stdout/stderr formatting and snapshot-friendly output paths
- converting Clap input into command-neutral engine requests
- selecting CLI-only behavior such as no-command help output
- launching the TUI when the `tui` feature is enabled and no command is supplied

The CLI layer must not own:

- backend selection
- platform-specific install paths or links
- config merge/write rules
- lockfile state
- service process behavior
- task graph execution
- agent skill materialization

### Engine Layer

The engine owns all reusable behavior:

- config discovery, parsing, validation, mutation, and preservation policy
- desired-state models for tools, packages, apps, env, services, tasks, agents
- trust decisions for project-defined executable behavior
- item kind inference and ambiguity errors
- backend selection, version resolution, and platform-specific name resolution
- lockfile reads/writes and installed-state comparison
- plan generation for install, sync, uninstall, run, task, services, agents, env,
  activate, config, list, doctor, and trust
- execution through backend and platform adapters
- typed request, result, progress, warning, and error values

The engine must not depend on Clap, TUI state, terminal APIs, or user-facing
formatting. It should not print stdout/stderr. If progress is needed, return
events or write through a typed progress reporter owned by the caller.

### TUI Layer

The `still-tui` crate owns:

- terminal setup and teardown
- keyboard/mouse input handling
- view state, tabs, menus, popups, and forms
- rendering and layout
- TUI-specific error presentation

The TUI should call engine APIs for real behavior. It should not parse CLI args,
dispatch CLI subcommands, duplicate install/sync/task logic, or know backend
internals.

## Current Module Ownership

Current modules should keep these responsibilities:

- `src/main.rs`: thin binary entrypoint that delegates to `still::cli::entry()`.
- `src/cli/args.rs`: public command spelling, flags, positional arguments, and
  Clap-specific parsing.
- `src/cli/commands`: command dispatch and presentation-facing handlers.
- `src/cli/output.rs`: stdout/stderr abstraction for commands and tests.
- `crates/engine/actions`: command-neutral operations such as install, sync,
  trust, task, doctor, env, agents, and services as they are added.
- `crates/engine/runtime.rs`: synchronous boundary used by frontends to call
  engine behavior.
- `crates/engine/specs`: typed models for Still config and external metadata.
- `crates/engine/registries`: metadata source adapters such as Homebrew, GitHub,
  package registries, and future Still public registries.
- `crates/engine/system.rs`: current host/platform entrypoint; grow this into
  platform adapters instead of adding OS checks throughout the engine.
- `crates/engine/utils`: filesystem, network, archive, hashing, link, and path
  helpers used by engine implementations.
- `crates/tui`: terminal application state, UI components, tabs, rendering, and
  terminal lifecycle only.

If behavior is needed by both CLI and TUI, move it into the engine or a typed
boundary instead of duplicating it.

## Target Engine Modules

The engine can grow into these modules as implementation demands it. Do not add
empty architecture for its own sake, but keep these ownership lines intact.

- `config`: project/global config discovery, TOML parsing, schema-level
  validation, source tracking, and mutation/preservation rules.
- `desired`: normalized desired-state graph for tools, packages, apps, env,
  services, tasks, and agents.
- `platform`: platform identifiers, host detection, platform filters, names,
  paths, links, app registration, services, shell activation, and permissions.
- `backends`: backend traits and implementations for native/core, GitHub, HTTP,
  Cargo, Go, npm, pipx, asdf, aqua, Homebrew, apt, dnf, pacman, winget,
  Chocolatey, Scoop, Flatpak, Nix, and future providers.
- `resolver`: item kind inference, backend selection, version resolution,
  metadata lookup, platform name normalization, and locked artifact selection.
- `state`: installed-state discovery, cache indexes, drift detection, and source
  attribution for `list` and `doctor`.
- `planner`: side-effect-free plans for install, sync, uninstall, run, task,
  services, agents, env, activate, config, trust, and doctor.
- `executor`: plan execution through backend/platform traits, progress events,
  rollback hooks where practical, and structured operation results.
- `lockfile`: resolved backend, platform, version, source, checksum, outputs,
  links, service metadata, and agent skill source/version data.
- `trust`: trust storage, scope checks, and policy for env files, tasks,
  services, project commands, backend scripts, and external agent skills.
- `agents`: target adapters, instruction propagation, skill resolution,
  materialization, managed `.gitignore` entries, and auto dependencies.

## Command Design Matrix

| Command | CLI owns | Engine owns | Side effects | Platform/trust focus |
| --- | --- | --- | --- | --- |
| `init` | flags, template choice, output | starter config, default detection, trust hook | create config, optional trust metadata | path rules per OS; trust newly created file |
| `trust` | scope flags, confirmation text | trust identity, storage, policy | write trust metadata | avoid trusting arbitrary executable behavior silently |
| `sync` | request flags, progress display | desired state, lockfile, plan, execution | installs, links, services, agents, lockfile writes | full platform/backend path |
| `install` | grouped `--tool`, `--package`, `--app` input | classification, resolution, plan, config write | install requested items, write config once on success | infer only when unambiguous |
| `list` | filters, table/detail output | active/all state, source attribution | read-only | show platform-filtered and inactive entries clearly |
| `uninstall` | target input, output | target resolution, plan, config policy | remove managed installs/links | never remove unmanaged paths |
| `run` | command argv capture, child exit mapping | env/PATH resolution, trust checks | spawn child process | preserve argv; avoid shell unless explicit |
| `task` | task name and display | task graph, env, services, trust | run commands and dependencies | trust-gated project commands |
| `services` | subcommand shape, status display | service graph, start/stop/status plan | process/service manager changes | OS service managers differ strongly |
| `agents` | target/action flags, review display | instructions, skills, auto deps, ignore policy | materialize generated files | trust external skills; preserve custom skills |
| `config check` | file selection, report format | parse, validate, diagnostics | read-only | portable schema and platform filter validation |
| `doctor` | check selection, report format | machine/config/cache/backend diagnostics | mostly read-only | OS-specific checks behind platform adapters |
| `env` | output format flags | resolved env, env files, PATH | read-only unless future cache | env files are trust-sensitive |
| `activate` | shell selection/output mode | activation plan and resolved environment | prints shell code | shell-specific output belongs behind platform/shell adapters |

Commands such as `translate`, `convert`, `web`, and `post-install` are not
priority commands unless `SPEC.md` gives them a concrete product role.

## Install And Sync Pattern

`install` and `sync` should share the same engine pipeline.

- `install` starts from explicit command input, mutates desired state, then
  applies the resulting plan.
- `sync` starts from existing desired state and reconciles installed state.
- Both use the same resolver, planner, lockfile, backend, platform, and executor
  code paths.
- Config writes for install happen once after the plan succeeds unless a future
  explicit partial mode is added.
- Failed installs should leave config unchanged and report the failed item.

Explicit item groups should be represented as typed input before resolution:

```text
Tools: jq, ripgrep, fd
Packages: openssl, llvm
Apps: zed, firefox
```

Unclassified items may be inferred later, but only when the answer is obvious.
Ambiguous input should fail with a message suggesting `--tool`, `--package`, or
`--app`.

## Platform Architecture

Platform-specific behavior belongs behind adapters. Core models should use
portable concepts and let the platform layer translate them.

Platform adapters own:

- platform and architecture detection
- cache, config, data, binary, app, service, and temp paths
- executable linking or shims
- app installation/registration/unregistration
- shell activation snippets
- service process managers
- permissions and executable bits
- host tool discovery
- OS-specific diagnostics

Core desired-state code must not hardcode Homebrew, macOS paths, Unix symlinks,
PowerShell syntax, launchd, systemd, Windows services, or package-manager
commands. Those details belong in platform/backend implementations.

Tests for platform behavior should inject platform data. Do not rely on the
developer's real OS for planner coverage.

## Backend Architecture

The user-facing item kind is separate from the provider:

- Kind answers what role the item plays: `tool`, `package`, or `app`.
- Backend answers how Still resolves and installs it.
- `auto` means backend selection, not a backend implementation.
- Backends may support one or more kinds, but kind-specific behavior should stay
  explicit in typed capabilities.

Backend adapters should expose typed operations such as:

- metadata lookup and search
- version resolution
- artifact/source selection
- checksum and signature metadata
- fetch/download/build/install
- link/shim output discovery
- uninstall/status/drift information

Backends should not decide CLI wording, config syntax, or UI rendering. Adding a
backend should usually require adding an implementation and tests, not changing
every command handler.

## Planning And Execution

Planning should be pure enough to test with fixtures:

- input: desired state, lockfile state, installed state, platform, backend
  metadata, trust context, and command intent
- output: ordered operations, warnings, required confirmations, expected config
  or lockfile writes, and reasoned no-ops
- no downloads, filesystem mutations, service starts, command execution, or
  generated skill writes during planning

Execution applies a plan:

- perform operations through backend/platform traits
- emit structured progress
- record successful outputs in typed results
- scope command-backed tool installers into Still-managed roots with backend
  flags or environment before execution
- preserve config and lockfile consistency
- avoid deleting or overwriting unmanaged files
- keep rollback explicit and conservative when full rollback is not possible

## Config, Lockfile, And State

Config is desired state. The lockfile is resolved state. Installed state is what
exists on disk or in platform services.

- Config parsing should produce source-aware typed values so diagnostics can
  point to the relevant section or entry.
- Config mutation should preserve unrelated sections and user formatting as much
  as practical.
- Duplicate desired-state entries should become no-ops or clear conflicts.
- Lockfiles should record resolved backend, platform, version, source, checksum,
  artifact identity, install outputs, linked executables, services, and agent
  skill sources.
- Installed-state discovery should never assume every file under a shared path is
  safe to delete; managed markers or lockfile ownership should prove ownership.
- Native package/app backends should track Still-owned receipts separately from
  OS-owned installation locations.

## Agents And Skills

Agents are desired state and should use the same config, planning, lockfile, and
execution shape as tools/packages/apps.

Agent engine code owns:

- resolving `targets`
- copying or generating instructions for each target
- resolving public, GitHub-owned, and URL skills
- materializing managed skill directories
- generating `.agents/skills/.gitignore` with only anchored managed entries
- refusing to overwrite unmarked custom skill directories
- expanding `auto = true` skill dependencies into normal desired state
- reporting missing dependencies when auto is not enabled

Generated skill folders are rebuildable state. User-authored skills are source.
The managed ignore file must never use broad rules such as `*`, `/*`, or
`.agents/skills/*`.

## Trust And Safety

Still may read config before trust, but executable project behavior must be
gated.

Trust-sensitive behavior includes:

- tasks
- services
- env files
- project-defined commands
- external agent skills
- backend post-install scripts when a backend can run arbitrary code

Trust checks belong in the engine so CLI and TUI behavior stays consistent. UI
layers may ask for confirmation, but the policy decision should be reusable and
testable.

## Error And Output Pattern

Use typed errors and diagnostics in the engine:

- validation errors for bad config or impossible command requests
- ambiguity errors for unclassified installs that cannot be inferred
- trust errors for blocked executable behavior
- platform unsupported errors
- backend unavailable or resolution errors
- plan conflicts and ownership conflicts
- execution failures with operation context

CLI and TUI code should format these values. Engine errors should carry enough
context for good output without embedding final user-facing prose everywhere.

## Testing Pattern

Test at the layer that owns the behavior:

- CLI tests cover command spelling, parsing, help text, exit codes, and formatted
  output.
- CLI handler tests use fake runtimes rather than real engine side effects.
- Engine config tests cover parsing, normalization, mutation, and diagnostics.
- Resolver/planner tests use fixture desired state, fake metadata, fake
  installed state, and injected platforms.
- Backend tests use fixtures and temp paths before real network/package-manager
  integration tests.
- Platform tests inject OS/architecture/path behavior and avoid depending on the
  developer's host where possible.
- TUI tests cover state transitions and rendering-adjacent behavior without an
  interactive terminal.

Docs-only changes do not require a Rust build. Code changes should run the
narrowest relevant check first, then broaden to feature and workspace checks
when shared contracts change.

## Refactor Path

Move toward this shape in small compiling steps:

- Introduce typed config and desired-state models before adding more command
  behavior.
- Replace direct engine `println!` calls with typed results, diagnostics, or
  progress events.
- Move Homebrew/macOS-specific install logic behind backend/platform boundaries.
- Share install and sync through a common resolver/planner/executor path.
- Add backend and platform traits before adding a second implementation of the
  same behavior.
- Move shared CLI/TUI behavior into engine actions as soon as both frontends need
  it.
- Keep public command spelling stable while internals move behind typed engine
  APIs.
- Record unresolved product choices in `SPEC.md`; record code invariants and
  structure here.
