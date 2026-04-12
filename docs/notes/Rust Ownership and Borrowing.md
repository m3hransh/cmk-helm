---
title: Rust Ownership and Borrowing
tags: [concept]
status: completed
priority: 3
publish: true
aliases: [ownership, borrow checker, borrowing]
---

# Rust Ownership and Borrowing

Ownership is Rust's most distinctive feature. It is how Rust achieves memory safety without a garbage collector. Every value has exactly one *owner*. When the owner goes out of scope, the value is dropped (freed).

---

## The Core Rules

1. Every value has exactly one owner
2. When the owner goes out of scope, the value is dropped
3. You can have either one `&mut` reference OR any number of `&` references — never both simultaneously

---

## Ownership Transfer (Move)

```rust
let s = String::from("hello");
let t = s;          // ownership MOVED to t — s is no longer valid
println!("{}", s);  // ← compile error: value used after move
```

This is a compile-time error. Rust's "move semantics" prevent double-free bugs.

---

## Borrowing With References

Instead of transferring ownership, you can *lend* a reference:

```rust
fn greet(name: &str) {        // borrows a string slice — caller keeps ownership
    println!("Hello, {name}!");
}

let s = String::from("world");
greet(&s);           // borrow — s is still valid here
println!("{}", s);   // fine: ownership was never transferred
```

**In this project:** `api::fetch_versions(base_url: &str)` takes `&str` so the caller keeps ownership of the URL string.

---

## `&self` vs `&mut self` vs `self`

In `impl App`, methods use different receivers depending on what they need:

| Receiver | What it means | Used for |
|----------|--------------|---------|
| `&self` | Read the struct, no changes | `render()` functions |
| `&mut self` | Read AND modify | Event handlers, `select_next()` |
| `self` | Consume the struct (it's moved in) | `run()` — the `App` is moved into the loop |

```rust
impl App {
    fn render(&self, frame: &mut Frame) { /* read-only */ }
    fn on_key_down(&mut self) { self.table_state.select_next(); }  // mutates
    fn run(mut self, terminal: &mut Terminal<...>) -> Result<()> { /* consumes self */ }
}
```

---

## Clone: Explicit Copying

When you need two owners of the same data, you clone:

```rust
// In App State Machine transitions:
self.screen = Screen::Configure {
    version: version.clone(),  // explicit deep copy — the original stays valid too
    edition,
    site_input: String::new(),
};
```

Rust makes you write `.clone()` explicitly — there is no silent deep copying. This makes performance costs visible in the code.

---

## Why This Matters for Rust Beginners

Coming from Python/Go/JS: those languages use a garbage collector (or GC-like runtime) to track references and free memory. Rust does this at compile time via the borrow checker. The rules feel strict at first but they prevent:
- Use-after-free bugs
- Double-free bugs
- Data races in concurrent code

The compiler error messages for ownership violations are very descriptive — read them carefully.

---

## Where It Shows Up in This Codebase

| Location | What's happening |
|---------|----------------|
| `api::fetch_versions(base_url: &str)` | Borrows URL, caller keeps ownership |
| `fn render(&self, ...)` | Read-only borrow of App |
| `fn on_key_down(&mut self)` | Mutable borrow to update state |
| `version.clone()` in state transitions | Explicit copy to give new screen variant its own data |

---

## Metadata

**Tags:** concept
**Related:** [[Rust Enums and Algebraic Data Types]], [[Rust String vs str]], [[Rust Closures]], [[App State Machine]]
