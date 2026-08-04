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
┌─ Installed Versions (1 unused) ─┐
│ ● 2.4.0p24.cee                  │  ← ● = a site runs this version
│ ★ 2.5.0b3.cre                   │  ← ★ = omd default version
│ ○ 2.6.0-2026.03.31.cee          │  ← ○ = unused, a prune candidate
├─ Installed Sites ───────────────┤
│ ★ mysite    2.4.0p24.cee        │  ← ★ = default site
│   testsite  2.5.0.raw           │
└─────────────────────────────────┘
```

The version markers exist so the effect of `D` ([[Prune Unused Versions]]) is visible before pressing it. The title shows the prune-candidate count, and is plain `Installed Versions` when there is nothing to clean up.

If `omd` is not available: "(none found)" in each section.

---

## Keys

| Key | Pane | Action |
|-----|------|--------|
| `j` / `k` | either | Move the selection |
| `d` | Installed Versions | Delete the selected version |
| `D` | Installed Versions | Prune every unused version — see [[Prune Unused Versions]] |
| `s` | Installed Versions | Create a site from the selected version |
| `d` | Installed Sites | Delete the selected site |

Destructive keys open an inline `y/n` confirmation (`RightPaneMode::ConfirmDelete`) rather than acting immediately.

---

## Data Source

Three `omd` calls, gathered together into a `LoadResult`:

```rust
let installed_versions = installer::list_installed_versions().unwrap_or_default();
let installed_sites    = installer::list_installed_sites().unwrap_or_default();
let default_version    = installer::default_version();
```

**`list_installed_versions()`** runs `omd versions -b`, parses stdout line by line.

**`default_version()`** runs `omd versions` (no `-b`) and returns the line marked `(default)`. Needed because that version backs `/omd/versions/default` and must be protected from prune.

**`list_installed_sites()`** runs `omd sites`, parses the output table into `InstalledSite`:

```rust
pub struct InstalledSite {
    pub name: String,
    pub version: String,
    pub is_default: bool,   // whether this is the OMD default site
}
```

See [[omd]] for the command output formats.

---

## When the Panel Refreshes

The panel is not a startup-only snapshot. It re-reads omd state:

- on the initial load (behind the splash screen),
- every time a job finishes — `refresh_installed()` runs from `drain_job_messages()`, so an install, delete, or prune is reflected as soon as it completes,
- on the periodic [[Background Refresh]].

Re-reading on every frame would mean subprocess calls at 60 fps — event-driven refresh gives the same freshness for a fraction of the cost.

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
**Related:** [[TUI Layout Design]], [[omd]], [[Error Handling Strategy]], [[Prune Unused Versions]], [[Background Refresh]]
