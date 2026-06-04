# Agent Instructions

## Crate Purpose

`crates/engine` owns Still's reusable core behavior. It should remain usable by
the root CLI, optional TUI, and future frontends without pulling in presentation
concerns.

Before non-trivial engine changes, read:

- `../../DESIGN.md` for code architecture and boundaries.
- `../../SPEC.md` for product behavior, command contract, and config/source syntax.
- `../../AGENTS.md` for workspace-wide CLI guidance.

## Responsibilities

- Parse and model source-aware tool/package/app specs.
- Discover, parse, validate, and mutate Still config.
- Normalize config into typed desired state.
- Resolve source choices.
- Plan and run installs, uninstalls, linking, cache writes, archive extraction,
  downloads, lockfile writes, inventory reads, and trust checks.
- Own filesystem, hashing, networking, path, source registry, platform, and source
  orchestration behavior.
- Return typed requests, results, diagnostics, progress events, and errors that
  callers can format.

## Boundaries

- Do not depend on Clap. CLI parsing belongs in `apps/cli/src/cli`.
- Do not print user-facing stdout/stderr from engine actions.
- Do not depend on TUI state, terminal APIs, indicatif progress types, miette
  reports, anstream, or anstyle.
- Do not encode CLI command names or help text in engine modules.
- Do not create the process Tokio runtime; callers own runtime setup.
- Do not introduce hidden global state when a typed request can carry the needed context.
- Use source terminology for new code. Older source-adapter naming is migration
  debt.

## Platform And Sources

- Production platform behavior should compile only the current target OS with
  `cfg(target_os)` and platform traits.
- Tests should inject fake platforms for planner/resolver coverage.
- Source selection must be typed and testable.
- Explicit source selection must not silently fall back to another source.
- Missing source or `auto` should use ordered, compiled-in, OS-compatible source
  candidates.
- Source crate optional dependencies and source feature wiring belong in
  `crates/engine`; the root binary forwards feature flags only.
- Shared installer machinery belongs in `crates/source-kit`; source-specific
  behavior belongs in `crates/sources/*`.

## Testing

- Keep small unit tests beside the code being tested.
- Put public behavior tests in `crates/engine/tests`.
- Prefer deterministic tests that use temp paths, fixtures, fake platforms, and
  fake source metadata over tests that mutate the real machine.
- Add integration coverage when changing install/link/cache behavior, source
  contracts, lockfiles, or inventory ownership.

## Comments

- Before adding, rewriting, or auditing comments, read and follow the project
  skill at `.agents/skills/code-comments/SKILL.md`.
- Engine docs should emphasize typed contracts, error behavior,
  filesystem/network side effects, source invariants, and platform invariants.
