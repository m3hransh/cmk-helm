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

```
~/.cmk-credentials
       │  read_credentials() → ("user", "pass")
       ▼
api::fetch_versions(CMK_DOWNLOAD_URL)
       │  HTTP GET with Basic Auth
       ▼
https://download.checkmk.com/checkmk/
       │  Apache autoindex HTML (not JSON!)
       ▼
parse_versions_from_html()
       │  regex on <a href="..."> tags
       ▼
Vec<Version>
       │  group_by_base_version()
       ▼
Vec<VersionGroup>  ──────────────────────────▶  App::version_groups
                                                       │
installer::list_installed_versions()          (tab display)
       │  runs: omd versions -b
       ▼
Vec<String>  ─────────────────────────────────▶  App::installed_versions

installer::list_installed_sites()
       │  runs: omd sites
       ▼
Vec<SiteInfo>  ───────────────────────────────▶  App::installed_sites
```

All three data fetches happen once in `main()` before the event loop starts. The UI loop never makes network calls.

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

When the user confirms on the Configure screen:

```
InstallConfig { version, edition, site_name }
       │
       ▼
installer::install_and_create_site()
       │
       ├─▶ tries: cmk-dev-install-site {version} -e {edition} -n {site_name}
       │          (combined shortcut — one subprocess)
       │
       └─▶ fallback (if binary not on PATH):
              cmk-dev-install {version} -e {edition}
              cmk-dev-site {omd_version}.{edition} -n {site_name}
```

`which_exists()` checks if `cmk-dev-install-site` is on PATH before attempting it. The fallback runs two separate subprocesses in sequence.

**Version string transformation:** the date in daily builds changes format when passed to `cmk-dev-install`:
- Server directory: `2.5.0-2026.04.03` (dots in date)
- CLI argument: `2.5.0-2026-04-03` (hyphens in date)

This transformation lives in `Version::to_install_arg()`.

---

## Rust Concepts at Work Here

| Concept | Where |
|---------|-------|
| [[Rust Async Await]] | `api::fetch_versions()` is async; startup awaits it |
| [[Rust LazyLock Static Variables]] | Regex compiled once, reused per-call |
| [[Rust Result Type and Error Propagation]] | Every step returns `Result<T>`, errors bubble up |
| [[Rust Option Type]] | `list_installed_*` return `Vec::new()` on error (non-fatal) |

---

## Metadata

**Tags:** architecture
**Reference:** `src/api/mod.rs`, `src/installer/mod.rs`, `src/main.rs`
**Related:** [[Module Boundaries]], [[Credential Auth]], [[Version List Screen]], [[Installing Screen]]
