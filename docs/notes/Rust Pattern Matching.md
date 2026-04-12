---
title: Rust Pattern Matching
tags: [concept]
status: completed
priority: 2
publish: true
aliases: [match, pattern matching, if let, matches!]
---

# Rust Pattern Matching

Pattern matching in Rust is a first-class language feature. `match` is exhaustive — the compiler errors if any case is unhandled. Combined with Rust's rich enum system, it replaces most conditional logic.

---

## `match` — Exhaustive Branching

```rust
match &self.screen {
    Screen::VersionList => self.render_version_list(frame, area),
    Screen::EditionPicker { version, list_state } => {
        // `version` and `list_state` are destructured and in scope here
        self.render_edition_picker(frame, area, version, list_state)
    }
    Screen::Configure { version, edition, site_input } => {
        self.render_configure(frame, area, version, edition, site_input)
    }
    Screen::Installing { config } => self.render_installing(frame, area, config),
    // No `_ => {}` needed — all variants are covered.
    // Add a new Screen variant and the compiler will error here until you handle it.
}
```

**Match guards** add extra conditions to a branch:

```rust
match key {
    KeyCode::Char('q') => self.should_quit = true,
    KeyCode::Char(c) if c.is_ascii_digit() => handle_digit(c),
    //                ^ guard: only matches if c is a digit
    KeyCode::Enter => self.advance(),
    _ => {}  // catch-all — when not all cases are handled explicitly
}
```

---

## `if let` — Single-Variant Match

When you only care about one variant and want to ignore the rest:

```rust
if let Some(index) = self.table_state.selected() {
    // `index` is in scope here
    do_something_with(index);
}
// if selected() returns None, this block is skipped silently
```

Equivalent to:
```rust
match self.table_state.selected() {
    Some(index) => do_something_with(index),
    None => {}
}
```

---

## `let … else` — Guard Clause

See [[Rust Let Else Pattern]] for the full explanation. Short version:

```rust
let Screen::Configure { site_input, .. } = &mut self.screen else { return };
// site_input is in scope, early return if not Configure
```

---

## `matches!` Macro — Pattern in Assertions

`matches!(expr, pattern)` returns `true` if `expr` matches `pattern`. Useful in tests and `.filter()` calls:

```rust
// In tests
assert!(matches!(
    &versions[0].kind,
    VersionKind::Daily { date } if date == "2026.04.03"
));

// In iterators
let dailies: Vec<_> = versions
    .iter()
    .filter(|v| matches!(v.kind, VersionKind::Daily { .. }))
    .collect();
```

The `..` inside a pattern means "I don't care about the other fields". The `if` adds a guard.

---

## Destructuring Structs and Tuples

Pattern matching works beyond enums:

```rust
// Destructure a tuple
let (username, password) = credentials;

// Destructure a struct
let Version { base, kind, timestamp } = &version;

// Partial destructure with `..`
let Version { base, .. } = &version;  // only care about `base`
```

---

## Nested Patterns

```rust
match &version.kind {
    VersionKind::Daily { date } if date.starts_with("2026") => {
        println!("Recent daily: {date}");
    }
    VersionKind::StablePatch { patch } => {
        println!("Stable: p{patch}");
    }
    _ => {}
}
```

---

## Metadata

**Tags:** concept
**Related:** [[Rust Enums and Algebraic Data Types]], [[Rust Let Else Pattern]], [[App State Machine]], [[Rust Option Type]]
