---
title: Configure Screen
tags: [feature]
status: completed
priority: 2
publish: true
aliases: [configure, site name input, Configure]
---

# Configure Screen

The third screen in the [[App State Machine]]. The user enters a site name and reviews the install configuration before confirming.

---

## What It Shows

```
┌─ Configure Installation ──────────────────────┐
│                                               │
│  Version:  2.5.0-2026-04-03                   │
│  Edition:  Cloud                              │
│                                               │
│  Site name: mydevsite█                        │
│             (type a name, Enter to confirm)   │
│                                               │
└───────────────────────────────────────────────┘
```

---

## Screen State

```rust
Screen::Configure {
    version: Version,      // carried from EditionPicker
    edition: Edition,      // carried from EditionPicker
    site_input: String,    // grows as user types
}
```

`site_input` is the active text field. It is mutated character by character as the user types.

---

## Text Input Handling

Ratatui doesn't have a built-in text input widget with cursor management. We implement basic input handling directly:

```rust
fn on_configure(&mut self, key: KeyCode) {
    let Screen::Configure { version, edition, site_input } = &mut self.screen else { return };

    match key {
        KeyCode::Char(c) => site_input.push(c),           // append character
        KeyCode::Backspace => { site_input.pop(); }       // remove last character
        KeyCode::Enter if !site_input.is_empty() => {
            // Transition to Installing
            self.screen = Screen::Installing {
                config: InstallConfig {
                    version: version.clone(),
                    edition: *edition,
                    site_name: site_input.clone(),
                },
            };
        }
        KeyCode::Esc => {
            // Go back to EditionPicker
            self.screen = Screen::EditionPicker {
                version: version.clone(),
                list_state: ListState::default(),
            };
        }
        _ => {}
    }
}
```

---

## Validation

Basic validation: `Enter` only advances if `site_input` is non-empty (`!site_input.is_empty()`). The site name is passed directly to `cmk-dev-site` — OMD enforces its own naming constraints (lowercase, alphanumeric, no spaces).

---

## Metadata

**Tags:** feature
**Related:** [[Edition Picker Screen]], [[Installing Screen]], [[App State Machine]], [[cmk-dev-install]]
