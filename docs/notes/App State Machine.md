---
title: App State Machine
tags: [architecture]
status: completed
priority: 3
publish: true
aliases: [state machine, screen flow, Screen enum]
---

# App State Machine

The UI is modelled as a finite state machine. Each state (screen) is a variant of the `Screen` enum. Transitions happen on user input. This is one of the most idiomatic Rust patterns in the codebase.

---

## Screen Flow

```
VersionList ──Enter──▶ EditionPicker ──Enter──▶ Configure ──Enter──▶ Installing
     ▲                      │                       │
     └──────────Esc──────────┴───────────Esc─────────┘
```

`q` quits from any screen.

---

## The Enum

```rust
// src/ui/mod.rs
enum Screen {
    VersionList,
    EditionPicker {
        version: Version,
        list_state: ListState,
    },
    Configure {
        version: Version,
        edition: Edition,
        site_input: String,
    },
    Installing {
        config: InstallConfig,
    },
}
```

This is **not** a C-style enum where variants are just integers. Each variant carries exactly the data that screen needs — nothing more, nothing less. This is Rust's [[Rust Enums and Algebraic Data Types|algebraic data type]] system.

---

## Why an Enum Instead of a Struct With Flags?

**The bad alternative** would be:

```rust
// Don't do this
struct App {
    current_screen: u8,       // 0=list, 1=picker, 2=configure, 3=installing
    selected_version: Option<Version>,   // only valid on screens 1-3
    selected_edition: Option<Edition>,   // only valid on screens 2-3
    site_name: Option<String>,           // only valid on screen 2
    install_config: Option<InstallConfig>, // only valid on screen 3
}
```

This compiles, but it lets you write `selected_version.unwrap()` on screen 0 — a runtime panic waiting to happen.

**The Rust way:** with enum variants, the compiler makes invalid states unrepresentable. You *cannot* access `site_input` while on `VersionList` — there is no field to access. The type system enforces the contract.

---

## Transitions in Code

Input handlers use `let … else` to destructure the current screen variant:

```rust
fn on_edition_picker(&mut self, key: KeyCode) {
    // This function only does work if we're in EditionPicker state.
    // If we're not, return immediately — no nested match needed.
    let Screen::EditionPicker { version, list_state } = &mut self.screen else { return };

    match key {
        KeyCode::Enter => {
            let edition = get_selected_edition(list_state);
            // Transition: move to Configure, carrying version + edition forward
            self.screen = Screen::Configure {
                version: version.clone(),
                edition,
                site_input: String::new(),
            };
        }
        KeyCode::Esc => self.screen = Screen::VersionList,
        // ... navigation keys
    }
}
```

See [[Rust Let Else Pattern]] for the `let … else` syntax.

---

## Render Dispatch

The render function matches on `self.screen` to draw the appropriate content in the left pane:

```rust
fn render(&self, frame: &mut Frame) {
    match &self.screen {
        Screen::VersionList => self.render_version_list(frame, left_area),
        Screen::EditionPicker { .. } => self.render_edition_picker(frame, left_area),
        Screen::Configure { .. } => self.render_configure(frame, left_area),
        Screen::Installing { .. } => self.render_installing(frame, left_area),
    }
}
```

`match` is exhaustive — if a new `Screen` variant is added, the compiler errors until the render and input handlers are updated. This is a compile-time safety net.

---

## Rust Concepts at Work Here

| Concept | Where |
|---------|-------|
| [[Rust Enums and Algebraic Data Types]] | The `Screen` enum itself |
| [[Rust Pattern Matching]] | `match &self.screen { ... }` |
| [[Rust Let Else Pattern]] | `let Screen::EditionPicker { .. } = &self.screen else { return }` |
| [[Rust Ownership and Borrowing]] | `version: version.clone()` when transitioning |

---

## Metadata

**Tags:** architecture
**Related:** [[Module Boundaries]], [[Rust Enums and Algebraic Data Types]], [[Rust Pattern Matching]], [[TUI Layout Design]], [[Version List Screen]]
