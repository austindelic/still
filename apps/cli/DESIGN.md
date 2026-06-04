# Still CLI Code Design

This document is the Rust architecture guide for the CLI workspace. `SPEC.md`
owns product behavior and config shape. `DESIGN.md` owns code structure,
boundaries, data flow, and implementation patterns. `AGENTS.md` owns workflow
guidance for humans and agents.

Still should stay simple at the edges and rigorous in the core: parse user
intent, normalize it into typed desired state, resolve from source metadata,
create a side-effect-free plan, then execute that plan through source and
platform adapters.

## Design Goals

- Multi-OS from the start: macOS, Linux, and Windows are first-class platforms.
- Compile out non-target OS code with `cfg(target_os)` and platform traits.
- Source-based installs instead of command wrappers around package managers.
- Typed data before side effects; avoid stringly install flows.
- Thin CLI and TUI layers; reusable behavior belongs in the engine.
- Shared installer machinery lives in `still-source-kit` to avoid duplicated
  download, checksum, archive, staging, receipt, and binary-discovery code.
- Source crates are independently feature-gated so future builds can include
  only selected sources.
- Deterministic tests should avoid the developer's real package managers.

Current code is transitional. Some install behavior still lives in legacy
CLI-in-engine modules, command-backed installers, and broad action files. Treat
those as migration debt: new work should move toward the boundaries in this doc.

## Crate Shape

The workspace ships one binary and several library layers:

- `still`: root package and `still` binary. Owns `apps/cli/src`, including
  `clap` parsing, command dispatch, exit codes, output formatting, Tokio runtime
  setup, `miette` diagnostics, `indicatif` progress, and future
  `anstream`/`anstyle` styling.
- `engine`: reusable core. Owns config, desired state, source selection,
  planning, platform traits, install orchestration, lockfiles, inventory, trust,
  and typed errors/results.
- `still-source-kit`: shared source implementation utilities.
- `still-source-*`: source-specific metadata parsing, resolution, artifact
  selection, dependency interpretation, and install rules.
- `still-tui`: optional terminal UI library. Owns terminal lifecycle, input
  state, rendering, tabs, menus, and TUI-specific errors.

The intended source crate layout is:

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

Source package names use `still-source-kit`, `still-source-homebrew`,
`still-source-cargo`, and the same `still-source-*` pattern.

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
  -> platform filtering and source/name normalization
  -> source and version resolution
  -> lockfile and installed-state comparison
  -> side-effect-free plan
  -> executor applies plan through source/platform traits
  -> typed result, diagnostics, warnings, and progress events
  -> CLI or TUI formatting
```

Rules:

- CLI parsing starts in `apps/cli/src/cli/args.rs`.
- CLI handlers translate parsed input into engine requests and call engine
  actions directly after parsing.
- CLI handlers do not perform install, config, lockfile, source, or platform
  work directly.
- TUI events translate UI intent into the same engine requests the CLI uses.
- Engine actions return typed results and diagnostics. CLI/TUI code decide how
  to print, group, color, render, or show progress.
- Side effects should happen only after a plan exists.

## Layer Ownership

### CLI Layer

`apps/cli/src` owns:

- `clap` command names, aliases, flags, positionals, and generated help
- command dispatch and exit-code mapping
- Tokio runtime setup for async engine calls
- `miette` diagnostic rendering at the user-facing boundary
- `indicatif` progress bars/spinners
- stdout/stderr formatting
- future `anstream`/`anstyle` color and style output
- feature-gated no-command TUI launch behavior

The CLI layer must not own:

- source selection
- platform-specific install paths or links
- config merge/write rules
- lockfile state
- service process behavior
- task graph execution
- agent skill materialization

### Engine Layer

The engine owns reusable behavior:

- config discovery, parsing, validation, mutation, and preservation policy
- desired-state models for tools, packages, apps, env, services, tasks, agents
- trust decisions for project-defined executable behavior
- item kind inference and ambiguity errors
- source selection, version resolution, and platform-specific name resolution
- lockfile reads/writes and installed-state comparison
- plan generation for install, sync, uninstall, run, task, services, agents, env,
  activate, config, list, doctor, and trust
- execution orchestration through source and platform traits
- typed request, result, progress, warning, and error values

The engine must not depend on Clap, TUI state, terminal APIs, progress bar
types, or user-facing formatting. It should not create the process Tokio runtime.
If progress is needed, return typed events that callers can format.

### Source Kit

`still-source-kit` owns duplicate-prone implementation building blocks:

- HTTP downloads and cache writes
- checksum and future signature verification
- tar/zip/archive extraction
- staging, promotion, and rollback helpers
- install receipts and marker serialization
- executable discovery and shim/link helpers
- dependency graph helpers
- package/source naming helpers

`still-source-kit` should not decide product policy, CLI wording, config syntax,
or UI rendering.

### Source Crates

Each `still-source-*` crate owns source-specific behavior:

- raw metadata file formats and indexes
- source-specific version resolution
- artifact selection for the current platform/architecture
- dependency interpretation
- install rules that call shared source-kit operations where possible

Sources should not shell out to package managers as the primary model. They
should read the raw metadata those ecosystems use, resolve artifacts, download
or build manually, and install into Still-owned layouts where practical.

### Platform Layer

Platform-specific behavior belongs behind traits and compile-time selection.
Production code should compile only the target OS implementation:

```rust
#[cfg(target_os = "macos")]
pub type HostPlatform = macos::MacosPlatform;

#[cfg(target_os = "linux")]
pub type HostPlatform = linux::LinuxPlatform;

#[cfg(target_os = "windows")]
pub type HostPlatform = windows::WindowsPlatform;
```

Platform adapters own:

- platform and architecture detection
- cache, config, data, binary, app, service, and temp paths
- executable linking or shims
- app registration/unregistration
- shell activation snippets
- service process managers
- permissions and executable bits
- host capability checks
- OS-specific diagnostics

Tests for platform behavior should inject fake platform data. Do not rely on the
developer's real OS for planner coverage.

### TUI Layer

The `still-tui` crate owns:

- terminal setup and teardown
- keyboard/mouse input handling
- view state, tabs, menus, popups, and forms
- rendering and layout
- TUI-specific error presentation

The TUI should call engine APIs for real behavior. It should not parse CLI args,
dispatch CLI subcommands, duplicate install/sync/task logic, or know source
internals.

## Target Engine Modules

The engine can grow into these modules as implementation demands it:

- `config`: project/global discovery, TOML parsing, validation, source tracking,
  and mutation/preservation rules.
- `desired`: normalized desired-state graph for tools, packages, apps, env,
  services, tasks, and agents.
- `platform`: platform identifiers, host selection, platform filters, names,
  paths, links, app registration, services, shell activation, and permissions.
- `sources`: source registry, feature-gated source wiring, source capabilities,
  and source selection.
- `resolver`: item kind inference, source selection, version resolution,
  metadata lookup, platform name normalization, and locked artifact selection.
- `inventory`: installed-state discovery, receipts, cache indexes, drift
  detection, and source attribution for `list` and `doctor`.
- `planner`: side-effect-free plans for command actions.
- `executor`: plan execution through source/platform traits, progress events,
  rollback hooks where practical, and structured operation results.
- `lockfile`: resolved source, platform, version, artifact identity, checksum,
  outputs, links, service metadata, and agent skill source/version data.
- `trust`: trust storage, scope checks, and policy for env files, tasks,
  services, project commands, source scripts, and external agent skills.

## Install And Sync Pattern

`install` and `sync` should share the same engine pipeline.

- `install` starts from explicit command input, mutates desired state, then
  applies the resulting plan.
- `sync` starts from existing desired state and reconciles installed state.
- Both use the same resolver, planner, lockfile, source, platform, inventory,
  and executor code paths.
- Config writes for install happen once after the plan succeeds unless a future
  explicit partial mode is added.
- Failed installs should leave config unchanged and report the failed item.

Explicit item groups should be represented as typed input before resolution:

```text
Tools: cargo:ripgrep@14.1.1, aqua:fd
Packages: homebrew:openssl, apt:curl
Apps: flatpak:org.mozilla.firefox
```

Unclassified items may be inferred later, but only when the answer is obvious.
Ambiguous input should fail with a message suggesting `--tool`, `--package`, or
`--app`.

## Source Architecture

The user-facing item kind is separate from the source:

- Kind answers what role the item plays: `tool`, `package`, or `app`.
- Source answers where Still resolves metadata and artifacts from.
- `auto` means source selection, not a source implementation.
- Sources may support one or more kinds, but kind-specific behavior should stay
  explicit in typed capabilities.

Source adapters expose typed operations:

- metadata lookup and search
- version resolution
- artifact/source selection
- checksum and signature metadata
- dependency graph data
- fetch/build/install through `still-source-kit`
- link/shim output discovery
- uninstall/status/drift information

Explicit source selection must be exact: if `cargo:ripgrep` cannot be resolved
or installed through the compiled `cargo` source on this platform, Still returns
an error and does not try another source. Missing source or `auto` uses ordered
candidate sources filtered by compiled features, item kind, and platform.

## Planning And Execution

Planning should be pure enough to test with fixtures:

- input: desired state, lockfile state, installed state, platform, source
  metadata, trust context, and command intent
- output: ordered operations, warnings, required confirmations, expected config
  or lockfile writes, and reasoned no-ops
- no downloads, filesystem mutations, service starts, command execution, or
  generated skill writes during planning

Execution applies a plan:

- perform operations through source/platform traits
- emit structured progress events for CLI/TUI presentation
- record successful outputs in typed results
- scope installers into Still-managed roots
- preserve config and lockfile consistency
- avoid deleting or overwriting unmanaged files
- keep rollback explicit and conservative when full rollback is not possible

## Config, Lockfile, And Inventory

Config is desired state. The lockfile is resolved state. Inventory is what Still
can prove exists through receipts, managed paths, or platform services.

- Config parsing should produce source-aware typed values so diagnostics can
  point to the relevant section or entry.
- Config mutation should preserve unrelated sections and user formatting as much
  as practical.
- Duplicate desired-state entries should become no-ops or clear conflicts.
- Lockfiles should record resolved source, platform, version, artifact identity,
  checksum, install outputs, linked executables, services, and agent skill
  sources.
- Installed-state discovery should never assume every file under a shared path is
  safe to delete; managed markers or lockfile ownership should prove ownership.

## Trust And Safety

Still may read config before trust, but executable project behavior must be
gated.

Trust-sensitive behavior includes:

- tasks
- services
- env files
- project-defined commands
- external agent skills
- source install scripts when a source can run arbitrary code

Trust checks belong in the engine so CLI and TUI behavior stays consistent. UI
layers may ask for confirmation, but the policy decision should be reusable and
testable.

## Testing Pattern

Test at the layer that owns the behavior:

- CLI tests cover command spelling, parsing, help text, exit codes, diagnostics,
  progress decisions, and formatted output.
- Engine tests cover config, normalization, source selection, planning,
  inventory, lockfiles, trust, and typed errors.
- Source-kit tests cover download/cache abstractions, checksums, archive
  extraction, staging/promotion, receipts, and executable discovery.
- Source crate tests use fixture metadata and temp paths before real integration
  tests.
- Platform tests inject OS/architecture/path behavior and avoid depending on the
  developer's host where possible.
- TUI tests cover state transitions and rendering-adjacent behavior without an
  interactive terminal.

Docs-only changes do not require a Rust build. Code changes should run the
narrowest relevant check first, then broaden to feature and workspace checks
when shared contracts change.

## Refactor Path

Move toward this shape in small compiling steps:

- Rewrite docs and agent guidance around source architecture.
- Move Clap, output, and runtime ownership from legacy engine CLI modules into
  `apps/cli/src`.
- Introduce source syntax parsing while treating old source-adapter names as
  migration debt.
- Extract source selection and platform traits before creating source crates.
- Add `still-source-kit` before duplicating installer machinery.
- Scaffold source crates behind features.
- Move Homebrew/Cargo-style manual install paths into source crates first.
- Expand native package ecosystems only after dependency and script semantics are
  designed.
