---
name: code-comments
description: Write, rewrite, audit, or standardize code comments and API documentation using compact, IDE-friendly docs. Use this skill when adding Rustdoc/JSDoc/docstrings, improving existing comments, removing noisy comments, or defining comment style for functions, constructors, structs, traits, enums, fields, and non-obvious implementation details.
---

# Code Comments

Use this skill when creating or editing comments. The goal is useful context in IDE hover and generated docs without comment noise.

## Core Rules

- Write comments for readers at the call site first: purpose, inputs, return meaning, failures, side effects, and invariants.
- Keep docs compact. Use standard headings only when they add signal.
- Do not add blank separator lines inside compact doc blocks.
- Do not write comments that repeat the item name, type, or obvious control flow.
- Do not document every enum variant when the variant names explain themselves.
- Update or delete stale comments when behavior changes.

## What To Document

Document these with `///` in Rust, equivalent doc comments in other languages:

- Public functions and constructors.
- Public structs, traits, enums, and fields whose meaning is not obvious.
- Runtime, command, API, storage, filesystem, network, security, and lifecycle boundaries.
- Private functions only when the behavior is non-obvious or has important side effects.

Use private `//` comments inside function bodies only for:

- Non-obvious intent.
- Important invariants.
- Side effects.
- Trust or security decisions.
- Filesystem behavior.
- Terminal or process lifecycle constraints.
- External API quirks.

## Rustdoc Format

Prefer this compact shape:

```rust
/// Installs one requested item into Still-managed storage and links its executable when found.
/// # Arguments
/// * `request` - Parsed package name and version request.
/// # Returns
/// The installed package identity, install path, and discovered executable path.
/// # Errors
/// Fails if registry lookup, bottle selection, download, checksum verification,
/// extraction, or linking fails.
/// # Side Effects
/// Downloads an archive, replaces the install directory, and may create a symlink.
pub async fn run(request: InstallRequest) -> Result<InstallResult> {
    todo!()
}
```

Use these headings only when relevant:

- `# Arguments`: for functions/constructors with meaningful parameters.
- `# Returns`: when the return value has semantics beyond the type.
- `# Errors`: for `Result` functions.
- `# Panics`: when panic conditions are part of the contract.
- `# Side Effects`: for filesystem, network, process, terminal, global state, cache, or symlink behavior.

Skip a heading when it would say nothing useful.

## Good Examples

### Constructor

```rust
/// Creates an action menu for the current modal state.
/// # Arguments
/// * `state` - Whether the menu is closed or which action is selected.
/// * `title` - Border title owned by the menu for rendering.
/// # Returns
/// A renderable menu value; construction does not draw or mutate terminal state.
pub fn new(state: ActionMenuState, title: String) -> Self {
    Self { state, title }
}
```

### Struct

```rust
/// Request to install one parsed tool or package spec.
///
/// Build this after CLI/config parsing has validated user input. The engine owns
/// resolution, download, extraction, and linking from this point forward.
pub struct InstallRequest {
    /// Parsed item requested by the caller.
    pub tool: ToolSpec,
}
```

### Enum

```rust
/// Direction used when moving the selected package row.
pub enum NavigationDirection {
    Up,
    Down,
}
```

Add variant docs only when the variant carries surprising data or an invariant:

```rust
/// Action menu modal state.
pub enum ActionMenuState {
    Closed,
    /// `selected_action` is an index into `Action::all()`.
    Open { selected_action: usize },
}
```

### Private Implementation Comment

```rust
// Skip malformed registry entries so one bad formula does not hide the rest of
// the package list.
if skipped < 3 {
    eprintln!("Warning: Skipping formula at index {idx}: {e}");
}
```

## Bad Examples

Do not write spacing-heavy docs when a compact contract is enough:

```rust
/// Installs one requested item.
///
/// # Arguments
///
/// * `request` - The request.
///
/// # Returns
///
/// The result.
pub async fn run(request: InstallRequest) -> Result<InstallResult> {
    todo!()
}
```

Do not restate code:

```rust
// Increment index by one.
index += 1;
```

Do not document self-explanatory enum variants:

```rust
pub enum InstallFilter {
    /// All packages.
    All,
    /// Installed packages.
    Installed,
    /// Available packages.
    Available,
}
```

Prefer:

```rust
/// Filter applied to package rows by install status.
pub enum InstallFilter {
    All,
    Installed,
    Available,
}
```

## Editing Workflow

1. Identify the reader: caller, maintainer, tester, or future implementer.
2. Delete comments that merely restate the code.
3. Add compact docs to public contracts and important boundaries.
4. Use headings only for facts that matter to callers.
5. Keep docs synchronized with generated CLI help and public API behavior.
6. Re-read the final comment in isolation; it should be useful in an IDE hover.
