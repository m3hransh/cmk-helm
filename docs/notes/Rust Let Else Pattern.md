---
title: Rust Let Else Pattern
tags: [concept]
status: completed
priority: 2
publish: true
aliases: [let else, let-else, guard clause, destructuring]
---

# Rust Let Else Pattern

`let … else` is a guard-clause syntax for destructuring a pattern that might not match. If the pattern fails to match, the `else` block must diverge (return, break, continue, or panic). Stable since Rust 1.65.

---

## Syntax

```rust
let PATTERN = EXPRESSION else {
    // must diverge: return, break, continue, or panic!
    return;
};
// PATTERN bindings are now in scope here
```

---

## Without Let-Else (Nested Alternative)

Before `let … else`, guarded enum access required nested `if let` or a `match`:

```rust
fn render_configure(&self, frame: &mut Frame, area: Rect) {
    if let Screen::Configure { version, edition, site_input } = &self.screen {
        // Everything is indented one level deeper
        // If there were multiple guards, this nests further
        let block = Block::bordered().title(format!("{} – {}", version.base, edition));
        // ... more rendering code
    }
    // Silently does nothing if screen != Configure — easy to miss
}
```

---

## With Let-Else (Guard Clause)

```rust
fn render_configure(&self, frame: &mut Frame, area: Rect) {
    // "I only do work on the Configure screen — bail out immediately otherwise"
    let Screen::Configure { version, edition, site_input } = &self.screen else { return };

    // version, edition, site_input are now in scope — no extra indentation
    let block = Block::bordered().title(format!("{} – {}", version.base, edition));
    // ... more rendering code
}
```

This is the **early return** pattern, equivalent to how you'd guard a condition in Go or Python. The `else { return }` makes the intent explicit: "if this condition isn't met, this function has nothing to do".

---

## Why It's Better

1. **Less indentation** — the happy path stays at the top level
2. **Clear intent** — the `else` block documents the "bail out" condition explicitly
3. **Bindings are in scope** — `version`, `edition`, `site_input` are available for the rest of the function
4. **Forced divergence** — the compiler errors if the `else` block could fall through, preventing silent "did nothing" bugs

---

## Other Diverging Forms

```rust
// return (most common in this codebase)
let Screen::Configure { .. } = &self.screen else { return };

// break (inside a loop)
let Some(item) = iter.next() else { break };

// panic (for "this should never happen" assertions)
let Ok(n) = value.parse::<i32>() else { panic!("BUG: expected integer") };

// continue (skip this loop iteration)
let Some(valid) = maybe_item else { continue };
```

---

## Where It Shows Up in This Codebase

Each input handler (`on_edition_picker`, `on_configure`, `on_installing`) uses `let … else { return }` to guard against being called on the wrong screen:

```rust
fn on_edition_picker(&mut self, key: KeyCode) {
    let Screen::EditionPicker { version, list_state } = &mut self.screen else { return };
    // ... handle key for edition picker
}

fn on_configure(&mut self, key: KeyCode) {
    let Screen::Configure { version, edition, site_input } = &mut self.screen else { return };
    // ... handle key for configure screen
}
```

---

## Metadata

**Tags:** concept
**Related:** [[Rust Pattern Matching]], [[Rust Enums and Algebraic Data Types]], [[App State Machine]]
