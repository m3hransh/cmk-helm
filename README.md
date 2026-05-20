# CMK Helm

A terminal UI for browsing, selecting, and installing [Checkmk](https://checkmk.com) packages interactively. Built with Rust and [Ratatui](https://ratatui.rs).

```
┌─ CMK Helm ── [2.6.0] [2.5.0] [2.4.0] ────────────────────────────────┐
├──────────────────────────┬────────────────────────────────────────────┤
│  Version Browser         │  Installed Versions                        │
│                          │   ▶ 2.6.0-2026.04.07.enterprise            │
│  ▶ daily  2026.04.07    ├────────────────────────────────────────────┤
│    stable p24            │  Installed Sites                           │
│                          │  ★ mysite  2.6.0.enterprise                │
├──────────────────────────┴────────────────────────────────────────────┤
│  ⟳ install 2.6.0  ✓ rm 2.5.0                                         │
└───────────────────────────────────────────────────────────────────────┘
```

---

## Requirements

- **[Nix](https://nixos.org/download)** with flakes enabled
- A `~/.cmk-credentials` file with your Checkmk download credentials:

```
username:password
```

The bundled Nix package includes `cmk-dev-install` and `cmk-dev-site` automatically — no separate installation needed.

### Enable Nix flakes

If you haven't enabled flakes yet, add this to `/etc/nix/nix.conf` (or `~/.config/nix/nix.conf`):

```
experimental-features = nix-command flakes
```

---

## Run directly from GitHub

No cloning or installation required — Nix fetches, builds, and runs in one step:

```bash
nix run github:your-org/cmk-helm
```

> Replace `your-org/cmk-helm` with the actual GitHub path (e.g. `github:mehran/cmk-helm`).

Nix pins all dependencies via `flake.lock`, so the build is fully reproducible — the same command produces the same binary for every user.

---

## Install to your Nix profile

To have `cmk-helm` available as a regular command:

```bash
nix profile install github:your-org/cmk-helm
```

Then just run:

```bash
cmk-helm
```

Upgrade later with:

```bash
nix profile upgrade cmk-helm
```

---

## Key bindings

### Version Browser

| Key | Action |
|-----|--------|
| `j` / `k` | Move down / up |
| `h` / `l` or `Tab` / `ShiftTab` | Switch base-version tab |
| `Enter` or `i` | Open edition picker |
| `r` | Refresh version list |
| `Alt+h/j/k/l` | Switch active pane |
| `q` / `Esc` | Quit |

### Edition Picker

| Key | Action |
|-----|--------|
| `j` / `k` | Select edition |
| `Enter` | Confirm → configure site name |
| `Esc` | Back |

### Configure (site name)

| Key | Action |
|-----|--------|
| type | Enter site name |
| `Backspace` | Delete character |
| `Enter` | Start install |
| `Esc` | Back |

### Installed Versions

| Key | Action |
|-----|--------|
| `j` / `k` | Navigate |
| `d` | Delete selected version |
| `s` | Create a new site from this version |
| `q` / `Esc` | Quit |

### Installed Sites

| Key | Action |
|-----|--------|
| `j` / `k` | Navigate |
| `d` | Delete selected site |
| `q` / `Esc` | Quit |

### Log Panel

| Key | Action |
|-----|--------|
| `h` / `l` | Switch between job tabs |
| `j` / `k` | Scroll output |
| `c` | Enter copy mode (terminal handles mouse for text selection) |
| `Esc` | Exit copy mode / quit |

---

## Development

```bash
# Clone
git clone https://github.com/your-org/cmk-helm
cd cmk-helm

# Enter the dev shell (automatic if you have direnv + .envrc)
nix develop

# Run
cargo run

# Auto-restart on file save
cargo watch -x run

# Lint / format
cargo clippy
cargo fmt

# Full Nix build (matches what `nix run` produces)
nix build
./result/bin/cmk-helm
```

### Run Nix checks (clippy + fmt + build)

```bash
nix flake check
```

---

## Architecture

The app is split into four modules:

| Module | Role |
|--------|------|
| `src/main.rs` | Entry point: terminal init, tokio runtime, startup load |
| `src/api/mod.rs` | HTTP fetch + HTML parsing of the Checkmk download server |
| `src/ui/` | Ratatui TUI — state, input handling, rendering |
| `src/installer/mod.rs` | Subprocess wrappers for `cmk-dev-install`, `cmk-dev-site`, `omd` |

See `docs/notes/` for architecture notes and Rust concept explanations.
