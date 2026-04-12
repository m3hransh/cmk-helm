---
title: Module Boundaries
tags: [architecture]
status: completed
priority: 3
publish: true
aliases: [modules, code structure]
---

# Module Boundaries

CMK Cockpit is split into three modules, each with a single responsibility. This separation makes each module independently testable and keeps concerns from leaking.

---

## The Three Modules

```
src/
├── api/mod.rs        HTTP + HTML parsing + data types
├── ui/mod.rs         App state machine + event loop + rendering
└── installer/mod.rs  Subprocess wrappers
```

| Module | Owns | Does NOT own |
|--------|------|--------------|
| `api/` | Server communication, credential reading, `Version`/`Edition` types | UI, subprocess logic |
| `ui/` | `App` struct, `Screen` enum, event loop, all Ratatui rendering | HTTP calls, subprocess spawning |
| `installer/` | Spawning `cmk-dev-install`, `cmk-dev-site`, `omd` | HTTP, UI state |

---

## Why This Split?

**Testability** — `api/` tests can run without a terminal or a subprocess. Regex parsing tests don't need a network connection. `installer/` can be replaced by a stub in tests without touching the real network.

**Single responsibility** — Each module answers a different question:
- `api/` answers "what versions exist on the server?"
- `ui/` answers "what does the user want to do?"
- `installer/` answers "how do we run the tools?"

**Allowed imports:**
- `ui/` imports from `api/` (needs `Version`, `Edition`)
- `ui/` imports from `installer/` (kicks off install)
- `api/` and `installer/` know nothing about each other

---

## `api/mod.rs` — What's Inside

```
Constants:    CMK_DOWNLOAD_URL, credentials file path
Static regex: ROW_RE (compiled once via LazyLock)
Types:        Version, VersionKind, VersionGroup, Edition
Functions:    read_credentials(), fetch_versions()
```

The key design decision: the server returns **HTML directory listings**, not JSON. Versions are parsed from `<a href>` tags using a regex. See [[Data Flow]] and [[Credential Auth]].

Versions are grouped by base version (`2.5.0`, `2.4.0`, …) into `VersionGroup` for tab display. See [[Version List Screen]].

---

## `ui/mod.rs` — What's Inside

```
Enum:     Screen { VersionList, EditionPicker, Configure, Installing }
Struct:   App { version_groups, active_tab, table_state, installed_*, screen }
Methods:  App::run() (event loop), on_*() handlers, render_*() functions
```

The `Screen` enum is the heart of the UI. Each variant carries the data that screen needs — nothing more. See [[App State Machine]].

---

## `installer/mod.rs` — What's Inside

```
Struct:    InstallConfig { version, edition, site_name }
Functions: install_package(), create_site(), install_and_create_site()
           list_installed_versions(), list_installed_sites()
Helper:    which_exists()
```

`install_and_create_site()` tries the combined `cmk-dev-install-site` shortcut first, then falls back to calling the two tools separately. See [[Installing Screen]].

`list_installed_versions()` and `list_installed_sites()` run `omd versions -b` and `omd sites` at startup to populate the right panel. See [[Installed Versions and Sites Panel]].

---

## `main.rs` — The Glue

`main.rs` is intentionally thin (~30 lines). It:
1. Calls `api::fetch_versions()` (async, one HTTP request)
2. Calls `installer::list_installed_versions/sites()` (sync, two subprocesses)
3. Initialises the Ratatui terminal
4. Constructs `App` with the data
5. Calls `app.run()` (the event loop)
6. Restores the terminal on exit (even if `run()` errors)

The terminal-restore logic uses a Rust pattern worth understanding: the alternate screen buffer is always restored, regardless of whether the app errored. See [[TUI Event Loop]].

---

## Rust Design Pattern: Illegal States Unrepresentable

The module boundary for `ui/` enforces a key Rust principle via the `Screen` enum: you cannot be in the `Configure` screen without having both a `Version` and an `Edition`. The type system prevents it. See [[App State Machine]] and [[Rust Enums and Algebraic Data Types]].

---

## Metadata

**Tags:** architecture
**Related:** [[App State Machine]], [[Data Flow]], [[CMK Cockpit]], [[Rust Enums and Algebraic Data Types]]
