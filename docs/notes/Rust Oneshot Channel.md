---
title: Rust Oneshot Channel
tags: [concept]
status: completed
priority: 3
publish: true
aliases: [oneshot, tokio oneshot, single-use channel]
---

# Rust Oneshot Channel

`tokio::sync::oneshot` is a single-use, one-sender/one-receiver channel. It carries exactly one value, then it's gone. Used in this codebase to hand data from background async tasks back to the synchronous UI loop.

---

## The Two Halves

```rust
let (tx, rx) = oneshot::channel::<Result<LoadResult>>();
```

`tx` (transmitter) and `rx` (receiver) are created as a matched pair. The type parameter says what travels through — here a `Result<LoadResult>`. After `tx.send(value)` is called once, both halves are consumed. You cannot send again.

---

## How It's Used: Initial Load

```rust
// main.rs — spawn the fetch before starting the TUI
let (tx, rx) = oneshot::channel::<Result<ui::LoadResult>>();

tokio::spawn(async move {
    let result = async {
        let version_groups = api::fetch_versions(CMK_DOWNLOAD_URL).await?;
        let installed_versions = installer::list_installed_versions().unwrap_or_default();
        let installed_sites = installer::list_installed_sites().unwrap_or_default();
        Ok(ui::LoadResult { version_groups, installed_versions, installed_sites })
    }.await;
    let _ = tx.send(result);   // fires once, then tx is dropped
});

// The TUI owns rx — it polls it every frame during the splash
ui::App::new_loading(rx).run(terminal).await
```

The background task owns `tx`. The TUI owns `rx`. They run concurrently — the splash animates while the HTTP fetch is in flight.

---

## Polling Without Blocking

`try_recv()` is non-blocking — it returns immediately:

```rust
fn poll_load_result(&mut self) {
    if let Some(mut rx) = self.load_rx.take() {
        match rx.try_recv() {
            Ok(Ok(data)) => {
                self.version_groups = data.version_groups;
                // … populate all fields …
                self.last_refresh = Instant::now();
                // load_rx stays None → main UI takes over next frame
            }
            Ok(Err(e))  => { self.should_quit = true; }
            Err(_)      => { self.load_rx = Some(rx); } // not ready, put back
        }
    }
}
```

The event loop calls this every 16 ms. If the fetch hasn't returned yet, `Err(_)` puts the receiver back and the splash keeps animating.

---

## The `take()` Trick

Why `self.load_rx.take()` instead of `if let Some(ref mut rx) = self.load_rx`?

The borrow checker won't allow you to hold a mutable borrow on `self.load_rx` while also writing to `self.version_groups`, `self.installed_versions`, etc. — they're all fields of the same `self`.

`take()` moves the receiver *out* of the `Option`, leaving `None` in place:

```rust
// Before take():  self.load_rx = Some(rx)   — rx is borrowed
// After  take():  self.load_rx = None        — self is free to mutate
```

If the data hasn't arrived yet, the last line puts the receiver back: `self.load_rx = Some(rx)`.

This pattern appears in both `poll_load_result` and `poll_refresh_result`.

---

## Why Oneshot and Not `mpsc`?

`mpsc` (multi-producer, single-consumer) is used for the job system because install tasks stream many output lines back over time.

`oneshot` encodes "this one thing happens exactly once" in the type. The HTTP fetch either succeeds or fails — there's nothing else to say. Using `oneshot` means the compiler prevents you from accidentally calling `tx.send()` twice (it's consumed on the first call).

| | `oneshot` | `mpsc` |
|---|---|---|
| Messages | Exactly 1 | Many |
| Use in this codebase | Version fetch results | Install/delete job output |
| After send | Channel is gone | Channel stays open |

---

## The Refresh Reuse

The background refresh (`spawn_refresh`) uses an identical pattern with a second `Option<oneshot::Receiver<...>>` stored in `refresh_rx`. The only difference is no splash screen — `is_refreshing: bool` tracks it instead.

See [[Background Refresh]] and [[Data Flow]].

---

## Metadata

**Tags:** concept
**Reference:** `src/ui/mod.rs` — `poll_load_result`, `poll_refresh_result`, `spawn_refresh`; `src/main.rs`
**Related:** [[Rust Async Await]], [[Tokio Async Runtime]], [[Background Refresh]], [[Data Flow]], [[TUI Event Loop]]
