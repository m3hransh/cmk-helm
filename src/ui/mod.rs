// UI module — owns the App state, the event loop, and all rendering.
//
// Architecture: the app is a simple state machine with three screens:
//
//   PackageList  → user browses and selects a version
//   Configure    → user enters site name (future: more config questions)
//   Installing   → shows live output while cmk-dev-install/site runs
//
// Rust concept: structs hold data; impl blocks hold behaviour.
// There's no `class` keyword — this is Rust's equivalent of OOP.

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Row, Table, TableState},
};

use crate::api::{Edition, Package};
use crate::installer::InstallConfig;

// ── Screen State Machine ─────────────────────────────────────────────────────

/// Rust concept: enum variants can carry data — here `Configure` carries the
/// selected package and `Installing` carries the full config. This makes
/// invalid states (e.g. "installing without a selected package") unrepresentable
/// at compile time, a pattern called "make illegal states unrepresentable".
#[derive(Debug)]
enum Screen {
    PackageList,
    Configure { selected: Package, site_input: String },
    Installing { config: InstallConfig },
}

// ── App State ────────────────────────────────────────────────────────────────

pub struct App {
    packages: Vec<Package>,
    table_state: TableState,
    screen: Screen,
    status_message: Option<String>,
    should_quit: bool,
}

impl App {
    /// Creates an App with stub packages. In production, replace the stub
    /// with `api::fetch_packages(CMK_DOWNLOAD_URL).await?` called in main.
    pub fn new() -> Self {
        let packages = vec![
            Package { base_version: "2.4.0".into(), release_date: "2025.04.07".into(), edition: Edition::Cee },
            Package { base_version: "2.4.0".into(), release_date: "2025.04.01".into(), edition: Edition::Cre },
            Package { base_version: "2.3.0".into(), release_date: "2025.03.15".into(), edition: Edition::Cee },
            Package { base_version: "2.3.0".into(), release_date: "2025.03.01".into(), edition: Edition::Cce },
        ];

        Self {
            packages,
            table_state: TableState::default().with_selected(0),
            screen: Screen::PackageList,
            status_message: None,
            should_quit: false,
        }
    }

    // ── Event Loop ───────────────────────────────────────────────────────────

    /// Draws and handles input until the user quits.
    ///
    /// Rust concept: `mut self` here would *consume* self — we use `&mut self`
    /// so ownership stays in the caller (main.rs) and the loop can iterate.
    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        while !self.should_quit {
            terminal.draw(|frame| self.render(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    // ── Input Handling ───────────────────────────────────────────────────────

    fn handle_events(&mut self) -> Result<()> {
        // `poll` blocks for at most 16 ms (≈ 60 fps) before returning false.
        // This prevents busy-waiting at 100% CPU between frames.
        if event::poll(std::time::Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    return Ok(());
                }
                // Rust concept: `match` is exhaustive — the compiler forces you
                // to handle every variant of `self.screen`.
                match &self.screen {
                    Screen::PackageList => self.handle_list_input(key.code),
                    Screen::Configure { .. } => self.handle_configure_input(key.code),
                    Screen::Installing { .. } => self.handle_installing_input(key.code),
                }
            }
        }
        Ok(())
    }

    fn handle_list_input(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Down | KeyCode::Char('j') => self.select_next(),
            KeyCode::Up | KeyCode::Char('k') => self.select_prev(),
            KeyCode::Enter => {
                if let Some(idx) = self.table_state.selected() {
                    let selected = self.packages[idx].clone();
                    self.screen = Screen::Configure {
                        selected,
                        site_input: String::new(),
                    };
                }
            }
            _ => {}
        }
    }

    fn handle_configure_input(&mut self, code: KeyCode) {
        // Rust concept: `if let` destructures an enum variant.
        // We need `&mut self.screen` but also a mutable borrow of self for
        // transitions — so we clone what we need before transitioning.
        if let Screen::Configure { selected, site_input } = &mut self.screen {
            match code {
                KeyCode::Esc => {
                    self.screen = Screen::PackageList;
                }
                KeyCode::Char(c) => {
                    site_input.push(c);
                }
                KeyCode::Backspace => {
                    site_input.pop();
                }
                KeyCode::Enter if !site_input.is_empty() => {
                    let config = InstallConfig {
                        version: selected.install_version_arg(),
                        edition: selected.edition.as_str().to_string(),
                        site_name: site_input.clone(),
                    };
                    self.screen = Screen::Installing { config };
                }
                _ => {}
            }
        }
    }

    fn handle_installing_input(&mut self, code: KeyCode) {
        if code == KeyCode::Char('q') || code == KeyCode::Esc {
            self.should_quit = true;
        }
    }

    fn select_next(&mut self) {
        let last = self.packages.len().saturating_sub(1);
        let next = self.table_state.selected()
            .map(|i| (i + 1).min(last))
            .unwrap_or(0);
        self.table_state.select(Some(next));
    }

    fn select_prev(&mut self) {
        let prev = self.table_state.selected()
            .map(|i| i.saturating_sub(1))
            .unwrap_or(0);
        self.table_state.select(Some(prev));
    }

    // ── Rendering ────────────────────────────────────────────────────────────

    /// Immediate-mode rendering: called every frame, describes the full UI.
    /// Ratatui does NOT keep a widget tree — just re-draw everything each tick.
    fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // header
                Constraint::Min(0),    // body
                Constraint::Length(3), // footer / status bar
            ])
            .split(area);

        // Header
        frame.render_widget(
            Block::default()
                .title("  CMK Cockpit  ")
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Cyan)),
            chunks[0],
        );

        // Body — delegate to the current screen
        match &self.screen {
            Screen::PackageList => self.render_package_list(frame, chunks[1]),
            Screen::Configure { .. } => self.render_configure(frame, chunks[1]),
            Screen::Installing { .. } => self.render_installing(frame, chunks[1]),
        }

        // Footer
        let hint = match &self.screen {
            Screen::PackageList => "  ↑/k  ↓/j  navigate    Enter  select    q  quit  ",
            Screen::Configure { .. } => "  Type site name    Enter  confirm    Esc  back  ",
            Screen::Installing { .. } => "  Installation in progress…    q  abort  ",
        };
        frame.render_widget(
            Block::default()
                .title(hint)
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::DarkGray)),
            chunks[2],
        );
    }

    fn render_package_list(&mut self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let header = Row::new(["Version", "Date", "Edition"])
            .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            .height(1);

        // Rust concept: `iter().map().collect()` is the idiomatic way to
        // transform a Vec. The type annotation `Vec<Row>` helps the compiler
        // infer what `.collect()` should build.
        let rows: Vec<Row> = self
            .packages
            .iter()
            .map(|p| {
                Row::new([
                    p.base_version.clone(),
                    p.release_date.clone(),
                    p.edition.display_name().to_string(),
                ])
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Percentage(30),
                Constraint::Percentage(35),
                Constraint::Percentage(35),
            ],
        )
        .header(header)
        .block(Block::default().title(" Available Packages ").borders(Borders::ALL))
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

        frame.render_stateful_widget(table, area, &mut self.table_state);
    }

    fn render_configure(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let Screen::Configure { selected, site_input } = &self.screen else {
            return;
        };

        let inner = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4), // selected package summary
                Constraint::Length(3), // site name input
                Constraint::Min(0),    // padding
            ])
            .split(area);

        // Summary of the selected package
        let summary = Paragraph::new(vec![
            Line::from(vec![
                Span::raw("  Version : "),
                Span::styled(&selected.base_version, Style::default().fg(Color::Cyan)),
            ]),
            Line::from(vec![
                Span::raw("  Date    : "),
                Span::styled(&selected.release_date, Style::default().fg(Color::Cyan)),
            ]),
            Line::from(vec![
                Span::raw("  Edition : "),
                Span::styled(selected.edition.display_name(), Style::default().fg(Color::Cyan)),
            ]),
        ])
        .block(Block::default().title(" Selected Package ").borders(Borders::ALL));
        frame.render_widget(summary, inner[0]);

        // Site name text input
        let input = Paragraph::new(format!(" {site_input}█"))
            .block(Block::default().title(" Site Name ").borders(Borders::ALL))
            .style(Style::default().fg(Color::White));
        frame.render_widget(input, inner[1]);

        // Clear padding area
        frame.render_widget(Clear, inner[2]);
    }

    fn render_installing(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let Screen::Installing { config } = &self.screen else {
            return;
        };

        let text = vec![
            Line::from(""),
            Line::from(vec![
                Span::raw("  "),
                Span::styled("Running installation…", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from(format!("  cmk-dev-install {} -e {}", config.version, config.edition)),
            Line::from(format!("  cmk-dev-site <version>.{} -n {}", config.edition, config.site_name)),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "  NOTE: Installation runs in the background terminal. Check the shell output.",
                    Style::default().fg(Color::Yellow),
                ),
            ]),
        ];

        let para = Paragraph::new(text)
            .block(Block::default().title(" Installing ").borders(Borders::ALL));
        frame.render_widget(para, area);
    }
}
