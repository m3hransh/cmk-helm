---
title: CMK Cockpit — Map of Content
tags: [index, moc]
status: completed
priority: 3
publish: true
aliases: [index, moc, home]
---

# CMK Cockpit — Map of Content

**CMK Cockpit** is a Rust TUI application for browsing, selecting, and installing Checkmk packages interactively in the terminal. It wraps the internal `cmk-dev-install` / `cmk-dev-site` / `omd` toolchain behind a keyboard-driven UI.

> New here? Start with [[Getting Started]] — it walks you through the dev setup and first run in under 10 minutes.

---

## The App

| Note | What you'll find |
|------|----------------|
| [[CMK Cockpit]] | What the app does, why it exists, and how it fits into the Checkmk dev workflow |
| [[Getting Started]] | Clone → dev shell → first run walkthrough |
| [[Development Workflow]] | Day-to-day commands, git flow, doc updates |

---

## Architecture & Design

| Note | What you'll find |
|------|----------------|
| [[Module Boundaries]] | Why the code is split into `api/`, `ui/`, `installer/` |
| [[App State Machine]] | How screens are modelled as a Rust enum — the key architectural pattern |
| [[Data Flow]] | End-to-end: credentials → HTTP → HTML parsing → user selection → subprocess |
| [[TUI Layout Design]] | Split layout, left/right panels, key hint bar |
| [[Async Boundary Design]] | What is async, what is sync, and why |
| [[Error Handling Strategy]] | `anyhow::Result<T>`, `?` propagation, `.with_context()` |

---

## Rust Concepts (Learning Reference)

These notes explain Rust patterns as they appear in this codebase. If you're coming from Python, Go, or TypeScript, start here.

| Note | What you'll find |
|------|----------------|
| [[Rust Ownership and Borrowing]] | The borrow checker — the most distinctive Rust concept |
| [[Rust Enums and Algebraic Data Types]] | Enums that carry data — not C enums |
| [[Rust Option Type]] | No null — `Option<T>` instead |
| [[Rust Result Type and Error Propagation]] | `Result<T, E>` and the `?` operator |
| [[Rust Closures]] | Anonymous functions that capture scope |
| [[Rust Iterators]] | `.map().filter().collect()` — lazy and zero-cost |
| [[Rust String vs str]] | Two string types and when to use each |
| [[Rust Async Await]] | `async fn`, `.await`, and the Tokio runtime |
| [[Rust Oneshot Channel]] | Single-use channel for background task → UI handoff |
| [[Rust Derive Macros]] | Auto-generating trait impls with `#[derive(...)]` |
| [[Rust LazyLock Static Variables]] | Compile-once statics for expensive initialisation |
| [[Rust Let Else Pattern]] | Guarded enum destructuring without nested `if let` |
| [[Rust Pattern Matching]] | `match`, `if let`, `matches!` — exhaustive branching |
| [[Rust Traits]] | Rust's equivalent of interfaces — what you'll need next |

---

## Ratatui & TUI Concepts

| Note | What you'll find |
|------|----------------|
| [[Ratatui TUI Framework]] | How Ratatui works: immediate mode, widgets, layouts |
| [[TUI Event Loop]] | The draw → poll → handle loop and 60fps timing |

---

## Tools & Dependencies

| Note | What you'll find |
|------|----------------|
| [[Ratatui TUI Framework]] | TUI rendering library |
| [[Tokio Async Runtime]] | Async executor for Rust |
| [[Reqwest HTTP Client]] | HTTP requests with rustls |
| [[anyhow Error Handling]] | Ergonomic error type with context chains |
| [[omd]] | OMD tool — manages Checkmk versions and sites |
| [[cmk-dev-install]] | Downloads and installs Checkmk .deb packages |
| [[Nix Flakes]] | Reproducible dev shell and build |
| [[Crane Build System]] | Cargo-aware Nix build helper |

---

## Features

| Note | What you'll find |
|------|----------------|
| [[Version List Screen]] | Browsing available versions grouped by base version |
| [[Edition Picker Screen]] | Choosing between Raw, Free, Cloud, Enterprise editions |
| [[Configure Screen]] | Entering site name and confirming install config |
| [[Installing Screen]] | Live install output and progress |
| [[Installed Versions and Sites Panel]] | Right-hand panel showing local OMD state |
| [[Background Refresh]] | Auto-refresh every 30 s and manual `r` key |
| [[Credential Auth]] | How `~/.cmk-credentials` is read and used |

---

## Workflows & Operations

| Note | What you'll find |
|------|----------------|
| [[Git Workflow]] | Commit types, message format, branching |
| [[Testing Workflow]] | `cargo test`, `#[ignore]`, integration tests |
| [[Nix Build and Install]] | `nix build`, `nix profile install`, `nix flake check` |

---

## Metadata

**Tags:** index, moc
**Related:** [[CMK Cockpit]], [[Getting Started]], [[Module Boundaries]]
