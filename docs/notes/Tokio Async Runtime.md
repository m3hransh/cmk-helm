---
title: Tokio Async Runtime
tags: [tool]
status: completed
priority: 2
publish: true
aliases: [tokio, async runtime, executor]
---

# Tokio

Tokio is the async runtime (executor) for Rust. It drives `async`/`await` — without a runtime, futures do nothing. Tokio is the de-facto standard for async Rust.

---

## Why We Need It

Rust's `async fn` returns a *future* — a description of work to be done. A runtime is needed to actually *run* the futures. `reqwest` (our HTTP client) is async-only, which is why Tokio is required even though we only make one network call.

See [[Rust Async Await]] for how async/await works in Rust.

---

## How We Use It

**`#[tokio::main]`** — macro that turns `async main()` into a regular synchronous entry point:

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let versions = api::fetch_versions(CMK_DOWNLOAD_URL).await?;
    // ...
    Ok(())
}
```

**`#[tokio::test]`** — macro for async test functions:

```rust
#[tokio::test]
async fn test_version_parsing() {
    let result = api::fetch_versions("http://...").await;
    assert!(result.is_ok());
}
```

---

## Single-Threaded vs Multi-Threaded

Tokio has two runtime flavours:

| Flavour | How | When to use |
|---------|-----|------------|
| `#[tokio::main]` (default) | Multi-threaded thread pool | Most applications |
| `#[tokio::main(flavor = "current_thread")]` | Single thread | Simple apps, WebAssembly |

We use the default (multi-threaded). For this app it makes no practical difference — we never spawn concurrent tasks. But `reqwest` requires multi-thread by default.

---

## Cargo.toml Configuration

```toml
tokio = { version = "1", features = ["full"] }
```

`features = ["full"]` enables all Tokio features: async I/O, timers, task spawning, channels, etc. For a production app you'd list only what you need; for a small dev tool `full` is fine.

---

## What We Don't Use (Yet)

- `tokio::spawn` — spawning concurrent background tasks
- `tokio::sync::mpsc` — channel for streaming install output to the UI
- `tokio::time` — async timers

The install-output streaming case (showing live `cmk-dev-install` output in the UI) would use `tokio::process::Command` + `mpsc` channels. See [[Async Boundary Design]].

---

## Metadata

**Tags:** tool
**Reference:** https://tokio.rs, https://crates.io/crates/tokio
**Related:** [[Rust Async Await]], [[Async Boundary Design]], [[Reqwest HTTP Client]]
