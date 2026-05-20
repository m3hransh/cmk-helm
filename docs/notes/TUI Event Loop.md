---
title: TUI Event Loop
tags: [architecture, concept]
status: completed
priority: 2
publish: true
aliases: [event loop, render loop, 60fps]
---

# TUI Event Loop

Ratatui apps follow a tight **poll → draw → handle** loop. This codebase adds three async tasks running alongside the loop: a background version fetch, an optional background refresh, and one task per install/delete job.

---

## The Loop

```rust
// src/ui/mod.rs — App::run()
pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
    crossterm::execute!(stdout(), EnableMouseCapture)?;

    while !self.should_quit {
        self.poll_load_result();      // initial fetch arrived?
        self.poll_refresh_result();   // periodic refresh arrived?

        // auto-refresh every 30 s once main UI is showing
        if self.load_rx.is_none() && !self.is_refreshing
            && self.last_refresh.elapsed() >= Duration::from_secs(300)
        {
            self.spawn_refresh();
        }

        self.drain_job_messages();              // install/delete job output
        terminal.draw(|frame| self.render(frame))?;
        self.handle_events()?;
        self.splash_tick = self.splash_tick.wrapping_add(1);
    }

    crossterm::execute!(stdout(), DisableMouseCapture)?;
    Ok(())
}
```

---

## Why 16ms?

`handle_events` calls `event::poll(Duration::from_millis(16))` internally. `event::poll` blocks until a key/mouse/resize event arrives **or** the timeout elapses. 16 ms ≈ 60 fps — the loop wakes up at least 60 times per second, keeping the splash animation smooth and refresh polling timely even when the user isn't typing.

---

## The Three Polling Functions

Each call is non-blocking — they inspect a `tokio::sync` channel and return immediately whether or not data has arrived, using `try_recv()`.

| Function | Channel | What it does when data arrives |
|---|---|---|
| `poll_load_result()` | `load_rx: Option<oneshot::Receiver>` | Populates all data, clears `load_rx`, resets `last_refresh` |
| `poll_refresh_result()` | `refresh_rx: Option<oneshot::Receiver>` | Same as above but also clears `is_refreshing` |
| `drain_job_messages()` | `job_rx: mpsc::UnboundedReceiver` | Appends output lines to the matching job's log |

The `Option<Receiver>` wrapper serves as both the "pending?" flag and the data source. See [[Rust Oneshot Channel]] for the `take()` trick that makes this borrow-checker-safe.

---

## Async Model

`run()` is `async` so it can live inside the Tokio runtime. **The loop body itself is synchronous** — `poll_*` and `drain_*` are non-blocking sync calls. The only things that actually use `await` are the background tasks:

```
main()
 └─ tokio::spawn  ──── api::fetch_versions().await ─────▶ oneshot tx (initial load)
 └─ App::run()
     ├─ spawn_refresh()
     │   └─ tokio::spawn  ── fetch_versions().await ────▶ oneshot tx (refresh)
     └─ spawn_install_job()
         └─ tokio::spawn  ── installer subprocess ──────▶ mpsc tx (streaming lines)
```

Background tasks send results into channels; the main loop drains them at 60 fps. This is the actor pattern without an actor framework.

See [[Async Boundary Design]] for why only `api::*` functions are async and the rest is sync.

---

## Splash Screen Phase

While `load_rx.is_some()`, `render()` calls `render_splash()` instead of the main pane layout. The splash animates via `splash_tick` (incremented every frame). When `poll_load_result()` receives the data, `load_rx` becomes `None` and the next frame draws the main UI.

---

## Raw Mode and the Alternate Screen

`ratatui::init()` enables:

- **Raw mode** — disables line buffering and echo; keypresses arrive immediately
- **Alternate screen buffer** — preserves the user's shell output; restored on exit

`ratatui::restore()` in `main.rs` runs even if `run()` returns an error:

```rust
let result = ui::App::new_loading(rx).run(terminal).await;
ratatui::restore();
result
```

---

## Immediate Mode

Ratatui is **immediate mode** — the entire UI is rebuilt from `App` state on every frame. There is no persistent widget tree. This makes state → UI reasoning simple: the screen is always exactly what `render()` produces for the current `App`.

---

## Metadata

**Tags:** architecture, concept
**Related:** [[App State Machine]], [[TUI Layout Design]], [[Ratatui TUI Framework]], [[Async Boundary Design]], [[Rust Oneshot Channel]], [[Background Refresh]]
