---
title: Rust LazyLock Static Variables
tags: [concept]
status: completed
priority: 2
publish: true
aliases: [LazyLock, lazy static, static variables, once_cell]
---

# Rust LazyLock Static Variables

`std::sync::LazyLock<T>` initialises a value the *first time it is accessed* and reuses it for the remainder of the program. It is the standard way to create global singletons that require runtime initialisation (like compiled regexes).

---

## The Problem It Solves

Regex compilation is expensive. If you compile a pattern inside a function that gets called many times, you pay the cost every call:

```rust
// Bad — compiles the regex on every call
fn parse_versions(html: &str) -> Vec<String> {
    let re = Regex::new(r#"href="([^"/]+)/""#).unwrap();  // expensive!
    re.captures_iter(html).map(|c| c[1].to_string()).collect()
}
```

`LazyLock` compiles it once at first access and caches it forever:

```rust
// Good — compiles once, reuses on every call
use std::sync::LazyLock;
use regex::Regex;

static ROW_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"href="([^"/]+)/""#).unwrap()
    //                                ^^^^^^^^ safe: pattern is a constant, cannot fail
});

fn parse_versions(html: &str) -> Vec<String> {
    ROW_RE.captures_iter(html).map(|c| c[1].to_string()).collect()
}
```

---

## Why `static` and Not `let`?

`static` variables have *program lifetime* — they live as long as the binary runs. This is required for types that need to be globally accessible without being tied to any particular scope.

`LazyLock` must be `static` because:
1. It needs to persist across function calls
2. `LazyLock` is `Sync` (thread-safe), suitable for statics

---

## The `.unwrap()` in `LazyLock::new`

The only `.unwrap()` calls in this codebase are inside `LazyLock::new(|| Regex::new(...).unwrap())`. This is safe because:
- The regex pattern is a string literal known at compile time
- If the pattern is syntactically invalid, the test suite catches it immediately
- There is no runtime input that could make a constant pattern fail

This exception to the "no .unwrap()" rule is explicitly documented in the [[Error Handling Strategy]].

---

## Stable Since Rust 1.80

`LazyLock` is in the standard library since Rust 1.80. Before that, the community used the `once_cell` or `lazy_static` crates for the same purpose. You may see those in older code — `LazyLock` is the modern replacement.

---

## Thread Safety

`LazyLock<T>` is thread-safe (it implements `Sync`). The initialisation closure runs exactly once, even if multiple threads race to access the variable simultaneously. This is guaranteed by the internal synchronisation primitive.

---

## Where It Shows Up in This Codebase

```rust
// src/api/mod.rs
static ROW_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?x)
        href="
        (?P<base>\d+\.\d+\.\d+)
        (?:
            -(?P<date>\d{4}\.\d{2}\.\d{2})  |  # daily build
            p(?P<patch>\d+)                  |  # stable patch
            b(?P<beta>\d+)                      # beta
        )
        /"
    "#).unwrap()
});
```

---

## Metadata

**Tags:** concept
**Related:** [[Rust Ownership and Borrowing]], [[Data Flow]], [[Error Handling Strategy]]
