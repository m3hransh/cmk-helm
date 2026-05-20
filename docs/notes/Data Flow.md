---
title: Data Flow
tags: [architecture]
status: completed
priority: 2
publish: true
aliases: [data pipeline, version fetching]
---

# Data Flow

This note traces the full path from credentials file to running Checkmk site — every step of what happens when you start the app and install a version.

---

## Startup Data Flow

Data loading is **async and non-blocking**. `main.rs` spawns a background task immediately, then starts the TUI so the splash screen can animate while the network request is in flight.

```
main()
  │
  ├─ tokio::spawn ─────────────────────────────────────────────────────▶ background task
  │                                                                              │
  │   ~/.cmk-credentials                                                        │
  │          │  read_credentials() → ("user", "pass")                           │
  │          ▼                                                                   │
  │   api::fetch_versions(CMK_DOWNLOAD_URL)                                     │
  │          │  HTTP GET + Basic Auth                                            │
  │          ▼                                                                   │
  │   https://download.checkmk.com/checkmk/                                     │
  │          │  Apache autoindex HTML (not JSON)                                 │
  │          ▼                                                                   │
  │   parse_versions_from_html() → Vec<VersionGroup>                            │
  │          │                                                                   │
  │   installer::list_installed_versions()  →  Vec<String>                      │
  │   installer::list_installed_sites()     →  Vec<InstalledSite>               │
  │          │                                                                   │
  │          └──────────── oneshot tx.send(LoadResult) ──────────────────────▶  │
  │                                                                              │
  ├─ ratatui::init() + App::new_loading(rx).run()                               │
  │      │                                                                       │
  │      └─ splash screen animates ──────── poll_load_result() ──────────────── ▶ data arrives
  │                                                                              │
  └─────────────────────────────────── main UI renders ◀────────────────────────┘
```

See [[Rust Oneshot Channel]] for how the channel handoff works, and [[Background Refresh]] for the 30-second auto-refresh that re-runs this fetch after startup.

---

## The HTML Parsing Step

The download server is an Apache autoindex — it returns HTML like:

```html
<a href="2.5.0-2026.04.03/">2.5.0-2026.04.03/</a>
<a href="2.4.0p24/">2.4.0p24/</a>
<a href="2.5.0b2/">2.5.0b2/</a>
```

There are three directory formats:

| Directory | Kind | cmk-dev-install arg |
|-----------|------|---------------------|
| `2.5.0-2026.04.03/` | Daily build | `2.5.0-2026-04-03` (dots → hyphens in date) |
| `2.4.0p24/` | Stable patch | `2.4.0p24` (unchanged) |
| `2.5.0b2/` | Beta | `2.5.0b2` (unchanged) |

A single static regex (via [[Rust LazyLock Static Variables]]) extracts and classifies every href in one pass.

**Why regex, not an HTML parser?** The structure is simple and stable — a full HTML parser crate would add weight without benefit. The same regex approach is used in the Python `cmk-dev-site` source this tool is based on.

---

## Install Data Flow

When the user confirms on the Configure screen, an async job is spawned:

```
InstallConfig { version, edition, site_name }
       │
       ▼
installer::spawn_install()
       │
       └─ tokio::spawn ──▶  cmk-dev-install {version} -e {edition}
                            cmk-dev-site {omd_version} -n {site_name}
                                   │  lines streamed via mpsc channel
                                   ▼
                            App::drain_job_messages() → log panel
```

**Version string transformation:** the date in daily builds changes format:
- Server directory: `2.5.0-2026.04.03` (dots in date)
- CLI argument: `2.5.0-2026-04-03` (hyphens in date)

This lives in `Version::install_arg()`.

---

## Rust Concepts at Work Here

| Concept | Where |
|---------|-------|
| [[Rust Async Await]] | `api::fetch_versions()` is async; startup and refresh await it |
| [[Rust Oneshot Channel]] | Background fetch → TUI handoff at startup and on refresh |
| [[Rust LazyLock Static Variables]] | Regex compiled once, reused per-call |
| [[Rust Result Type and Error Propagation]] | Every step returns `Result<T>`, errors bubble up |
| [[Rust Option Type]] | `list_installed_*` return `Vec::new()` on error (non-fatal) |

---

## Metadata

**Tags:** architecture
**Reference:** `src/api/mod.rs`, `src/installer/mod.rs`, `src/main.rs`, `src/ui/mod.rs`
**Related:** [[Module Boundaries]], [[Credential Auth]], [[Rust Oneshot Channel]], [[Background Refresh]], [[TUI Event Loop]]
