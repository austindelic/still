# Still

Still is a config-first project environment manager. It is built around one project file, one managed cache, and one CLI for tools, packages, apps, environment variables, tasks, and services.

Still is early. The current repository is shaping the CLI, config model, engine boundaries, and optional TUI.

## Goals

- Describe a project environment in `still.toml`.
- Install and link tools/packages/apps into Still-managed storage.
- Reconcile local state from config with `still sync`.
- Run commands and tasks inside the managed environment.
- Keep project-defined executable behavior behind explicit trust.
- Support deterministic installs through lockfiles and content-addressed cache paths.
- Keep the default CLI small, with the TUI behind an optional Rust feature.

## Core Workflow

```bash
still init
still trust
still sync
still run cargo test
still task lint
```

The intended model is:

- `still.toml` declares desired project state.
- `still trust` allows project-defined tasks, services, hooks, and env-file behavior.
- `still sync` resolves and installs missing state.
- `still run` executes an arbitrary command inside that environment.
- `still task` runs named project tasks.

## Repository Layout

- `apps/cli`: Rust CLI and engine workspace.
- `apps/web`: SvelteKit web app.
- `apps/docs`: Next/Fumadocs documentation app.
- `packages/ui`: Shared React UI package.
- `packages/eslint-config`: Shared ESLint config.
- `packages/typescript-config`: Shared TypeScript config.

## CLI

The Rust CLI package builds a `still` binary.

```bash
cd apps/cli
cargo run -p still --bin still -- --help
cargo run -p still --features tui --bin still -- --help
```

The default binary is CLI-only. Building with `--features tui` includes the optional TUI and makes bare `still` open the TUI.

See `apps/cli/README.md` for the CLI product model.
