---
title: Git Workflow
tags: [workflow]
status: completed
priority: 2
publish: true
aliases: [git, commits, commit format]
---

# Git Workflow

---

## Commit Cadence

Commit after each logical unit of change. A "unit" is: one feature, one fix, one refactor, or one docs update. Don't batch unrelated changes into one commit.

---

## Commit Message Format

```
<type>: <short description>

- bullet point for detail
- another detail if needed
```

**Types:**

| Type | When |
|------|------|
| `feat` | New feature or screen |
| `fix` | Bug fix |
| `chore` | Maintenance (dependency updates, config) |
| `docs` | Documentation updates (Obsidian vault) |
| `refactor` | Code reorganisation — no behaviour change |
| `test` | Adding or fixing tests |

**Examples:**
```
feat: add edition picker screen

- Screen::EditionPicker variant carries version + ListState
- let-else guard pattern in on_edition_picker handler
- transitions to Configure on Enter, back to VersionList on Esc
```

```
fix: restore terminal on panic

- use defer-style cleanup via std::panic::catch_unwind
- ensures alternate screen is always exited
```

---

## Docs Update Rule

Update the Obsidian vault (`docs/notes/`) whenever:
- A new Rust pattern appears in the code
- An architectural decision is made
- A new feature is added

Use `docs: update vault` as the commit type for vault-only commits.

---

## Cargo Format Before Commit

Always run `cargo fmt` before committing. Clippy warnings are treated as errors in `nix flake check` — fix them before pushing.

---

## Metadata

**Tags:** workflow
**Related:** [[Development Workflow]], [[Testing Workflow]]
