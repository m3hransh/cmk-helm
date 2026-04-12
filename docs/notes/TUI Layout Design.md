---
title: TUI Layout Design
tags: [architecture]
status: completed
priority: 2
publish: true
aliases: [layout, TUI layout, split pane]
---

# TUI Layout Design

The terminal window is divided into three horizontal bands: tabs, body, footer. The body is further split into a left pane (60%) and a right pane (40%).

---

## Layout Diagram

```
┌─ CMK Cockpit ── [2.6.0] [2.5.0] [2.4.0] … ──────────────────────────┐
├──────────────────────────────────┬───────────────────────────────────┤
│  Left pane (60%)                 │  Right pane (40%) — always shown  │
│                                  │                                   │
│  Changes per screen:             │  Installed Versions               │
│  VersionList → version table     │  ─────────────────────────────    │
│  EditionPicker → edition list    │  2.4.0p24.cee                     │
│  Configure → input form          │  2.5.0-2026.04.03.raw             │
│  Installing → progress output    │                                   │
│                                  │  Installed Sites (★ = default)    │
│                                  │  ─────────────────────────────    │
│                                  │  ★ mysite   2.4.0p24.cee          │
│                                  │    testsite  2.5.0.raw            │
├──────────────────────────────────┴───────────────────────────────────┤
│  Key hints (context-sensitive): ↑↓ navigate  Enter confirm  Esc back │
└──────────────────────────────────────────────────────────────────────┘
```

---

## How Ratatui Layouts Work

Ratatui uses a constraint-based layout system. You describe proportions and minimums; Ratatui calculates pixel positions:

```rust
use ratatui::layout::{Constraint, Direction, Layout};

// Split the frame into three vertical bands
let [tabs_area, body_area, footer_area] = Layout::vertical([
    Constraint::Length(3),     // tabs: fixed 3 rows
    Constraint::Min(0),        // body: takes all remaining space
    Constraint::Length(3),     // footer: fixed 3 rows
]).areas(frame.area());

// Split body horizontally: 60% left, 40% right
let [left_area, right_area] = Layout::horizontal([
    Constraint::Percentage(60),
    Constraint::Percentage(40),
]).areas(body_area);
```

See [[Ratatui TUI Framework]] for more on the widget system.

---

## The Right Pane Is Persistent

The right pane always shows the same content regardless of which screen is active on the left. It renders `App::installed_versions` and `App::installed_sites`, populated once at startup.

This is a deliberate UX decision: engineers want to see what's already installed while choosing what to install next.

If `omd` is unavailable, both lists show "(none found)" — the app does not crash. See [[Installed Versions and Sites Panel]] and [[Error Handling Strategy]].

---

## Footer Key Hints

The footer shows context-sensitive key hints based on `self.screen`. This is a simple `match` on the screen variant returning a static string. It avoids cluttering the main content area with help text.

---

## Tabs

The tabs widget displays base versions as tab labels: `2.6.0 | 2.5.0 | 2.4.0 | …`. Switching tabs (h/l or ←/→) changes `App::active_tab`, which filters which `VersionGroup` is shown in the left pane table.

See [[Version List Screen]] for version grouping logic.

---

## Metadata

**Tags:** architecture
**Related:** [[App State Machine]], [[Ratatui TUI Framework]], [[TUI Event Loop]], [[Installed Versions and Sites Panel]]
