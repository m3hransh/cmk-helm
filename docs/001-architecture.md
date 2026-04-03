---
id: "001"
title: Architecture Decisions
tags: [architecture, design, rust]
created: 2025-04-03
---

# Architecture Decisions

← [[000-index]]

---

## Module Boundaries

The code is split into three modules, each with a single responsibility:

| Module | Owns | Does NOT own |
|--------|------|--------------|
| `api/` | Server communication, data types, credential reading | UI, install logic |
| `ui/` | All rendering, App state, event loop | HTTP, subprocess spawning |
| `installer/` | Subprocess wrapping (`cmk-dev-install`, `omd`) | HTTP, UI |

**Why this split?**
Each module can be tested independently. The `api/` tests run without a terminal or a subprocess. The `installer/` module can be stubbed in tests without touching the real network.

---

## State Machine Pattern

The UI is a state machine with three screens:

```
PackageList  ──Enter──▶  Configure  ──Enter──▶  Installing
     ▲                      │
     └────────Esc────────────┘
```

Represented in Rust as an enum:

```rust
enum Screen {
    PackageList,
    Configure { selected: Package, site_input: String },
    Installing { config: InstallConfig },
}
```

**Why an enum?**  
Rust enums with data make *illegal states unrepresentable*. It is impossible to be in `Installing` without an `InstallConfig`, and impossible to have an `InstallConfig` without a selected `Package`. The compiler enforces this — no runtime null checks needed.

---

## Async Boundary

Only two things are async:

1. `main()` — needs to bootstrap tokio
2. `api::fetch_packages()` — network I/O

The TUI event loop in `ui::App::run()` is **not** async in the hot path. Event polling uses a 16ms timeout, keeping the loop synchronous and frame-timing predictable.

**Why not make the event loop fully async?**  
Ratatui's rendering is synchronous. Mixing async I/O and synchronous rendering in the same loop adds complexity without benefit for this use case. The right pattern (future work) is to fetch packages once before entering the loop and store the result in `App::packages`.

---

## Error Handling Strategy

All functions return `anyhow::Result<T>`. The `?` operator propagates errors upward, attaching context at each level:

```rust
let html = client.get(url)
    .send().await
    .with_context(|| format!("Failed to reach {url}"))?;
```

At the top level (`main`), any unhandled error prints a user-readable message and exits. No `unwrap()` in production paths — only in:
- `Regex::new(...)` for compile-time-constant patterns (cannot fail)
- Test code

---

## Why Ratatui?

- **Active community** — the most maintained Rust TUI library as of 2025
- **Immediate mode** — no widget tree to synchronize, just describe what to draw
- **Backend agnostic** — works with crossterm (cross-platform) out of the box
- **Composable layouts** — `Constraint`-based layout system is straightforward

---

## Data Flow

```
~/.cmk-credentials
      │
      ▼
api::read_credentials()
      │
      ▼
api::fetch_packages(CMK_DOWNLOAD_URL)
      │ HTTP GET + Basic Auth
      ▼
download.checkmk.com/checkmk/   (HTML directory listing)
      │
      ▼
parse_packages_from_html()      (regex on <a href> tags)
      │
      ▼
Vec<Package>  ──────────────▶  ui::App::packages
                                     │
                               User selects
                                     │
                                     ▼
                            installer::install_and_create_site(config)
                                     │
                              cmk-dev-install
                              cmk-dev-site
                              omd (managed internally by those tools)
```

---

## Future Work

- [ ] Replace stub packages with real `fetch_packages()` call in `main()`
- [ ] Add edition filter in the package list screen
- [ ] Add a text popup for showing live install output (requires spawning with piped stdout)
- [ ] Support `git:branch:hash` version specifier for developer builds
- [ ] Add a config screen for advanced OMD options (`--omd-configs`)
