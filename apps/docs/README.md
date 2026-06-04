# Still Docs

`apps/docs` is the Next/Fumadocs documentation app for Still. It publishes the
user-facing guide to the CLI, `still.toml`, commands, source selection, tasks,
services, and trust behavior.

## Layout

- `content/docs`: MDX documentation pages and docs navigation metadata.
- `src/app/(home)`: docs app home routes.
- `src/app/docs`: Fumadocs docs layout and route handling.
- `src/app/llms-full.txt/route.ts`: generated LLM-friendly docs text endpoint.
- `src/lib/source.ts`: Fumadocs content loader.
- `src/lib/layout.shared.tsx`: shared Fumadocs layout configuration.

## Commands

Run from the repo root:

```bash
bun run --filter docs dev
bun run --filter docs check
bun run --filter docs types:check
bun run --filter docs build
```

Run from `apps/docs` when working inside the package:

```bash
bun run dev
bun run check
bun run types:check
bun run build
```

## Writing Guidance

- Keep docs terminology aligned with `apps/cli/SPEC.md`: item kinds are tools,
  packages, and apps; install sources are sources.
- Keep current behavior and intended pre-1.0 direction clearly separated.
- Prefer command examples that work with the current `still` binary name and
  source syntax.
- Update `content/docs/meta.json` whenever docs pages are renamed, added, or
  removed.
