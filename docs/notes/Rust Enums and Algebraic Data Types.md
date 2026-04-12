---
title: Rust Enums and Algebraic Data Types
tags: [concept]
status: completed
priority: 3
publish: true
aliases: [enums, ADT, algebraic data types, tagged union]
---

# Rust Enums and Algebraic Data Types

Rust enums are fundamentally different from C/Java enums. Each variant can carry its own data, making them *algebraic data types* (also called tagged unions or sum types). This is one of the most powerful features in the language.

---

## C Enum vs Rust Enum

```c
// C enum — just named integers
enum Color { Red, Green, Blue };
```

```rust
// Rust enum — each variant can have different data
enum Screen {
    VersionList,                                          // no data
    EditionPicker { version: Version, list_state: ListState },  // named fields
    Configure { version: Version, edition: Edition, site_input: String }, // named fields
    Installing { config: InstallConfig },                 // named field
}
```

A `Screen` value is *one of* these variants, carrying exactly the data for that variant. It's like a type-safe union.

---

## Pattern Matching Is Exhaustive

```rust
match &self.screen {
    Screen::VersionList => { /* draw version table */ }
    Screen::EditionPicker { version, list_state } => { /* draw edition list */ }
    Screen::Configure { version, edition, site_input } => { /* draw form */ }
    Screen::Installing { config } => { /* draw progress */ }
    // No `_` needed — all variants are covered.
    // If you add a new Screen variant, the compiler ERRORS here until you handle it.
}
```

This is a compile-time safety net: adding a new screen state forces you to handle it everywhere. You can never forget a case.

---

## Illegal States Unrepresentable

The most important design principle this enables: **make invalid states unrepresentable**.

**Bad (struct with optional fields):**
```rust
struct App {
    screen_id: u8,
    selected_version: Option<Version>,   // may or may not be set
    selected_edition: Option<Edition>,   // may or may not be set
    // runtime panic waiting to happen: selected_version.unwrap() on screen 0
}
```

**Good (enum with required fields per variant):**
```rust
enum Screen {
    VersionList,
    EditionPicker { version: Version, list_state: ListState },
    // There is no way to reach Configure without both version and edition.
    Configure { version: Version, edition: Edition, site_input: String },
}
```

If the code tries to access `site_input` while on `VersionList`, the compiler rejects it. There is no `site_input` field on `VersionList`.

---

## `VersionKind` — Another Enum in This Codebase

```rust
// src/api/mod.rs
#[derive(Debug, Clone, PartialEq, Eq)]
enum VersionKind {
    Daily { date: String },    // e.g. 2026.04.03
    StablePatch { patch: String }, // e.g. p24
    Beta { suffix: String },   // e.g. b2
}
```

And `Edition`:

```rust
enum Edition {
    Raw,        // check-mk-raw
    Free,       // check-mk-free
    Cloud,      // check-mk-cloud
    Enterprise, // check-mk-enterprise
    MSP,        // check-mk-managed-services
}
```

Editions are parsed from `.deb` filenames inside version directories. Representing them as an enum instead of a `String` means the compiler catches misspellings at compile time.

---

## How to Destructure Enum Variants

**`match` — handle all variants:**
```rust
match version.kind {
    VersionKind::Daily { date } => format!("{base}-{date}"),
    VersionKind::StablePatch { patch } => format!("{base}{patch}"),
    VersionKind::Beta { suffix } => format!("{base}{suffix}"),
}
```

**`if let` — handle one variant, ignore the rest:**
```rust
if let Screen::Configure { site_input, .. } = &mut self.screen {
    site_input.push(c);  // only runs if we're in Configure
}
```

**`let … else` — guard clause pattern (see [[Rust Let Else Pattern]]):**
```rust
let Screen::Configure { version, edition, site_input } = &self.screen else { return };
// now version, edition, site_input are in scope
```

---

## Metadata

**Tags:** concept
**Related:** [[Rust Pattern Matching]], [[Rust Let Else Pattern]], [[App State Machine]], [[Rust Ownership and Borrowing]]
