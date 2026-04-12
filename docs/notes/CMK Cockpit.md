---
title: CMK Cockpit
tags: [app]
status: inprogress
priority: 3
publish: true
aliases: [cmk-cockpit, the app]
---

# CMK Cockpit

CMK Cockpit is a **Rust TUI application** that makes installing Checkmk development environments interactive and discoverable. Instead of remembering which CLI flags to pass to which tools in which order, you get a keyboard-driven terminal UI.

---

## The Problem It Solves

The existing workflow requires knowing:
1. Which version string to pass to `cmk-dev-install` (daily builds use hyphens, stable uses `p` suffix)
2. Which edition identifier to pass to `cmk-dev-site` (there are five editions)
3. Which OMD version string `omd` expects (different format again)

CMK Cockpit fetches the available versions from the download server, lets you browse and select, and calls the right tools with the right arguments.

---

## Tech Stack

| Layer | Technology |
|-------|----------|
| Language | Rust (2021 edition) |
| TUI framework | [[Ratatui TUI Framework]] |
| Terminal backend | Crossterm |
| Async runtime | [[Tokio Async Runtime]] |
| HTTP client | [[Reqwest HTTP Client]] |
| Error handling | [[anyhow Error Handling]] |
| Build system | [[Nix Flakes]] + [[Crane Build System]] |

---

## Screens

The UI is a four-screen [[App State Machine]]:

1. **[[Version List Screen]]** — Tabs by base version (2.6.0, 2.5.0, …), rows by build date
2. **[[Edition Picker Screen]]** — Choose Raw / Free / Cloud / Enterprise / MSP
3. **[[Configure Screen]]** — Enter site name, review config
4. **[[Installing Screen]]** — Live output from install subprocess

A persistent **[[Installed Versions and Sites Panel]]** on the right shows what is already installed locally, regardless of which screen is active.

---

## External Tools Required at Runtime

These must be on `PATH` when running the app. They live in `~/Projects/cmk-dev-site/`.

| Tool | Role |
|------|------|
| `cmk-dev-install` | Downloads and installs a Checkmk `.deb` package |
| `cmk-dev-site` | Creates and configures an OMD site for a version |
| `cmk-dev-install-site` | Combined shortcut for the above two |
| `omd` | OMD (Open Monitoring Distribution) — manages all installed versions/sites |

See [[cmk-dev-install]] and [[omd]] for details.

---

## Credentials

The app reads `~/.cmk-credentials` — one line: `username:password`. It uses HTTP Basic Auth against the download server. See [[Credential Auth]].

---

## Source Layout

```
src/
├── main.rs           Entry point — terminal init, tokio bootstrap, cleanup
├── api/mod.rs        HTTP + HTML parsing, Version/Edition types
├── ui/mod.rs         App struct, Screen state machine, event loop, rendering
└── installer/mod.rs  Subprocess wrappers for cmk-dev-install / omd
```

See [[Module Boundaries]] for why the code is split this way.

---

## Key Architectural Decisions

- **[[App State Machine]]** — illegal states unrepresentable via enum variants with data
- **[[Async Boundary Design]]** — only `main()` and `api::*` are async; TUI loop is sync
- **[[Error Handling Strategy]]** — `anyhow::Result<T>` throughout, `.with_context()`
- **HTML not JSON** — the download server returns Apache autoindex; we parse with regex

---

## Metadata

**Tags:** app
**Reference:** `~/Projects/cmk-helm/`, `~/Projects/cmk-dev-site/`
**Related:** [[00 Index]], [[Module Boundaries]], [[App State Machine]], [[Getting Started]]
