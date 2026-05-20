---
title: App State Machine
tags: [architecture]
status: completed
priority: 3
publish: true
aliases: [state machine, screen flow, pane modes]
---

# App State Machine

The UI is modelled as a set of nested state machines. Rather than a single `Screen` enum, the design uses two independent mode enums — one per side of the split-pane layout — that run concurrently.

---

## Pane Layout

```
┌─────────────────────┬───────────────────────┐
│  Version Browser    │  Installed Versions   │
│  (LeftPaneMode)     │  (RightPaneMode)      │
│                     ├───────────────────────┤
│                     │  Installed Sites      │
│                     │  (RightPaneMode)      │
├─────────────────────┴───────────────────────┤
│  Log Panel                                  │
└─────────────────────────────────────────────┘
```

`ActivePane` tracks which pane currently has keyboard focus. `h/j/k/l` (or `Alt+hjkl`) move focus spatially between panes.

---

## ActivePane

```rust
// src/ui/state.rs
enum ActivePane {
    VersionBrowser,
    InstalledVersions,
    InstalledSites,
    LogPanel,
}
```

Pane focus is separate from the content state — changing focus doesn't reset `LeftPaneMode` or `RightPaneMode`.

---

## LeftPaneMode

The version browser has its own inline state machine:

```rust
enum LeftPaneMode {
    Browse,
    EditionPicker { group_idx, version_idx, list_state },
    Configure     { group_idx, version_idx, edition, site_input },
}
```

```
Browse ──Enter──▶ EditionPicker ──Enter──▶ Configure ──Enter──▶ (spawns install job)
          ▲             │                       │
          └────Esc───────┴──────────Esc──────────┘
```

Each variant carries exactly the data that mode needs. You cannot access `site_input` while in `Browse` — there's no field. This is the [[Rust Enums and Algebraic Data Types|algebraic data type]] principle: invalid states are unrepresentable.

---

## RightPaneMode

The installed versions/sites panes share one mode enum:

```rust
enum RightPaneMode {
    Browse,
    ConfirmDelete { label, target: DeleteTarget },
    SiteNameInput { omd_version, site_input },
}
```

Both right panes read from the same `right_mode` field — whichever pane is focused uses it. `DeleteTarget` distinguishes whether a version or a site is being deleted.

---

## Splash State

Before data loads, `App::load_rx` holds the `oneshot::Receiver` from the background fetch task. While `load_rx.is_some()`, the splash screen renders instead of the pane layout. When data arrives, `load_rx` becomes `None` and the main UI takes over.

This doubles as both a "loading?" flag and the data source — no separate boolean needed. See [[Rust Oneshot Channel]].

---

## Transitions in Code

Input handlers use `let … else` to destructure the current mode variant:

```rust
fn on_edition_picker(&mut self, code: KeyCode) {
    let LeftPaneMode::EditionPicker { group_idx, version_idx, list_state }
        = &mut self.left_mode else { return };

    match code {
        KeyCode::Enter => {
            let edition = editions[list_state.selected().unwrap_or(0)].clone();
            self.left_mode = LeftPaneMode::Configure {
                group_idx: *group_idx,
                version_idx: *version_idx,
                edition,
                site_input: String::new(),
            };
        }
        KeyCode::Esc => self.left_mode = LeftPaneMode::Browse,
        // … navigation
    }
}
```

See [[Rust Let Else Pattern]].

---

## Render Dispatch

`render_left` delegates to the correct render function based on `left_mode`:

```rust
fn render_left(&mut self, frame: &mut Frame, area: Rect) {
    match &self.left_mode {
        LeftPaneMode::Browse          => self.render_version_list(frame, area),
        LeftPaneMode::EditionPicker { .. } => self.render_edition_picker(frame, area),
        LeftPaneMode::Configure { .. }     => self.render_configure(frame, area),
    }
}
```

`match` is exhaustive — adding a new variant without handling it is a compile error.

---

## Rust Concepts at Work Here

| Concept | Where |
|---------|-------|
| [[Rust Enums and Algebraic Data Types]] | `LeftPaneMode`, `RightPaneMode`, `ActivePane` |
| [[Rust Pattern Matching]] | `match &self.left_mode { … }` |
| [[Rust Let Else Pattern]] | `let LeftPaneMode::EditionPicker { .. } = … else { return }` |
| [[Rust Oneshot Channel]] | Splash `load_rx` as both flag and data source |

---

## Metadata

**Tags:** architecture
**Related:** [[Module Boundaries]], [[Rust Enums and Algebraic Data Types]], [[Rust Pattern Matching]], [[Rust Let Else Pattern]], [[Rust Oneshot Channel]], [[TUI Layout Design]]
