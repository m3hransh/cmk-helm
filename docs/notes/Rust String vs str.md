---
title: Rust String vs str
tags: [concept]
status: completed
priority: 2
publish: true
aliases: [String, &str, string types, string slice]
---

# Rust String vs `str`

Rust has two main string types. Knowing when to use each is one of the first practical Rust skills.

---

## The Two Types

| Type | What it is | Owned? | Growable? |
|------|-----------|--------|----------|
| `String` | Heap-allocated, owned UTF-8 string | Yes | Yes |
| `&str` | Borrowed reference to a UTF-8 string slice | No (borrowed) | No |

---

## Quick Rules

**Function parameters that just *read* a string → use `&str`:**
```rust
fn greet(name: &str) { println!("Hello, {name}!"); }

// Works with both String and &str:
greet("world");                          // &str literal
greet(&my_string);                       // &String coerces to &str
greet(my_string.as_str());               // explicit
```

**Storing a string in a struct → use `String`:**
```rust
struct Version {
    base: String,    // owned — the Version owns this string data
    date: String,
}
```

**Building a string dynamically → use `String`:**
```rust
let mut s = String::new();
s.push_str("2.5.0");
s.push('-');
s.push_str("2026-04-03");
```

---

## String Literals

```rust
let s = "hello";     // type is &'static str
//       ^ lives in the binary itself, valid for the entire program lifetime
```

String literals are `&str`, not `String`. They are baked into the compiled binary.

---

## Converting Between Them

```rust
// &str → String (allocates)
let owned: String = "hello".to_string();
let owned: String = "hello".into();
let owned: String = String::from("hello");

// String → &str (borrows, free)
let borrowed: &str = &my_string;
let borrowed: &str = my_string.as_str();
```

---

## Why This Matters for Rust Beginners

This is usually the first confusing thing in Rust. The rule of thumb:
- When you're **passing** a string to a function: `&str`
- When you're **storing** a string in a struct or returning an owned string: `String`

If the compiler says "expected `&str`, found `String`", add `&` or `.as_str()`. If it says "expected `String`, found `&str`", add `.to_string()`.

---

## Where It Shows Up in This Codebase

| Location | Why |
|---------|-----|
| `fetch_versions(base_url: &str)` | Borrows the URL constant — no need to own it |
| `struct Version { base: String, date: String }` | Version owns its data |
| `Row::new([v.base.as_str(), ...])` | Ratatui Row takes `&str`, so we borrow from String |
| `site_input: String` in `Screen::Configure` | Input field grows as user types |

---

## Metadata

**Tags:** concept
**Related:** [[Rust Ownership and Borrowing]], [[Rust Enums and Algebraic Data Types]]
