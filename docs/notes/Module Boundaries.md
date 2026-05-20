---
title: Module Boundaries
tags: [architecture]
status: completed
priority: 3
publish: true
aliases: [modules, code structure]
---

# Module Boundaries

CMK Helm is split into four top-level modules, each with a single responsibility. The `ui/` module is further split into four files.

---

## Top-Level Modules

```
src/
├── api/mod.rs        HTTP + HTML parsing + data types
├── ui/               App state machine + event loop + rendering (4 files)
├── installer/mod.rs  Subprocess wrappers + async job system
└── main.rs           Entry point: runtime init, background fetch, TUI handoff
```

| Module | Owns | Does NOT own |
|--------|------|--------------|
| `api/` | Server communication, credential reading, `Version`/`Edition` types | UI, subprocess logic |
| `ui/` | `App` struct, pane state machines, event loop, all Ratatui rendering | HTTP calls, subprocess spawning |
| `installer/` | Spawning `cmk-dev-install`, `cmk-dev-site`, `omd`; async job streaming | HTTP, UI state |

---

## `ui/` — Split Into Four Files

The `ui/` module is large enough to warrant its own internal split. Rust allows multiple `impl` blocks for the same type across files — the compiler merges them.

```
src/ui/
├── mod.rs    App struct + constructors + event loop + job/refresh system
├── state.rs  Shared types: ActivePane, LeftPaneMode, RightPaneMode, PaneRects, LoadResult
├── input.rs  impl App — all keyboard/mouse event handling
└── render.rs impl App — all Ratatui rendering
```

**Visibility rules used here:**
- `state.rs` types are `pub(crate)` — visible to sibling submodules via `use super::state::*`
- `handle_events` and `render` are `pub(super)` — callable from `mod.rs` (the parent), not from outside `ui/`
- Private methods in `mod.rs` (e.g. `spawn_install_job`, `spawn_refresh`) are accessible to child modules `input.rs` and `render.rs` — Rust's rule that private items are visible to all descendants of the declaring module

---

## `api/mod.rs` — What's Inside

```
Constants:    CMK_DOWNLOAD_URL, credentials file path
Static regex: ROW_RE (compiled once via LazyLock)
Types:        Version, VersionKind, VersionGroup, Edition
Functions:    read_credentials(), fetch_versions()
```

The server returns **HTML directory listings**, not JSON. Versions are parsed from `<a href>` tags using a regex. See [[Data Flow]] and [[Credential Auth]].

Versions are grouped by base version (`2.5.0`, `2.4.0`, …) into `VersionGroup` for tab display.

---

## `ui/mod.rs` — What's Inside

```
Struct:   App { version_groups, active_tab, table_state, pane state, jobs, refresh_rx, … }
Methods:  App::new_loading() (constructor), run() (event loop),
          poll_load_result(), poll_refresh_result(), spawn_refresh(),
          drain_job_messages(), spawn_install_job()
```

The `App` struct owns all mutable state. `run()` is the only async method — it drives the draw/poll/handle loop and checks both the splash receiver (`load_rx`) and the background refresh receiver (`refresh_rx`) every frame.

See [[App State Machine]] and [[Background Refresh]].

---

## `ui/state.rs` — What's Inside

```
Constants: CMK_BLUE, CMK_GREEN (brand colours)
Types:     LoadResult, ActivePane, LeftPaneMode, RightPaneMode, DeleteTarget, PaneRects
```

Splitting types into their own file avoids import cycles between `input.rs` and `render.rs` — both need the same enums but neither should depend on the other.

---

## `installer/mod.rs` — What's Inside

```
Structs:   InstallConfig, Job, JobMessage, JobStatus
Functions: spawn_install(), spawn_delete_version(), spawn_delete_site(), spawn_create_site()
           list_installed_versions(), list_installed_sites()
           ensure_sudo()
```

Each `spawn_*` function creates a `tokio::task` that streams output lines back to the UI via an `mpsc::unbounded_channel`. The UI drains the channel each frame in `drain_job_messages()`. See [[Async Boundary Design]].

---

## `main.rs` — The Glue

`main.rs` is intentionally thin (~45 lines). It:
1. Calls `installer::ensure_sudo()` (before raw-mode takes over the terminal)
2. Spawns a `tokio::task` that fetches versions + installed state, sends result via `oneshot`
3. Initialises the Ratatui terminal (raw mode + alternate screen)
4. Calls `App::new_loading(rx).run(terminal).await`
5. Restores the terminal on exit regardless of error

See [[Rust Oneshot Channel]] for why `oneshot` is used here.

---

## Allowed Imports

```
main.rs  →  api, ui, installer
ui/      →  api (Version, Edition types), installer (Job, InstallConfig, …)
api/     →  (nothing from this codebase)
installer/ → (nothing from this codebase)
```

`api/` and `installer/` are intentionally unaware of each other and of `ui/`.

---

## Metadata

**Tags:** architecture
**Related:** [[App State Machine]], [[Data Flow]], [[Rust Oneshot Channel]], [[Background Refresh]], [[Async Boundary Design]]
