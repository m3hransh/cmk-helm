---
title: Crane Build System
tags: [tool]
status: completed
priority: 1
publish: true
aliases: [crane, cargo nix build]
---

# Crane

Crane is a Nix library for building Rust projects. It's used instead of Nix's built-in `rustPlatform.buildRustPackage` because it handles dependency caching much better.

---

## The Key Advantage: Dependency Caching

`rustPlatform.buildRustPackage` builds everything in one derivation. Change one line of source code → rebuild all 100+ dependencies from scratch.

Crane splits the build into two derivations:

1. **`cargoArtifacts`** — builds only the dependencies from `Cargo.lock`. This derivation is cached by Nix as long as `Cargo.lock` doesn't change.
2. **`cmk-cockpit`** — builds only your crate, using `cargoArtifacts` as input. This rebuilds only when your source changes.

In practice: changing `src/ui/mod.rs` takes seconds (only your crate compiles), not minutes (all deps).

---

## Extra Checks

Crane has built-in support for quality checks:

```nix
# In flake.nix checks:
checks = {
  my-clippy = crane.cargoClippy cargoArgs;
  my-fmt    = crane.cargoFmt    cargoArgs;
};
```

`nix flake check` runs all of these. This is what CI should call.

---

## Comparison

| | `rustPlatform.buildRustPackage` | `crane` |
|--|--|--|
| Dep caching | No (one derivation) | Yes (two derivations) |
| Lock file | Needs `cargoHash` (annoying to update) | Uses `Cargo.lock` directly |
| Built-in checks | Manual | `cargoClippy`, `cargoFmt` |
| Maturity | Stable, in nixpkgs | Actively developed |

---

## Metadata

**Tags:** tool
**Reference:** https://crane.dev, https://github.com/ipetkov/crane
**Related:** [[Nix Flakes]], [[Nix Build and Install]]
