---
title: Rust Closures
tags: [concept]
status: completed
priority: 2
publish: true
aliases: [closures, anonymous functions, lambdas]
---

# Rust Closures

A closure is an anonymous function that can *capture* variables from its surrounding scope. In Rust, closures are statically typed and zero-cost — they compile down to the same code as a regular function call.

---

## Basic Syntax

```rust
// Regular function
fn add_one(x: i32) -> i32 { x + 1 }

// Equivalent closure
let add_one = |x: i32| x + 1;

// Multi-line closure
let add_one = |x: i32| {
    let result = x + 1;
    result
};
```

The `|arg| body` syntax is Rust's equivalent of Python's `lambda`. Types are usually inferred.

---

## Capturing Variables

The key difference from regular functions: closures can use variables from the enclosing scope.

```rust
let threshold = 5;
let is_big = |x: i32| x > threshold;  // captures `threshold` by reference
println!("{}", is_big(10));  // true
```

---

## How Closures Are Used in This Codebase

**`terminal.draw()` — captures `&mut self`:**
```rust
// src/ui/mod.rs
terminal.draw(|frame| self.render(frame))?;
//            ^ closure captures &mut self to call render
```

**`.with_context()` — lazy error message:**
```rust
// src/api/mod.rs
fs::read_to_string(&path)
    .with_context(|| format!("Failed to read from {}", path.display()))?;
//               ^ closure only runs if there's an error (lazy evaluation)
```

**`.map()` on iterators:**
```rust
// Transform each version into a table row
let rows: Vec<Row> = versions
    .iter()
    .map(|v| Row::new([v.base.as_str(), v.date.as_str()]))
    .collect();
```

**`.filter()`:**
```rust
let dailies: Vec<&Version> = versions
    .iter()
    .filter(|v| matches!(v.kind, VersionKind::Daily { .. }))
    .collect();
```

---

## Closure Traits: `Fn`, `FnMut`, `FnOnce`

Rust has three closure traits that describe how a closure captures its environment:

| Trait | Captures by | Can call | When used |
|-------|-------------|---------|-----------|
| `Fn` | Reference `&T` | Multiple times | Most closures: `.map()`, `.filter()` |
| `FnMut` | Mutable reference `&mut T` | Multiple times | Closures that modify captured state |
| `FnOnce` | Value (move) | Once only | Closures that consume captured values |

You rarely need to think about this — Rust infers the right trait. You'll encounter these when a function takes a closure as a parameter:

```rust
fn apply_twice<F: Fn(i32) -> i32>(f: F, x: i32) -> i32 {
    f(f(x))
}
```

---

## Move Closures

Sometimes you need the closure to own the captured value rather than borrow it:

```rust
let name = String::from("Alice");
let greet = move || println!("Hello, {name}!");  // `name` is moved into the closure
// `name` is no longer valid here
```

This is needed when the closure outlives the current scope — for example, when sending it to another thread.

---

## Metadata

**Tags:** concept
**Related:** [[Rust Iterators]], [[Rust Ownership and Borrowing]], [[Rust Result Type and Error Propagation]]
