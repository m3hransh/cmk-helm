---
title: TUI Event Loop
tags: [architecture, concept]
status: completed
priority: 2
publish: true
aliases: [event loop, render loop, 60fps]
---

# TUI Event Loop

Ratatui apps follow a tight **draw → poll → handle** loop. Understanding this loop is essential for working on the UI code.

---

## The Loop

```rust
// src/ui/mod.rs — App::run()
pub fn run(mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    while !self.should_quit {
        // 1. Draw the current frame
        terminal.draw(|frame| self.render(frame))?;

        // 2. Wait for an event (or 16ms timeout)
        if event::poll(Duration::from_millis(16))? {
            // 3. Handle the event
            self.handle_events()?;
        }
    }
    Ok(())
}
```

---

## Why 16ms?

`16ms ≈ 1/60 second ≈ 60 frames per second`. 

`event::poll(Duration)` **blocks** until:
- A keyboard/mouse/resize event arrives, OR
- The timeout elapses

Without the timeout, `event::read()` would block forever waiting for input — the screen would never refresh. With a 16ms timeout, the loop wakes up at least 60 times per second to redraw even if no input arrives.

For a TUI that only needs to update on input, you could use a longer timeout (100ms, 250ms). 16ms is comfortable for any interactive UI.

---

## Raw Mode and the Alternate Screen

`ratatui::init()` enables two terminal features:

**Raw mode** (`crossterm::terminal::enable_raw_mode()`):
- Disables line buffering — keypresses arrive immediately, not after Enter
- Disables echo — typed characters aren't printed automatically
- Lets the app handle all input directly

**Alternate screen buffer** (`crossterm::execute!(stdout, EnterAlternateScreen)`):
- The terminal has two screen buffers. Switching to the alternate buffer preserves the user's normal shell output
- When the app exits, switching back restores everything — the user's previous terminal content reappears

Both are restored on exit, even if the app crashes — `main.rs` uses a pattern like:

```rust
let result = app.run(&mut terminal);
// Restore terminal regardless of whether run() errored
ratatui::restore();
result  // now propagate any error
```

---

## Immediate Mode vs Retained Mode

Ratatui is **immediate mode** — you describe what to draw on every frame. There is no persistent widget tree that you update; you rebuild the entire UI from state each frame.

This is the opposite of browser DOM / Android Views / React's virtual DOM (retained mode), where you mutate a tree and the framework diffs it.

**Immediate mode advantages:**
- Simple mental model: state → pixels, every frame
- No synchronisation bugs between UI widget state and app state
- Easy to reason about: the UI is always exactly what `render()` would return for the current `App` state

**Immediate mode trade-off:**
- Slightly more CPU (redraws everything every frame) — negligible for a TUI

---

## Render Function Structure

```rust
fn render(&self, frame: &mut Frame) {
    // 1. Calculate layout areas
    let [tabs_area, body_area, footer_area] = Layout::vertical([...]).areas(frame.area());
    let [left_area, right_area] = Layout::horizontal([...]).areas(body_area);

    // 2. Render persistent elements
    self.render_tabs(frame, tabs_area);
    self.render_right_panel(frame, right_area);
    self.render_footer(frame, footer_area);

    // 3. Render screen-specific left pane
    match &self.screen {
        Screen::VersionList => self.render_version_list(frame, left_area),
        Screen::EditionPicker { .. } => self.render_edition_picker(frame, left_area),
        Screen::Configure { .. } => self.render_configure(frame, left_area),
        Screen::Installing { .. } => self.render_installing(frame, left_area),
    }
}
```

---

## Metadata

**Tags:** architecture, concept
**Related:** [[TUI Layout Design]], [[App State Machine]], [[Ratatui TUI Framework]], [[Async Boundary Design]]
