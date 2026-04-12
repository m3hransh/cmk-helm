---
title: Getting Started
tags: [guide]
status: completed
priority: 3
publish: true
aliases: [onboarding, setup, first run]
---

# Getting Started

This guide gets a new engineer from zero to a running CMK Cockpit in ~10 minutes.

---

## Prerequisites

- Nix with flakes enabled (see [[Nix Flakes]] if not set up)
- `~/.cmk-credentials` file with your download credentials (`username:password`)
- `cmk-dev-install`, `cmk-dev-site`, and `omd` on your PATH (from `~/Projects/cmk-dev-site/`)

---

## Steps

### 1. Clone and enter the project

```bash
git clone <repo-url> ~/Projects/cmk-helm
cd ~/Projects/cmk-helm
```

### 2. Enter the dev shell

```bash
nix develop
# or, if you have direnv installed:
direnv allow   # run once — after that, cd'ing in auto-activates the shell
```

This gives you: pinned `rustc`, `cargo`, `rust-analyzer`, `clippy`, `rustfmt`, `cargo-watch`, `cargo-expand`.

### 3. Build and run

```bash
cargo run
```

First build downloads and compiles all dependencies — takes a few minutes. Subsequent builds are fast.

### 4. Use the app

| Key | Action |
|-----|--------|
| `j` / `↓` | Move down |
| `k` / `↑` | Move up |
| `h` / `←` | Previous tab |
| `l` / `→` | Next tab |
| `Enter` | Confirm / advance to next screen |
| `Esc` | Go back |
| `q` | Quit |

---

## Development Loop

```bash
cargo watch -x run   # auto-restarts on file save
cargo test           # run all unit tests
cargo clippy         # lint (warnings are errors in CI)
cargo fmt            # format code
```

See [[Development Workflow]] for the full day-to-day workflow.

---

## Project Orientation

| If you want to… | Look at… |
|----------------|---------|
| Understand the overall design | [[CMK Cockpit]] and [[Module Boundaries]] |
| Understand the screen flow | [[App State Machine]] |
| Understand how versions are fetched | [[Data Flow]] |
| Learn Rust patterns used here | [[Rust Ownership and Borrowing]], [[Rust Enums and Algebraic Data Types]], [[Rust Result Type and Error Propagation]] |
| Understand the Nix build | [[Nix Flakes]], [[Crane Build System]] |
| Add a new screen | [[App State Machine]], [[TUI Layout Design]] |

---

## Common Pitfalls

**`~/.cmk-credentials` not found** — The app will fail on startup with a descriptive error. Create the file with your credentials in `username:password` format.

**`cmk-dev-install` not on PATH** — The install step will fail. Make sure `~/Projects/cmk-dev-site/` is in your `PATH`.

**`omd` not on PATH** — The installed versions/sites panel will show "(none found)" but the app will still run. This is safe — see [[Installed Versions and Sites Panel]].

**Cargo.lock must be committed** — Nix uses it to resolve deps. If you accidentally gitignore it, `nix develop` fails. See [[Nix Flakes]].

---

## Metadata

**Tags:** guide
**Related:** [[CMK Cockpit]], [[Development Workflow]], [[Nix Flakes]], [[Module Boundaries]]
