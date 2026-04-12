---
title: Installed Versions and Sites Panel
tags: [feature]
status: completed
priority: 2
publish: true
aliases: [right panel, installed panel, omd panel]
---

# Installed Versions and Sites Panel

The right-hand panel (40% of screen width) that is always visible regardless of which left-pane screen is active. It shows what Checkmk versions and sites are currently installed locally.

---

## What It Shows

```
┌─ Installed Versions ────────────┐
│ 2.4.0p24.cee                    │
│ 2.5.0-2026.04.03.raw            │
│                                 │
├─ Installed Sites ───────────────┤
│ ★ mysite    2.4.0p24.cee        │  ← ★ = default site
│   testsite  2.5.0.raw           │
└─────────────────────────────────┘
```

If `omd` is not available: "(none found)" in each section.

---

## Data Source

Populated once at startup in `main()`:

```rust
let installed_versions = installer::list_installed_versions().unwrap_or_default();
let installed_sites    = installer::list_installed_sites().unwrap_or_default();
```

**`list_installed_versions()`** runs `omd versions -b`, parses stdout line by line.

**`list_installed_sites()`** runs `omd sites`, parses the output table into `SiteInfo`:

```rust
pub struct SiteInfo {
    pub name: String,
    pub version: String,
    pub is_default: bool,   // whether this is the OMD default site
}
```

See [[omd]] for the command output formats.

---

## Why Fetch Once at Startup?

The right panel is a snapshot — it shows what was installed when the app started. If you install a new version during the session, the panel won't update automatically (the app exits after install anyway).

Fetching on every frame render would be expensive (subprocess calls). The static approach is correct for this use case.

---

## Graceful Degradation

Both `list_installed_versions()` and `list_installed_sites()` return `Result<Vec<_>>`. In `main()`, errors are converted to empty Vecs via `.unwrap_or_default()`:

```rust
// If omd isn't installed, we get an empty Vec — not a crash
let installed_versions = installer::list_installed_versions().unwrap_or_default();
```

The panel then renders "(none found)". This allows the app to run on machines where Checkmk isn't installed yet — useful when first setting up a dev environment.

---

## Metadata

**Tags:** feature
**Related:** [[TUI Layout Design]], [[omd]], [[Error Handling Strategy]]
