mod api;
mod debug;
mod installer;
mod ui;

use anyhow::{Context, Result};
use api::CMK_DOWNLOAD_URL;

#[tokio::main]
async fn main() -> Result<()> {
    debug::init();
    debug::log("cmk-cockpit starting");

    // Fetch everything before entering raw mode so any error prints cleanly.

    let version_groups = api::fetch_versions(CMK_DOWNLOAD_URL)
        .await
        .context("Failed to fetch version list from server")?;

    debug::log(&format!("fetched {} version groups", version_groups.len()));
    for g in &version_groups {
        debug::log(&format!("  tab {}: {} versions", g.base, g.versions.len()));
    }

    // `omd` may not be available in all environments — treat absence as empty lists.
    // Rust concept: `.unwrap_or_default()` on a Result<Vec<_>> gives an empty Vec
    // instead of propagating the error, so a missing `omd` binary is non-fatal.
    let installed_versions = installer::list_installed_versions().unwrap_or_default();
    let installed_sites = installer::list_installed_sites().unwrap_or_default();

    let terminal = ratatui::init();
    let result = ui::App::new(version_groups, installed_versions, installed_sites)
        .run(terminal)
        .await;
    ratatui::restore();

    result
}
