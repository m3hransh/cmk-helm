---
id: "002"
title: Nix Flake Setup
tags: [nix, flakes, devshell, build]
created: 2025-04-03
---

# Nix Flake Setup

← [[000-index]]

---

## Why Nix?

Nix gives us:
- **Reproducible builds** — every developer gets the exact same Rust toolchain, version of OpenSSL, etc.
- **Hermetic CI** — `nix flake check` runs clippy + fmt + build in a clean environment
- **Installable artifact** — `nix profile install github:you/cmk-cockpit` works without Cargo on the target machine

---

## Flake Inputs

```nix
inputs = {
  nixpkgs            # The main package set
  rust-overlay       # Pinned Rust toolchain (replaces rustup)
  crane              # Cargo-aware build helper for Nix
  flake-utils        # Reduces multi-platform boilerplate
};
```

### Why `rust-overlay` instead of `pkgs.rustc`?

`nixpkgs.rustc` tracks the Nix release cycle, not the Rust release cycle. `rust-overlay` lets us pin to `stable.latest` or a specific date, and add extensions like `rust-analyzer` and `clippy` alongside the compiler. Critically, it stays in sync with `Cargo.toml` edition requirements.

### Why `crane` instead of `rustPlatform.buildRustPackage`?

| | `rustPlatform.buildRustPackage` | `crane` |
|---|---|---|
| Dependency caching | Single derivation — rebuilds all deps on any change | Splits deps into a separate cached derivation |
| Lock file handling | Needs `cargoHash` (annoying to update) | Uses `Cargo.lock` directly |
| Extra checks | Manual | `cargoClippy`, `cargoFmt` built in |
| Maturity | Stable, in nixpkgs | Actively developed, recommended for new projects |

Crane pre-builds all your Cargo.lock dependencies in one derivation (`cargoArtifacts`). When you change your source code, only your crate is recompiled — not the entire dependency tree. This makes incremental Nix builds fast.

---

## Dev Shell

```bash
nix develop    # or automatic with direnv + use flake
```

What you get:
- Pinned `rustc` + `cargo` + `rust-analyzer` + `clippy` + `rustfmt`
- `pkg-config` and OpenSSL headers (needed to compile `reqwest` on Linux)
- `cargo-watch` — auto-restart on file save
- `cargo-expand` — unfold macros (great for learning what `#[derive]` generates)

The `shellHook` sets `PKG_CONFIG_PATH` so `cargo build` finds OpenSSL without any extra flags.

---

## Direnv Integration

`.envrc` contains one line: `use flake`

With `nix-direnv` installed, `cd`-ing into the project directory automatically:
1. Evaluates `flake.nix`
2. Enters `devShells.default`
3. Puts all tools on PATH

**Setup (fish shell):**
```fish
echo 'eval "$(direnv hook fish)"' >> ~/.config/fish/config.fish
direnv allow   # run once in the project directory
```

---

## Outputs Summary

| Command | What it does |
|---------|-------------|
| `nix develop` | Enter the dev shell |
| `nix build` | Build `result/bin/cmk-cockpit` |
| `nix run` | Build and execute immediately |
| `nix flake check` | Run clippy + fmt + full build |
| `nix profile install .` | Install to user profile (permanent) |

---

## First-time setup after cloning

```bash
# 1. Enter dev shell (or `direnv allow` if using direnv)
nix develop

# 2. Generate Cargo.lock (not committed — Nix uses it to resolve deps)
cargo generate-lockfile

# 3. Build and run
cargo run
```

> **Cargo.lock must be committed and NOT gitignored.**
> crane's `cleanCargoSource` strips files that match `.gitignore`. If `Cargo.lock` is ignored, crane cannot find it in the Nix store and the dev shell evaluation fails with:
> `error: unable to find Cargo.lock at /nix/store/...`
> For binary crates, committing `Cargo.lock` is the correct practice anyway — it ensures reproducible builds.

---

## Pinning the Rust Toolchain Version

`flake.nix` uses `pkgs.rust-bin.stable."1.88.0"` instead of `stable.latest`. This is intentional:

- `stable.latest` changes whenever `nix flake update` is run, which can silently break builds if a dependency bumped its MSRV (Minimum Supported Rust Version)
- A pinned version makes the toolchain an explicit, reviewed choice — like pinning a Docker image tag

**Lesson learned:** `ratatui 0.28+` pulls in `instability` (for marking unstable APIs) and `darling` (a proc-macro helper). Both bumped MSRV to 1.88 in their 0.3.12 / 0.23.0 releases respectively. The fix was:

1. `cargo update instability@0.3.12 --precise 0.3.7` — this also downgraded `darling` to 0.20.11 automatically (instability depends on darling)
2. Pin `flake.nix` toolchain to `1.88.0` — so future users in the Nix dev shell won't hit the same issue once the transitive deps are upgraded again

---

## OpenSSL vs rustls

`reqwest` is configured with `rustls-tls` and `default-features = false`. This means:
- No OpenSSL dependency at **runtime** — the binary is self-contained
- OpenSSL headers are still needed at **build time** (cargo links against it during compilation even with rustls)
- The Nix build provides `openssl.dev` in `nativeBuildInputs` to satisfy this

This avoids the classic "wrong OpenSSL version" problem on deployment machines.
