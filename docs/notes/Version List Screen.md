---
title: Version List Screen
tags: [feature]
status: completed
priority: 2
publish: true
aliases: [version list, package browser, VersionList]
---

# Version List Screen

The first screen the user sees. Displays available Checkmk versions fetched from the download server, grouped by base version into tabs.

---

## What It Shows

```
[2.6.0] [2.5.0] [2.4.0] [2.3.0]     ← Tabs (one per base version)
┌─────────────────────────────────┐
│ Date         Kind               │
│ ──────────── ─────────────────  │
│ 2026-04-03   Daily              │  ← active row highlighted
│ 2026-04-02   Daily              │
│ 2026-03-28   Daily              │
│ 2.5.0p2      Stable Patch       │  ← stable patches mixed in
└─────────────────────────────────┘
```

---

## Data Model

Versions from the server are grouped into `VersionGroup`:

```rust
// src/api/mod.rs
pub struct VersionGroup {
    pub base: String,           // "2.5.0"
    pub versions: Vec<Version>, // all builds for this base
}

pub struct Version {
    pub base: String,           // "2.5.0"
    pub kind: VersionKind,      // Daily/StablePatch/Beta
    pub timestamp: String,      // display string
}

pub enum VersionKind {
    Daily { date: String },         // "2026.04.03"
    StablePatch { patch: String },  // "p2"
    Beta { suffix: String },        // "b1"
}
```

Grouping happens in `api::group_versions()` after parsing. Each base version becomes one tab.

---

## Navigation

| Key | Action |
|-----|--------|
| `j` / `↓` | Move selection down |
| `k` / `↑` | Move selection up |
| `h` / `←` | Previous tab (previous base version) |
| `l` / `→` | Next tab (next base version) |
| `Enter` | Advance to [[Edition Picker Screen]] |
| `q` | Quit |

---

## State

The selected row is tracked by `App::table_state: TableState`. `TableState` is a Ratatui type that `Table` uses to know which row to highlight. It is updated in `on_key_down()` and `on_key_up()`.

The active tab is tracked by `App::active_tab: usize` — an index into `App::version_groups`.

---

## Rendering

```rust
// Simplified render logic
fn render_version_list(&self, frame: &mut Frame, area: Rect) {
    let group = &self.version_groups[self.active_tab];

    let rows: Vec<Row> = group.versions.iter()
        .map(|v| Row::new([v.timestamp.as_str(), v.kind_label()]))
        .collect();

    let table = Table::new(rows, [Constraint::Fill(1), Constraint::Length(15)])
        .block(Block::bordered().title("Select Version"))
        .highlight_style(Style::new().bold().yellow());

    frame.render_stateful_widget(table, area, &mut self.table_state);
}
```

---

## Metadata

**Tags:** feature
**Related:** [[Edition Picker Screen]], [[App State Machine]], [[Data Flow]], [[Ratatui TUI Framework]]
