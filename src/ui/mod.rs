// UI module — App state machine, event loop, and all Ratatui rendering.
//
// Layout (always visible):
//
//   ┌─ CMK Cockpit ── [2.6.0] [2.5.0] [2.4.0] … ──────────────────────────┐
//   ├────────────────────────────────────┬─────────────────────────────────┤
//   │  Left pane (screen-dependent)      │  Installed Versions             │
//   │  VersionList / EditionPicker /     │  2.6.0-…ultimate                │
//   │  Configure / Installing            │  …                              │
//   │                                    ├─────────────────────────────────┤
//   │                                    │  Installed Sites                │
//   │                                    │ ★ test  2.6.0-…ultimate         │
//   │                                    │   v240  2.4.0-….cce             │
//   ├────────────────────────────────────┴─────────────────────────────────┤
//   │  key hints                                                            │
//   └──────────────────────────────────────────────────────────────────────┘
//
// The right panel is *always* shown regardless of which left-pane screen is
// active — it gives a constant overview of what's installed locally.
//
// Screen flow (left pane):
//   VersionList ──Enter──▶ EditionPicker ──Enter──▶ Configure ──Enter──▶ Installing
//        ▲                      │                       │
//        └──────Esc─────────────┴──────Esc──────────────┘

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Row, Table, TableState, Tabs},
};

use crate::api::{Edition, VersionGroup, VersionKind};
use crate::installer::{InstalledSite, InstallConfig};

// ── Screen State Machine ─────────────────────────────────────────────────────

#[derive(Debug)]
enum Screen {
    VersionList,
    EditionPicker { group_idx: usize, version_idx: usize, list_state: ListState },
    Configure { group_idx: usize, version_idx: usize, edition: Edition, site_input: String },
    Installing { config: InstallConfig },
}

// ── App ───────────────────────────────────────────────────────────────────────

pub struct App {
    /// Versions grouped by base version — one group per tab.
    version_groups: Vec<VersionGroup>,
    /// Currently selected tab index.
    active_tab: usize,
    /// Highlighted row in the version list of the active tab.
    table_state: TableState,
    screen: Screen,
    installed_versions: Vec<String>,
    installed_sites: Vec<InstalledSite>,
    should_quit: bool,
}

impl App {
    pub fn new(
        version_groups: Vec<VersionGroup>,
        installed_versions: Vec<String>,
        installed_sites: Vec<InstalledSite>,
    ) -> Self {
        Self {
            version_groups,
            active_tab: 0,
            table_state: TableState::default().with_selected(0),
            screen: Screen::VersionList,
            installed_versions,
            installed_sites,
            should_quit: false,
        }
    }

    // ── Event Loop ───────────────────────────────────────────────────────────

    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        while !self.should_quit {
            terminal.draw(|frame| self.render(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    // ── Input ────────────────────────────────────────────────────────────────

    fn handle_events(&mut self) -> Result<()> {
        if event::poll(std::time::Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    self.dispatch(key.code);
                }
            }
        }
        Ok(())
    }

    fn dispatch(&mut self, code: KeyCode) {
        // Rust concept: using a temporary variable to avoid borrow conflicts.
        // We can't call `&mut self` methods while also holding a borrow of
        // `self.screen`, so we match first, then act.
        match &self.screen {
            Screen::VersionList => self.on_version_list(code),
            Screen::EditionPicker { .. } => self.on_edition_picker(code),
            Screen::Configure { .. } => self.on_configure(code),
            Screen::Installing { .. } => self.on_installing(code),
        }
    }

    // ── VersionList input ─────────────────────────────────────────────────────

    fn on_version_list(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,

            // Tab navigation: ← / → (or h/l)
            KeyCode::Left | KeyCode::Char('h') | KeyCode::BackTab => self.prev_tab(),
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => self.next_tab(),

            // Row navigation: ↑ / ↓ (or k/j)
            KeyCode::Up | KeyCode::Char('k') => self.select_prev_row(),
            KeyCode::Down | KeyCode::Char('j') => self.select_next_row(),

            KeyCode::Enter => {
                if let Some(vi) = self.table_state.selected() {
                    let gi = self.active_tab;
                    let mut list_state = ListState::default();
                    list_state.select(Some(0));
                    self.screen = Screen::EditionPicker {
                        group_idx: gi,
                        version_idx: vi,
                        list_state,
                    };
                }
            }
            _ => {}
        }
    }

    fn prev_tab(&mut self) {
        if self.active_tab > 0 {
            self.active_tab -= 1;
            self.table_state.select(Some(0));
        }
    }

    fn next_tab(&mut self) {
        if self.active_tab + 1 < self.version_groups.len() {
            self.active_tab += 1;
            self.table_state.select(Some(0));
        }
    }

    fn select_next_row(&mut self) {
        let len = self.current_group_len();
        let next = self.table_state.selected().map(|i| (i + 1).min(len.saturating_sub(1))).unwrap_or(0);
        self.table_state.select(Some(next));
    }

    fn select_prev_row(&mut self) {
        let prev = self.table_state.selected().map(|i| i.saturating_sub(1)).unwrap_or(0);
        self.table_state.select(Some(prev));
    }

    fn current_group_len(&self) -> usize {
        self.version_groups
            .get(self.active_tab)
            .map(|g| g.versions.len())
            .unwrap_or(0)
    }

    // ── EditionPicker input ───────────────────────────────────────────────────

    fn on_edition_picker(&mut self, code: KeyCode) {
        let Screen::EditionPicker { group_idx, version_idx, list_state } = &mut self.screen else { return };
        let editions = self.version_groups[*group_idx].versions[*version_idx].available_editions();

        match code {
            KeyCode::Esc => {
                self.screen = Screen::VersionList;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let prev = list_state.selected().map(|i| i.saturating_sub(1)).unwrap_or(0);
                list_state.select(Some(prev));
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let last = editions.len().saturating_sub(1);
                let next = list_state.selected().map(|i| (i + 1).min(last)).unwrap_or(0);
                list_state.select(Some(next));
            }
            KeyCode::Enter => {
                let gi = *group_idx;
                let vi = *version_idx;
                let edition = list_state
                    .selected()
                    .map(|i| editions[i].clone())
                    .unwrap_or_else(|| editions[0].clone());
                self.screen = Screen::Configure {
                    group_idx: gi,
                    version_idx: vi,
                    edition,
                    site_input: String::new(),
                };
            }
            _ => {}
        }
    }

    // ── Configure input ───────────────────────────────────────────────────────

    fn on_configure(&mut self, code: KeyCode) {
        let Screen::Configure { group_idx, version_idx, edition, site_input } = &mut self.screen else { return };

        match code {
            KeyCode::Esc => {
                let gi = *group_idx;
                let vi = *version_idx;
                let mut list_state = ListState::default();
                list_state.select(Some(0));
                self.screen = Screen::EditionPicker { group_idx: gi, version_idx: vi, list_state };
            }
            KeyCode::Char(c) => site_input.push(c),
            KeyCode::Backspace => { site_input.pop(); }
            KeyCode::Enter if !site_input.is_empty() => {
                let version = &self.version_groups[*group_idx].versions[*version_idx];
                let config = InstallConfig {
                    version: version.install_arg(),
                    edition: edition.as_str().to_string(),
                    site_name: site_input.clone(),
                };
                self.screen = Screen::Installing { config };
            }
            _ => {}
        }
    }

    // ── Installing input ──────────────────────────────────────────────────────

    fn on_installing(&mut self, code: KeyCode) {
        if matches!(code, KeyCode::Char('q') | KeyCode::Esc) {
            self.should_quit = true;
        }
    }

    // ── Rendering ─────────────────────────────────────────────────────────────

    fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();

        // Outer layout: tabs bar / body / footer
        let outer = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // tab bar
                Constraint::Min(0),    // body
                Constraint::Length(3), // footer
            ])
            .split(area);

        // Body: left panel (60%) and right panel (40%)
        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(outer[1]);

        self.render_tabs(frame, outer[0]);
        self.render_left(frame, body[0]);
        self.render_right(frame, body[1]);
        self.render_footer(frame, outer[2]);
    }

    // ── Tab bar ───────────────────────────────────────────────────────────────

    fn render_tabs(&self, frame: &mut Frame, area: Rect) {
        let titles: Vec<Line> = self
            .version_groups
            .iter()
            .map(|g| Line::from(format!(" {} ", g.base)))
            .collect();

        let tabs = Tabs::new(titles)
            .block(
                Block::default()
                    .title(" CMK Cockpit ")
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

    // ── Left panel ────────────────────────────────────────────────────────────

    fn render_left(&mut self, frame: &mut Frame, area: Rect) {
        match &self.screen {
            Screen::VersionList => self.render_version_list(frame, area),
            Screen::EditionPicker { .. } => self.render_edition_picker(frame, area),
            Screen::Configure { .. } => self.render_configure(frame, area),
            Screen::Installing { .. } => self.render_installing(frame, area),
        }
    }

    fn render_version_list(&mut self, frame: &mut Frame, area: Rect) {
        let Some(group) = self.version_groups.get(self.active_tab) else {
            return;
        };

        let header = Row::new(["Type", "Detail", "Released"])
            .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));

        let rows: Vec<Row> = group.versions.iter().map(|v| {
            let type_style = match &v.kind {
                VersionKind::Daily { .. }       => Style::default().fg(Color::Green),
                VersionKind::StablePatch { .. } => Style::default().fg(Color::Blue),
                VersionKind::Beta { .. }        => Style::default().fg(Color::Magenta),
            };
            Row::new([v.kind_label().to_string(), v.detail(), v.timestamp.clone()])
                .style(type_style)
        }).collect();

        let table = Table::new(
            rows,
            [
                Constraint::Length(7),   // "stable"
                Constraint::Length(14),  // "2026.04.03" or "p24"
                Constraint::Min(16),     // "2026-04-03 13:50"
            ],
        )
        .header(header)
        .block(
            Block::default()
                .title(format!(" {} Versions ", group.base))
                .borders(Borders::ALL),
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
        let Screen::EditionPicker { group_idx, version_idx, list_state } = &mut self.screen else { return };
        let version = &self.version_groups[*group_idx].versions[*version_idx];

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(4), Constraint::Min(0)])
            .split(area);

        // Selected version summary
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
            .block(Block::default().title(" Selected Version ").borders(Borders::ALL)),
            chunks[0],
        );

        // Edition list
        let editions = version.available_editions();
        let items: Vec<ListItem> = editions
            .iter()
            .map(|e| ListItem::new(format!("  {}  [-e {}]", e.display_name(), e.as_str())))
            .collect();

        let list = List::new(items)
            .block(Block::default().title(" Select Edition ").borders(Borders::ALL))
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
        let Screen::Configure { group_idx, version_idx, edition, site_input } = &self.screen else { return };
        let version = &self.version_groups[*group_idx].versions[*version_idx];

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(6), Constraint::Length(3), Constraint::Min(0)])
            .split(area);

        frame.render_widget(
            Paragraph::new(vec![
                Line::from(vec![Span::raw("  Base    : "), Span::styled(&version.base, Style::default().fg(Color::Cyan))]),
                Line::from(vec![Span::raw("  Build   : "), Span::styled(version.detail(), Style::default().fg(Color::Cyan))]),
                Line::from(vec![Span::raw("  Released: "), Span::styled(&version.timestamp, Style::default().fg(Color::DarkGray))]),
                Line::from(vec![Span::raw("  Edition : "), Span::styled(edition.display_name(), Style::default().fg(Color::Green))]),
                Line::from(vec![
                    Span::raw("  Command : "),
                    Span::styled(
                        format!("cmk-dev-install {} -e {}", version.install_arg(), edition.as_str()),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]),
            ])
            .block(Block::default().title(" Installation Plan ").borders(Borders::ALL)),
            chunks[0],
        );

        frame.render_widget(
            Paragraph::new(format!(" {site_input}█"))
                .block(Block::default().title(" OMD Site Name ").borders(Borders::ALL))
                .style(Style::default().fg(Color::White)),
            chunks[1],
        );
    }

    fn render_installing(&self, frame: &mut Frame, area: Rect) {
        let Screen::Installing { config } = &self.screen else { return };

        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                "  Ready to install — run in terminal:",
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                format!("  cmk-dev-install {} -e {}", config.version, config.edition),
                Style::default().fg(Color::Cyan),
            )),
            Line::from(Span::styled(
                format!("  cmk-dev-site <omd_ver>.{} -n {}", config.edition, config.site_name),
                Style::default().fg(Color::Cyan),
            )),
        ];

        frame.render_widget(
            Paragraph::new(lines)
                .block(Block::default().title(" Install ").borders(Borders::ALL)),
            area,
        );
    }

    // ── Right panel (always visible) ──────────────────────────────────────────

    fn render_right(&self, frame: &mut Frame, area: Rect) {
        // Split right panel: installed versions (top 45%) + sites (bottom 55%)
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
            .split(area);

        self.render_installed_versions(frame, chunks[0]);
        self.render_installed_sites(frame, chunks[1]);
    }

    fn render_installed_versions(&self, frame: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = if self.installed_versions.is_empty() {
            vec![ListItem::new("  (none found)").style(Style::default().fg(Color::DarkGray))]
        } else {
            self.installed_versions
                .iter()
                .map(|v| ListItem::new(format!(" {v}")).style(Style::default().fg(Color::Gray)))
                .collect()
        };

        frame.render_widget(
            List::new(items).block(
                Block::default()
                    .title(" Installed Versions ")
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::DarkGray)),
            ),
            area,
        );
    }

    fn render_installed_sites(&self, frame: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = if self.installed_sites.is_empty() {
            vec![ListItem::new("  (none found)").style(Style::default().fg(Color::DarkGray))]
        } else {
            self.installed_sites
                .iter()
                .map(|s| {
                    // ★ marks the default site in Yellow; others in Gray
                    let (marker, fg) = if s.is_default {
                        ("★ ", Color::Yellow)
                    } else {
                        ("  ", Color::Gray)
                    };
                    // Truncate version to fit narrow right pane
                    let ver_short: String = s.version.chars().take(22).collect();
                    ListItem::new(
                        Line::from(vec![
                            Span::styled(format!("{marker}{:<14}", s.name), Style::default().fg(fg)),
                            Span::styled(ver_short, Style::default().fg(Color::DarkGray)),
                        ])
                    )
                })
                .collect()
        };

        frame.render_widget(
            List::new(items).block(
                Block::default()
                    .title(" Installed Sites ")
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::DarkGray)),
            ),
            area,
        );
    }

    // ── Footer ────────────────────────────────────────────────────────────────

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let hint = match &self.screen {
            Screen::VersionList =>
                "  ↑/k ↓/j  row    ←/h →/l Tab  tab    Enter  select    q  quit  ",
            Screen::EditionPicker { .. } =>
                "  ↑/k ↓/j  edition    Enter  confirm    Esc  back  ",
            Screen::Configure { .. } =>
                "  Type site name    Enter  confirm    Esc  back  ",
            Screen::Installing { .. } =>
                "  Copy the commands above and run them in a terminal    q  quit  ",
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
