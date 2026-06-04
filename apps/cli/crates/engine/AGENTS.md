# Agent Instructions

## Crate Purpose

`crates/engine` owns Still's core behavior and the `engine::cli` adapter surface. Non-CLI engine modules should remain reusable by the optional TUI and future frontends without pulling in presentation concerns.

## Responsibilities

- Parse and model tool/package specs.
- Own the CLI contract, command dispatch, and CLI output formatting under `cli/`.
- Resolve backend/provider choices.
- Plan and run installs, uninstalls, linking, cache writes, archive extraction, and downloads.
- Own filesystem, hashing, networking, path, registry, and platform behavior.
- Return typed requests, results, and errors that callers can format.

## Boundaries

- Limit Clap usage to `cli/`.
- Do not print user-facing stdout/stderr from engine actions; CLI formatting belongs in `cli/`.
- Do not depend on TUI state or terminal APIs.
- Do not encode CLI command names or help text outside `cli/`.
- Do not introduce hidden global state when a typed request can carry the needed context.

## Testing

- Keep small unit tests beside the code being tested.
- Put public behavior tests in `crates/engine/tests`.
- Prefer deterministic tests that use temp paths and fake inputs over tests that mutate the real machine.
- Add integration coverage when changing install/link/cache behavior or backend contracts.

## Comments

- Before adding, rewriting, or auditing comments, read and follow the project skill at `.agents/skills/code-comments/SKILL.md`.
- Engine docs should emphasize typed contracts, error behavior, filesystem/network side effects, and backend/platform invariants.
