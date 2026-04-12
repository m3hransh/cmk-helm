---
title: Ratatui TUI Framework
tags: [tool]
status: completed
priority: 3
publish: true
aliases: [ratatui, TUI, terminal UI framework]
---

# Ratatui

Ratatui is the Rust library we use for all terminal UI rendering. It is the most actively maintained TUI library in the Rust ecosystem as of 2025.

---

## Why Ratatui?

- **Active community** — the most maintained Rust TUI library
- **Immediate mode** — no widget tree to synchronise, just describe what to draw each frame
- **Backend agnostic** — we use the `crossterm` backend (cross-platform: Linux, macOS, Windows)
- **Composable layouts** — `Constraint`-based layout system is intuitive

---

## Core Concepts

### Immediate Mode Rendering

You do not create widget objects and update them over time. Every frame, you describe the entire UI from scratch based on current application state. Ratatui diffs the output and updates only changed cells.

```rust
terminal.draw(|frame| {
    // frame.area() gives you the full terminal size as a Rect
    // You render widgets into areas
    frame.render_widget(
        Paragraph::new("Hello, world!"),
        frame.area()
    );
})?;
```

See [[TUI Event Loop]] for how this fits in the loop.

### Layouts

Ratatui uses constraint-based layouts to divide the terminal area:

```rust
use ratatui::layout::{Constraint, Layout};

// Split vertically: 3-row header, rest for content, 3-row footer
let [header, body, footer] = Layout::vertical([
    Constraint::Length(3),
    Constraint::Min(0),
    Constraint::Length(3),
]).areas(frame.area());

// Split body horizontally: 60% left, 40% right
let [left, right] = Layout::horizontal([
    Constraint::Percentage(60),
    Constraint::Percentage(40),
]).areas(body);
```

Constraints:
- `Length(n)` — exactly n rows/columns
- `Percentage(n)` — n% of available space
- `Min(n)` — at least n, takes remaining space
- `Max(n)` — at most n
- `Ratio(a, b)` — a/b of available space

### Widgets

Widgets are values that implement `Widget` or `StatefulWidget`. Common ones used in this project:

| Widget | Use |
|--------|-----|
| `Block` | Border + title around any area |
| `Paragraph` | Static or styled text |
| `Table` + `TableState` | Scrollable table with selectable rows |
| `List` + `ListState` | Scrollable list with selection highlight |
| `Tabs` | Horizontal tab bar |

```rust
// Table with a selected row highlight
frame.render_stateful_widget(
    Table::new(rows, widths)
        .block(Block::bordered().title("Versions"))
        .highlight_style(Style::new().bold().fg(Color::Yellow)),
    area,
    &mut self.table_state,  // TableState tracks which row is selected
);
```

### Styles

```rust
use ratatui::style::{Color, Modifier, Style, Stylize};

let style = Style::new()
    .fg(Color::Yellow)
    .bg(Color::Black)
    .bold();

// Shorthand via Stylize trait
let text = "selected".bold().yellow();
```

---

## Version Pinning

We pin `ratatui` to `<0.29` in `Cargo.toml`:

```toml
ratatui = { version = ">=0.28, <0.29" }
```

Ratatui 0.29+ pulled in transitive deps that bumped their MSRV (Minimum Supported Rust Version) to 1.88, conflicting with our pinned Rust 1.88.0 toolchain. See [[Nix Flakes]] for the full MSRV story.

---

## Metadata

**Tags:** tool
**Reference:** https://ratatui.rs, https://crates.io/crates/ratatui
**Related:** [[TUI Event Loop]], [[TUI Layout Design]], [[Crossterm]], [[Rust Traits]]
