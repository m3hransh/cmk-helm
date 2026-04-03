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
// Rust concept: `std::process::Command` lets us spawn child processes.
// It is synchronous — we use it here because the installs are long-running
// terminal commands that the user watches in real time.

use anyhow::{bail, Context, Result};
use std::process::Command;

// ── Install Config ───────────────────────────────────────────────────────────

/// Everything needed to drive an installation.
///
/// Rust concept: a struct is the idiomatic way to group related parameters
/// instead of passing many individual arguments to a function.
#[derive(Debug, Clone)]
pub struct InstallConfig {
    /// Version string in cmk-dev-install format: `{base}-{YYYY-MM-DD}`.
    /// Example: "2.4.0-2025-04-07"
    pub version: String,

    /// Edition code: "cee", "cre", "cme", "cce", "cse", "pro"
    pub edition: String,

    /// OMD site name to create after installation.
    /// Example: "v240", "mysite"
    pub site_name: String,
}

// ── Public Functions ─────────────────────────────────────────────────────────

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
        bail!(
            "cmk-dev-install exited with code {:?}",
            status.code()
        );
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
        bail!(
            "cmk-dev-site exited with code {:?}",
            status.code()
        );
    }

    Ok(())
}

/// Combined — install package and create site in one call.
///
/// Uses `cmk-dev-install-site` if available, otherwise falls back to
/// running `install_package` then `create_site` in sequence.
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
            bail!(
                "cmk-dev-install-site exited with code {:?}",
                status.code()
            );
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
    let stdout = std::str::from_utf8(&output.stdout)
        .context("omd output is not valid UTF-8")?;

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

    let stdout = std::str::from_utf8(&output.stdout)
        .context("omd sites output is not valid UTF-8")?;

    let sites = stdout
        .lines()
        .skip(1) // skip the "SITE  VERSION  COMMENTS" header
        .filter(|l| !l.trim().is_empty())
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

// ── Helper ───────────────────────────────────────────────────────────────────

/// Returns true if `name` can be found on PATH.
fn which_exists(name: &str) -> bool {
    Command::new("which")
        .arg(name)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
