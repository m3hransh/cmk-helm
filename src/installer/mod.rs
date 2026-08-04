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
use tokio::time::{sleep, Duration};

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

/// Returns the version omd uses as its default, if any.
///
/// Runs `omd versions` (without `-b`), which marks one line with "(default)":
///
/// ```text
/// 2.3.0-2026.07.10.cee
/// 2.4.0-2026.07.28.cre (default)
/// ```
///
/// The default backs the `/omd/versions/default` symlink and is what
/// `omd create` picks without `-V`, so it must never be pruned even when no
/// site uses it. Returns `None` if omd fails — callers treat that as "unknown"
/// and simply lose the protection rather than erroring out.
pub fn default_version() -> Option<String> {
    let output = Command::new("omd").args(["versions"]).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = std::str::from_utf8(&output.stdout).ok()?;

    stdout.lines().find_map(|line| {
        let line = line.trim();
        // `strip_suffix` returns Some(rest) only when the suffix matched —
        // an Option-returning "check and trim" in one step.
        line.strip_suffix("(default)")
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    })
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

/// Returns the installed versions that nothing is using — the prune candidates.
///
/// A version counts as "in use" when either
///   * some site's version string equals it, or
///   * it is omd's default version (`default_version`) — removing that breaks
///     `/omd/versions/default` and bare `omd create`.
///
/// Both lists come from omd and use the same full version format, so an exact
/// string comparison is enough:
///
///   `omd versions -b` → "2.6.0-2026.03.31.ultimate"
///   `omd sites`       → "2.6.0-2026.03.31.ultimate" (VERSION column)
///
/// Rust concept: the parameters are slices (`&[String]`) and an `Option<&str>`,
/// not owned values — the caller keeps ownership and we only borrow to read.
/// The return value is owned (`Vec<String>`) because it outlives the borrow.
pub fn unused_versions(
    installed: &[String],
    sites: &[InstalledSite],
    default_version: Option<&str>,
) -> Vec<String> {
    // A HashSet of borrowed `&str` gives O(1) membership tests without
    // allocating a copy of every site version.
    let mut in_use: std::collections::HashSet<&str> =
        sites.iter().map(|s| s.version.trim()).collect();
    if let Some(default) = default_version {
        in_use.insert(default.trim());
    }

    installed
        .iter()
        .filter(|v| !in_use.contains(v.trim()))
        .cloned()
        .collect()
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
    /// Full label shown in the log header line.
    pub label: String,
    /// Short label shown in the tab bar (≤ ~18 chars).
    pub short_label: String,
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

                // Cloud edition requires mock-auth to be running before site setup.
                if config.edition == "cloud" {
                    ensure_mock_auth(job_id, &tx).await;
                }

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

/// Removes a Checkmk version — mirrors `cmk-dev-install`'s `remove_package`:
///
///   1. `sudo rm -rf /omd/versions/{version}`  — removes the installation directory
///   2. `dpkg -l {pkg_name}`                    — checks if the deb is installed
///   3. `sudo apt-get purge -y {pkg_name}`      — removes the deb package
///
/// The package name is derived from the version string. The omd version
/// format is `{base}-{date}.{edition}` (daily) or `{base}p{n}.{edition}`
/// (stable). The deb package name is `check-mk-{edition_long}-{base_version}`.
pub fn spawn_delete_version(version: String, job_id: JobId, tx: mpsc::UnboundedSender<JobMessage>) {
    crate::debug::log(&format!(
        "spawn_delete_version: job_id={job_id} version={version}"
    ));

    tokio::spawn(async move {
        let success = delete_version(&version, job_id, &tx).await;
        let _ = tx.send(JobMessage::Finished(job_id, success));
    });
}

/// Removes several versions **sequentially** inside a single job.
///
/// Sequential rather than concurrent on purpose: each removal may run
/// `apt-get purge`, and apt holds an exclusive lock on its database — two
/// purges at once would fail with "could not get lock /var/lib/dpkg/lock".
///
/// The job succeeds only if every removal succeeded; a per-version summary is
/// streamed to the log so a partial failure is visible.
pub fn spawn_delete_versions(
    versions: Vec<String>,
    job_id: JobId,
    tx: mpsc::UnboundedSender<JobMessage>,
) {
    crate::debug::log(&format!(
        "spawn_delete_versions: job_id={job_id} count={} versions={}",
        versions.len(),
        versions.join(", ")
    ));

    tokio::spawn(async move {
        let total = versions.len();
        let mut failed: Vec<&String> = Vec::new();

        // Rust concept: `iter().enumerate()` yields (index, &item) pairs —
        // the borrow keeps `versions` alive for the whole loop, which is why
        // `failed` can hold references into it.
        for (i, version) in versions.iter().enumerate() {
            let _ = tx.send(JobMessage::Output(
                job_id,
                format!("── [{}/{total}] {version} ──", i + 1),
            ));
            if !delete_version(version, job_id, &tx).await {
                failed.push(version);
            }
        }

        let _ = tx.send(JobMessage::Output(
            job_id,
            format!("removed {}/{total} unused versions", total - failed.len()),
        ));
        for version in &failed {
            let _ = tx.send(JobMessage::Output(job_id, format!("  failed: {version}")));
        }

        let _ = tx.send(JobMessage::Finished(job_id, failed.is_empty()));
    });
}

/// Removes one version. Returns true on success.
///
/// Shared by `spawn_delete_version` (single) and `spawn_delete_versions`
/// (batch) — neither sends `Finished`, that is the caller's job so a batch
/// reports one result for the whole run.
async fn delete_version(
    version: &str,
    job_id: JobId,
    tx: &mpsc::UnboundedSender<JobMessage>,
) -> bool {
    let install_path = format!("/omd/versions/{version}");

    // Step 1: remove the installation directory.
    let _ = tx.send(JobMessage::Output(
        job_id,
        format!("→ sudo rm -rf {install_path}"),
    ));
    let ok = run_streaming("sudo", &["rm", "-rf", &install_path], job_id, tx)
        .await
        .unwrap_or(false);
    if !ok {
        return false;
    }

    // Step 2: derive the deb package name and purge it if installed.
    // omd version format: "2.6.0-2026.04.10.ultimate" or "2.4.0p24.cee"
    // The edition is the suffix after the last dot.
    if let Some(pkg_name) = omd_version_to_pkg_name(version) {
        // Check if the deb is installed before trying to purge.
        let _ = tx.send(JobMessage::Output(job_id, format!("→ dpkg -l {pkg_name}")));
        let installed = run_streaming("dpkg", &["-l", &pkg_name], job_id, tx)
            .await
            .unwrap_or(false);

        if installed {
            let _ = tx.send(JobMessage::Output(
                job_id,
                format!("→ sudo apt-get purge -y {pkg_name}"),
            ));
            let _ = run_streaming("sudo", &["apt-get", "purge", "-y", &pkg_name], job_id, tx).await;
        } else {
            let _ = tx.send(JobMessage::Output(
                job_id,
                "  (deb package not found, skipping purge)".to_string(),
            ));
        }
    }

    true
}

/// Converts an omd version string to an apt package name.
///
/// Examples:
///   "2.6.0-2026.04.10.ultimate"  →  "check-mk-ultimate-2.6.0-2026.04.10"
///   "2.4.0p24.cee"               →  "check-mk-enterprise-2.4.0p24"
///   "2.5.0b3.cre"                →  "check-mk-raw-2.5.0b3"
fn omd_version_to_pkg_name(omd_version: &str) -> Option<String> {
    // The edition is the last dot-separated segment.
    let dot_pos = omd_version.rfind('.')?;
    let base = &omd_version[..dot_pos];
    let edition_short = &omd_version[dot_pos + 1..];

    // v2.5+ uses new edition names (cloud, ultimate, …), v2.4- uses short
    // codes (cce, cee, …). The deb package name matches the edition exactly
    // — they're different packages, not aliases.
    let edition_long = match edition_short {
        "cre" => "raw",
        "cee" => "enterprise",
        "cce" => "cce",
        "cme" => "managed",
        "community" => "raw",
        "pro" => "enterprise",
        "cloud" => "cloud",
        "ultimate" => "ultimate",
        "ultimatemt" => "saas",
        other => other, // fallback: use as-is
    };

    Some(format!("check-mk-{edition_long}-{base}"))
}

/// Spawns `sudo omd rm {site_name}` to remove an OMD site.
/// Requires root — omd manages system sites.
pub fn spawn_delete_site(site_name: String, job_id: JobId, tx: mpsc::UnboundedSender<JobMessage>) {
    crate::debug::log(&format!(
        "spawn_delete_site: job_id={job_id} site={site_name}"
    ));

    tokio::spawn(async move {
        let _ = tx.send(JobMessage::Output(
            job_id,
            format!("→ sudo omd rm {site_name}"),
        ));

        let success = run_streaming(
            "sudo",
            &["omd", "-f", "rm", "--kill", "--apache-reload", &site_name],
            job_id,
            &tx,
        )
        .await
        .unwrap_or(false);

        let _ = tx.send(JobMessage::Finished(job_id, success));
    });
}

/// Spawns `cmk-dev-site {omd_version} -n {site_name}` to create a site
/// from an already-installed version.
pub fn spawn_create_site(
    omd_version: String,
    site_name: String,
    job_id: JobId,
    tx: mpsc::UnboundedSender<JobMessage>,
) {
    crate::debug::log(&format!(
        "spawn_create_site: job_id={job_id} omd_version={omd_version} site={site_name}"
    ));

    tokio::spawn(async move {
        // Cloud edition requires mock-auth to be running before cmk-dev-site.
        if omd_version.ends_with(".cloud") {
            ensure_mock_auth(job_id, &tx).await;
        }

        let _ = tx.send(JobMessage::Output(
            job_id,
            format!("→ cmk-dev-site {omd_version} -n {site_name}"),
        ));

        let success = run_streaming(
            "cmk-dev-site",
            &[&omd_version, "-n", &site_name],
            job_id,
            &tx,
        )
        .await
        .unwrap_or(false);

        let _ = tx.send(JobMessage::Finished(job_id, success));
    });
}

/// Ensures `cmk-dev-site-mock-auth` is running before cloud site setup.
///
/// Cloud edition uses a mock authentication service that must be running
/// before `cmk-dev-site` creates the site. This function:
///   1. Checks if it is already running with `pgrep -f`.
///   2. If not, spawns it detached in the background.
///   3. Waits briefly so it can bind its port before `cmk-dev-site` proceeds.
///
/// The function is intentionally non-fatal — if it fails to start the process
/// a warning is emitted and the caller continues (cmk-dev-site will fail with
/// a clear error if mock-auth is truly required).
async fn ensure_mock_auth(job_id: JobId, tx: &mpsc::UnboundedSender<JobMessage>) {
    let already_running = tokio::process::Command::new("pgrep")
        .args(["-f", "cmk-dev-site-mock-auth"])
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false);

    if already_running {
        crate::debug::log("ensure_mock_auth: already running");
        let _ = tx.send(JobMessage::Output(
            job_id,
            "  (cmk-dev-site-mock-auth already running)".to_string(),
        ));
        return;
    }

    let _ = tx.send(JobMessage::Output(
        job_id,
        "→ cmk-dev-site-mock-auth &".to_string(),
    ));

    // Spawn detached: we intentionally do not await or stream the child's
    // output — mock-auth is a long-running server, not a one-shot command.
    match tokio::process::Command::new("cmk-dev-site-mock-auth")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(_child) => {
            crate::debug::log("ensure_mock_auth: started, waiting 2 s for bind");
            // Give the server time to bind its port before cmk-dev-site connects.
            sleep(Duration::from_secs(2)).await;
            let _ = tx.send(JobMessage::Output(
                job_id,
                "  (cmk-dev-site-mock-auth started)".to_string(),
            ));
        }
        Err(e) => {
            crate::debug::log(&format!("ensure_mock_auth: failed to start: {e}"));
            let _ = tx.send(JobMessage::Output(
                job_id,
                format!("  warning: failed to start cmk-dev-site-mock-auth: {e}"),
            ));
        }
    }
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

/// Caches sudo credentials before the TUI enters raw mode.
///
/// `sudo -n true` checks if credentials are already cached (non-interactive).
/// If not, `sudo -v` prompts for the password in the normal terminal.
/// This must be called before `ratatui::init()` — once in raw mode, sudo
/// can't display its password prompt.
pub fn ensure_sudo() {
    // Check if sudo is already cached — `-n` means non-interactive.
    let cached = Command::new("sudo")
        .args(["-n", "true"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !cached {
        println!("This app needs sudo for install/delete operations.");
        // `-v` validates and caches the credential, prompting if needed.
        let _ = Command::new("sudo").arg("-v").status();
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────
//
// Rust concept: `#[cfg(test)]` compiles this module only for `cargo test`, so
// test-only code never lands in the release binary. `use super::*` pulls in
// everything from the parent module, including private items.
#[cfg(test)]
mod tests {
    use super::*;

    fn site(name: &str, version: &str) -> InstalledSite {
        InstalledSite {
            name: name.to_string(),
            version: version.to_string(),
            is_default: false,
        }
    }

    #[test]
    fn unused_versions_excludes_versions_backing_a_site() {
        let installed = vec![
            "2.6.0-2026.03.31.ultimate".to_string(),
            "2.4.0p24.cee".to_string(),
            "2.5.0b3.cre".to_string(),
        ];
        let sites = vec![
            site("test", "2.6.0-2026.03.31.ultimate"),
            site("v240", "2.4.0p24.cee"),
        ];

        assert_eq!(
            unused_versions(&installed, &sites, None),
            vec!["2.5.0b3.cre".to_string()]
        );
    }

    #[test]
    fn unused_versions_returns_everything_when_no_sites_exist() {
        let installed = vec!["2.4.0p24.cee".to_string(), "2.5.0b3.cre".to_string()];
        assert_eq!(unused_versions(&installed, &[], None), installed);
    }

    #[test]
    fn unused_versions_is_empty_when_every_version_is_in_use() {
        let installed = vec!["2.4.0p24.cee".to_string()];
        // Two sites can share one version — it is still in use.
        let sites = vec![site("a", "2.4.0p24.cee"), site("b", "2.4.0p24.cee")];
        assert!(unused_versions(&installed, &sites, None).is_empty());
    }

    #[test]
    fn unused_versions_ignores_surrounding_whitespace() {
        let installed = vec!["  2.4.0p24.cee  ".to_string()];
        let sites = vec![site("a", "2.4.0p24.cee")];
        assert!(unused_versions(&installed, &sites, None).is_empty());
    }

    #[test]
    fn unused_versions_protects_the_omd_default_version() {
        let installed = vec!["2.4.0p24.cee".to_string(), "2.5.0b3.cre".to_string()];
        // No site uses either version, but the default must survive a prune.
        assert_eq!(
            unused_versions(&installed, &[], Some("2.4.0p24.cee")),
            vec!["2.5.0b3.cre".to_string()]
        );
    }

    #[test]
    fn omd_version_to_pkg_name_maps_editions() {
        assert_eq!(
            omd_version_to_pkg_name("2.6.0-2026.04.10.ultimate").as_deref(),
            Some("check-mk-ultimate-2.6.0-2026.04.10")
        );
        assert_eq!(
            omd_version_to_pkg_name("2.4.0p24.cee").as_deref(),
            Some("check-mk-enterprise-2.4.0p24")
        );
        assert_eq!(
            omd_version_to_pkg_name("2.5.0b3.cre").as_deref(),
            Some("check-mk-raw-2.5.0b3")
        );
        // No dot → no edition suffix → cannot derive a package name.
        assert_eq!(omd_version_to_pkg_name("nonsense"), None);
    }
}
