# CMK Cockpit

A Rust TUI application (Ratatui) that provides an interactive terminal interface for browsing, selecting, and installing Checkmk packages via the `cmk-dev-site` / `cmk-dev-install` toolchain.

The developer learning Rust while building this — explain key Rust concepts in comments and in the `docs/` vault.

---

## Project Structure

```
src/
├── main.rs           Entry point: terminal init, tokio runtime, cleanup on exit
├── api/mod.rs        HTTP + parsing: reads ~/.cmk-credentials, fetches HTML
│                     directory listing, parses versions with regex
├── ui/mod.rs         App state machine (PackageList → Configure → Installing),
│                     event loop, all Ratatui rendering
└── installer/mod.rs  Subprocess wrappers for cmk-dev-install, cmk-dev-site, omd

docs/                 Obsidian vault — architecture decisions and Rust learning notes
flake.nix             Nix build + dev shell (crane-based, rust-overlay toolchain)
Cargo.toml            Dependencies: ratatui, tokio, reqwest, regex, anyhow
.envrc                `use flake` — automatic dev shell via direnv
```

---

## Key Commands

| Command | Purpose |
|---------|---------|
| `nix develop` | Enter dev shell (or automatic with `direnv allow`) |
| `cargo run` | Run the TUI |
| `cargo watch -x run` | Auto-restart on file save |
| `cargo test` | Run unit tests (includes HTML parsing tests in api/) |
| `cargo clippy` | Lint |
| `cargo fmt` | Format code |
| `cargo expand` | Unfold macros — great for understanding `#[derive]` |
| `nix build` | Build via Nix → `result/bin/cmk-cockpit` |
| `nix run` | Build and run via Nix |
| `nix flake check` | Run clippy + fmt + full build |
| `nix profile install .` | Install to user Nix profile |

---

## External Dependencies (Runtime)

These must be on PATH when running the tool:

| Tool | Where it lives | Role |
|------|---------------|------|
| `cmk-dev-install` | `~/Projects/cmk-dev-site/` | Downloads + installs .deb |
| `cmk-dev-site` | `~/Projects/cmk-dev-site/` | Creates + configures OMD site |
| `cmk-dev-install-site` | `~/Projects/cmk-dev-site/` | Combined shortcut |
| `omd` | System | Manages Checkmk versions and sites |

---

## Credentials

The app reads `~/.cmk-credentials` — same file as cmk-dev-site:

```
username:password
```

HTTP Basic Auth against `https://download.checkmk.com/checkmk/`.

---

## Server Response Format

The download server returns **HTML directory listings**, not JSON. Example href:

```
2.4.0-2025.04.07.cee/
```

Format: `{base_version}-{YYYY.MM.DD}.{edition}/`

The `api` module parses this with a regex identical to cmk-dev-site's `VersionParser`.
When passing a version to `cmk-dev-install`, the date dots become hyphens:
`2.4.0-2025-04-07`

---

## Coding Conventions

- **Errors**: `anyhow::Result<T>` everywhere. Use `.with_context(|| ...)` to add context before propagating with `?`. No `.unwrap()` except for compile-time-constant regex patterns.
- **No null**: use `Option<T>`. Use `.map()`, `.unwrap_or()`, `if let` — never `.unwrap()` without a comment explaining why it can't fail.
- **Modules**: one concern per module. No cross-module imports except `ui` importing from `api` and `installer`.
- **Async**: only `main()` and `api::*` functions are async. The TUI event loop stays synchronous.
- **Comments**: explain *why*, not *what*. Add Rust concept explanations in block comments — this is a learning project.
- **Tests**: unit tests live in `#[cfg(test)]` blocks at the bottom of each module. `cargo test` must pass.
- **Formatting**: `cargo fmt` before every commit. Clippy warnings are errors in CI.

---

## Git Workflow

Commit after each logical unit of change. Commit message format:

```
<type>: <short description>

- bullet points for details
```

Types: `feat`, `fix`, `chore`, `docs`, `refactor`, `test`

---

## Documentation

See the `docs/` Obsidian vault:

- [000 Index](docs/000-index.md) — start here
- [001 Architecture](docs/001-architecture.md) — design decisions and data flow
- [002 Nix Setup](docs/002-nix-setup.md) — flake walkthrough
- [003 Rust Learning](docs/003-rust-learning.md) — Rust concepts encountered

Update the vault whenever a new pattern, decision, or Rust concept appears.
