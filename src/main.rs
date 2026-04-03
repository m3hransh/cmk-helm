// CMK Cockpit — entry point
//
// Rust concept: `mod` declares a module. Rust looks for the implementation
// in either `src/<name>.rs` or `src/<name>/mod.rs`.
mod api;
mod installer;
mod ui;

use anyhow::{Context, Result};
use api::CMK_DOWNLOAD_URL;

// `#[tokio::main]` rewrites `main` into an async runtime bootstrap.
// Only the top-level and `api/` need to be async — the TUI event loop
// stays synchronous for predictable frame timing.
#[tokio::main]
async fn main() -> Result<()> {
    // Fetch packages BEFORE entering raw mode so that any error (missing
    // credentials, network failure) prints cleanly to the normal terminal
    // instead of corrupting the alternate screen buffer.
    let versions = api::fetch_versions(CMK_DOWNLOAD_URL)
        .await
        .context("Failed to fetch version list from server")?;

    // `ratatui::init()` enables raw mode and switches to the alternate
    // screen buffer. Everything after this point must call `ratatui::restore()`
    // before exiting, even on error — otherwise the shell is left broken.
    let terminal = ratatui::init();

    let result = ui::App::new(versions).run(terminal).await;

    // Restore terminal unconditionally — if the app panicked, the panic
    // hook fires first, but `restore()` here covers the normal error path.
    ratatui::restore();

    result
}
