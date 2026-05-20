// ── Rendering for App ─────────────────────────────────────────────────────────
//
// All Ratatui rendering lives here. The event loop in mod.rs calls `render`
// (pub(super)) each frame; everything else is private to this module.
//
// Rust concept: multiple `impl App` blocks across files are valid — the
// compiler merges them. Private fields of App (defined in mod.rs, the `ui`
// module) are accessible here because child modules can see parent-private items.

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Row, Table, Tabs},
    Frame,
};

use crate::api::VersionKind;
use crate::installer::JobStatus;

use super::state::{ActivePane, CMK_BLUE, CMK_GREEN, LeftPaneMode, RightPaneMode};
use super::App;

impl App {
    // ── Main render dispatcher ────────────────────────────────────────────────

    pub(super) fn render(&mut self, frame: &mut Frame) {
        if self.load_rx.is_some() {
            self.render_splash(frame);
            return;
        }
        let area = frame.area();

        // Outer layout: tab bar / body / log panel / footer
        //
        // Rust concept: `Constraint::Min(0)` tells Ratatui "use all remaining
        // space" — it flexes to fill whatever the fixed-size sections don't need.
        let outer = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // tab bar
                Constraint::Min(0),    // body (flexes)
                Constraint::Length(8), // log panel
                Constraint::Length(3), // footer
            ])
            .split(area);

        // Body: left pane (60%) and right pane (40%)
        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(outer[1]);

        // Right pane: installed versions (45%) + installed sites (55%)
        let right = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
            .split(body[1]);

        // Store pane geometry so mouse clicks can be mapped to panes.
        self.pane_rects = super::state::PaneRects {
            version_browser: body[0],
            installed_versions: right[0],
            installed_sites: right[1],
            log_panel: outer[2],
        };

        self.render_tabs(frame, outer[0]);
        self.render_left(frame, body[0]);
        self.render_installed_versions(frame, right[0]);
        self.render_installed_sites(frame, right[1]);
        self.render_log_panel(frame, outer[2]);
        self.render_footer(frame, outer[3]);
    }

    // ── Splash screen ─────────────────────────────────────────────────────────
    //
    // A simple face that blinks while the version list loads.
    // Face cycle (using splash_tick % 64):
    //   0–59   →  (•‿•)  normal
    //  60–63   →  (-‿-)  blink

    fn render_splash(&self, frame: &mut Frame) {
        let area = frame.area();

        let face = match self.splash_tick % 64 {
            60..=63 => "(-‿-)",
            _ => "(•‿•)",
        };

        const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let spinner = SPINNER[(self.splash_tick as usize / 2) % 10];

        // 9 rows: brand + 2 gaps + face + 2 gaps + name + gap + spinner
        let content_h: u16 = 9;
        let start_y = area.y + area.height.saturating_sub(content_h) / 2;
        let row = |offset: u16| Rect::new(area.x, start_y + offset, area.width, 1);

        // Row 0 — ✓ Checkmk
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "✓ Checkmk",
                Style::default().fg(CMK_GREEN).add_modifier(Modifier::BOLD),
            )))
            .alignment(Alignment::Center),
            row(0),
        );

        // Row 2 — animated face
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                face,
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            )))
            .alignment(Alignment::Center),
            row(2),
        );

        // Row 5 — CMK Helm
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "CMK Helm",
                Style::default().fg(CMK_BLUE).add_modifier(Modifier::BOLD),
            )))
            .alignment(Alignment::Center),
            row(5),
        );

        // Row 7 — subtitle
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "Version Browser & Installer",
                Style::default().fg(Color::DarkGray),
            )))
            .alignment(Alignment::Center),
            row(7),
        );

        // Row 8 — spinner + loading text
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(spinner, Style::default().fg(CMK_GREEN)),
                Span::styled(" Fetching versions…", Style::default().fg(Color::DarkGray)),
            ]))
            .alignment(Alignment::Center),
            row(8),
        );
    }

    // ── Tab bar ───────────────────────────────────────────────────────────────

    fn render_tabs(&self, frame: &mut Frame, area: Rect) {
        let titles: Vec<Line> = self
            .version_groups
            .iter()
            .map(|g| Line::from(format!(" {} ", g.base)))
            .collect();

        // Show a braille spinner in the title while a background refresh runs.
        const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let title = if self.is_refreshing {
            let s = SPINNER[(self.splash_tick as usize / 2) % 10];
            format!(" CMK Helm {s} ")
        } else {
            " CMK Helm ".to_string()
        };

        let tabs = Tabs::new(titles)
            .block(
                Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::Cyan)),
            )
            .select(self.active_tab)
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .divider("│");

        frame.render_widget(tabs, area);
    }

    // ── Left pane ─────────────────────────────────────────────────────────────

    fn render_left(&mut self, frame: &mut Frame, area: Rect) {
        match &self.left_mode {
            LeftPaneMode::Browse => self.render_version_list(frame, area),
            LeftPaneMode::EditionPicker { .. } => self.render_edition_picker(frame, area),
            LeftPaneMode::Configure { .. } => self.render_configure(frame, area),
        }
    }

    fn render_version_list(&mut self, frame: &mut Frame, area: Rect) {
        let Some(group) = self.version_groups.get(self.active_tab) else {
            return;
        };
        let is_focused = self.active_pane == ActivePane::VersionBrowser;

        let header = Row::new(["Type", "Detail", "Released"]).style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

        let rows: Vec<Row> = group
            .versions
            .iter()
            .map(|v| {
                let type_style = match &v.kind {
                    VersionKind::Daily { .. } => Style::default().fg(Color::Green),
                    VersionKind::StablePatch { .. } => Style::default().fg(Color::Blue),
                    VersionKind::Beta { .. } => Style::default().fg(Color::Magenta),
                };
                Row::new([v.kind_label().to_string(), v.detail(), v.timestamp.clone()])
                    .style(type_style)
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Length(7),  // "stable"
                Constraint::Length(14), // "2026.04.03" or "p24"
                Constraint::Min(16),    // "2026-04-03 13:50"
            ],
        )
        .header(header)
        .block(
            Block::default()
                .title(format!(" {} Versions ", group.base))
                .borders(Borders::ALL)
                .border_style(border_style(is_focused)),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

        frame.render_stateful_widget(table, area, &mut self.table_state);
    }

    fn render_edition_picker(&mut self, frame: &mut Frame, area: Rect) {
        let LeftPaneMode::EditionPicker {
            group_idx,
            version_idx,
            list_state,
        } = &mut self.left_mode
        else {
            return;
        };
        let version = &self.version_groups[*group_idx].versions[*version_idx];
        let is_focused = self.active_pane == ActivePane::VersionBrowser;

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(4), Constraint::Min(0)])
            .split(area);

        frame.render_widget(
            Paragraph::new(vec![
                Line::from(vec![
                    Span::raw("  Base    : "),
                    Span::styled(&version.base, Style::default().fg(Color::Cyan)),
                ]),
                Line::from(vec![
                    Span::raw("  Build   : "),
                    Span::styled(version.detail(), Style::default().fg(Color::Cyan)),
                ]),
                Line::from(vec![
                    Span::raw("  Released: "),
                    Span::styled(&version.timestamp, Style::default().fg(Color::DarkGray)),
                ]),
            ])
            .block(
                Block::default()
                    .title(" Selected Version ")
                    .borders(Borders::ALL)
                    .border_style(border_style(is_focused)),
            ),
            chunks[0],
        );

        let editions = version.available_editions();
        let items: Vec<ListItem> = editions
            .iter()
            .map(|e| ListItem::new(format!("  {}  [-e {}]", e.display_name(), e.as_str())))
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .title(" Select Edition ")
                    .borders(Borders::ALL)
                    .border_style(border_style(is_focused)),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::Blue)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▶ ");

        frame.render_stateful_widget(list, chunks[1], list_state);
    }

    fn render_configure(&self, frame: &mut Frame, area: Rect) {
        let LeftPaneMode::Configure {
            group_idx,
            version_idx,
            edition,
            site_input,
        } = &self.left_mode
        else {
            return;
        };
        let version = &self.version_groups[*group_idx].versions[*version_idx];
        let is_focused = self.active_pane == ActivePane::VersionBrowser;

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(6),
                Constraint::Length(3),
                Constraint::Min(0),
            ])
            .split(area);

        frame.render_widget(
            Paragraph::new(vec![
                Line::from(vec![
                    Span::raw("  Base    : "),
                    Span::styled(&version.base, Style::default().fg(Color::Cyan)),
                ]),
                Line::from(vec![
                    Span::raw("  Build   : "),
                    Span::styled(version.detail(), Style::default().fg(Color::Cyan)),
                ]),
                Line::from(vec![
                    Span::raw("  Released: "),
                    Span::styled(&version.timestamp, Style::default().fg(Color::DarkGray)),
                ]),
                Line::from(vec![
                    Span::raw("  Edition : "),
                    Span::styled(edition.display_name(), Style::default().fg(Color::Green)),
                ]),
                Line::from(vec![
                    Span::raw("  Command : "),
                    Span::styled(
                        format!(
                            "cmk-dev-install {} -e {}",
                            version.install_arg(),
                            edition.as_str()
                        ),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]),
            ])
            .block(
                Block::default()
                    .title(" Installation Plan ")
                    .borders(Borders::ALL)
                    .border_style(border_style(is_focused)),
            ),
            chunks[0],
        );

        frame.render_widget(
            Paragraph::new(format!(" {site_input}█"))
                .block(
                    Block::default()
                        .title(" OMD Site Name ")
                        .borders(Borders::ALL)
                        .border_style(border_style(is_focused)),
                )
                .style(Style::default().fg(Color::White)),
            chunks[1],
        );
    }

    // ── Right panel: Installed Versions ───────────────────────────────────────

    fn render_installed_versions(&mut self, frame: &mut Frame, area: Rect) {
        let is_focused = self.active_pane == ActivePane::InstalledVersions;

        // If there's an active prompt and this pane is focused, split area.
        let (list_area, prompt_area) =
            if is_focused && !matches!(self.right_mode, RightPaneMode::Browse) {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(0), Constraint::Length(3)])
                    .split(area);
                (chunks[0], Some(chunks[1]))
            } else {
                (area, None)
            };

        let items: Vec<ListItem> = if self.installed_versions.is_empty() {
            vec![ListItem::new("  (none found)").style(Style::default().fg(Color::DarkGray))]
        } else {
            self.installed_versions
                .iter()
                .map(|v| ListItem::new(format!(" {v}")).style(Style::default().fg(Color::Gray)))
                .collect()
        };

        let list = List::new(items)
            .block(
                Block::default()
                    .title(" Installed Versions ")
                    .borders(Borders::ALL)
                    .border_style(border_style(is_focused)),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▶ ");

        frame.render_stateful_widget(list, list_area, &mut self.versions_list_state);

        if let Some(prompt_area) = prompt_area {
            self.render_right_prompt(frame, prompt_area);
        }
    }

    // ── Right panel: Installed Sites ──────────────────────────────────────────

    fn render_installed_sites(&mut self, frame: &mut Frame, area: Rect) {
        let is_focused = self.active_pane == ActivePane::InstalledSites;

        let (list_area, prompt_area) =
            if is_focused && !matches!(self.right_mode, RightPaneMode::Browse) {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(0), Constraint::Length(3)])
                    .split(area);
                (chunks[0], Some(chunks[1]))
            } else {
                (area, None)
            };

        let items: Vec<ListItem> = if self.installed_sites.is_empty() {
            vec![ListItem::new("  (none found)").style(Style::default().fg(Color::DarkGray))]
        } else {
            self.installed_sites
                .iter()
                .map(|s| {
                    let (marker, fg) = if s.is_default {
                        ("★ ", Color::Yellow)
                    } else {
                        ("  ", Color::Gray)
                    };
                    let ver_short: String = s.version.chars().take(22).collect();
                    ListItem::new(Line::from(vec![
                        Span::styled(format!("{marker}{:<14}", s.name), Style::default().fg(fg)),
                        Span::styled(ver_short, Style::default().fg(Color::DarkGray)),
                    ]))
                })
                .collect()
        };

        let list = List::new(items)
            .block(
                Block::default()
                    .title(" Installed Sites ")
                    .borders(Borders::ALL)
                    .border_style(border_style(is_focused)),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▶ ");

        frame.render_stateful_widget(list, list_area, &mut self.sites_list_state);

        if let Some(prompt_area) = prompt_area {
            self.render_right_prompt(frame, prompt_area);
        }
    }

    /// Renders the inline prompt for confirm/site-name-input in the right panel.
    fn render_right_prompt(&self, frame: &mut Frame, area: Rect) {
        match &self.right_mode {
            RightPaneMode::Browse => {}
            RightPaneMode::ConfirmDelete { label, .. } => {
                frame.render_widget(
                    Paragraph::new(format!(" {label}  (y/n)"))
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_style(Style::default().fg(Color::Red)),
                        )
                        .style(Style::default().fg(Color::Red)),
                    area,
                );
            }
            RightPaneMode::SiteNameInput { site_input, .. } => {
                frame.render_widget(
                    Paragraph::new(format!(" {site_input}█"))
                        .block(
                            Block::default()
                                .title(" Site Name ")
                                .borders(Borders::ALL)
                                .border_style(Style::default().fg(Color::Cyan)),
                        )
                        .style(Style::default().fg(Color::White)),
                    area,
                );
            }
        }
    }

    // ── Log Panel ─────────────────────────────────────────────────────────────

    fn render_log_panel(&self, frame: &mut Frame, area: Rect) {
        let is_focused = self.active_pane == ActivePane::LogPanel;

        let title = if self.copy_mode {
            " Log [COPY MODE — Esc to exit] "
        } else {
            " Log "
        };

        // Draw the outer border first, then work inside it.
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(border_style(is_focused));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if self.jobs.is_empty() {
            frame.render_widget(
                Paragraph::new(
                    "  No jobs yet. Select a version and press Enter to install.",
                )
                .style(Style::default().fg(Color::DarkGray)),
                inner,
            );
            return;
        }

        // Split inner area: one-line tab bar on top, log content below.
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(0)])
            .split(inner);
        let (tab_area, content_area) = (chunks[0], chunks[1]);

        // ── Tab bar ──────────────────────────────────────────────────────────
        let tab_spans: Vec<Span> = self
            .jobs
            .iter()
            .enumerate()
            .map(|(i, job)| {
                let (icon, icon_color) = match job.status {
                    JobStatus::Running => ("⟳", Color::Yellow),
                    JobStatus::Done => ("✓", Color::Green),
                    JobStatus::Failed => ("✗", Color::Red),
                };
                let text = format!(" {icon} {} ", job.short_label);
                if self.selected_job == Some(i) {
                    Span::styled(
                        text,
                        Style::default()
                            .bg(CMK_BLUE)
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    Span::styled(text, Style::default().fg(icon_color))
                }
            })
            .collect();
        frame.render_widget(Paragraph::new(Line::from(tab_spans)), tab_area);

        // ── Log content for the selected job ─────────────────────────────────
        let lines: Vec<Line> = self
            .selected_job
            .and_then(|idx| self.jobs.get(idx))
            .map(|job| {
                // First line: full job description as a dim header.
                let mut out = vec![Line::styled(
                    format!("  {}", job.label),
                    Style::default().fg(Color::DarkGray),
                )];
                out.extend(job.output.iter().map(|line| {
                    let style = if line.contains("✓") || line.contains("done") {
                        Style::default().fg(Color::Green)
                    } else if line.contains("✗")
                        || line.contains("error")
                        || line.contains("Error")
                        || line.contains("failed")
                    {
                        Style::default().fg(Color::Red)
                    } else if line.starts_with('→') {
                        Style::default().fg(Color::Cyan)
                    } else {
                        Style::default().fg(Color::Gray)
                    };
                    Line::styled(format!("  {line}"), style)
                }));
                out
            })
            .unwrap_or_default();

        // Scroll: show the latest lines by default; log_scroll shifts the
        // viewport up so the user can read earlier output.
        let visible_height = content_area.height as usize;
        let total = lines.len();
        let start = total.saturating_sub(visible_height + self.log_scroll);
        let end = total.saturating_sub(self.log_scroll).min(total);
        let visible: Vec<Line> = lines[start..end].to_vec();

        frame.render_widget(Paragraph::new(visible), content_area);
    }

    // ── Footer ────────────────────────────────────────────────────────────────

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let hint = match self.active_pane {
            ActivePane::VersionBrowser => match &self.left_mode {
                LeftPaneMode::Browse => {
                    "  j/k row  h/l or Tab/ShiftTab tab  Enter/i select  r refresh  Alt+hjkl pane  q/Esc quit"
                }
                LeftPaneMode::EditionPicker { .. } => {
                    "  j/k edition  Enter confirm  Esc back  Alt+hjkl pane  q quit"
                }
                LeftPaneMode::Configure { .. } => {
                    "  Type site name  Backspace del  Enter install  Esc back  Alt+hjkl pane"
                }
            },
            ActivePane::InstalledVersions => match &self.right_mode {
                RightPaneMode::ConfirmDelete { .. } => "  y/Enter confirm  n/Esc cancel  q quit",
                RightPaneMode::SiteNameInput { .. } => {
                    "  Type site name  Backspace del  Enter create  Esc cancel"
                }
                RightPaneMode::Browse => {
                    "  j/k navigate  d delete  s create site  Alt+hjkl pane  q/Esc quit"
                }
            },
            ActivePane::InstalledSites => match &self.right_mode {
                RightPaneMode::ConfirmDelete { .. } => "  y/Enter confirm  n/Esc cancel  q quit",
                _ => "  j/k navigate  d delete site  Alt+hjkl pane  q/Esc quit",
            },
            ActivePane::LogPanel => {
                if self.copy_mode {
                    "  j/k scroll  h/l tabs  Esc exit copy mode"
                } else {
                    "  j/k scroll  h/l tabs  c copy mode  Alt+hjkl pane  q quit"
                }
            }
        };

        frame.render_widget(
            Block::default()
                .title(hint)
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::DarkGray)),
            area,
        );
    }
}

// ── Free helpers ──────────────────────────────────────────────────────────────

/// Returns the border style for a pane based on whether it's focused.
///
/// Rust concept: this is a free function rather than a method because it
/// doesn't need access to `self`. Keeping it free avoids borrow-checker
/// conflicts when we need to call it while holding a mutable borrow on
/// a field of `App` (e.g. `left_mode` in `render_edition_picker`).
fn border_style(is_focused: bool) -> Style {
    if is_focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}
