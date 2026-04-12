---
title: anyhow Error Handling
tags: [tool]
status: completed
priority: 2
publish: true
aliases: [anyhow, error handling crate]
---

# anyhow

`anyhow` is the error handling library used throughout this codebase. It provides `anyhow::Error` — an opaque error type that can wrap any error — and ergonomic context chaining.

---

## Why anyhow (Not Custom Error Types)?

This is an **application**, not a library. Application code typically doesn't need downstream code to programmatically match on error variants — it just needs good error messages for humans. `anyhow` gives that with zero boilerplate.

Libraries should use `thiserror` (structured, matchable errors). Applications should use `anyhow` (human-readable chains). This is a widely accepted Rust convention.

---

## Core API

```toml
# Cargo.toml
anyhow = "1"
```

```rust
use anyhow::{anyhow, bail, Context, Result};

// Result<T> is shorthand for Result<T, anyhow::Error>
fn read_credentials() -> Result<(String, String)> {

    // .context() — add a static string layer
    let raw = fs::read_to_string(path).context("Could not read credentials file")?;

    // .with_context(|| ...) — add a dynamic string layer (lazy, closure only runs on error)
    let raw = fs::read_to_string(path)
        .with_context(|| format!("Could not read {}", path.display()))?;

    // bail! — return an error immediately with a formatted message
    let Some((user, pass)) = raw.split_once(':') else {
        bail!("Credentials file must contain 'username:password'");
    };

    // anyhow! — create an anyhow::Error from a message
    if user.is_empty() {
        return Err(anyhow!("Username cannot be empty"));
    }

    Ok((user.to_string(), pass.to_string()))
}
```

---

## Error Chain Display

When errors propagate through multiple layers with `.with_context()`, `anyhow` prints them as a chain:

```
Error: Failed to initialise app

Caused by:
    0: Failed to fetch versions from https://download.checkmk.com/checkmk/
    1: Failed to reach server
    2: Connection refused (os error 111)
```

The first line is the outermost context; the last is the root cause.

---

## The `?` Operator With anyhow

`anyhow::Error` implements `From<E>` for any error type `E` that implements `std::error::Error`. This means `?` auto-converts any error into an `anyhow::Error`:

```rust
let n = "42".parse::<i32>()?;  // ParseIntError auto-converts to anyhow::Error
```

No manual `map_err` needed.

---

## anyhow in main()

`fn main() -> anyhow::Result<()>` is fully supported. If main returns `Err`, Rust prints the error (via `Debug`) and exits with code 1. With `anyhow`, the `Debug` output shows the full error chain.

---

## Metadata

**Tags:** tool
**Reference:** https://crates.io/crates/anyhow, https://docs.rs/anyhow
**Related:** [[Rust Result Type and Error Propagation]], [[Error Handling Strategy]]
