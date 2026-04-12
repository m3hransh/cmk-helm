---
title: Edition Picker Screen
tags: [feature]
status: completed
priority: 2
publish: true
aliases: [edition picker, EditionPicker]
---

# Edition Picker Screen

The second screen in the [[App State Machine]]. After selecting a version, the user picks which Checkmk edition to install.

---

## The Editions

| Edition | Identifier | Notes |
|---------|-----------|-------|
| Raw | `raw` | Open source (GPLv2) |
| Free | `free` | Free tier of Enterprise |
| Cloud | `cloud` | Cloud-optimised |
| Enterprise | `enterprise` | Full enterprise (CEE) |
| MSP | `managed-services` | Managed Service Provider |

---

## Screen State

The selected edition is tracked by a `ListState` stored inside the `Screen::EditionPicker` enum variant:

```rust
Screen::EditionPicker {
    version: Version,       // carried forward from VersionList
    list_state: ListState,  // which edition is highlighted
}
```

See [[Rust Enums and Algebraic Data Types]] for why the state lives inside the variant.

---

## Navigation

| Key | Action |
|-----|--------|
| `j` / `↓` | Next edition |
| `k` / `↑` | Previous edition |
| `Enter` | Confirm — advance to [[Configure Screen]] with `version` + `edition` |
| `Esc` | Go back to [[Version List Screen]] |
| `q` | Quit |

---

## Transition

On `Enter`, the handler creates a new `Screen::Configure` carrying both the version and the edition:

```rust
let Screen::EditionPicker { version, list_state } = &mut self.screen else { return };

let edition = EDITIONS[list_state.selected().unwrap_or(0)];

self.screen = Screen::Configure {
    version: version.clone(),
    edition,
    site_input: String::new(),
};
```

The `else { return }` guard is the [[Rust Let Else Pattern]].

---

## Why Not Parse Editions From the Server?

Edition information is in the filenames *inside* each version directory (e.g. `check-mk-cloud-2.5.0-2026.04.03_0.noble_amd64.deb`). Fetching and parsing those would require N+1 HTTP requests (one per version directory).

The editions available for any version are a fixed, well-known set. We hardcode them and let the install fail gracefully if an edition doesn't exist for a particular version.

---

## Metadata

**Tags:** feature
**Related:** [[Version List Screen]], [[Configure Screen]], [[App State Machine]]
