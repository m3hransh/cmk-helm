// ── Shared types for the UI module ───────────────────────────────────────────
//
// These are split here so input.rs and render.rs can each reference them
// without importing from each other. All items are pub(crate) so sibling
// modules can access them via `use super::state::*`.

use ratatui::{layout::Rect, style::Color, widgets::ListState};

use crate::api::{Edition, VersionGroup};
use crate::installer::InstalledSite;

// ── Brand colours ─────────────────────────────────────────────────────────────
pub(crate) const CMK_BLUE: Color = Color::Rgb(0, 115, 197);
pub(crate) const CMK_GREEN: Color = Color::Rgb(144, 238, 144);

// ── Startup data bundle ───────────────────────────────────────────────────────
//
// Rust concept: grouping return values in a struct avoids a large tuple and
// lets the oneshot channel carry a single typed payload.
pub struct LoadResult {
    pub version_groups: Vec<VersionGroup>,
    pub installed_versions: Vec<String>,
    pub installed_sites: Vec<InstalledSite>,
}

// ── Pane Focus ────────────────────────────────────────────────────────────────
//
// Rust concept: using an enum to represent which pane has keyboard focus.
// This replaces the old `Screen` enum — instead of separate screens, we have
// persistent panes that the user can switch between with h/l.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActivePane {
    VersionBrowser,
    InstalledVersions,
    InstalledSites,
    LogPanel,
}

// ── Left Pane Sub-modes ───────────────────────────────────────────────────────
//
// The version browser pane has its own internal state machine. These are
// inline modes — the user stays in the left pane while picking an edition
// or typing a site name, unlike the old design where each was a full screen.
#[derive(Debug)]
pub(crate) enum LeftPaneMode {
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

// ── Right Pane Sub-modes ──────────────────────────────────────────────────────
//
// The installed versions and sites panes can enter inline modes for
// confirmation prompts and site name input, similar to the left pane's
// EditionPicker / Configure modes.
#[derive(Debug)]
pub(crate) enum RightPaneMode {
    /// Normal browsing — j/k navigates, d/s triggers actions.
    Browse,

    /// Confirming a destructive action. Shows "Are you sure? y/n".
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
pub(crate) enum DeleteTarget {
    Version(String),
    Site(String),
}

// ── Pane Geometry ─────────────────────────────────────────────────────────────
//
// Stored each frame so mouse clicks can be mapped to panes via hit-testing.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct PaneRects {
    pub(crate) version_browser: Rect,
    pub(crate) installed_versions: Rect,
    pub(crate) installed_sites: Rect,
    pub(crate) log_panel: Rect,
}
