---
title: omd
tags: [tool]
status: completed
priority: 2
publish: true
aliases: [OMD, Open Monitoring Distribution, omd tool]
---

# omd

`omd` (Open Monitoring Distribution) is the system tool that manages Checkmk installations and sites. CMK Cockpit reads from `omd` at startup to show what's already installed.

---

## What omd Does

- Manages multiple Checkmk **versions** installed side by side (one per `.deb` you install)
- Manages multiple Checkmk **sites** — isolated monitoring instances on top of a version
- Each site runs independently: separate config, data, ports

---

## Commands Used by CMK Cockpit

**`omd versions -b`** — list installed versions in bare format (one per line):

```
2.4.0p24.cee
2.5.0-2026.04.03.raw
```

**`omd sites`** — list all sites with their OMD version:

```
SITE            VERSION         COMMENTS
mysite          2.4.0p24.cee    default version
testsite        2.5.0.raw
```

The `default` site is marked with `is_default: true` in `SiteInfo`, displayed with `★` in the UI.

---

## omd Version String Format

The version string `omd` uses includes the edition suffix:
- `2.4.0p24.cee` — stable patch 24 of 2.4.0, Cloud edition
- `2.5.0-2026.04.03.raw` — daily build, Raw edition
- `2.5.0b2.enterprise` — beta 2, Enterprise edition

This is different from the directory format on the download server (see [[Data Flow]]).

---

## Graceful Degradation

If `omd` is not on PATH, `list_installed_versions()` and `list_installed_sites()` return empty Vecs (via `.unwrap_or_default()`). The right panel shows "(none found)" but the app continues normally.

This allows the app to run on machines where Checkmk isn't installed — for browsing versions and preparing for an install.

---

## Metadata

**Tags:** tool
**Reference:** `~/Projects/cmk-dev-site/`, `omd --help`
**Related:** [[cmk-dev-install]], [[Installed Versions and Sites Panel]], [[Data Flow]]
