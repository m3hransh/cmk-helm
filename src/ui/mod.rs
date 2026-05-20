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
use crossterm::event::{
    self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Row, Table, TableState, Tabs},
    DefaultTerminal, Frame,
};

use tokio::sync::mpsc;

use crate::api::{Edition, VersionGroup, VersionKind};
use crate::installer::{self, InstallConfig, InstalledSite, Job, JobId, JobMessage, JobStatus};

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

// ── Right Pane Sub-modes ─────────────────────────────────────────────────────
//
// The installed versions and sites panes can enter inline modes for
// confirmation prompts and site name input, similar to the left pane's
// EditionPicker / Configure modes.

#[derive(Debug)]
enum RightPaneMode {
    /// Normal browsing — j/k navigates, d/s triggers actions.
    Browse,

    /// Confirming a destructive action. Shows "Are you sure? y/n".
    /// `action` is the label shown, `on_confirm` is called on 'y'.
    ConfirmDelete { label: String, target: DeleteTarget },

    /// Typing a site name to create a site from an installed version.
    SiteNameInput {
        /// The omd version string to pass to cmk-dev-site.
        omd_version: String,
        site_input: String,
    },
}

/// What a delete confirmation is targeting.
#[derive(Debug)]
enum DeleteTarget {
    Version(String),
    Site(String),
}

// ── Pane Geometry ────────────────────────────────────────────────────────────
//
// Stored each frame so mouse clicks can be mapped to panes via hit-testing.

#[derive(Debug, Default, Clone, Copy)]
struct PaneRects {
    version_browser: Rect,
    installed_versions: Rect,
    installed_sites: Rect,
    log_panel: Rect,
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
    right_mode: RightPaneMode,

    // ── Pane geometry (updated each render for mouse hit-testing) ────────
    pane_rects: PaneRects,

    // ── Right panel state (now interactive with ListState) ──────────────────
    installed_versions: Vec<String>,
    versions_list_state: ListState,
    installed_sites: Vec<InstalledSite>,
    sites_list_state: ListState,

    // ── Job system ──────────────────────────────────────────────────────────
    //
    // Rust concept: `mpsc::unbounded_channel` creates a channel with no
    // capacity limit. The sender is cloned into each spawned tokio task;
    // the receiver stays in the App and is drained each frame with `try_recv()`.
    // "Unbounded" is fine here because job output is bounded by subprocess speed.
    jobs: Vec<Job>,
    next_job_id: JobId,
    job_tx: mpsc::UnboundedSender<JobMessage>,
    job_rx: mpsc::UnboundedReceiver<JobMessage>,

    // ── Log panel ───────────────────────────────────────────────────────────
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

        let (job_tx, job_rx) = mpsc::unbounded_channel();

        Self {
            version_groups,
            active_tab: 0,
            table_state: TableState::default().with_selected(0),
            active_pane: ActivePane::VersionBrowser,
            left_mode: LeftPaneMode::Browse,
            right_mode: RightPaneMode::Browse,
            pane_rects: PaneRects::default(),
            installed_versions,
            versions_list_state,
            installed_sites,
            sites_list_state,
            jobs: Vec::new(),
            next_job_id: 0,
            job_tx,
            job_rx,
            log_scroll: 0,
            should_quit: false,
        }
    }

    // ── Event Loop ───────────────────────────────────────────────────────────

    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        // Rust concept: crossterm mouse capture must be explicitly enabled.
        // Without this, `Event::Mouse` is never emitted. We disable it on
        // exit so the terminal returns to normal mouse behavior (text select).
        crossterm::execute!(std::io::stdout(), crossterm::event::EnableMouseCapture)?;

        while !self.should_quit {
            // Drain all pending job messages before rendering so the UI
            // shows the latest output. `try_recv` is non-blocking — it
            // returns immediately if the channel is empty.
            self.drain_job_messages();

            terminal.draw(|frame| self.render(frame))?;
            self.handle_events()?;
        }

        crossterm::execute!(std::io::stdout(), crossterm::event::DisableMouseCapture)?;
        Ok(())
    }

    // ── Job Message Drain ────────────────────────────────────────────────────
    //
    // Rust concept: `try_recv()` on an `mpsc::UnboundedReceiver` returns
    // `Ok(msg)` if a message is available, or `Err(TryRecvError::Empty)` if
    // the channel is empty. We drain in a loop until empty, which keeps the
    // UI responsive — we never block waiting for messages.

    fn drain_job_messages(&mut self) {
        while let Ok(msg) = self.job_rx.try_recv() {
            match msg {
                JobMessage::Output(job_id, ref line) => {
                    crate::debug::log(&format!("job[{job_id}] output: {line}"));
                    if let Some(job) = self.jobs.iter_mut().find(|j| j.id == job_id) {
                        job.output.push(line.clone());
                    }
                    // Reset scroll to bottom when new output arrives.
                    self.log_scroll = 0;
                }
                JobMessage::Finished(job_id, success) => {
                    crate::debug::log(&format!("job[{job_id}] finished: success={success}"));
                    if let Some(job) = self.jobs.iter_mut().find(|j| j.id == job_id) {
                        job.status = if success {
                            JobStatus::Done
                        } else {
                            JobStatus::Failed
                        };
                        let status_label = if success { "✓ done" } else { "✗ failed" };
                        job.output.push(format!("── {status_label} ──"));
                    }
                    // Refresh installed versions/sites after any job completes.
                    self.refresh_installed();
                }
            }
        }
    }

    /// Re-reads installed versions and sites from omd.
    fn refresh_installed(&mut self) {
        self.installed_versions = installer::list_installed_versions().unwrap_or_default();
        self.installed_sites = installer::list_installed_sites().unwrap_or_default();

        // Keep selection in bounds after refresh.
        if self.installed_versions.is_empty() {
            self.versions_list_state.select(None);
        } else if self
            .versions_list_state
            .selected()
            .is_none_or(|i| i >= self.installed_versions.len())
        {
            self.versions_list_state.select(Some(0));
        }
        if self.installed_sites.is_empty() {
            self.sites_list_state.select(None);
        } else if self
            .sites_list_state
            .selected()
            .is_none_or(|i| i >= self.installed_sites.len())
        {
            self.sites_list_state.select(Some(0));
        }
    }

    // ── Job Spawning ─────────────────────────────────────────────────────────

    /// Creates a new Job, registers it, and spawns the async install task.
    fn spawn_install_job(&mut self, config: InstallConfig) {
        let job_id = self.next_job_id;
        self.next_job_id += 1;

        let label = format!(
            "install {} -e {} → {}",
            config.version, config.edition, config.site_name
        );

        crate::debug::log(&format!("spawn_install_job: id={job_id} label={label}"));

        self.jobs.push(Job {
            id: job_id,
            label,
            status: JobStatus::Running,
            output: Vec::new(),
        });

        // Clone the sender and pass it to the background task.
        // Rust concept: `mpsc::UnboundedSender` implements `Clone` — each
        // clone can send independently. The single receiver in App drains all.
        installer::spawn_install(config, job_id, self.job_tx.clone());
    }

    // ── Input ────────────────────────────────────────────────────────────────

    fn handle_events(&mut self) -> Result<()> {
        if event::poll(std::time::Duration::from_millis(16))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    self.dispatch(key.code, key.modifiers);
                }
                Event::Mouse(mouse) => {
                    if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
                        self.handle_mouse_click(mouse.column, mouse.row);
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Checks which pane was clicked and switches focus to it.
    fn handle_mouse_click(&mut self, col: u16, row: u16) {
        let r = self.pane_rects;
        if contains(r.version_browser, col, row) {
            self.active_pane = ActivePane::VersionBrowser;
        } else if contains(r.installed_versions, col, row) {
            self.active_pane = ActivePane::InstalledVersions;
        } else if contains(r.installed_sites, col, row) {
            self.active_pane = ActivePane::InstalledSites;
        } else if contains(r.log_panel, col, row) {
            self.active_pane = ActivePane::LogPanel;
        }
    }

    fn dispatch(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        let ctrl = modifiers.contains(KeyModifiers::CONTROL);

        // Global keys that work regardless of pane focus.
        if matches!(code, KeyCode::Char('q')) && !ctrl {
            // In Configure mode, 'q' is a text character — don't quit.
            if !matches!(self.left_mode, LeftPaneMode::Configure { .. })
                || self.active_pane != ActivePane::VersionBrowser
            {
                self.should_quit = true;
                return;
            }
        }

        // Pane switching: Ctrl+arrows or Alt+h/j/k/l — spatial navigation.
        //
        // Ctrl+h/j are unusable — terminals encode Ctrl+h as backspace (0x08)
        // and Ctrl+j as newline (0x0A), so crossterm never sees them as
        // Char('h')/Char('j') with CONTROL. Alt doesn't have this problem.
        let alt = modifiers.contains(KeyModifiers::ALT);
        if ctrl {
            match code {
                KeyCode::Left => {
                    self.focus_left();
                    return;
                }
                KeyCode::Right => {
                    self.focus_right();
                    return;
                }
                KeyCode::Down => {
                    self.focus_down();
                    return;
                }
                KeyCode::Up => {
                    self.focus_up();
                    return;
                }
                _ => {}
            }
        }
        if alt {
            match code {
                KeyCode::Char('h') => {
                    self.focus_left();
                    return;
                }
                KeyCode::Char('l') => {
                    self.focus_right();
                    return;
                }
                KeyCode::Char('j') => {
                    self.focus_down();
                    return;
                }
                KeyCode::Char('k') => {
                    self.focus_up();
                    return;
                }
                _ => {}
            }
        }

        // Delegate to the focused pane's handler.
        // Within-pane navigation uses j/k and arrow keys.
        match self.active_pane {
            ActivePane::VersionBrowser => self.on_version_browser(code),
            ActivePane::InstalledVersions => self.on_installed_versions(code),
            ActivePane::InstalledSites => self.on_installed_sites(code),
            ActivePane::LogPanel => self.on_log_panel(code),
        }
    }

    // ── Pane Focus Navigation ────────────────────────────────────────────────
    //
    // Spatial pane layout matching the screen:
    //
    //   ┌─────────────┬──────────────────┐
    //   │ Version     │ Installed        │
    //   │ Browser     │ Versions         │
    //   │      →l     │      ↓j          │
    //   │      ↓j     ├──────────────────┤
    //   │             │ Installed        │
    //   │             │ Sites            │
    //   │         ←h←←│                  │
    //   ├─────────────┴──────────────────┤
    //   │ Log Panel                      │
    //   │      ↑k → VersionBrowser      │
    //   └────────────────────────────────┘
    //
    // h/j/k/l = pane focus (spatial), arrow keys = within-pane navigation.

    fn focus_right(&mut self) {
        self.active_pane = match self.active_pane {
            ActivePane::VersionBrowser => ActivePane::InstalledVersions,
            ActivePane::LogPanel => ActivePane::InstalledSites,
            other => other, // already rightmost
        };
    }

    fn focus_left(&mut self) {
        self.active_pane = match self.active_pane {
            ActivePane::InstalledVersions => ActivePane::VersionBrowser,
            ActivePane::InstalledSites => ActivePane::VersionBrowser,
            ActivePane::LogPanel => ActivePane::VersionBrowser,
            other => other, // already leftmost
        };
    }

    fn focus_down(&mut self) {
        self.active_pane = match self.active_pane {
            ActivePane::VersionBrowser => ActivePane::LogPanel,
            ActivePane::InstalledVersions => ActivePane::InstalledSites,
            ActivePane::InstalledSites => ActivePane::LogPanel,
            other => other, // already at bottom
        };
    }

    fn focus_up(&mut self) {
        self.active_pane = match self.active_pane {
            ActivePane::LogPanel => ActivePane::VersionBrowser,
            ActivePane::InstalledSites => ActivePane::InstalledVersions,
            ActivePane::InstalledVersions => ActivePane::VersionBrowser,
            other => other, // already at top
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
            KeyCode::Char('l') => self.next_tab(),
            KeyCode::Char('h') => self.prev_tab(),

            // Row navigation within the version list.
            KeyCode::Up | KeyCode::Char('k') => self.select_prev_row(),
            KeyCode::Down | KeyCode::Char('j') => self.select_next_row(),

            // Select a version → open edition picker inline.
            KeyCode::Enter | KeyCode::Char('i') => {
                if let Some(vi) = self.table_state.selected() {
                    let gi = self.active_tab;
                    let version = &self.version_groups[gi].versions[vi];
                    crate::debug::log(&format!(
                        "on_browse: selected group_idx={gi} version_idx={vi} base={} kind={:?}",
                        version.base, version.kind
                    ));
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

                crate::debug::log(&format!(
                    "on_configure: group_idx={} version_idx={} version.base={} version.kind={:?}",
                    *group_idx, *version_idx, version.base, version.kind
                ));

                let config = InstallConfig {
                    version: version.install_arg(),
                    omd_version: version.dir_name(),
                    edition: edition.as_str().to_string(),
                    site_name: site_input.clone(),
                };

                crate::debug::log(&format!(
                    "on_configure: InstallConfig {{ version={}, omd_version={}, edition={}, site_name={} }}",
                    config.version, config.omd_version, config.edition, config.site_name
                ));

                // Spawn an async install job. The job runs in the background
                // and sends output lines through the mpsc channel. The UI
                // drains the channel each frame in `drain_job_messages()`.
                self.spawn_install_job(config);

                // Return to browse mode so user can start another install.
                self.left_mode = LeftPaneMode::Browse;
            }
            _ => {}
        }
    }

    // ── Installed Versions Input ─────────────────────────────────────────────

    fn on_installed_versions(&mut self, code: KeyCode) {
        match &self.right_mode {
            RightPaneMode::Browse => self.on_installed_versions_browse(code),
            RightPaneMode::ConfirmDelete { .. } => self.on_right_confirm(code),
            RightPaneMode::SiteNameInput { .. } => self.on_right_site_input(code),
        }
    }

    fn on_installed_versions_browse(&mut self, code: KeyCode) {
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
            KeyCode::Char('d') => {
                if let Some(idx) = self.versions_list_state.selected() {
                    if let Some(version) = self.installed_versions.get(idx) {
                        self.right_mode = RightPaneMode::ConfirmDelete {
                            label: format!("Delete version {version}?"),
                            target: DeleteTarget::Version(version.clone()),
                        };
                    }
                }
            }
            KeyCode::Char('s') => {
                if let Some(idx) = self.versions_list_state.selected() {
                    if let Some(version) = self.installed_versions.get(idx) {
                        self.right_mode = RightPaneMode::SiteNameInput {
                            omd_version: version.clone(),
                            site_input: String::new(),
                        };
                    }
                }
            }
            KeyCode::Esc => self.should_quit = true,
            _ => {}
        }
    }

    // ── Installed Sites Input ────────────────────────────────────────────────

    fn on_installed_sites(&mut self, code: KeyCode) {
        match &self.right_mode {
            RightPaneMode::Browse => self.on_installed_sites_browse(code),
            RightPaneMode::ConfirmDelete { .. } => self.on_right_confirm(code),
            RightPaneMode::SiteNameInput { .. } => self.on_right_site_input(code),
        }
    }

    fn on_installed_sites_browse(&mut self, code: KeyCode) {
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
            KeyCode::Char('d') => {
                if let Some(idx) = self.sites_list_state.selected() {
                    if let Some(site) = self.installed_sites.get(idx) {
                        self.right_mode = RightPaneMode::ConfirmDelete {
                            label: format!("Delete site {}?", site.name),
                            target: DeleteTarget::Site(site.name.clone()),
                        };
                    }
                }
            }
            KeyCode::Esc => self.should_quit = true,
            _ => {}
        }
    }

    // ── Right Pane Confirm / Site Input ──────────────────────────────────────

    fn on_right_confirm(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('y') | KeyCode::Enter => {
                // Extract target before resetting mode (borrow rules).
                let RightPaneMode::ConfirmDelete { target, .. } = &self.right_mode else {
                    return;
                };
                match target {
                    DeleteTarget::Version(version) => {
                        let job_id = self.next_job_id;
                        self.next_job_id += 1;
                        let label = format!("delete version {version}");
                        self.jobs.push(installer::Job {
                            id: job_id,
                            label,
                            status: installer::JobStatus::Running,
                            output: Vec::new(),
                        });
                        installer::spawn_delete_version(
                            version.clone(),
                            job_id,
                            self.job_tx.clone(),
                        );
                    }
                    DeleteTarget::Site(name) => {
                        let job_id = self.next_job_id;
                        self.next_job_id += 1;
                        let label = format!("delete site {name}");
                        self.jobs.push(installer::Job {
                            id: job_id,
                            label,
                            status: installer::JobStatus::Running,
                            output: Vec::new(),
                        });
                        installer::spawn_delete_site(name.clone(), job_id, self.job_tx.clone());
                    }
                }
                self.right_mode = RightPaneMode::Browse;
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                self.right_mode = RightPaneMode::Browse;
            }
            _ => {}
        }
    }

    fn on_right_site_input(&mut self, code: KeyCode) {
        let RightPaneMode::SiteNameInput {
            omd_version,
            site_input,
        } = &mut self.right_mode
        else {
            return;
        };

        match code {
            KeyCode::Esc => {
                self.right_mode = RightPaneMode::Browse;
            }
            KeyCode::Char(c) => site_input.push(c),
            KeyCode::Backspace => {
                site_input.pop();
            }
            KeyCode::Enter if !site_input.is_empty() => {
                let job_id = self.next_job_id;
                self.next_job_id += 1;
                let label = format!("create site from {omd_version}");
                let omd_ver = omd_version.clone();
                let site = site_input.clone();
                self.jobs.push(installer::Job {
                    id: job_id,
                    label,
                    status: installer::JobStatus::Running,
                    output: Vec::new(),
                });
                installer::spawn_create_site(omd_ver, site, job_id, self.job_tx.clone());
                self.right_mode = RightPaneMode::Browse;
            }
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

        // Store pane geometry so mouse clicks can be mapped to panes.
        self.pane_rects = PaneRects {
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

    // ── Right panel: Installed Sites ─────────────────────────────────────────

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

    // ── Log Panel ────────────────────────────────────────────────────────────

    fn render_log_panel(&self, frame: &mut Frame, area: Rect) {
        let is_focused = self.active_pane == ActivePane::LogPanel;

        // Flatten all job output into a single line stream with job labels.
        // Each job's output is prefixed with a header showing its label and status.
        let lines: Vec<Line> = if self.jobs.is_empty() {
            vec![Line::styled(
                "  No jobs running. Select a version and press Enter to install.",
                Style::default().fg(Color::DarkGray),
            )]
        } else {
            let mut all_lines: Vec<Line> = Vec::new();
            for job in &self.jobs {
                // Job header with status indicator.
                let (status_icon, status_color) = match job.status {
                    JobStatus::Running => ("⟳", Color::Yellow),
                    JobStatus::Done => ("✓", Color::Green),
                    JobStatus::Failed => ("✗", Color::Red),
                };
                all_lines.push(Line::from(vec![
                    Span::styled(
                        format!("[{status_icon}] "),
                        Style::default().fg(status_color),
                    ),
                    Span::styled(&job.label, Style::default().fg(Color::Cyan)),
                ]));

                // Job output lines.
                for line in &job.output {
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
                    all_lines.push(Line::styled(format!("  {line}"), style));
                }
            }
            all_lines
        };

        // Scroll: show latest lines by default, scroll_offset shifts viewport up.
        let visible_height = area.height.saturating_sub(2) as usize; // minus borders
        let total = lines.len();
        let start = total.saturating_sub(visible_height + self.log_scroll);
        let end = total.saturating_sub(self.log_scroll).min(total);
        let visible: Vec<Line> = lines[start..end].to_vec();

        // Show running job count in the title.
        let running = self
            .jobs
            .iter()
            .filter(|j| j.status == JobStatus::Running)
            .count();
        let title = if running > 0 {
            format!(" Log ({running} running) ")
        } else {
            " Log ".to_string()
        };

        frame.render_widget(
            Paragraph::new(visible).block(
                Block::default()
                    .title(title)
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
                    "  j/k row  Tab/ShiftTab version  Enter select  Alt+hjkl pane  q quit"
                }
                LeftPaneMode::EditionPicker { .. } => {
                    "  j/k edition  Enter confirm  Esc back  Alt+hjkl pane"
                }
                LeftPaneMode::Configure { .. } => {
                    "  Type site name  Enter install  Esc back  Alt+hjkl pane"
                }
            },
            ActivePane::InstalledVersions => match &self.right_mode {
                RightPaneMode::ConfirmDelete { .. } => "  y confirm  n/Esc cancel",
                RightPaneMode::SiteNameInput { .. } => "  Type site name  Enter create  Esc cancel",
                RightPaneMode::Browse => {
                    "  j/k navigate  d delete  s create site  Alt+hjkl pane  q quit"
                }
            },
            ActivePane::InstalledSites => match &self.right_mode {
                RightPaneMode::ConfirmDelete { .. } => "  y confirm  n/Esc cancel",
                _ => "  j/k navigate  d delete site  Alt+hjkl pane  q quit",
            },
            ActivePane::LogPanel => "  j/k scroll  Alt+hjkl pane  q quit",
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

/// Hit-test: is the point (col, row) inside the given Rect?
fn contains(rect: Rect, col: u16, row: u16) -> bool {
    col >= rect.x && col < rect.x + rect.width && row >= rect.y && row < rect.y + rect.height
}
