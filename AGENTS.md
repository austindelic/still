# Agent Instructions

## Project Overview

Still is a monorepo for a Rust-based package and toolchain manager with companion web and documentation apps. The root workspace is managed with Bun and Turborepo.

## Repository Layout

- `apps/cli`: Rust CLI workspace for the `still_s` binary, with `engine` and `ui` crates.
- `apps/web`: SvelteKit app. Follow the nested `apps/web/AGENTS.md` for Svelte-specific guidance.
- `apps/docs`: Next/Fumadocs documentation app.
- `packages/ui`: Shared React UI package.
- `packages/eslint-config`: Shared ESLint configs.
- `packages/typescript-config`: Shared TypeScript configs.

## Common Commands

Run JavaScript and TypeScript workspace commands from the repository root:

- `bun install`: Install workspace dependencies.
- `bun run build`: Build all Turborepo build targets.
- `bun run lint`: Run configured lint tasks.
- `bun run check`: Run configured check tasks.

Package-specific commands:

- `bun --filter web run check`: Type-check the SvelteKit app.
- `bun --filter web run lint`: Lint the SvelteKit app.
- `bun --filter web run build`: Build the SvelteKit app.
- `bun --filter docs run check`: Run Biome checks for docs.
- `bun --filter docs run types:check`: Generate docs types and run TypeScript checks.
- `bun --filter docs run build`: Build the docs app.

Run Rust commands from `apps/cli`:

- `cargo fmt`: Format Rust code.
- `cargo check`: Check the CLI workspace.
- `cargo test`: Run Rust tests.
- `cargo build`: Build the CLI workspace.

## Workflow Guidance

- Use `rg` and `rg --files` for searching whenever possible.
- Keep changes scoped to the task and follow existing local patterns.
- Respect nested `AGENTS.md` files; their guidance applies within their directories.
- Do not rewrite unrelated files or revert changes you did not make.
- Avoid changing lockfiles unless dependency changes require it.
- Prefer existing workspace scripts and package-local commands over ad hoc commands.
- For CLI backend/provider work, follow `apps/cli/AGENTS.md`; v1 backends are `core`/`native`, `github`, `http`, `cargo`, `go`, `npm`, `pipx`, `asdf`, and `aqua`.

## Validation

For documentation-only changes, a read-through is usually enough. For code changes, run the narrowest relevant checks first, then broaden to workspace-level checks when the change touches shared behavior or contracts.
