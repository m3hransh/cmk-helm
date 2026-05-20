// UI module — Multi-pane TUI with concurrent job support.
//
// Layout:
//
//   ┌─ CMK Helm ── [2.6.0] [2.5.0] [2.4.0] ── Tab/ShiftTab ───────┐
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
//   h/l (←/→) — switch active pane (Tab/ShiftTab for version browser tabs)
//   j/k (↑/↓) — navigate within active pane
//   Enter — select / confirm
//   Esc — back / cancel sub-mode
//   q — quit
//
// Module structure (Rust concept: splitting `impl` across files):
//   state.rs  — shared types: ActivePane, LeftPaneMode, RightPaneMode, …
//   input.rs  — impl App: keyboard/mouse event handling
//   render.rs — impl App: all Ratatui rendering
//   mod.rs    — App struct, constructors, event loop, job system (this file)

mod state;
mod input;
mod render;

use anyhow::Result;
use ratatui::{
    widgets::{ListState, TableState},
    DefaultTerminal,
};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};

use crate::api::VersionGroup;
use crate::installer::{self, InstallConfig, InstalledSite, Job, JobId, JobMessage, JobStatus};

use state::{ActivePane, LeftPaneMode, PaneRects, RightPaneMode};

// Re-export so main.rs can refer to it as `ui::LoadResult`.
pub use state::LoadResult;

// ── App ───────────────────────────────────────────────────────────────────────

pub struct App {
    // ── Version data ─────────────────────────────────────────────────────────
    version_groups: Vec<VersionGroup>,
    active_tab: usize,
    table_state: TableState,

    // ── Pane focus ────────────────────────────────────────────────────────────
    active_pane: ActivePane,
    left_mode: LeftPaneMode,
    right_mode: RightPaneMode,

    // ── Pane geometry (updated each render for mouse hit-testing) ─────────────
    pane_rects: PaneRects,

    // ── Right panel state ─────────────────────────────────────────────────────
    installed_versions: Vec<String>,
    versions_list_state: ListState,
    installed_sites: Vec<InstalledSite>,
    sites_list_state: ListState,

    // ── Job system ────────────────────────────────────────────────────────────
    //
    // Rust concept: `mpsc::unbounded_channel` creates a channel with no
    // capacity limit. The sender is cloned into each spawned tokio task;
    // the receiver stays in the App and is drained each frame with `try_recv()`.
    // "Unbounded" is fine here because job output is bounded by subprocess speed.
    jobs: Vec<Job>,
    next_job_id: JobId,
    job_tx: mpsc::UnboundedSender<JobMessage>,
    job_rx: mpsc::UnboundedReceiver<JobMessage>,

    // ── Log panel ─────────────────────────────────────────────────────────────
    log_scroll: usize,
    /// Which job tab is currently shown. None only when jobs is empty.
    selected_job: Option<usize>,
    /// When true, mouse capture is disabled so the terminal can handle text
    /// selection for copy-paste. Esc exits this mode.
    copy_mode: bool,

    // ── Splash screen ─────────────────────────────────────────────────────────
    //
    // Rust concept: `Option<oneshot::Receiver<...>>` doubles as both the
    // "are we loading?" flag and the data source. When `Some`, we show the
    // animated splash; when `None`, we show the main UI. `try_recv()` is
    // non-blocking — it returns immediately if no data has arrived yet.
    splash_tick: u8,
    load_rx: Option<oneshot::Receiver<Result<LoadResult>>>,

    // ── Background refresh ────────────────────────────────────────────────────
    //
    // Refresh re-uses the same LoadResult oneshot pattern as initial load, but
    // without a splash screen. `last_refresh` is reset every time fresh data
    // arrives so the 30-second clock runs from the last successful fetch.
    refresh_rx: Option<oneshot::Receiver<Result<LoadResult>>>,
    is_refreshing: bool,
    last_refresh: Instant,

    should_quit: bool,
}

impl App {
    /// Called from main after data is already loaded (used in tests / direct launch).
    pub fn new(
        version_groups: Vec<VersionGroup>,
        installed_versions: Vec<String>,
        installed_sites: Vec<InstalledSite>,
    ) -> Self {
        let mut app = Self::new_loading_inner();
        app.load_rx = None;
        app.version_groups = version_groups;
        app.installed_versions = installed_versions;
        app.installed_sites = installed_sites;
        app
    }

    /// Called from main — shows the animated splash until the oneshot fires.
    pub fn new_loading(rx: oneshot::Receiver<Result<LoadResult>>) -> Self {
        let mut app = Self::new_loading_inner();
        app.load_rx = Some(rx);
        app
    }

    fn new_loading_inner() -> Self {
        let (job_tx, job_rx) = mpsc::unbounded_channel();
        Self {
            version_groups: Vec::new(),
            active_tab: 0,
            table_state: TableState::default().with_selected(0),
            active_pane: ActivePane::VersionBrowser,
            left_mode: LeftPaneMode::Browse,
            right_mode: RightPaneMode::Browse,
            pane_rects: PaneRects::default(),
            installed_versions: Vec::new(),
            versions_list_state: ListState::default(),
            installed_sites: Vec::new(),
            sites_list_state: ListState::default(),
            jobs: Vec::new(),
            next_job_id: 0,
            job_tx,
            job_rx,
            log_scroll: 0,
            selected_job: None,
            copy_mode: false,
            splash_tick: 0,
            load_rx: None,
            refresh_rx: None,
            is_refreshing: false,
            last_refresh: Instant::now(),
            should_quit: false,
        }
    }

    // ── Event Loop ────────────────────────────────────────────────────────────

    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        // Rust concept: crossterm mouse capture must be explicitly enabled.
        // Without this, `Event::Mouse` is never emitted. We disable it on
        // exit so the terminal returns to normal mouse behavior (text select).
        crossterm::execute!(std::io::stdout(), crossterm::event::EnableMouseCapture)?;

        while !self.should_quit {
            self.poll_load_result();
            self.poll_refresh_result();

            // Auto-refresh every 30 seconds once the main UI is showing.
            if self.load_rx.is_none()
                && !self.is_refreshing
                && self.last_refresh.elapsed() >= Duration::from_secs(30)
            {
                self.spawn_refresh();
            }

            self.drain_job_messages();
            terminal.draw(|frame| self.render(frame))?;
            self.handle_events()?;
            // Advance the splash animation every frame (poll timeout = 16 ms).
            self.splash_tick = self.splash_tick.wrapping_add(1);
        }

        crossterm::execute!(std::io::stdout(), crossterm::event::DisableMouseCapture)?;
        Ok(())
    }

    // ── Splash / Load Polling ─────────────────────────────────────────────────
    //
    // Rust concept: `oneshot::Receiver::try_recv()` returns
    //   Ok(value)         – sender sent something
    //   Err(TryRecvError) – not yet / sender dropped
    // We match only the Ok arm; the error arm silently loops until data arrives.

    fn poll_load_result(&mut self) {
        // `take()` moves the receiver out of the Option so we can match on it
        // without keeping a borrow on `self.load_rx` while also writing to
        // other fields — a common Rust borrow-checker trick.
        if let Some(mut rx) = self.load_rx.take() {
            match rx.try_recv() {
                Ok(Ok(data)) => {
                    self.version_groups = data.version_groups;
                    self.installed_versions = data.installed_versions;
                    self.installed_sites = data.installed_sites;
                    if !self.installed_versions.is_empty() {
                        self.versions_list_state.select(Some(0));
                    }
                    if !self.installed_sites.is_empty() {
                        self.sites_list_state.select(Some(0));
                    }
                    // Start the 30-second refresh clock from now.
                    self.last_refresh = Instant::now();
                    // load_rx stays None → main UI takes over
                }
                Ok(Err(e)) => {
                    crate::debug::log(&format!("load error: {e}"));
                    self.should_quit = true;
                }
                Err(_) => {
                    // Not ready yet — put the receiver back.
                    self.load_rx = Some(rx);
                }
            }
        }
    }

    // ── Background Refresh ────────────────────────────────────────────────────

    /// Spawns a background fetch task exactly like the initial load, but
    /// without a splash screen. Safe to call from synchronous context because
    /// `tokio::spawn` schedules the task without blocking.
    fn spawn_refresh(&mut self) {
        if self.is_refreshing || self.load_rx.is_some() {
            return;
        }
        self.is_refreshing = true;
        let (tx, rx) = oneshot::channel::<Result<LoadResult>>();
        self.refresh_rx = Some(rx);
        tokio::spawn(async move {
            let result = async {
                let version_groups =
                    crate::api::fetch_versions(crate::api::CMK_DOWNLOAD_URL).await?;
                let installed_versions =
                    crate::installer::list_installed_versions().unwrap_or_default();
                let installed_sites =
                    crate::installer::list_installed_sites().unwrap_or_default();
                Ok(LoadResult { version_groups, installed_versions, installed_sites })
            }
            .await;
            let _ = tx.send(result);
        });
    }

    /// Checks whether the background refresh task has finished and applies the
    /// new data. Uses the same `take()` trick as `poll_load_result`.
    fn poll_refresh_result(&mut self) {
        if let Some(mut rx) = self.refresh_rx.take() {
            match rx.try_recv() {
                Ok(Ok(data)) => {
                    self.version_groups = data.version_groups;
                    self.installed_versions = data.installed_versions;
                    self.installed_sites = data.installed_sites;
                    self.is_refreshing = false;
                    self.last_refresh = Instant::now();

                    // Keep tab index in bounds if version groups changed.
                    if !self.version_groups.is_empty()
                        && self.active_tab >= self.version_groups.len()
                    {
                        self.active_tab = self.version_groups.len() - 1;
                        self.table_state.select(Some(0));
                    }
                    // Keep installed-list selections in bounds.
                    let iv_len = self.installed_versions.len();
                    if iv_len == 0 {
                        self.versions_list_state.select(None);
                    } else if self.versions_list_state.selected().is_none_or(|i| i >= iv_len) {
                        self.versions_list_state.select(Some(0));
                    }
                    let is_len = self.installed_sites.len();
                    if is_len == 0 {
                        self.sites_list_state.select(None);
                    } else if self.sites_list_state.selected().is_none_or(|i| i >= is_len) {
                        self.sites_list_state.select(Some(0));
                    }
                }
                Ok(Err(e)) => {
                    crate::debug::log(&format!("refresh error: {e}"));
                    self.is_refreshing = false;
                    self.last_refresh = Instant::now();
                }
                Err(_) => {
                    // Not ready yet — put the receiver back.
                    self.refresh_rx = Some(rx);
                }
            }
        }
    }

    // ── Job Message Drain ─────────────────────────────────────────────────────
    //
    // Rust concept: `try_recv()` on an `mpsc::UnboundedReceiver` returns
    // `Ok(msg)` if a message is available, `Err(TryRecvError::Empty)` if not.
    // Draining in a loop keeps the UI responsive — we never block.

    fn drain_job_messages(&mut self) {
        while let Ok(msg) = self.job_rx.try_recv() {
            match msg {
                JobMessage::Output(job_id, ref line) => {
                    crate::debug::log(&format!("job[{job_id}] output: {line}"));
                    if let Some(job) = self.jobs.iter_mut().find(|j| j.id == job_id) {
                        job.output.push(line.clone());
                    }
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

    // ── Job Spawning ──────────────────────────────────────────────────────────

    /// Creates a new Job, registers it, and spawns the async install task.
    fn spawn_install_job(&mut self, config: InstallConfig) {
        let job_id = self.next_job_id;
        self.next_job_id += 1;

        let label = format!(
            "install {} -e {} → {}",
            config.version, config.edition, config.site_name
        );
        // Short tab title: base version only (drop the date/patch suffix).
        // omd_version is e.g. "2.6.0-2026.04.07" or "2.4.0p24".
        let base = config.omd_version.split('-').next().unwrap_or(&config.omd_version);
        let short_label = format!("install {base}");

        crate::debug::log(&format!("spawn_install_job: id={job_id} label={label}"));

        self.jobs.push(Job {
            id: job_id,
            label,
            short_label,
            status: JobStatus::Running,
            output: Vec::new(),
        });
        self.selected_job = Some(self.jobs.len() - 1);
        self.log_scroll = 0;

        // Rust concept: `mpsc::UnboundedSender` implements `Clone` — each
        // clone can send independently. The single receiver in App drains all.
        installer::spawn_install(config, job_id, self.job_tx.clone());
    }
}
