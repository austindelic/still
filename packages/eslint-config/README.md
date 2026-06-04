# `@repo/eslint-config`

Shared ESLint configuration package for the Still JavaScript/TypeScript
workspace.

## Exports

- `@repo/eslint-config/base`: base TypeScript and Turborepo rules.
- `@repo/eslint-config/next-js`: Next.js/Fumadocs app rules.
- `@repo/eslint-config/react-internal`: shared React package rules.

## Usage

Import the relevant config from package-local ESLint configuration files in
apps or packages. Keep broad rule changes here so the docs, web app, and shared
packages stay consistent.
