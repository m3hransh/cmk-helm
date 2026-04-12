---
title: Nix Flakes
tags: [tool]
status: completed
priority: 2
publish: true
aliases: [nix, flakes, nix flakes, devshell]
---

# Nix Flakes

Nix Flakes provide the reproducible dev environment and build system for CMK Cockpit. Every developer gets the exact same Rust toolchain and dependencies.

---

## Why Nix?

- **Reproducible builds** — pinned toolchain, same environment on every machine
- **Hermetic CI** — `nix flake check` runs clippy + fmt + build in a clean environment
- **Installable binary** — `nix profile install .` works without Cargo on the target

---

## Flake Inputs

```nix
inputs = {
  nixpkgs       # the main package set
  rust-overlay  # pinned Rust toolchain (replaces rustup)
  crane         # Cargo-aware Nix build helper
  flake-utils   # multi-platform boilerplate reduction
};
```

**Why `rust-overlay` instead of `pkgs.rustc`?**
`nixpkgs.rustc` tracks Nix release cycles, not Rust. `rust-overlay` lets us pin to a specific Rust version (e.g. `1.88.0`) and add extensions like `rust-analyzer` and `clippy`.

**Why `crane` instead of `rustPlatform.buildRustPackage`?**
Crane pre-builds dependencies in a separate cached derivation (`cargoArtifacts`). Changing your source code only recompiles your crate — not the entire dependency tree. Also has `cargoClippy`/`cargoFmt` checks built in. See [[Crane Build System]].

---

## Dev Shell

```bash
nix develop    # enter the dev shell
# or: direnv allow  (if nix-direnv is installed — auto-activates on cd)
```

What you get in the dev shell:
- Pinned `rustc` + `cargo` + `rust-analyzer` + `clippy` + `rustfmt`
- `cargo-watch` — auto-restart on file save
- `cargo-expand` — unfold macros (invaluable for learning what `#[derive]` generates)
- `pkg-config` + OpenSSL headers (needed to compile `reqwest` on Linux)

The `shellHook` sets `PKG_CONFIG_PATH` so `cargo build` finds OpenSSL without extra flags.

---

## Direnv Integration

`.envrc` contains one line: `use flake`

With `nix-direnv`, entering the project directory automatically enters the dev shell.

**Setup (fish shell):**
```fish
echo 'eval "$(direnv hook fish)"' >> ~/.config/fish/config.fish
direnv allow   # run once in the project directory
```

---

## Commands

| Command | What it does |
|---------|-------------|
| `nix develop` | Enter the dev shell |
| `nix build` | Build → `result/bin/cmk-cockpit` |
| `nix run` | Build and execute immediately |
| `nix flake check` | Clippy + fmt check + full build |
| `nix profile install .` | Install to user Nix profile (permanent) |
| `nix flake update` | Update all flake inputs (update nixpkgs, rust-overlay, crane) |

---

## Toolchain Pinning

`flake.nix` pins to `pkgs.rust-bin.stable."1.88.0"` instead of `stable.latest`. This is intentional — `stable.latest` changes on `nix flake update` and can silently break builds if a dep bumped its MSRV.

**MSRV incident:** `ratatui 0.28+` pulled in `instability` and `darling`, which bumped to MSRV 1.88 in later minor versions. Fix was:
```bash
cargo update instability@0.3.12 --precise 0.3.7
```
This also downgraded `darling` automatically (it's a dependency of `instability`). Then pinned toolchain to 1.88.0 in `flake.nix`.

---

## Cargo.lock Must Be Committed

Crane uses `Cargo.lock` to resolve deps in the Nix store. If `Cargo.lock` is in `.gitignore`, `nix develop` fails with:
```
error: unable to find Cargo.lock at /nix/store/...
```
For binary crates, committing `Cargo.lock` is correct practice anyway — it ensures reproducible builds.

---

## OpenSSL vs rustls

`reqwest` uses `rustls-tls` (no OpenSSL at runtime). But OpenSSL headers are still needed at **build time**. The Nix build provides `openssl.dev` in `nativeBuildInputs`. This avoids "wrong OpenSSL version" errors on machines with different OpenSSL installed.

---

## Metadata

**Tags:** tool
**Reference:** `flake.nix`, https://nixos.org/manual/nix/stable/command-ref/new-cli/nix3-flake
**Related:** [[Crane Build System]], [[Nix Build and Install]], [[Getting Started]]
