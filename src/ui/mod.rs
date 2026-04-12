// UI module — Multi-pane TUI with concurrent job support.
//
// Layout:
//
//   ┌─ CMK Cockpit ── [2.6.0] [2.5.0] [2.4.0] ── Tab/ShiftTab ──────┐
//   ├──────────────────────────┬───────────────────────────────────────┤
//   │  Version Browser (left)  │  Installed Versions (right-top)      │
//   │                          │   ▶ 2.6.0-….ultimate                 │
//   │  ▶ daily  2026.04.03    │   [d]elete  [s]ite from version      │
//   │    stable p24            ├───────────────────────────────────────┤
//   │                          │  Installed Sites (right-bottom)      │
//   │  Enter → edition picker  │  ★ test  2.6.0.ultimate              │
//   │  → configure → install   │   [d]elete site                      │
//   ├──────────────────────────┴───────────────────────────────────────┤
//   │  Log Panel (bottom) — live output from install/management jobs  │
//   ├──────────────────────────────────────────────────────────────────┤
//   │  key hints (footer)                                              │
//   └──────────────────────────────────────────────────────────────────┘
//
// Navigation:
//   h/l (←/→) — switch active pane
//   j/k (↑/↓) — navigate within active pane
//   Tab/ShiftTab — switch base-version tabs (version browser only)
//   Enter — select / confirm
//   Esc — back / cancel sub-mode
//   q — quit

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Row, Table, TableState, Tabs},
    DefaultTerminal, Frame,
};

use crate::api::{Edition, VersionGroup, VersionKind};
use crate::installer::{InstallConfig, InstalledSite};

// ── Pane Focus ───────────────────────────────────────────────────────────────
//
// Rust concept: using an enum to represent which pane has keyboard focus.
// This replaces the old `Screen` enum — instead of separate screens, we have
// persistent panes that the user can switch between with h/l.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActivePane {
    VersionBrowser,
    InstalledVersions,
    InstalledSites,
    LogPanel,
}

// ── Left Pane Sub-modes ──────────────────────────────────────────────────────
//
// The version browser pane has its own internal state machine. These are
// inline modes — the user stays in the left pane while picking an edition
// or typing a site name, unlike the old design where each was a full screen.

#[derive(Debug)]
enum LeftPaneMode {
    /// Browsing the version list. j/k navigates, Enter opens edition picker.
    Browse,

    /// Picking an edition for the selected version. j/k navigates editions,
    /// Enter confirms and moves to Configure, Esc goes back to Browse.
    EditionPicker {
        group_idx: usize,
        version_idx: usize,
        list_state: ListState,
    },

    /// Typing a site name. Enter spawns the install job, Esc goes back.
    Configure {
        group_idx: usize,
        version_idx: usize,
        edition: Edition,
        site_input: String,
    },
}

// ── App ──────────────────────────────────────────────────────────────────────

pub struct App {
    // ── Version data ────────────────────────────────────────────────────────
    /// Versions grouped by base version — one group per tab.
    version_groups: Vec<VersionGroup>,
    /// Currently selected base-version tab index.
    active_tab: usize,
    /// Highlighted row in the version list of the active tab.
    table_state: TableState,

    // ── Pane focus ──────────────────────────────────────────────────────────
    active_pane: ActivePane,
    left_mode: LeftPaneMode,

    // ── Right panel state (now interactive with ListState) ──────────────────
    installed_versions: Vec<String>,
    versions_list_state: ListState,
    installed_sites: Vec<InstalledSite>,
    sites_list_state: ListState,

    // ── Log panel ───────────────────────────────────────────────────────────
    /// Log lines from all jobs. Each entry is a formatted string.
    log_lines: Vec<String>,
    /// Scroll offset for the log panel (0 = show latest at bottom).
    log_scroll: usize,

    should_quit: bool,
}

impl App {
    pub fn new(
        version_groups: Vec<VersionGroup>,
        installed_versions: Vec<String>,
        installed_sites: Vec<InstalledSite>,
    ) -> Self {
        // Initialise list states with first item selected if lists aren't empty.
        let mut versions_list_state = ListState::default();
        if !installed_versions.is_empty() {
            versions_list_state.select(Some(0));
        }
        let mut sites_list_state = ListState::default();
        if !installed_sites.is_empty() {
            sites_list_state.select(Some(0));
        }

        Self {
            version_groups,
            active_tab: 0,
            table_state: TableState::default().with_selected(0),
            active_pane: ActivePane::VersionBrowser,
            left_mode: LeftPaneMode::Browse,
            installed_versions,
            versions_list_state,
            installed_sites,
            sites_list_state,
            log_lines: Vec::new(),
            log_scroll: 0,
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
        // Global keys that work regardless of pane focus.
        if matches!(code, KeyCode::Char('q')) {
            // In Configure mode, 'q' is a text character — don't quit.
            if !matches!(self.left_mode, LeftPaneMode::Configure { .. })
                || self.active_pane != ActivePane::VersionBrowser
            {
                self.should_quit = true;
                return;
            }
        }

        // Pane switching: h/l (or ←/→) — but only when not typing text.
        if !self.is_text_input_active() {
            match code {
                KeyCode::Char('h') | KeyCode::Left => {
                    self.focus_prev_pane();
                    return;
                }
                KeyCode::Char('l') | KeyCode::Right => {
                    self.focus_next_pane();
                    return;
                }
                _ => {}
            }
        }

        // Delegate to the focused pane's handler.
        match self.active_pane {
            ActivePane::VersionBrowser => self.on_version_browser(code),
            ActivePane::InstalledVersions => self.on_installed_versions(code),
            ActivePane::InstalledSites => self.on_installed_sites(code),
            ActivePane::LogPanel => self.on_log_panel(code),
        }
    }

    /// Returns true when the user is typing into a text field (Configure mode).
    fn is_text_input_active(&self) -> bool {
        self.active_pane == ActivePane::VersionBrowser
            && matches!(self.left_mode, LeftPaneMode::Configure { .. })
    }

    // ── Pane Focus Navigation ────────────────────────────────────────────────
    //
    // Pane order for h/l cycling:
    //   VersionBrowser ↔ InstalledVersions ↔ InstalledSites ↔ LogPanel
    //
    // Rust concept: matching on an enum value and returning the next/prev
    // variant. This is a simple state machine for focus cycling.

    fn focus_next_pane(&mut self) {
        self.active_pane = match self.active_pane {
            ActivePane::VersionBrowser => ActivePane::InstalledVersions,
            ActivePane::InstalledVersions => ActivePane::InstalledSites,
            ActivePane::InstalledSites => ActivePane::LogPanel,
            ActivePane::LogPanel => ActivePane::VersionBrowser,
        };
    }

    fn focus_prev_pane(&mut self) {
        self.active_pane = match self.active_pane {
            ActivePane::VersionBrowser => ActivePane::LogPanel,
            ActivePane::InstalledVersions => ActivePane::VersionBrowser,
            ActivePane::InstalledSites => ActivePane::InstalledVersions,
            ActivePane::LogPanel => ActivePane::InstalledSites,
        };
    }

    // ── Version Browser Input ────────────────────────────────────────────────

    fn on_version_browser(&mut self, code: KeyCode) {
        match &self.left_mode {
            LeftPaneMode::Browse => self.on_browse(code),
            LeftPaneMode::EditionPicker { .. } => self.on_edition_picker(code),
            LeftPaneMode::Configure { .. } => self.on_configure(code),
        }
    }

    fn on_browse(&mut self, code: KeyCode) {
        match code {
            // Tab navigation for base-version tabs.
            KeyCode::Tab => self.next_tab(),
            KeyCode::BackTab => self.prev_tab(),

            // Row navigation within the version list.
            KeyCode::Up | KeyCode::Char('k') => self.select_prev_row(),
            KeyCode::Down | KeyCode::Char('j') => self.select_next_row(),

            // Select a version → open edition picker inline.
            KeyCode::Enter => {
                if let Some(vi) = self.table_state.selected() {
                    let gi = self.active_tab;
                    let mut list_state = ListState::default();
                    list_state.select(Some(0));
                    self.left_mode = LeftPaneMode::EditionPicker {
                        group_idx: gi,
                        version_idx: vi,
                        list_state,
                    };
                }
            }

            KeyCode::Esc => self.should_quit = true,
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
        let next = self
            .table_state
            .selected()
            .map(|i| (i + 1).min(len.saturating_sub(1)))
            .unwrap_or(0);
        self.table_state.select(Some(next));
    }

    fn select_prev_row(&mut self) {
        let prev = self
            .table_state
            .selected()
            .map(|i| i.saturating_sub(1))
            .unwrap_or(0);
        self.table_state.select(Some(prev));
    }

    fn current_group_len(&self) -> usize {
        self.version_groups
            .get(self.active_tab)
            .map(|g| g.versions.len())
            .unwrap_or(0)
    }

    // ── Edition Picker Input ─────────────────────────────────────────────────

    fn on_edition_picker(&mut self, code: KeyCode) {
        // Rust concept: `let ... else` — destructure or return early.
        // We need mutable access to the list_state inside the enum variant.
        let LeftPaneMode::EditionPicker {
            group_idx,
            version_idx,
            list_state,
        } = &mut self.left_mode
        else {
            return;
        };
        let editions = self.version_groups[*group_idx].versions[*version_idx].available_editions();

        match code {
            KeyCode::Esc => {
                self.left_mode = LeftPaneMode::Browse;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let prev = list_state
                    .selected()
                    .map(|i| i.saturating_sub(1))
                    .unwrap_or(0);
                list_state.select(Some(prev));
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let last = editions.len().saturating_sub(1);
                let next = list_state
                    .selected()
                    .map(|i| (i + 1).min(last))
                    .unwrap_or(0);
                list_state.select(Some(next));
            }
            KeyCode::Enter => {
                let gi = *group_idx;
                let vi = *version_idx;
                let edition = list_state
                    .selected()
                    .map(|i| editions[i].clone())
                    .unwrap_or_else(|| editions[0].clone());
                self.left_mode = LeftPaneMode::Configure {
                    group_idx: gi,
                    version_idx: vi,
                    edition,
                    site_input: String::new(),
                };
            }
            _ => {}
        }
    }

    // ── Configure Input ──────────────────────────────────────────────────────

    fn on_configure(&mut self, code: KeyCode) {
        let LeftPaneMode::Configure {
            group_idx,
            version_idx,
            edition,
            site_input,
        } = &mut self.left_mode
        else {
            return;
        };

        match code {
            KeyCode::Esc => {
                let gi = *group_idx;
                let vi = *version_idx;
                let mut list_state = ListState::default();
                list_state.select(Some(0));
                self.left_mode = LeftPaneMode::EditionPicker {
                    group_idx: gi,
                    version_idx: vi,
                    list_state,
                };
            }
            KeyCode::Char(c) => site_input.push(c),
            KeyCode::Backspace => {
                site_input.pop();
            }
            KeyCode::Enter if !site_input.is_empty() => {
                let version = &self.version_groups[*group_idx].versions[*version_idx];
                let config = InstallConfig {
                    version: version.install_arg(),
                    edition: edition.as_str().to_string(),
                    site_name: site_input.clone(),
                };

                // TODO (Phase 2): Spawn this as an async job instead of just logging.
                self.log_lines.push(format!(
                    "[install] cmk-dev-install {} -e {} && cmk-dev-site ...{} -n {}",
                    config.version, config.edition, config.edition, config.site_name
                ));
                self.log_lines
                    .push("  → queued (async execution in Phase 2)".to_string());

                // Return to browse mode so user can start another install.
                self.left_mode = LeftPaneMode::Browse;
            }
            _ => {}
        }
    }

    // ── Installed Versions Input ─────────────────────────────────────────────

    fn on_installed_versions(&mut self, code: KeyCode) {
        let len = self.installed_versions.len();
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                let prev = self
                    .versions_list_state
                    .selected()
                    .map(|i| i.saturating_sub(1))
                    .unwrap_or(0);
                self.versions_list_state.select(Some(prev));
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let next = self
                    .versions_list_state
                    .selected()
                    .map(|i| (i + 1).min(len.saturating_sub(1)))
                    .unwrap_or(0);
                self.versions_list_state.select(Some(next));
            }
            // TODO (Phase 4): 'd' to delete version, 's' to create site from version.
            KeyCode::Esc => self.should_quit = true,
            _ => {}
        }
    }

    // ── Installed Sites Input ────────────────────────────────────────────────

    fn on_installed_sites(&mut self, code: KeyCode) {
        let len = self.installed_sites.len();
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                let prev = self
                    .sites_list_state
                    .selected()
                    .map(|i| i.saturating_sub(1))
                    .unwrap_or(0);
                self.sites_list_state.select(Some(prev));
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let next = self
                    .sites_list_state
                    .selected()
                    .map(|i| (i + 1).min(len.saturating_sub(1)))
                    .unwrap_or(0);
                self.sites_list_state.select(Some(next));
            }
            // TODO (Phase 4): 'd' to delete site.
            KeyCode::Esc => self.should_quit = true,
            _ => {}
        }
    }

    // ── Log Panel Input ──────────────────────────────────────────────────────

    fn on_log_panel(&mut self, code: KeyCode) {
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.log_scroll = self.log_scroll.saturating_add(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.log_scroll = self.log_scroll.saturating_sub(1);
            }
            KeyCode::Esc => self.should_quit = true,
            _ => {}
        }
    }

    // ── Rendering ────────────────────────────────────────────────────────────

    fn render(&mut self, frame: &mut Frame) {
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

        self.render_tabs(frame, outer[0]);
        self.render_left(frame, body[0]);
        self.render_installed_versions(frame, right[0]);
        self.render_installed_sites(frame, right[1]);
        self.render_log_panel(frame, outer[2]);
        self.render_footer(frame, outer[3]);
    }

    // ── Tab bar ──────────────────────────────────────────────────────────────

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

    // ── Left pane ────────────────────────────────────────────────────────────

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
            .block(
                Block::default()
                    .title(" Selected Version ")
                    .borders(Borders::ALL)
                    .border_style(border_style(is_focused)),
            ),
            chunks[0],
        );

        // Edition list
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

    // ── Right panel: Installed Versions ──────────────────────────────────────

    fn render_installed_versions(&mut self, frame: &mut Frame, area: Rect) {
        let is_focused = self.active_pane == ActivePane::InstalledVersions;

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

        frame.render_stateful_widget(list, area, &mut self.versions_list_state);
    }

    // ── Right panel: Installed Sites ─────────────────────────────────────────

    fn render_installed_sites(&mut self, frame: &mut Frame, area: Rect) {
        let is_focused = self.active_pane == ActivePane::InstalledSites;

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

        frame.render_stateful_widget(list, area, &mut self.sites_list_state);
    }

    // ── Log Panel ────────────────────────────────────────────────────────────

    fn render_log_panel(&self, frame: &mut Frame, area: Rect) {
        let is_focused = self.active_pane == ActivePane::LogPanel;

        let lines: Vec<Line> = if self.log_lines.is_empty() {
            vec![Line::styled(
                "  No jobs running. Select a version and press Enter to install.",
                Style::default().fg(Color::DarkGray),
            )]
        } else {
            self.log_lines
                .iter()
                .map(|l| {
                    if l.starts_with("[install]") {
                        Line::styled(l.as_str(), Style::default().fg(Color::Cyan))
                    } else if l.contains("done") || l.contains("✓") {
                        Line::styled(l.as_str(), Style::default().fg(Color::Green))
                    } else if l.contains("error") || l.contains("✗") {
                        Line::styled(l.as_str(), Style::default().fg(Color::Red))
                    } else {
                        Line::styled(l.as_str(), Style::default().fg(Color::Gray))
                    }
                })
                .collect()
        };

        // Scroll: show latest lines by default, scroll_offset shifts viewport up.
        let visible_height = area.height.saturating_sub(2) as usize; // minus borders
        let total = lines.len();
        let start = total.saturating_sub(visible_height + self.log_scroll);
        let end = total.saturating_sub(self.log_scroll).min(total);
        let visible: Vec<Line> = lines[start..end].to_vec();

        frame.render_widget(
            Paragraph::new(visible).block(
                Block::default()
                    .title(" Log ")
                    .borders(Borders::ALL)
                    .border_style(border_style(is_focused)),
            ),
            area,
        );
    }

    // ── Footer ───────────────────────────────────────────────────────────────

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let hint = match self.active_pane {
            ActivePane::VersionBrowser => match &self.left_mode {
                LeftPaneMode::Browse => {
                    "  j/k row  Tab/ShiftTab version tab  Enter select  h/l pane  q quit"
                }
                LeftPaneMode::EditionPicker { .. } => {
                    "  j/k edition  Enter confirm  Esc back  h/l pane"
                }
                LeftPaneMode::Configure { .. } => "  Type site name  Enter install  Esc back",
            },
            ActivePane::InstalledVersions => {
                "  j/k navigate  d delete  s create site  h/l pane  q quit"
            }
            ActivePane::InstalledSites => "  j/k navigate  d delete site  h/l pane  q quit",
            ActivePane::LogPanel => "  j/k scroll  h/l pane  q quit",
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

// ── Free helpers ─────────────────────────────────────────────────────────────

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
