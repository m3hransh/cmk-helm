---
title: Rust Traits
tags: [concept]
status: backlog
priority: 2
publish: true
aliases: [traits, interfaces, trait bounds]
---

# Rust Traits

A trait defines a set of methods (a contract) that types can implement. Traits are Rust's equivalent of interfaces in Go/Java or abstract base classes in Python. They are how polymorphism works in Rust.

---

## Basic Definition

```rust
// Define a trait
trait Describe {
    fn describe(&self) -> String;
}

// Implement it for a type
impl Describe for Version {
    fn describe(&self) -> String {
        format!("{} ({})", self.base, self.timestamp)
    }
}

// Use it
let v = Version { base: "2.5.0".into(), /* ... */ };
println!("{}", v.describe());
```

---

## Trait Bounds — Generics With Constraints

Traits let you write generic code that works for any type that satisfies the trait:

```rust
// T must implement both Display and Debug
fn print_info<T: std::fmt::Display + std::fmt::Debug>(item: &T) {
    println!("Display: {item}");
    println!("Debug:   {item:?}");
}
```

This is how Ratatui accepts different widget types — functions take `impl Widget` as a parameter.

---

## Common Standard Library Traits

| Trait | What it enables | Derivable? |
|-------|----------------|-----------|
| `Debug` | `{:?}` formatting | Yes — `#[derive(Debug)]` |
| `Display` | `{}` formatting | No — write manually |
| `Clone` | `.clone()` | Yes |
| `PartialEq` / `Eq` | `==` operator | Yes |
| `Hash` | Use as HashMap key | Yes |
| `Default` | `T::default()` | Yes (if all fields implement it) |
| `From<T>` / `Into<T>` | Type conversion | Implement `From`, get `Into` free |

See [[Rust Derive Macros]] for the ones that can be derived automatically.

---

## Traits vs Structs

- A **struct** defines data (what a type *has*)
- A **trait** defines behaviour (what a type *can do*)

A type can implement many traits. Trait implementation is explicit — a type only gets a trait's methods if you write `impl TraitName for TypeName`.

---

## What to Explore Next

- `impl Trait` syntax in function signatures (return type polymorphism)
- Trait objects (`dyn Trait`) — runtime polymorphism
- Blanket implementations — `impl<T: Debug> MyTrait for T`
- `Iterator` trait — the basis of the iterator system (see [[Rust Iterators]])

---

## Metadata

**Tags:** concept
**Related:** [[Rust Derive Macros]], [[Rust Iterators]], [[Ratatui TUI Framework]]
