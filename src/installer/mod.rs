// Installer module — wraps cmk-dev-install, cmk-dev-site, and omd.
//
// The installation chain works like this:
//
//   1. cmk-dev-install {version} -e {edition}
//      Downloads the .deb from download.checkmk.com, verifies its SHA256 hash,
//      installs it via `apt`, applies ACLs, and registers the version with omd.
//
//   2. cmk-dev-site {omd_version} -n {site_name}   (optional)
//      Creates an OMD site on top of the installed version, configures it,
//      and starts it. Internally calls `sudo omd -V {version} create`.
//
//   3. cmk-dev-install-site {version} {edition} -n {site_name}   (combined)
//      Convenience wrapper that runs steps 1 + 2 in sequence.
//
// Two flavours of each function:
//   - Sync (`std::process::Command`) — used by the old blocking path.
//   - Async streaming (`tokio::process::Command`) — spawns the subprocess,
//     pipes stdout/stderr line-by-line through an `mpsc` channel so the TUI
//     can render live output while the process runs.

use anyhow::{bail, Context, Result};
use std::process::Command;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;

// ── Install Config ───────────────────────────────────────────────────────────

/// Everything needed to drive an installation.
///
/// Rust concept: a struct is the idiomatic way to group related parameters
/// instead of passing many individual arguments to a function.
#[derive(Debug, Clone)]
pub struct InstallConfig {
    /// Version string in cmk-dev-install format: `{base}-{YYYY-MM-DD}`.
    /// Example: "2.4.0-2025-04-07" (daily), "2.4.0p24" (stable)
    pub version: String,

    /// OMD version string — the full version with dots in the date, used by
    /// `cmk-dev-site` and `omd`. Examples:
    ///   daily:  "2.6.0-2026.04.03" (dots in date, no edition suffix)
    ///   stable: "2.4.0p24"
    ///   beta:   "2.5.0b2"
    ///
    /// `cmk-dev-site` receives `{omd_version}.{edition}` as its first argument.
    pub omd_version: String,

    /// Edition code: "cee", "cre", "cme", "cce", "cse", "pro"
    pub edition: String,

    /// OMD site name to create after installation.
    /// Example: "v240", "mysite"
    pub site_name: String,
}

// ── Public Functions ─────────────────────────────────────────────────────────
//
// These are used in Phase 3 (install flow) — allow dead_code until then.

/// Step 1 — Download and install a Checkmk package via `cmk-dev-install`.
///
/// This runs:
///   `cmk-dev-install {version} -e {edition}`
///
/// The command handles download, SHA256 verification, apt install, and
/// `sudo omd setversion` automatically.
///
/// Rust concept: `Result<()>` — the `()` is the "unit type", Rust's equivalent
/// of `void`. We only care whether it succeeded, not about a return value.
#[allow(dead_code)]
pub fn install_package(config: &InstallConfig) -> Result<()> {
    println!(
        "→ Running: cmk-dev-install {} -e {}",
        config.version, config.edition
    );

    let status = Command::new("cmk-dev-install")
        .args([&config.version, "-e", &config.edition])
        .status()
        .context("Failed to launch cmk-dev-install — is it in PATH?")?;

    if !status.success() {
        bail!("cmk-dev-install exited with code {:?}", status.code());
    }

    Ok(())
}

/// Step 2 — Create and start an OMD site via `cmk-dev-site`.
///
/// This runs:
///   `cmk-dev-site {omd_version}.{edition} -n {site_name}`
///
/// The omd_version is the base version, e.g. "2.4.0".
/// cmk-dev-site handles `sudo omd create`, configuration, and site start.
#[allow(dead_code)]
pub fn create_site(config: &InstallConfig) -> Result<()> {
    // The first positional arg to cmk-dev-site is the OMD version string:
    // "{base_version}.{edition}" — e.g. "2.4.0.cee"
    // We reconstruct this from the install version ("2.4.0-2025-04-07" → "2.4.0").
    let base_version = config
        .version
        .split('-')
        .next()
        .context("Invalid version format — expected {base}-{date}")?;
    let omd_version = format!("{}.{}", base_version, config.edition);

    println!(
        "→ Running: cmk-dev-site {} -n {}",
        omd_version, config.site_name
    );

    let status = Command::new("cmk-dev-site")
        .args([&omd_version, "-n", &config.site_name])
        .status()
        .context("Failed to launch cmk-dev-site — is it in PATH?")?;

    if !status.success() {
        bail!("cmk-dev-site exited with code {:?}", status.code());
    }

    Ok(())
}

/// Combined — install package and create site in one call.
///
/// Uses `cmk-dev-install-site` if available, otherwise falls back to
/// running `install_package` then `create_site` in sequence.
#[allow(dead_code)]
pub fn install_and_create_site(config: &InstallConfig) -> Result<()> {
    // Try the combined tool first (it's the most ergonomic and prints a
    // preview of both commands before executing them).
    if which_exists("cmk-dev-install-site") {
        let base_version = config
            .version
            .split('-')
            .next()
            .context("Invalid version format")?;

        println!(
            "→ Running: cmk-dev-install-site {} {} -n {}",
            base_version, config.edition, config.site_name
        );

        let status = Command::new("cmk-dev-install-site")
            .args([base_version, &config.edition, "-n", &config.site_name])
            .status()
            .context("Failed to launch cmk-dev-install-site")?;

        if !status.success() {
            bail!("cmk-dev-install-site exited with code {:?}", status.code());
        }
    } else {
        // Fall back: run the two steps manually.
        install_package(config)?;
        create_site(config)?;
    }

    Ok(())
}

/// List all currently installed OMD versions.
///
/// Runs `omd versions -b` which prints one version per line, e.g.:
///   2.4.0.cee
///   2.3.0.cee
pub fn list_installed_versions() -> Result<Vec<String>> {
    let output = Command::new("omd")
        .args(["versions", "-b"])
        .output()
        .context("Failed to run omd — is it installed?")?;

    if !output.status.success() {
        bail!("omd versions failed");
    }

    // Rust concept: converting bytes → String can fail (invalid UTF-8),
    // so `from_utf8` returns a Result. We map the error with `?`.
    let stdout = std::str::from_utf8(&output.stdout).context("omd output is not valid UTF-8")?;

    let versions = stdout
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    Ok(versions)
}

// ── Installed Sites ───────────────────────────────────────────────────────────

/// A site registered with OMD.
#[derive(Debug, Clone)]
pub struct InstalledSite {
    /// Site name, e.g. "v240"
    pub name: String,
    /// Full OMD version string, e.g. "2.4.0p23.cee"
    pub version: String,
    /// True if this is the system default version.
    pub is_default: bool,
}

/// Lists all OMD sites by running `omd sites`.
///
/// `omd sites` output format:
/// ```text
/// SITE             VERSION                  COMMENTS
/// v240             2.4.0-2026.03.22.cce
/// test             2.6.0-2026.03.31.ultimate default version
/// ```
///
/// The first line is a header and is skipped. Each subsequent non-empty line
/// has: SITE (col 0), VERSION (col 1), optional COMMENTS (col 2+).
/// "default version" in the comments marks the system default site.
pub fn list_installed_sites() -> Result<Vec<InstalledSite>> {
    let output = Command::new("omd")
        .args(["sites"])
        .output()
        .context("Failed to run omd sites — is omd installed?")?;

    if !output.status.success() {
        bail!("omd sites exited with status {:?}", output.status.code());
    }

    let stdout =
        std::str::from_utf8(&output.stdout).context("omd sites output is not valid UTF-8")?;

    let sites = stdout
        .lines()
        // Skip header row if present. `omd sites` prints a "SITE VERSION COMMENTS"
        // header only when run interactively — detect it by checking for "SITE" at
        // the start. Without this guard, the first actual site gets skipped.
        .filter(|l| !l.trim().is_empty() && !l.starts_with("SITE"))
        .filter_map(|line| {
            // Rust concept: `split_whitespace()` splits on any whitespace and
            // returns an iterator. Collecting into a Vec lets us index by column.
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() < 2 {
                return None;
            }
            let comments = cols[2..].join(" ");
            Some(InstalledSite {
                name: cols[0].to_string(),
                version: cols[1].to_string(),
                is_default: comments.contains("default"),
            })
        })
        .collect();

    Ok(sites)
}

// ── Job System ───────────────────────────────────────────────────────────────
//
// Rust concept: we use an `mpsc` (multi-producer, single-consumer) channel to
// bridge async tasks and the synchronous TUI event loop. Each spawned job
// sends `JobMessage` values through a cloned `Sender`; the TUI drains the
// `Receiver` with `try_recv()` each frame — non-blocking, so the UI stays
// responsive while installs run in the background.

/// Unique identifier for a background job.
pub type JobId = usize;

/// Messages sent from a background job task to the UI.
#[derive(Debug)]
pub enum JobMessage {
    /// A line of stdout or stderr output from the subprocess.
    Output(JobId, String),
    /// The job finished. `success` is true if exit code was 0.
    Finished(JobId, bool),
}

/// Status of a background job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobStatus {
    Running,
    Done,
    Failed,
}

/// A background job tracked by the UI.
#[derive(Debug, Clone)]
pub struct Job {
    pub id: JobId,
    pub label: String,
    pub status: JobStatus,
    pub output: Vec<String>,
}

// ── Async Streaming Installer ────────────────────────────────────────────────
//
// Rust concept: `tokio::process::Command` is the async equivalent of
// `std::process::Command`. By piping stdout/stderr, we can read output
// line-by-line with `AsyncBufReadExt::lines()` inside a `tokio::spawn`
// task — each line is sent through the mpsc channel to the UI.

/// Spawns `cmk-dev-install {version} -e {edition}` as an async task.
/// Output lines are sent through `tx` tagged with `job_id`.
/// Sends a `Finished` message when the process exits.
pub fn spawn_install(config: InstallConfig, job_id: JobId, tx: mpsc::UnboundedSender<JobMessage>) {
    crate::debug::log(&format!(
        "spawn_install: job_id={job_id} version={} edition={} site={}",
        config.version, config.edition, config.site_name
    ));

    tokio::spawn(async move {
        let label = format!("cmk-dev-install {} -e {}", config.version, config.edition);
        let _ = tx.send(JobMessage::Output(job_id, format!("→ {label}")));

        crate::debug::log(&format!(
            "spawn_install[{job_id}]: running cmk-dev-install args=[{}, -e, {}]",
            config.version, config.edition
        ));

        match run_streaming(
            "cmk-dev-install",
            &[&config.version, "-e", &config.edition],
            job_id,
            &tx,
        )
        .await
        {
            Ok(true) => {
                // Step 1 succeeded — now create the site.
                // Use the omd_version (which has dots in dates), not config.version
                // (which has hyphens). e.g. "2.6.0-2026.04.03.ultimate"
                let omd_full = format!("{}.{}", config.omd_version, config.edition);
                let site_label = format!("cmk-dev-site {omd_full} -n {}", config.site_name);

                crate::debug::log(&format!(
                    "spawn_install[{job_id}]: install ok, running cmk-dev-site args=[{omd_full}, -n, {}]",
                    config.site_name
                ));

                let _ = tx.send(JobMessage::Output(job_id, format!("→ {site_label}")));

                let success = run_streaming(
                    "cmk-dev-site",
                    &[&omd_full, "-n", &config.site_name],
                    job_id,
                    &tx,
                )
                .await
                .unwrap_or(false);

                crate::debug::log(&format!(
                    "spawn_install[{job_id}]: cmk-dev-site finished success={success}"
                ));
                let _ = tx.send(JobMessage::Finished(job_id, success));
            }
            Ok(false) => {
                crate::debug::log(&format!("spawn_install[{job_id}]: cmk-dev-install failed"));
                let _ = tx.send(JobMessage::Finished(job_id, false));
            }
            Err(e) => {
                crate::debug::log(&format!("spawn_install[{job_id}]: error: {e:#}"));
                let _ = tx.send(JobMessage::Output(job_id, format!("error: {e:#}")));
                let _ = tx.send(JobMessage::Finished(job_id, false));
            }
        }
    });
}

/// Runs a command asynchronously, streaming stdout and stderr line-by-line
/// through the channel. Returns `Ok(true)` if exit code is 0.
///
/// Rust concept: `tokio::process::Command` with `Stdio::piped()` captures
/// the child's output without printing to our terminal (which is in raw mode).
/// `BufReader::new(stdout).lines()` yields an async stream of lines.
async fn run_streaming(
    program: &str,
    args: &[&str],
    job_id: JobId,
    tx: &mpsc::UnboundedSender<JobMessage>,
) -> Result<bool> {
    use std::process::Stdio;

    crate::debug::log(&format!(
        "run_streaming[{job_id}]: {program} {}",
        args.join(" ")
    ));

    let mut child = tokio::process::Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("Failed to launch {program} — is it in PATH?"))?;

    // Rust concept: `take()` moves the handle out of the Option, giving us
    // ownership. The child process still runs — we just detach the pipe handle
    // so we can read from it independently.
    //
    // We read stdout and stderr in a single task using `tokio::select!` to
    // avoid interleaving. Two separate tasks can send partial lines that
    // overlap in the channel, causing garbled output. `select!` reads from
    // whichever stream has data available, one line at a time.
    let mut stdout_lines = child.stdout.take().map(|s| BufReader::new(s).lines());
    let mut stderr_lines = child.stderr.take().map(|s| BufReader::new(s).lines());

    let tx_clone = tx.clone();
    let reader_task = tokio::spawn(async move {
        let mut stdout_done = stdout_lines.is_none();
        let mut stderr_done = stderr_lines.is_none();

        while !stdout_done || !stderr_done {
            tokio::select! {
                result = async {
                    match stdout_lines.as_mut() {
                        Some(lines) => lines.next_line().await,
                        None => std::future::pending().await,
                    }
                }, if !stdout_done => {
                    match result {
                        Ok(Some(line)) => {
                            for clean in clean_terminal_line(&line) {
                                let _ = tx_clone.send(JobMessage::Output(job_id, clean));
                            }
                        }
                        _ => stdout_done = true,
                    }
                }
                result = async {
                    match stderr_lines.as_mut() {
                        Some(lines) => lines.next_line().await,
                        None => std::future::pending().await,
                    }
                }, if !stderr_done => {
                    match result {
                        Ok(Some(line)) => {
                            for clean in clean_terminal_line(&line) {
                                let _ = tx_clone.send(JobMessage::Output(job_id, clean));
                            }
                        }
                        _ => stderr_done = true,
                    }
                }
            }
        }
    });

    // Wait for the reader to finish, then wait for the process.
    let _ = reader_task.await;
    let status = child
        .wait()
        .await
        .context("Failed to wait on child process")?;

    Ok(status.success())
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Simulates how a real terminal handles `\r` (carriage return).
///
/// Subprocesses like `cmk-dev-install` use `\r` to overwrite the current line
/// for progress updates. `BufReader::lines()` splits on `\n` only, so a single
/// "line" may contain embedded `\r`:
///
///   "Downloading... 10%\rDownloading... 50%\rDownloading... 100%"
///
/// A real terminal would show only "Downloading... 100%" (last segment after \r).
/// We split on `\r`, strip ANSI escape codes, and return each non-empty segment
/// as a separate line — the last one is what you'd see on screen, but we keep
/// intermediate ones too so the log panel shows the full history.
fn clean_terminal_line(raw: &str) -> Vec<String> {
    raw.split('\r')
        .map(|segment| strip_ansi(segment).trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Strips ANSI escape sequences (colors, cursor movement, etc.).
///
/// These show up as literal `\x1b[...m` characters in Ratatui's Paragraph
/// because it doesn't interpret them — it just renders the raw bytes.
fn strip_ansi(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip until we hit a letter (the terminator of an ANSI sequence).
            for esc_char in chars.by_ref() {
                if esc_char.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Returns true if `name` can be found on PATH.
#[allow(dead_code)]
fn which_exists(name: &str) -> bool {
    Command::new("which")
        .arg(name)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
