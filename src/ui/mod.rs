// UI module — App state machine, event loop, and all Ratatui rendering.
//
// Screen flow:
//
//   VersionList  ──Enter──▶  EditionPicker  ──Enter──▶  Configure  ──Enter──▶  Installing
//        ▲                        │                          │
//        └─────────Esc────────────┴──────────Esc─────────────┘

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Row, Table, TableState},
};

use crate::api::{Edition, Version, VersionKind};
use crate::installer::InstallConfig;

// ── Screen State Machine ─────────────────────────────────────────────────────

#[derive(Debug)]
enum Screen {
    VersionList,
    EditionPicker {
        version: Version,
        list_state: ListState,
    },
    Configure {
        version: Version,
        edition: Edition,
        site_input: String,
    },
    Installing {
        config: InstallConfig,
    },
}

// ── App ───────────────────────────────────────────────────────────────────────

pub struct App {
    versions: Vec<Version>,
    table_state: TableState,
    screen: Screen,
    should_quit: bool,
}

impl App {
    pub fn new(versions: Vec<Version>) -> Self {
        Self {
            versions,
            table_state: TableState::default().with_selected(0),
            screen: Screen::VersionList,
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

    // ── Input Handling ───────────────────────────────────────────────────────

    fn handle_events(&mut self) -> Result<()> {
        if event::poll(std::time::Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    return Ok(());
                }
                // Rust concept: we need to match on `self.screen` but also
                // mutate `self` to transition between screens. We can't hold
                // an `&self.screen` borrow while calling `&mut self` methods,
                // so we use a temporary key variable to break the borrow.
                match key.code {
                    code => self.dispatch(code),
                }
            }
        }
        Ok(())
    }

    fn dispatch(&mut self, code: KeyCode) {
        match &self.screen {
            Screen::VersionList => self.on_version_list(code),
            Screen::EditionPicker { .. } => self.on_edition_picker(code),
            Screen::Configure { .. } => self.on_configure(code),
            Screen::Installing { .. } => self.on_installing(code),
        }
    }

    fn on_version_list(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Down | KeyCode::Char('j') => {
                let last = self.versions.len().saturating_sub(1);
                let next = self.table_state.selected().map(|i| (i + 1).min(last)).unwrap_or(0);
                self.table_state.select(Some(next));
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let prev = self.table_state.selected().map(|i| i.saturating_sub(1)).unwrap_or(0);
                self.table_state.select(Some(prev));
            }
            KeyCode::Enter => {
                if let Some(idx) = self.table_state.selected() {
                    let version = self.versions[idx].clone();
                    let mut list_state = ListState::default();
                    list_state.select(Some(0));
                    self.screen = Screen::EditionPicker { version, list_state };
                }
            }
            _ => {}
        }
    }

    fn on_edition_picker(&mut self, code: KeyCode) {
        let Screen::EditionPicker { version, list_state } = &mut self.screen else { return };
        let editions = version.available_editions();

        match code {
            KeyCode::Esc => {
                self.screen = Screen::VersionList;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let last = editions.len().saturating_sub(1);
                let next = list_state.selected().map(|i| (i + 1).min(last)).unwrap_or(0);
                list_state.select(Some(next));
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let prev = list_state.selected().map(|i| i.saturating_sub(1)).unwrap_or(0);
                list_state.select(Some(prev));
            }
            KeyCode::Enter => {
                // Clone what we need before transitioning (ends the borrow on self.screen).
                let edition = list_state
                    .selected()
                    .map(|i| editions[i].clone())
                    .unwrap_or_else(|| editions[0].clone());
                let version = version.clone();
                self.screen = Screen::Configure {
                    version,
                    edition,
                    site_input: String::new(),
                };
            }
            _ => {}
        }
    }

    fn on_configure(&mut self, code: KeyCode) {
        let Screen::Configure { site_input, version, edition } = &mut self.screen else { return };

        match code {
            KeyCode::Esc => {
                let version = version.clone();
                let mut list_state = ListState::default();
                list_state.select(Some(0));
                self.screen = Screen::EditionPicker { version, list_state };
            }
            KeyCode::Char(c) => site_input.push(c),
            KeyCode::Backspace => { site_input.pop(); }
            KeyCode::Enter if !site_input.is_empty() => {
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

    fn on_installing(&mut self, code: KeyCode) {
        if matches!(code, KeyCode::Char('q') | KeyCode::Esc) {
            self.should_quit = true;
        }
    }

    // ── Rendering ────────────────────────────────────────────────────────────

    fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(3)])
            .split(area);

        // Header
        frame.render_widget(
            Block::default()
                .title("  CMK Cockpit  ")
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Cyan)),
            chunks[0],
        );

        // Body
        match &self.screen {
            Screen::VersionList => self.render_version_list(frame, chunks[1]),
            Screen::EditionPicker { .. } => self.render_edition_picker(frame, chunks[1]),
            Screen::Configure { .. } => self.render_configure(frame, chunks[1]),
            Screen::Installing { .. } => self.render_installing(frame, chunks[1]),
        }

        // Footer hints
        let hint = match &self.screen {
            Screen::VersionList =>    "  ↑/k  ↓/j  navigate    Enter  select    q  quit  ",
            Screen::EditionPicker { .. } => "  ↑/k  ↓/j  choose edition    Enter  confirm    Esc  back  ",
            Screen::Configure { .. } => "  Type site name    Enter  start install    Esc  back  ",
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

    fn render_version_list(&mut self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let header = Row::new(["Base Version", "Type", "Detail"])
            .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));

        let rows: Vec<Row> = self.versions.iter().map(|v| {
            let type_style = match &v.kind {
                VersionKind::Daily { .. } => Style::default().fg(Color::Green),
                VersionKind::StablePatch { .. } => Style::default().fg(Color::Blue),
                VersionKind::Beta { .. } => Style::default().fg(Color::Magenta),
            };
            Row::new([
                v.base.clone(),
                v.kind_label().to_string(),
                v.detail(),
            ]).style(type_style)
        }).collect();

        let table = Table::new(
            rows,
            [Constraint::Percentage(30), Constraint::Percentage(20), Constraint::Percentage(50)],
        )
        .header(header)
        .block(Block::default().title(" Available Versions ").borders(Borders::ALL))
        .highlight_style(Style::default().bg(Color::Blue).fg(Color::White).add_modifier(Modifier::BOLD))
        .highlight_symbol("▶ ");

        frame.render_stateful_widget(table, area, &mut self.table_state);
    }

    fn render_edition_picker(&mut self, frame: &mut Frame, area: ratatui::layout::Rect) {
        // Rust concept: `if let ... else { return }` is the idiomatic way to
        // extract data from an enum variant in a method that may be called
        // even when the screen is something else (defensive guard).
        let Screen::EditionPicker { version, list_state } = &mut self.screen else { return };

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(area);

        // Selected version summary
        frame.render_widget(
            Paragraph::new(format!("  {} — {}  ({})", version.base, version.detail(), version.kind_label()))
                .block(Block::default().title(" Selected Version ").borders(Borders::ALL))
                .style(Style::default().fg(Color::Cyan)),
            chunks[0],
        );

        // Edition list
        let items: Vec<ListItem> = version
            .available_editions()
            .iter()
            .map(|e| ListItem::new(format!("  {}  ({})", e.display_name(), e.as_str())))
            .collect();

        let list = List::new(items)
            .block(Block::default().title(" Select Edition ").borders(Borders::ALL))
            .highlight_style(
                Style::default().bg(Color::Blue).fg(Color::White).add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▶ ");

        frame.render_stateful_widget(list, chunks[1], list_state);
    }

    fn render_configure(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let Screen::Configure { version, edition, site_input } = &self.screen else { return };

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(5), Constraint::Length(3), Constraint::Min(0)])
            .split(area);

        // Summary
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(vec![Span::raw("  Version : "), Span::styled(version.base.clone(), Style::default().fg(Color::Cyan))]),
                Line::from(vec![Span::raw("  Build   : "), Span::styled(version.detail(), Style::default().fg(Color::Cyan))]),
                Line::from(vec![Span::raw("  Edition : "), Span::styled(edition.display_name(), Style::default().fg(Color::Green))]),
            ])
            .block(Block::default().title(" Installation Plan ").borders(Borders::ALL)),
            chunks[0],
        );

        // Site name input
        frame.render_widget(
            Paragraph::new(format!(" {site_input}█"))
                .block(Block::default().title(" OMD Site Name ").borders(Borders::ALL))
                .style(Style::default().fg(Color::White)),
            chunks[1],
        );

        frame.render_widget(Clear, chunks[2]);
    }

    fn render_installing(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let Screen::Installing { config } = &self.screen else { return };

        let text = vec![
            Line::from(""),
            Line::from(vec![
                Span::raw("  "),
                Span::styled("Ready to install", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from(format!("  $ cmk-dev-install {} -e {}", config.version, config.edition)),
            Line::from(format!("  $ cmk-dev-site <omd-version>.{} -n {}", config.edition, config.site_name)),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "  TODO: wire up installer::install_and_create_site(config) here",
                    Style::default().fg(Color::Yellow),
                ),
            ]),
        ];

        frame.render_widget(
            Paragraph::new(text).block(Block::default().title(" Install ").borders(Borders::ALL)),
            area,
        );
    }
}
