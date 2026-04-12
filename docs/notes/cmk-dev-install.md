---
title: cmk-dev-install
tags: [tool]
status: completed
priority: 2
publish: true
aliases: [cmk-dev-install, cmk-dev-site, cmk-dev-install-site, install toolchain]
---

# cmk-dev-install

The `cmk-dev-install` / `cmk-dev-site` / `cmk-dev-install-site` toolchain is what CMK Cockpit wraps. These scripts live in `~/Projects/cmk-dev-site/` and must be on PATH.

---

## The Three Scripts

| Script | Does |
|--------|------|
| `cmk-dev-install {version} -e {edition}` | Downloads the `.deb` for the given version + edition, installs it |
| `cmk-dev-site {omd_version}.{edition} -n {site_name}` | Creates an OMD site with the given name |
| `cmk-dev-install-site {version} -e {edition} -n {site_name}` | Combined: installs package AND creates site in one call |

---

## Version String Formats

The format passed to these tools differs from the download server directory name:

| Server directory | cmk-dev-install argument |
|-----------------|-------------------------|
| `2.5.0-2026.04.03/` | `2.5.0-2026-04-03` (date dots → hyphens) |
| `2.4.0p24/` | `2.4.0p24` (unchanged) |
| `2.5.0b2/` | `2.5.0b2` (unchanged) |

The transformation lives in `Version::to_install_arg()` in `src/api/mod.rs`.

---

## The Fallback Strategy

`installer::install_and_create_site()` tries the combined script first, then falls back:

```rust
if which_exists("cmk-dev-install-site") {
    // One subprocess: combined install + site creation
    Command::new("cmk-dev-install-site")
        .args([&version, "-e", &edition, "-n", &site_name])
        .spawn()?.wait()?;
} else {
    // Two subprocesses: install then create
    Command::new("cmk-dev-install").args([&version, "-e", &edition]).spawn()?.wait()?;
    Command::new("cmk-dev-site").args([&omd_version, "-n", &site_name]).spawn()?.wait()?;
}
```

---

## Edition Identifiers

| Edition | Identifier passed to tools |
|---------|---------------------------|
| Raw | `raw` |
| Free | `free` |
| Cloud | `cloud` |
| Enterprise | `enterprise` |
| MSP | `managed-services` |

---

## Metadata

**Tags:** tool
**Reference:** `~/Projects/cmk-dev-site/`
**Related:** [[omd]], [[Installing Screen]], [[Data Flow]]
