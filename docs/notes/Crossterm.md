---
title: Crossterm
tags: [tool]
status: completed
priority: 1
publish: true
aliases: [crossterm, terminal backend]
---

# Crossterm

Crossterm is the terminal backend library that handles raw terminal I/O. Ratatui uses it to read keyboard events and write rendered output to the terminal.

---

## Role in the Stack

```
Your code (App state + Ratatui widgets)
         ↓
  Ratatui (layout + rendering logic)
         ↓
  Crossterm (raw terminal I/O)
         ↓
  OS terminal (stdin/stdout, escape codes)
```

Ratatui supports multiple backends (crossterm, termion, termwiz). We use crossterm because it is cross-platform (Linux, macOS, Windows) and the most widely used.

---

## What Crossterm Provides

**Raw mode** — disables line buffering and echo so keypresses arrive immediately:
```rust
crossterm::terminal::enable_raw_mode()?;
// all input now arrives character by character, not line by line
crossterm::terminal::disable_raw_mode()?;  // restore on exit
```

**Alternate screen** — switches to a separate terminal buffer (preserves the user's shell history):
```rust
crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
// ... run the TUI ...
crossterm::execute!(stdout, crossterm::terminal::LeaveAlternateScreen)?;
```

**Event polling** — non-blocking check for keyboard/mouse/resize events:
```rust
use crossterm::event::{self, Event, KeyCode};

if event::poll(Duration::from_millis(16))? {
    match event::read()? {
        Event::Key(key) => { /* handle key */ }
        Event::Resize(w, h) => { /* terminal was resized */ }
        _ => {}
    }
}
```

---

## Ratatui Wraps This

In practice, you rarely call crossterm directly. `ratatui::init()` handles raw mode and alternate screen setup. `ratatui::restore()` restores them on exit. The main thing you call directly is `event::poll()` and `event::read()` in the event loop.

---

## Version

```toml
crossterm = { version = "0.27" }
```

Pinned to 0.27 to match what `ratatui 0.28` expects internally.

---

## Metadata

**Tags:** tool
**Reference:** https://crates.io/crates/crossterm
**Related:** [[Ratatui TUI Framework]], [[TUI Event Loop]]
