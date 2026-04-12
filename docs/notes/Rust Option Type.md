---
title: Rust Option Type
tags: [concept]
status: completed
priority: 3
publish: true
aliases: [Option, Option<T>, no null]
---

# Rust Option Type

Rust has no `null`. The concept of "a value that might not exist" is modelled explicitly as `Option<T>`. This eliminates null-pointer exceptions at the type level.

---

## The Type

```rust
// Defined in the standard library — always in scope
enum Option<T> {
    Some(T),  // there is a value
    None,     // there is no value
}
```

If a function can return "nothing", its return type must be `Option<T>`. The caller is forced to handle both cases.

---

## Common Patterns

**`.map()` — transform if Some, pass None through:**
```rust
// From src/ui/mod.rs — move up in the list
let prev = self.table_state.selected()  // returns Option<usize>
    .map(|i| i.saturating_sub(1))       // subtract 1 if Some
    .unwrap_or(0);                       // default to 0 if None
```

**`if let` — branch on Some:**
```rust
if let Some(index) = self.table_state.selected() {
    // use index here
}
```

**`unwrap_or_default()` — use the type's default value if None:**
```rust
// Non-fatal failure: if omd is unavailable, use an empty Vec
let installed = list_installed_versions().unwrap_or_default();
// Vec<String>'s default is Vec::new() — an empty Vec
```

**`?` with Option — propagate None early:**
```rust
fn first_char(s: &str) -> Option<char> {
    let c = s.chars().next()?;  // returns None early if string is empty
    Some(c.to_uppercase().next()?)
}
```

---

## `saturating_sub` — Safe Arithmetic

When navigating up in a list, you need `index - 1` but `index` is a `usize` (unsigned integer — cannot be negative). Subtracting 1 from 0 would panic in debug mode (integer overflow).

```rust
// Wrong: panics in debug, wraps to usize::MAX in release
let prev = index - 1;

// Right: clamps at 0 instead of underflowing
let prev = index.saturating_sub(1);
// 3.saturating_sub(1) == 2
// 0.saturating_sub(1) == 0  (clamped, not wrapped)
```

---

## Option vs Option in Other Languages

| Language | "no value" |
|---------|-----------|
| Python | `None` (untyped, any variable can be None) |
| TypeScript | `T \| undefined` (similar to Rust's Option) |
| Go | zero values + `nil` pointers |
| Java | `null` (causes NullPointerException) |
| Rust | `Option<T>` — the compiler forces handling |

The key difference: in Rust, if a function's return type is `T` (not `Option<T>`), there is guaranteed to be a value. You do not need defensive null checks on all function returns.

---

## Where It Shows Up in This Codebase

| Location | What None means |
|---------|----------------|
| `TableState::selected()` → `Option<usize>` | No row is currently selected |
| `list_installed_versions()` → `Result<Vec<String>>` converted to `Option` via `ok()` | `omd` not installed |
| `split_once(':')` → `Option<(&str, &str)>` | Credentials file had no `:` separator |

---

## Metadata

**Tags:** concept
**Related:** [[Rust Result Type and Error Propagation]], [[Rust Pattern Matching]], [[Rust Enums and Algebraic Data Types]]
