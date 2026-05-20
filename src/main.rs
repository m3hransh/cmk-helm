mod api;
mod debug;
mod installer;
mod ui;

use anyhow::{Context, Result};
use api::CMK_DOWNLOAD_URL;
use tokio::sync::oneshot;

#[tokio::main]
async fn main() -> Result<()> {
    debug::init();
    debug::log("cmk-helm starting");

    // sudo prompt must happen before ratatui::init() takes over the terminal,
    // because raw mode would hide sudo's password prompt.
    installer::ensure_sudo();

    // Spawn the HTTP fetch as a background task so the TUI can display the
    // animated splash screen immediately while waiting for the network.
    //
    // Rust concept: `oneshot::channel` creates a single-use send/receive pair.
    // The sender is moved into the async task; the receiver is handed to the
    // App and polled each frame with `try_recv()` until the data arrives.
    let (tx, rx) = oneshot::channel::<Result<ui::LoadResult>>();
    tokio::spawn(async move {
        let result = async {
            let version_groups = api::fetch_versions(CMK_DOWNLOAD_URL)
                .await
                .context("Failed to fetch version list from server")?;
            debug::log(&format!("fetched {} version groups", version_groups.len()));
            let installed_versions = installer::list_installed_versions().unwrap_or_default();
            let installed_sites = installer::list_installed_sites().unwrap_or_default();
            Ok(ui::LoadResult { version_groups, installed_versions, installed_sites })
        }
        .await;
        let _ = tx.send(result);
    });

    let terminal = ratatui::init();
    let result = ui::App::new_loading(rx).run(terminal).await;
    ratatui::restore();

    result
}
