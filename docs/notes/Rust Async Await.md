---
title: Rust Async Await
tags: [concept]
status: completed
priority: 2
publish: true
aliases: [async, await, async/await, futures]
---

# Rust Async Await

Async/await is Rust's mechanism for writing non-blocking I/O code that reads like synchronous code. An `async fn` returns a *future* — a value that represents a computation that hasn't finished yet. `.await` suspends execution until the future completes.

---

## The Basics

```rust
// A regular (synchronous) function
fn add(a: i32, b: i32) -> i32 {
    a + b
}

// An async function — returns a Future<Output = i32>
async fn fetch_number() -> i32 {
    // .await suspends here until the HTTP request finishes
    // other tasks can run while we wait
    reqwest::get("http://example.com/number")
        .await
        .unwrap()
        .text()
        .await
        .unwrap()
        .parse()
        .unwrap()
}

// Calling it:
let n = fetch_number().await;  // must be inside another async fn
```

---

## Why Async Exists

Async I/O allows a single thread to handle many concurrent operations. While waiting for an HTTP response, the thread can work on something else — without spawning OS threads.

For CMK Cockpit, we only have one async operation (fetching the version list at startup). We don't need concurrency. But `reqwest`'s HTTP client is async-only, which is why we need the Tokio runtime at all.

---

## Tokio: The Runtime

Rust async does nothing on its own. You need an *executor* — a runtime that actually polls futures and drives them to completion. We use [[Tokio Async Runtime]].

```rust
// #[tokio::main] creates a Tokio runtime and runs the async main function
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let versions = api::fetch_versions(URL).await?;
    //                                        ^ suspends here until response arrives
    Ok(())
}
```

`#[tokio::main]` is a macro that expands to roughly:

```rust
fn main() {
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(async { /* your async main body */ })
}
```

---

## Async in Tests

The standard `#[test]` attribute doesn't understand `async`. For async test functions, use `#[tokio::test]`:

```rust
#[tokio::test]
async fn test_fetches_versions() {
    let result = api::fetch_versions("http://...").await;
    assert!(result.is_ok());
}
```

`#[tokio::test]` spins up a fresh Tokio runtime just for that test.

---

## `#[ignore]` for External Tests

Tests that need network access or credentials are marked `#[ignore]` so they don't run in CI:

```rust
#[tokio::test]
#[ignore = "requires ~/.cmk-credentials and network access"]
async fn integration_fetch_real_server() {
    let versions = api::fetch_versions(CMK_DOWNLOAD_URL).await.unwrap();
    assert!(!versions.is_empty());
}
```

- `cargo test` — skips them
- `cargo test -- --ignored` — runs only ignored tests
- `cargo test -- --include-ignored` — runs all

---

## What's NOT Async in This Codebase

The TUI event loop stays synchronous — see [[Async Boundary Design]] for why.

Subprocess calls (`std::process::Command`) are also synchronous. The `cmk-dev-install` run blocks the thread until it finishes.

---

## Metadata

**Tags:** concept
**Related:** [[Tokio Async Runtime]], [[Async Boundary Design]], [[Rust Result Type and Error Propagation]]
