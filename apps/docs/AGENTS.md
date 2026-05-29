# Agent Instructions

## App Overview

`apps/docs` is the Next.js documentation app built with Fumadocs and MDX content.

## Layout

- `content/docs`: Documentation MDX content.
- `src/app/(home)`: Home page routes.
- `src/app/docs`: Documentation layout and catch-all docs routes.
- `src/app/api/search/route.ts`: Search route handler.
- `src/lib/source.ts`: Fumadocs content source adapter.
- `src/lib/layout.shared.tsx`: Shared Fumadocs layout options.

## Commands

Run these from the repository root:

- `bun --filter docs run dev`: Start the docs dev server.
- `bun --filter docs run check`: Run Biome checks.
- `bun --filter docs run types:check`: Generate Fumadocs/Next types and run TypeScript checks.
- `bun --filter docs run build`: Build the docs app.

Run these from `apps/docs` only when working directly inside the package:

- `bun run dev`
- `bun run check`
- `bun run types:check`
- `bun run build`

## Guidance

- Keep documentation content in MDX under `content/docs` unless a route or component change is required.
- Use existing Fumadocs layout and source helpers instead of creating parallel docs plumbing.
- Prefer shared layout configuration in `src/lib/layout.shared.tsx` for docs-wide behavior.
- Run `types:check` after changes that affect routes, MDX source config, or generated docs types.
- Keep content edits concise and aligned with the root project terminology for Still.
