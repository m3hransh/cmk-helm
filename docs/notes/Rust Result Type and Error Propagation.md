---
title: Rust Result Type and Error Propagation
tags: [concept]
status: completed
priority: 3
publish: true
aliases: [Result, Result<T>, question mark operator, ? operator, error propagation]
---

# Rust Result Type and Error Propagation

Functions that can fail return `Result<T, E>`. The `?` operator propagates errors up the call stack, eliminating the need for nested error checks.

---

## The Type

```rust
// Defined in the standard library — always in scope
enum Result<T, E> {
    Ok(T),   // success, contains a value of type T
    Err(E),  // failure, contains an error of type E
}
```

In this codebase, we use `anyhow::Result<T>` which is shorthand for `Result<T, anyhow::Error>`. `anyhow::Error` can wrap any error type.

---

## The `?` Operator

`?` is syntactic sugar for "if this is Ok, give me the value; if Err, return the error from this function":

```rust
// With ?
fn fetch(url: &str) -> Result<String> {
    let response = reqwest::get(url).await?;   // returns Err early if request fails
    let text = response.text().await?;          // returns Err early if reading fails
    Ok(text)
}

// What ? expands to (roughly):
let response = match reqwest::get(url).await {
    Ok(r) => r,
    Err(e) => return Err(e.into()),
};
```

`?` only works in functions that return `Result` (or `Option`).

---

## Adding Context With `anyhow`

Raw error messages like "No such file or directory" are unhelpful. `.with_context()` adds a layer explaining *what you were trying to do*:

```rust
use anyhow::Context;

let content = fs::read_to_string(&path)
    .with_context(|| format!("Failed to read credentials from {}", path.display()))?;
```

The result is a chain:
```
Error: Failed to read credentials from /home/user/.cmk-credentials

Caused by:
    No such file or directory (os error 2)
```

`.with_context(|| ...)` takes a closure — the string is only allocated if an error actually occurred (lazy evaluation, see [[Rust Closures]]).

`.context("static string")` is the non-lazy version for string literals.

---

## Propagation Chain

Errors bubble up with context added at each level:

```
fs::read_to_string()
  → Err("No such file")
      + .with_context("Failed to read credentials")
        → Err("Failed to read credentials\n  caused by: No such file")
            + propagated via ? in read_credentials()
              → Err in fetch_versions()
                  + .with_context("Failed to initialise HTTP client")
                    → Err in main()
                      → printed to stderr, process exits non-zero
```

---

## `main() -> Result<()>`

When `main()` returns `Result<()>`, Rust's runtime prints the error chain automatically if it's `Err`:

```rust
fn main() -> anyhow::Result<()> {
    let versions = api::fetch_versions(URL).await?;
    // if fetch_versions fails, ? returns Err, main() returns it,
    // Rust prints the anyhow error chain and exits with code 1
    Ok(())
}
```

---

## Difference from Exception-Based Languages

| Language | Failure model |
|---------|--------------|
| Python/Java | Exceptions — thrown and caught, can be silently swallowed |
| Go | `(value, error)` tuples — errors are values but easy to ignore |
| Rust | `Result<T, E>` — errors are values; ignoring them is a compiler warning |

In Rust, you cannot silently ignore an error. If you call a function returning `Result` and don't use the value, the compiler warns with `#[must_use]`.

---

## Metadata

**Tags:** concept
**Related:** [[Rust Option Type]], [[anyhow Error Handling]], [[Error Handling Strategy]], [[Rust Closures]]
