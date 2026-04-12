// Debug logging module — writes to /tmp/cmk-debug.log.
//
// Since the TUI owns the terminal (raw mode), println! corrupts the UI.
// This module writes timestamped lines to a file instead. Watch it with:
//
//   tail -f /tmp/cmk-debug.log
//
// Rust concept: `std::sync::OnceLock` ensures the file path is set once
// at startup and shared safely across threads. `std::fs::OpenOptions` with
// `.append(true)` lets multiple calls write without overwriting.

use std::io::Write;

const LOG_PATH: &str = "/tmp/cmk-debug.log";

/// Clears the log file at startup so each run starts fresh.
pub fn init() {
    let _ = std::fs::write(LOG_PATH, "");
}

/// Appends a timestamped line to the debug log.
pub fn log(msg: &str) {
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(LOG_PATH)
    {
        let _ = writeln!(f, "{msg}");
    }
}
