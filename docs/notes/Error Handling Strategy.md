---
title: Error Handling Strategy
tags: [architecture]
status: completed
priority: 2
publish: true
aliases: [error handling, anyhow, Result]
---

# Error Handling Strategy

All public functions in this codebase return `anyhow::Result<T>`. Errors are propagated upward with `?` and annotated with context at each level.

---

## The Pattern

```rust
use anyhow::{Context, Result};

fn read_credentials(path: &Path) -> Result<(String, String)> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read credentials from {}", path.display()))?;
    //  ^ adds a human-readable layer before propagating

    let (user, pass) = content
        .trim()
        .split_once(':')
        .context("Credentials file must be in 'username:password' format")?;

    Ok((user.to_string(), pass.to_string()))
}
```

If `fs::read_to_string` fails, the error chain looks like:
```
Error: Failed to read credentials from /home/user/.cmk-credentials

Caused by:
    No such file or directory (os error 2)
```

The `?` operator propagates, `.with_context()` adds a layer. See [[Rust Result Type and Error Propagation]].

---

## Rules

**Use `anyhow::Result<T>` everywhere** — not `Box<dyn Error>`, not custom error types. This is an application, not a library. `anyhow` gives a clean error chain with zero boilerplate.

**Use `.with_context(|| ...)` before `?`** — the closure is only evaluated if an error occurs (lazy), and it adds the "what were we trying to do" context that makes error messages useful.

**No `.unwrap()` in production paths** — two exceptions:
1. `Regex::new(PATTERN).unwrap()` inside a `LazyLock` — the pattern is a compile-time constant and cannot fail
2. Test code — panics are fine in tests

**Non-fatal failures use `.unwrap_or_default()`** — if `omd` is not installed, `list_installed_versions()` returns `Vec::new()` instead of crashing. The UI shows "(none found)" and the app continues. See [[Rust Option Type]].

---

## Where Errors Are Handled

```
api::fetch_versions()      →  Result<Vec<VersionGroup>>
  │  propagates with ?
  ▼
main()
  │  if Err: prints error chain and exits with non-zero code
  ▼  (anyhow does this automatically with `fn main() -> Result<()>`)

installer::list_installed_versions()  →  Result<Vec<String>>
  │  non-fatal: caller uses .unwrap_or_default()
  ▼
App::installed_versions = Vec::new()   (empty panel shown, no crash)
```

---

## Rust Concepts at Work Here

| Concept | Where |
|---------|-------|
| [[Rust Result Type and Error Propagation]] | `Result<T>`, `?`, `.with_context()` |
| [[anyhow Error Handling]] | The `anyhow` crate itself |
| [[Rust Option Type]] | `.unwrap_or_default()` for non-fatal paths |

---

## Metadata

**Tags:** architecture
**Related:** [[Rust Result Type and Error Propagation]], [[anyhow Error Handling]], [[Module Boundaries]]
