---
title: Rust Derive Macros
tags: [concept]
status: completed
priority: 2
publish: true
aliases: [derive, #[derive], proc macros, derive macros]
---

# Rust Derive Macros

`#[derive(...)]` is a code-generation attribute. It tells the compiler to automatically implement common traits for a type based on its fields/variants. This eliminates boilerplate while keeping the implementations correct.

---

## Syntax

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Version {
    pub base: String,
    pub kind: VersionKind,
    pub timestamp: String,
}
```

The compiler generates implementations of `Debug`, `Clone`, `PartialEq`, `Eq`, and `Hash` for `Version`, doing the right thing for each field.

---

## Common Derives and What They Give You

| Derive | What you can do |
|--------|----------------|
| `Debug` | `println!("{:?}", v)` — prints struct fields for debugging |
| `Clone` | `v.clone()` — makes a deep copy |
| `PartialEq` | `v1 == v2` — field-by-field equality comparison |
| `Eq` | Marker: equality is total (no NaN-like edge cases). Requires `PartialEq`. |
| `Hash` | Use as a `HashMap` key |
| `Default` | `Version::default()` — all fields set to their default values |
| `Serialize` / `Deserialize` | JSON/TOML serialisation via `serde` (not used currently) |

---

## How to See What Gets Generated

The `cargo-expand` tool (included in the Nix dev shell) shows you exactly what the macro generates:

```bash
cargo expand api   # expand all macros in src/api/mod.rs
```

For `#[derive(Debug)]` on a struct, you'd see something like:

```rust
impl std::fmt::Debug for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Version")
            .field("base", &self.base)
            .field("kind", &self.kind)
            .field("timestamp", &self.timestamp)
            .finish()
    }
}
```

`cargo expand` is invaluable for understanding what macros do. Use it freely.

---

## Derives on Enums

Derives work on enums too:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edition {
    Raw,
    Free,
    Cloud,
    Enterprise,
    MSP,
}
```

`Debug` on an enum prints the variant name: `println!("{:?}", Edition::Cloud)` prints `"Cloud"`.

`PartialEq` on an enum compares variant identity (and any carried data).

---

## Traits: The Underlying Concept

`derive` generates *trait implementations*. A trait is Rust's equivalent of an interface — it defines a set of methods a type must provide. See [[Rust Traits]] for a deeper explanation.

When you write `#[derive(Clone)]`, Rust generates `impl Clone for Version { ... }`. You could write this by hand, but `derive` does it correctly for you.

---

## Metadata

**Tags:** concept
**Related:** [[Rust Traits]], [[Rust Enums and Algebraic Data Types]], [[Rust Ownership and Borrowing]]
