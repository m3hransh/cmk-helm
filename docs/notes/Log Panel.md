---
title: Log Panel
tags: [feature]
status: completed
priority: 2
publish: true
aliases: [log panel, job tabs, copy mode, install log]
---

# Log Panel

The log panel sits at the bottom of the [[TUI Layout Design|multi-pane layout]] and shows live output from background jobs — installs, version deletes, and site operations. Each job gets its own tab; the user can switch between them and copy output to the clipboard.

---

## Layout

```
┌── Log [COPY MODE — Esc to exit] ──────────────────────────────────────────┐
│  ⟳ install 2.6.0  ✓ rm 2.5.0  ✗ site mysite                              │  ← tab bar
│  install 2.6.0-2026.04.07 -e enterprise → mysite                          │  ← full label (dim)
│  → Downloading package…                                                    │
│  → Installing package…                                                     │
│  ── ✓ done ──                                                              │
└────────────────────────────────────────────────────────────────────────────┘
```

The tab bar shows one entry per job. The selected tab is highlighted in CMK blue. Each entry shows a status icon and the short label (`install 2.6.0`, `rm 2.4.0`, `site mysite`, etc.). The content area shows only the selected job's output.

---

## Tab Bar

| Icon | Colour | Meaning |
|---|---|---|
| `⟳` | Yellow | Job is running |
| `✓` | Green | Job finished successfully |
| `✗` | Red | Job failed |

The selected tab renders with `bg(CMK_BLUE)` + bold white text. Non-selected tabs use the status icon colour.

`short_label` on `Job` is generated at spawn time and kept to ≤ ~18 characters so tabs don't overflow on narrow terminals:

| Operation | short_label example |
|---|---|
| Install version | `install 2.6.0` |
| Delete version | `rm 2.6.0` |
| Delete site | `rm mysite` |
| Create site | `site mysite` |

---

## Navigation

| Key | Action |
|---|---|
| `h` / ← | Previous job tab |
| `l` / → | Next job tab |
| `j` / ↓ | Scroll output down (towards newer lines) |
| `k` / ↑ | Scroll output up (towards older lines) |
| `c` | Enter copy mode |
| `Esc` | Exit copy mode (if active) / quit (otherwise) |

`h` and `l` are bare — they switch job tabs within the log panel. `Alt+h` / `Alt+l` still switch active panes (handled by `dispatch` before the pane handler sees them).

---

## Copy Mode

Pressing `c` disables mouse capture via `crossterm::execute!(DisableMouseCapture)`. In this state the terminal handles mouse events natively, so the user can click-drag to select text and copy it with the terminal's built-in clipboard. The log panel border title changes to `Log [COPY MODE — Esc to exit]` and the footer hint updates.

Pressing `Esc` re-enables mouse capture (`EnableMouseCapture`) and returns to normal mode.

Scrolling (`j`/`k`) and tab switching (`h`/`l`) still work in copy mode — only mouse events are redirected to the terminal.

```rust
// src/ui/input.rs — on_log_panel

KeyCode::Char('c') if !self.copy_mode => {
    self.copy_mode = true;
    let _ = crossterm::execute!(std::io::stdout(), crossterm::event::DisableMouseCapture);
}
// Esc in copy mode exits copy mode; Esc outside it quits the app.
```

---

## Job Lifecycle

Every operation (install, delete version, delete site, create site) follows the same pattern:

1. Build `Job { id, label, short_label, status: Running, output: [] }` and push to `self.jobs`
2. `self.selected_job = Some(self.jobs.len() - 1)` — auto-focus the new tab
3. `self.log_scroll = 0` — jump to top of (empty) output
4. Spawn the background task via `installer::spawn_*`, passing `job_id` and a clone of `job_tx`

The background task streams `JobMessage::Output(id, line)` and `JobMessage::Finished(id, success)` over the `mpsc` channel. `drain_job_messages()` (called every frame in the event loop) appends output lines to the matching `Job` and updates its status when it finishes.

See [[Rust Async Await]] and [[Data Flow]] for the channel mechanics.

---

## Output Line Colouring

| Condition | Colour |
|---|---|
| Contains `✓` or `done` | Green |
| Contains `✗`, `error`, `Error`, or `failed` | Red |
| Starts with `→` | Cyan |
| Everything else | Gray |

---

## Implementation

```
src/ui/mod.rs     — App struct: jobs, selected_job, copy_mode, job_tx/rx
src/ui/input.rs   — on_log_panel: scrolling, tab switching, copy mode
src/ui/render.rs  — render_log_panel: tab bar + per-job content
src/installer/mod.rs — Job struct: id, label, short_label, status, output
```

---

## Metadata

**Tags:** feature
**Reference:** `src/ui/input.rs` — `on_log_panel`; `src/ui/render.rs` — `render_log_panel`; `src/ui/mod.rs` — `drain_job_messages`, `spawn_install_job`
**Related:** [[Data Flow]], [[Rust Async Await]], [[TUI Layout Design]], [[Background Refresh]]
