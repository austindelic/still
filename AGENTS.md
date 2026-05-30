# Agent Instructions

## Project Overview

Still is a monorepo for a Rust-based project environment manager with companion web and documentation apps. The root JavaScript/TypeScript workspace is managed with Bun and Turborepo.

## Repository Layout

- `apps/cli`: Rust workspace for the `still` binary, `engine` crate, and optional TUI crate.
- `apps/web`: SvelteKit app. Follow `apps/web/AGENTS.md`.
- `apps/docs`: Next/Fumadocs documentation app. Follow `apps/docs/AGENTS.md`.
- `packages/ui`: Shared React UI package.
- `packages/eslint-config`: Shared ESLint config.
- `packages/typescript-config`: Shared TypeScript config.

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
- `cargo check -p still`: Check the default CLI build.
- `cargo test -p still`: Test the default CLI build.
- `cargo check -p still --features tui`: Check the TUI-enabled build.
- `cargo test --workspace`: Run Rust workspace tests.

## Workflow Guidance

- Use `rg` and `rg --files` for searching whenever possible.
- Keep changes scoped to the task and follow existing local patterns.
- Respect nested `AGENTS.md` files; their guidance applies within their directories.
- Do not rewrite unrelated files or revert changes you did not make.
- Avoid changing lockfiles unless dependency changes require it.
- Prefer existing workspace scripts and package-local commands over ad hoc commands.
- Public product goals belong in README files. Agent-only implementation guidance belongs in AGENTS files.
- For CLI backend/provider work, follow `apps/cli/AGENTS.md` and `apps/cli/crates/engine/AGENTS.md`.
- Before adding, rewriting, or auditing code comments, read and follow `.agents/skills/code-comments/SKILL.md`. Use compact, caller-oriented docs and avoid comments that merely repeat what the code already says.

## Validation

For documentation-only changes, a read-through and stale-term search is usually enough. For code changes, run the narrowest relevant checks first, then broaden to workspace-level checks when the change touches shared behavior or contracts.
