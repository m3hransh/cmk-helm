// CMK Cockpit — entry point
//
// Rust concept: `mod` declares a module. Rust looks for the implementation
// in either `src/<name>.rs` or `src/<name>/mod.rs`.
mod api;
mod installer;
mod ui;

use anyhow::Result;

// `#[tokio::main]` rewrites `main` into an async runtime bootstrap.
// Only the top-level and anything in `api/` needs to be async — the
// TUI event loop itself stays synchronous for predictable frame timing.
#[tokio::main]
async fn main() -> Result<()> {
    // `ratatui::init()` enables raw mode (keypresses delivered immediately,
    // no line buffering) and switches to the alternate screen buffer so the
    // normal terminal contents are preserved and restored on exit.
    let terminal = ratatui::init();

    // Run the application. `?` propagates any error upward.
    // We capture the result first so we can always restore the terminal,
    // even if the app crashed — otherwise the user's shell stays broken.
    let result = ui::App::new().run(terminal).await;

    // `ratatui::restore()` disables raw mode and returns to the main screen.
    // Called unconditionally so a panic mid-session doesn't wreck the terminal.
    ratatui::restore();

    result
}
