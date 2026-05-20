---
title: Installing Screen
tags: [feature]
status: archived
priority: 1
publish: false
aliases: [installing, install screen, Installing]
---

> **Archived** — the app no longer has a dedicated "Installing" screen. Live install output now streams into the [[Log Panel]] at the bottom of the multi-pane layout. This note is kept for historical context only.

---


# Installing Screen

The final screen in the [[App State Machine]]. Triggered when the user confirms on the [[Configure Screen]]. Runs the actual `cmk-dev-install` / `cmk-dev-site` subprocesses.

---

## Current Implementation

The screen currently shows a static "Installing…" message while the subprocess runs synchronously in the background. There is no live output streaming yet.

```rust
// Simplified — Installing screen renders a static status message
fn render_installing(&self, frame: &mut Frame, area: Rect) {
    let Screen::Installing { config } = &self.screen else { return };

    let text = format!(
        "Installing {} ({})...\nSite: {}",
        config.version.base, config.edition, config.site_name
    );
    frame.render_widget(
        Paragraph::new(text).block(Block::bordered().title("Installing")),
        area,
    );
}
```

The subprocess call happens in the event handler, not the render function:

```rust
fn on_installing(&mut self) -> Result<()> {
    let Screen::Installing { config } = &self.screen else { return Ok(()) };
    installer::install_and_create_site(config)?;
    self.should_quit = true;  // exit after install
    Ok(())
}
```

---

## Subprocess Execution

`installer::install_and_create_site()` spawns the install subprocess. It:
1. Checks if `cmk-dev-install-site` is on PATH (`which_exists()`)
2. If yes: runs it with version + edition + site name in one call
3. If no: runs `cmk-dev-install` then `cmk-dev-site` separately

Both paths use `std::process::Command` — synchronous, blocking until complete. See [[cmk-dev-install]].

---

## Future Work: Live Output Streaming

The current approach blocks the UI thread during the install (which can take minutes). The planned improvement:

1. Spawn `cmk-dev-install` with `tokio::process::Command` and piped stdout
2. Spawn a Tokio task that reads output lines and sends them to an `mpsc::Sender<String>`
3. The UI loop drains the receiver each frame and appends lines to a scrollable `Paragraph`

This requires making `on_installing()` async, which changes [[Async Boundary Design]] — the install would need to be driven from within the event loop rather than blocking it.

---

## Screen State

```rust
Screen::Installing {
    config: InstallConfig,  // version + edition + site_name — all info needed
}
```

`InstallConfig` is defined in `installer/mod.rs`:
```rust
pub struct InstallConfig {
    pub version: String,    // the install argument (e.g. "2.5.0-2026-04-03")
    pub edition: Edition,
    pub site_name: String,
}
```

---

## Metadata

**Tags:** feature
**Related:** [[Configure Screen]], [[App State Machine]], [[cmk-dev-install]], [[Async Boundary Design]]
