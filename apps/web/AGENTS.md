# Agent Instructions

## App Overview

`apps/web` is the SvelteKit companion site for Still. Keep public copy aligned
with the source-based CLI direction in `apps/cli/SPEC.md` and the sharp product
story in the root `README.md`.

## Commands

Run these from the repository root:

- `bun run --filter web dev`: Start the SvelteKit dev server.
- `bun run --filter web check`: Run Svelte type checks.
- `bun run --filter web lint`: Run formatting checks and ESLint.
- `bun run --filter web build`: Build the web app.

## Guidance

- Keep the landing page honest about pre-1.0 status.
- Use current command syntax, such as
  `still install --tool cargo:ripgrep@14.1.1`.
- Prefer SvelteKit and Svelte 5 patterns already present in the app.
- Do not duplicate CLI specs in marketing copy; link or point back to
  `apps/cli/SPEC.md` for detailed behavior.

## Svelte Documentation Tools

When asked about Svelte or SvelteKit topics, use the Svelte MCP documentation
tools when available:

1. `list-sections`: discover relevant documentation sections first.
2. `get-documentation`: fetch all relevant sections after reviewing the list.
3. `svelte-autofixer`: analyze Svelte code after writing it and repeat until no
   issues remain.
4. `playground-link`: generate a playground link only after user confirmation
   and never for code already written to files in this project.
