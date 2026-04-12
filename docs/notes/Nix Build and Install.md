---
title: Nix Build and Install
tags: [deployment]
status: completed
priority: 1
publish: true
aliases: [nix build, nix install, nix profile install, building]
---

# Nix Build and Install

How to build and install CMK Cockpit using Nix outside of the dev loop.

---

## Build Commands

```bash
nix build              # builds → result/bin/cmk-cockpit
nix run                # builds + runs immediately (no persistent artifact)
nix flake check        # clippy + fmt check + full build (what CI runs)
```

`nix build` creates a `result` symlink in the project root pointing to the built binary in the Nix store. The binary is self-contained (no runtime OpenSSL needed — it uses rustls).

---

## Install to Nix Profile

```bash
nix profile install .  # install from local flake
# or:
nix profile install github:you/cmk-cockpit  # install from GitHub
```

After installing, `cmk-cockpit` is on your PATH permanently (until you remove it).

**Remove:**
```bash
nix profile list                          # find the index of cmk-cockpit
nix profile remove <index>
```

---

## What `nix flake check` Does

`nix flake check` runs all derivations in the `checks` output:
- `cargoClippy` — runs `cargo clippy` with warnings as errors
- `cargoFmt` — runs `cargo fmt --check` (fails if formatting is off)
- The full binary build — fails if it doesn't compile

This is what CI should call. Running it locally before pushing catches the same issues.

---

## Build Internals (Crane)

The build is split into two derivations by [[Crane Build System]]:
1. `cargoArtifacts` — builds all dependencies from `Cargo.lock`
2. `cmk-cockpit` — builds the binary using the cached artifacts

Nix caches derivations by content hash. If `Cargo.lock` hasn't changed, step 1 is a cache hit — only your code compiles.

---

## Runtime Requirements

The installed binary still needs the external tools at runtime:
- `cmk-dev-install`, `cmk-dev-site` on PATH
- `omd` on PATH (optional — app degrades gracefully)
- `~/.cmk-credentials` file

These are NOT bundled in the Nix build — they're external scripts from a separate repo.

---

## Metadata

**Tags:** deployment
**Related:** [[Nix Flakes]], [[Crane Build System]], [[Getting Started]]
