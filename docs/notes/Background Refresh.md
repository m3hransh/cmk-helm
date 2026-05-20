---
title: Background Refresh
tags: [feature]
status: completed
priority: 2
publish: true
aliases: [auto-refresh, version refresh, r key]
---

# Background Refresh

The version list refreshes automatically every 5 minutes and can be triggered manually with `r`. The refresh runs in a background task and doesn't block the UI.

---

## How to Trigger

| Trigger | Condition |
|---|---|
| Press `r` | From the Version Browser in Browse mode |
| Automatic | 5 minutes after the last successful fetch |

A refresh is silently ignored if one is already in progress (`is_refreshing == true`) or if the initial splash is still showing (`load_rx.is_some()`).

---

## What Gets Refreshed

The same `LoadResult` as the initial load — all three data sources in one background task:

1. `api::fetch_versions()` — version list from the download server
2. `installer::list_installed_versions()` — what's installed locally (via `omd`)
3. `installer::list_installed_sites()` — which sites exist locally

---

## Implementation

```rust
// src/ui/mod.rs

fn spawn_refresh(&mut self) {
    if self.is_refreshing || self.load_rx.is_some() { return; }
    self.is_refreshing = true;

    let (tx, rx) = oneshot::channel::<Result<LoadResult>>();
    self.refresh_rx = Some(rx);

    tokio::spawn(async move {
        let result = async {
            let version_groups = crate::api::fetch_versions(CMK_DOWNLOAD_URL).await?;
            let installed_versions = list_installed_versions().unwrap_or_default();
            let installed_sites    = list_installed_sites().unwrap_or_default();
            Ok(LoadResult { version_groups, installed_versions, installed_sites })
        }.await;
        let _ = tx.send(result);
    });
}
```

`tokio::spawn` is not an async function — it just schedules the task and returns immediately. So `spawn_refresh` can be a regular sync method called from the event loop.

The 5-minute auto-trigger lives in `run()`:

```rust
if self.load_rx.is_none()
    && !self.is_refreshing
    && self.last_refresh.elapsed() >= Duration::from_secs(300)
{
    self.spawn_refresh();
}
```

`last_refresh: Instant` is reset to `Instant::now()` every time data arrives — whether from the initial load or a subsequent refresh. So the 5 minutes are always measured from the last successful fetch.

---

## UI Feedback

While `is_refreshing` is `true`, the tab bar title shows an animated braille spinner:

```
 CMK Helm ⠹    (animates through ⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏)
```

The spinner reuses `splash_tick` (incremented every frame) so no extra state is needed.

---

## Error Handling

If the refresh fetch fails, the error is logged to the debug log and `is_refreshing` is cleared. The existing version list is left untouched — a failed refresh never removes data the user can already see. `last_refresh` is reset on error too, so the next auto-refresh won't trigger immediately.

---

## Rust Concepts at Work Here

| Concept | Where |
|---|---|
| [[Rust Oneshot Channel]] | `refresh_rx` — same pattern as initial load |
| [[Rust Async Await]] | `tokio::spawn(async move { … })` |
| [[Rust Option Type]] | `refresh_rx: Option<Receiver<…>>` as both "in flight?" flag and data source |

---

## Metadata

**Tags:** feature
**Reference:** `src/ui/mod.rs` — `spawn_refresh`, `poll_refresh_result`; `src/ui/input.rs` — `on_browse`; `src/ui/render.rs` — `render_tabs`
**Related:** [[Data Flow]], [[Rust Oneshot Channel]], [[TUI Event Loop]]
