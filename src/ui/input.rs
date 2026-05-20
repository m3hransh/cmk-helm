// ── Input handling for App ────────────────────────────────────────────────────
//
// All keyboard / mouse event processing lives here. The event loop in mod.rs
// calls `handle_events` (pub(super)) each frame; everything else is private
// to this module.
//
// Rust concept: splitting `impl App` across multiple files is valid — the
// compiler stitches all impl blocks for the same type together. Methods
// defined in mod.rs (the `ui` module) are accessible here because child
// modules can see private items declared in their parent module.

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::{layout::Rect, widgets::ListState};

use crate::installer;

use super::state::{ActivePane, DeleteTarget, LeftPaneMode, RightPaneMode};
use super::App;

impl App {
    // ── Event Loop Entry Point ────────────────────────────────────────────────
    //
    // `pub(super)` — visible to the parent `ui` module (i.e. mod.rs/run),
    // but not to callers outside `ui`. All other methods here are private
    // to `ui::input`.

    pub(super) fn handle_events(&mut self) -> Result<()> {
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
        match self.active_pane {
            ActivePane::VersionBrowser => self.on_version_browser(code),
            ActivePane::InstalledVersions => self.on_installed_versions(code),
            ActivePane::InstalledSites => self.on_installed_sites(code),
            ActivePane::LogPanel => self.on_log_panel(code),
        }
    }

    // ── Pane Focus Navigation ─────────────────────────────────────────────────
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

    fn focus_right(&mut self) {
        self.active_pane = match self.active_pane {
            ActivePane::VersionBrowser => ActivePane::InstalledVersions,
            ActivePane::LogPanel => ActivePane::InstalledSites,
            other => other,
        };
    }

    fn focus_left(&mut self) {
        self.active_pane = match self.active_pane {
            ActivePane::InstalledVersions => ActivePane::VersionBrowser,
            ActivePane::InstalledSites => ActivePane::VersionBrowser,
            ActivePane::LogPanel => ActivePane::VersionBrowser,
            other => other,
        };
    }

    fn focus_down(&mut self) {
        self.active_pane = match self.active_pane {
            ActivePane::VersionBrowser => ActivePane::LogPanel,
            ActivePane::InstalledVersions => ActivePane::InstalledSites,
            ActivePane::InstalledSites => ActivePane::LogPanel,
            other => other,
        };
    }

    fn focus_up(&mut self) {
        self.active_pane = match self.active_pane {
            ActivePane::LogPanel => ActivePane::VersionBrowser,
            ActivePane::InstalledSites => ActivePane::InstalledVersions,
            ActivePane::InstalledVersions => ActivePane::VersionBrowser,
            other => other,
        };
    }

    // ── Version Browser Input ─────────────────────────────────────────────────

    fn on_version_browser(&mut self, code: KeyCode) {
        match &self.left_mode {
            LeftPaneMode::Browse => self.on_browse(code),
            LeftPaneMode::EditionPicker { .. } => self.on_edition_picker(code),
            LeftPaneMode::Configure { .. } => self.on_configure(code),
        }
    }

    fn on_browse(&mut self, code: KeyCode) {
        match code {
            KeyCode::Tab => self.next_tab(),
            KeyCode::BackTab => self.prev_tab(),
            KeyCode::Char('l') => self.next_tab(),
            KeyCode::Char('h') => self.prev_tab(),
            KeyCode::Up | KeyCode::Char('k') => self.select_prev_row(),
            KeyCode::Down | KeyCode::Char('j') => self.select_next_row(),
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
            KeyCode::Char('r') => self.spawn_refresh(),
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

    // ── Edition Picker Input ──────────────────────────────────────────────────

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

    // ── Configure Input ───────────────────────────────────────────────────────

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

                let config = crate::installer::InstallConfig {
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

                // Return to browse mode so the user can start another install.
                self.left_mode = LeftPaneMode::Browse;
            }
            _ => {}
        }
    }

    // ── Installed Versions Input ──────────────────────────────────────────────

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

    // ── Installed Sites Input ─────────────────────────────────────────────────

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

    // ── Right Pane Confirm / Site Input ───────────────────────────────────────

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

    // ── Log Panel Input ───────────────────────────────────────────────────────

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
}

/// Hit-test: is the point (col, row) inside the given Rect?
fn contains(rect: Rect, col: u16, row: u16) -> bool {
    col >= rect.x && col < rect.x + rect.width && row >= rect.y && row < rect.y + rect.height
}
