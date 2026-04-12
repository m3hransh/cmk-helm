---
title: Async Boundary Design
tags: [architecture]
status: completed
priority: 2
publish: true
aliases: [async boundary, sync vs async]
---

# Async Boundary Design

Only two things in this codebase are async. Everything else is synchronous. This is an intentional design decision.

---

## What Is Async

```
async:
  main()                  — bootstraps Tokio, awaits fetch_versions()
  api::fetch_versions()   — network I/O

sync (everything else):
  ui::App::run()          — the TUI event loop
  installer::*            — subprocess spawning (std::process::Command)
  api::read_credentials() — file I/O (small file, sync is fine)
```

---

## Why the Event Loop is Synchronous

Ratatui's rendering API is synchronous. `terminal.draw(|frame| ...)` blocks until the frame is rendered. There is no async draw.

Mixing async I/O and synchronous rendering in the same loop — e.g. polling for install progress while also polling for keystrokes — adds complexity. The `crossterm::event::poll(Duration)` call already handles the timing:

```rust
// 16ms timeout ≈ 60fps. Blocks until an event arrives OR 16ms elapses.
// This gives a responsive UI without 100% CPU usage.
if event::poll(Duration::from_millis(16))? {
    // handle the event
}
```

For future work (streaming install output), the right pattern is `tokio::sync::mpsc` channels: an async task sends output lines into a channel, and the sync event loop drains the channel each frame.

---

## `#[tokio::main]`

```rust
#[tokio::main]
async fn main() -> Result<()> {
    let versions = api::fetch_versions(CMK_DOWNLOAD_URL).await?;
    // ...
    app.run()?;  // sync — blocks until user quits
    Ok(())
}
```

`#[tokio::main]` is a procedural macro that wraps `main()` in a Tokio runtime. It expands roughly to:

```rust
fn main() {
    tokio::runtime::Runtime::new().unwrap()
        .block_on(async { /* your async main body */ })
}
```

You need this because Rust's `main()` is not async by default. See [[Rust Async Await]] for a deeper explanation.

---

## Why Not Async Subprocess Calls?

`installer::*` uses `std::process::Command::spawn()` — the standard library's sync subprocess API — not `tokio::process::Command`. 

During the install, the UI shows a static "installing…" message. There is no live output streaming yet (future work). Since we don't need to read subprocess stdout while also handling keystrokes, the sync API is simpler and sufficient.

---

## Rust Concepts at Work Here

| Concept | Where |
|---------|-------|
| [[Rust Async Await]] | `async fn`, `.await`, the runtime model |
| [[Tokio Async Runtime]] | The executor driving async tasks |

---

## Metadata

**Tags:** architecture
**Related:** [[Module Boundaries]], [[TUI Event Loop]], [[Rust Async Await]], [[Tokio Async Runtime]]
