# Still Web

`apps/web` is the SvelteKit companion site for Still. It is a product-facing
web surface for the project while the CLI, engine, docs, and source crates take
shape.

## Layout

- `src/routes/+page.svelte`: current landing page.
- `src/routes/+layout.svelte`: app shell and favicon wiring.
- `src/routes/layout.css`: global CSS and Tailwind import.
- `src/lib/assets`: local static assets imported by the app.
- `static`: public static files served by SvelteKit.

## Commands

Run from the repo root:

```bash
bun run --filter web dev
bun run --filter web check
bun run --filter web lint
bun run --filter web build
```

Run from `apps/web` when working inside the package:

```bash
bun run dev
bun run check
bun run lint
bun run build
```

## Writing Guidance

- Keep public copy aligned with the source-based CLI architecture in
  `apps/cli/SPEC.md`.
- Be honest about pre-1.0 status; do not imply every source installer is
  complete.
- Use current command syntax, for example
  `still install --tool cargo:ripgrep@14.1.1`.
