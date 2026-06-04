# Agent Instructions

## Crate Purpose

`crates/tui` is the optional text UI library for Still. It is compiled into the root `still` binary through the `tui` feature.

## Responsibilities

- Own terminal UI state, rendering, input handling, tabs, menus, and terminal lifecycle.
- Expose TUI entry APIs such as `launch_tui()`.
- Use engine/root APIs for actual package, task, service, and config behavior.

## Boundaries

- Do not parse CLI args here.
- Do not dispatch CLI subcommands here.
- Do not duplicate install/sync/task logic that belongs in the root CLI or engine.
- Keep terminal-specific errors and lifecycle handling local to the TUI layer.

## Testing

- Keep TUI state-machine and rendering-adjacent unit tests near the relevant modules.
- Avoid tests that require an interactive terminal.
- Use engine fakes or typed state when testing interactions that would otherwise mutate the machine.

## Comments

- Before adding, rewriting, or auditing comments, read and follow the project skill at `.agents/skills/code-comments/SKILL.md`.
- TUI docs should emphasize terminal lifecycle, process/input side effects, rendering contracts, and state invariants.
