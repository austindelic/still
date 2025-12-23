# Still

**Still** is a fast, parallel, Rust-based package and toolchain manager that combines the Homebrew ecosystem with mise-style per-project environments.

> One CLI. One cache. One lockfile.  
> System packages + runtime versions, installed **deterministically and in parallel**.

---

## Why Still?

Homebrew is great — but:

- installs are mostly serial
- global state is hard to reason about
- reproducible dev environments are bolted on, not built in

mise is great — but:

- it doesn’t manage system packages
- you still need brew (or something else)

**Still unifies both.**

---

## What it does

- ✅ Installs Homebrew formulae (and later casks)
- ✅ Manages per-project tool versions (Node, Python, Rust, etc.)
- ✅ Downloads dependencies **in parallel**
- ✅ Uses a content-addressed cache (no duplicate work)
- ✅ Produces deterministic installs via a lockfile
- ✅ Safe, atomic installs (no half-broken systems)

---

## Example

```bash
still init
still install ripgrep jq
still use node@20 python@3.12
still sync
```
