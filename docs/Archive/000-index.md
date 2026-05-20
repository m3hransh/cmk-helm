---
id: "000"
title: CMK Helm — Map of Content
tags: [index, moc]
created: 2025-04-03
---

# CMK Helm — Map of Content

CMK Helm is a **Rust TUI application** that provides an interactive terminal interface for installing and managing Checkmk sites via the internal `cmk-dev-site` / `cmk-dev-install` toolchain.

## Why this exists

The existing CLI tools (`cmk-dev-install`, `cmk-dev-site`, `omd`) are powerful but require knowing the right arguments and order of operations. CMK Helm makes the workflow interactive and discoverable:

1. Browse available package versions from the download server
2. Select a version and edition
3. Enter configuration (site name, etc.)
4. Watch the installation run

---

## Notes in this vault

| Note | Topic |
|------|-------|
| [[001-architecture]] | Module structure, state machine pattern, design decisions |
| [[002-nix-setup]] | Nix flake explained — inputs, crane, dev shell, `nix build` |
| [[003-rust-learning]] | Rust concepts encountered: ownership, enums, async, TUI event loop |

---

## Project structure

```
src/
├── main.rs           Entry point — terminal init, tokio runtime, cleanup
├── api/mod.rs        HTTP client — HTML parsing, Basic Auth, Package struct
├── ui/mod.rs         App state machine, event loop, all rendering (Ratatui)
└── installer/mod.rs  Wraps cmk-dev-install / cmk-dev-site / omd commands

docs/                 This Obsidian vault
flake.nix             Nix build + dev shell (crane-based)
Cargo.toml            Rust dependencies
```

## Key external tools

| Tool | Role |
|------|------|
| `cmk-dev-install` | Downloads + installs a Checkmk .deb package |
| `cmk-dev-site` | Creates and configures an OMD site |
| `cmk-dev-install-site` | Combined shortcut for the above two |
| `omd` | OMD (Open Monitoring Distribution) — manages Checkmk versions and sites |

## Links

- Source: `~/Projects/cmk-helm/`
- cmk-dev-site source: `~/Projects/cmk-dev-site/`
- Credentials: `~/.cmk-credentials` (format: `username:password`)
- Download server: `https://download.checkmk.com/checkmk/`
