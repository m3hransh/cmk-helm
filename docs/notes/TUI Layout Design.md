---
title: TUI Layout Design
tags: [architecture]
status: completed
priority: 2
publish: true
aliases: [layout, TUI layout, split pane]
---

# TUI Layout Design

The terminal window is divided into four horizontal bands: tab bar, body, log panel, footer. The body is further split into a left pane (60%) and a right pane (40%).

---

## Layout Diagram

```
┌─ CMK Helm ── [2.6.0] [2.5.0] [2.4.0] ── Tab/ShiftTab ───────────────┐
├──────────────────────────┬────────────────────────────────────────────┤
│  Version Browser (left)  │  Installed Versions (right-top)           │
│                          │   ▶ 2.6.0-….ultimate                      │
│  ▶ daily  2026.04.03    │   [d]elete  [s]ite from version           │
│    stable p24            ├────────────────────────────────────────────┤
│                          │  Installed Sites (right-bottom)           │
│  Enter → edition picker  │  ★ test  2.6.0.ultimate                   │
│  → configure → install   │   [d]elete site                           │
├──────────────────────────┴────────────────────────────────────────────┤
│  Log Panel — job tabs + live output from install/management jobs      │
│  ⟳ install 2.6.0  ✓ rm 2.5.0                                         │
├───────────────────────────────────────────────────────────────────────┤
│  Key hints (context-sensitive)                                        │
└───────────────────────────────────────────────────────────────────────┘
```

---

## How Ratatui Layouts Work

Ratatui uses a constraint-based layout system. You describe proportions and minimums; Ratatui calculates pixel positions:

```rust
use ratatui::layout::{Constraint, Direction, Layout};

// Four vertical bands
let outer = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
        Constraint::Length(3),  // tab bar
        Constraint::Min(0),     // body (flexes to fill)
        Constraint::Length(8),  // log panel
        Constraint::Length(3),  // footer
    ])
    .split(area);

// Body: left 60%, right 40%
let body = Layout::default()
    .direction(Direction::Horizontal)
    .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
    .split(outer[1]);
```

See [[Ratatui TUI Framework]] for more on the widget system.

---

## Pane Focus

Four panes can hold focus: `VersionBrowser`, `InstalledVersions`, `InstalledSites`, `LogPanel`. The active pane has a cyan border; inactive panes use dark-grey borders.

Focus switching:
- `Alt+h/l/j/k` (or `Ctrl+←/→/↓/↑`) — spatial navigation between panes
- Mouse click — clicks the clicked pane into focus

---

## The Right Pane Is Persistent

The right pane always shows installed versions (top) and sites (bottom) regardless of left pane state. Engineers want to see what's already installed while choosing what to install next.

If `omd` is unavailable, both lists show "(none found)". See [[Installed Versions and Sites Panel]] and [[Error Handling Strategy]].

---

## Log Panel

The log panel is always visible at the bottom. It shows a tab bar (one tab per background job) and the output of the selected job. See [[Log Panel]] for the full feature description.

---

## Footer Key Hints

The footer shows context-sensitive key hints based on `active_pane` and the current sub-mode. A single `match` on `(active_pane, left_mode, right_mode)` returns a static string. It avoids cluttering the content area with help text.

---

## Version Tabs

The top tab bar displays base versions: `2.6.0 | 2.5.0 | 2.4.0 | …`. `Tab`/`ShiftTab` cycles through them; changing `active_tab` filters which `VersionGroup` is shown in the version browser.

A braille spinner (`⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`) appears next to the app name while a background refresh is in flight. See [[Background Refresh]].

---

## Metadata

**Tags:** architecture
**Related:** [[App State Machine]], [[Ratatui TUI Framework]], [[TUI Event Loop]], [[Installed Versions and Sites Panel]], [[Log Panel]], [[Background Refresh]]
