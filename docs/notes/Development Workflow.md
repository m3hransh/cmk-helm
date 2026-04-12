---
title: Development Workflow
tags: [workflow]
status: completed
priority: 3
publish: true
aliases: [dev workflow, daily workflow, cargo commands]
---

# Development Workflow

Day-to-day workflow for developing CMK Cockpit.

---

## Enter the Dev Shell

```bash
# Manual:
nix develop

# Automatic (if you have direnv configured):
cd ~/Projects/cmk-helm   # shell activates automatically
```

Once in the dev shell, all tools are on PATH: `cargo`, `rustc`, `rust-analyzer`, `clippy`, `rustfmt`, `cargo-watch`, `cargo-expand`.

---

## Core Commands

| Command | Use |
|---------|-----|
| `cargo run` | Build and run (slow first time, fast after) |
| `cargo watch -x run` | Auto-rebuild and restart on any file save |
| `cargo test` | Run all unit tests |
| `cargo clippy` | Lint — warnings are errors in CI |
| `cargo fmt` | Format — run before every commit |
| `cargo expand api` | See what macros in `src/api/mod.rs` generate |
| `nix flake check` | Full CI check: clippy + fmt + nix build |

---

## The Inner Loop

```bash
# In one terminal:
cargo watch -x run

# Edit code in your editor — app restarts on save
# Check test output:
cargo test
```

`cargo watch -x run` is the fastest feedback loop. It uses `cargo-watch` (installed in the dev shell) to watch for file changes and re-run `cargo run`.

---

## Adding a New Feature

1. Identify which module it belongs to: `api/`, `ui/`, or `installer/` (see [[Module Boundaries]])
2. If it's a new screen: add a variant to `Screen` enum in `ui/mod.rs` — the compiler will tell you everywhere you need to update (see [[App State Machine]])
3. Write the feature
4. Add unit tests in a `#[cfg(test)]` block
5. Run `cargo test` and `cargo clippy`
6. Run `cargo fmt`
7. Commit (see [[Git Workflow]])
8. Update relevant notes in `docs/notes/`

---

## Macro Exploration

When you encounter a `#[derive(...)]` or other macro and want to understand what it generates:

```bash
cargo expand api       # expand src/api/mod.rs
cargo expand ui        # expand src/ui/mod.rs
cargo expand           # expand main.rs
```

See [[Rust Derive Macros]] for what the common derives generate.

---

## Updating Docs

After any significant change, update `docs/notes/`. The [[Git Workflow]] convention is a `docs:` commit for vault-only updates. Don't let the vault go stale — it's a learning resource, not an afterthought.

---

## Metadata

**Tags:** workflow
**Related:** [[Getting Started]], [[Git Workflow]], [[Testing Workflow]], [[Nix Flakes]]
