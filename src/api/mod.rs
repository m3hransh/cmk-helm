// API module — fetches the available package list from the Checkmk download server.
//
// The server at https://download.checkmk.com/checkmk/ returns a plain HTML
// directory listing (Apache autoindex style), NOT JSON. We parse it with a
// regex that matches the `href` attributes of <a> tags, exactly mirroring
// what cmk-dev-site's VersionParser does in Python.
//
// Auth: HTTP Basic Auth, credentials read from ~/.cmk-credentials (user:pass).

use anyhow::{bail, Context, Result};
use regex::Regex;
use std::path::PathBuf;

// ── Constants ────────────────────────────────────────────────────────────────

/// Primary download server — public packages (cee, cre, cme, cce).
pub const CMK_DOWNLOAD_URL: &str = "https://download.checkmk.com/checkmk";

/// Internal build server — used for cloud / release-candidate builds.
pub const TSBUILD_URL: &str = "https://tstbuilds-artifacts.lan.tribe29.com";

/// Path to the credentials file that cmk-dev-site also reads.
fn credentials_path() -> PathBuf {
    // Rust concept: `~` is NOT expanded by the OS automatically in Rust.
    // We expand it manually using the HOME environment variable.
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
    PathBuf::from(home).join(".cmk-credentials")
}

// ── Data Types ───────────────────────────────────────────────────────────────

/// A single available package entry parsed from the server's HTML listing.
///
/// Example href that produces this: `2.4.0-2025.04.07.cee/`
///   base_version = "2.4.0"
///   release_date = "2025.04.07"   (dots, as on the server)
///   edition      = Edition::Cee
///
/// Rust concept: `#[derive(Debug, Clone)]` auto-generates trait impls.
/// `Debug` lets you print with `{:?}`. `Clone` lets you call `.clone()`
/// to make an owned copy — needed because Ratatui widgets consume their data.
#[derive(Debug, Clone)]
pub struct Package {
    pub base_version: String, // "2.4.0"
    pub release_date: String, // "2025.04.07"  (dots, server format)
    pub edition: Edition,
}

impl Package {
    /// Returns the version string in the format cmk-dev-install expects:
    /// `{base_version}-{YYYY-MM-DD}` (date with hyphens, not dots).
    ///
    /// Example: "2.4.0-2025-04-07"
    pub fn install_version_arg(&self) -> String {
        format!("{}-{}", self.base_version, self.release_date.replace('.', "-"))
    }

    /// Human-readable one-liner for the TUI table.
    pub fn display_version(&self) -> String {
        format!("{} ({})", self.base_version, self.release_date)
    }
}

// ── Edition ──────────────────────────────────────────────────────────────────

/// Rust concept: `enum` with named variants — much richer than C enums.
/// Each variant can optionally hold data; here they're all unit variants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edition {
    Cee, // Enterprise
    Cre, // Community (Raw / open source)
    Cme, // Managed Services Enterprise
    Cce, // Cloud
    Cse, // SaaS / Ultimate
    Pro, // "pro" branding introduced in 2.5+
}

impl Edition {
    /// Parse the suffix from a server href (e.g. ".cee", ".cre").
    pub fn from_suffix(s: &str) -> Option<Self> {
        match s {
            "cee" => Some(Self::Cee),
            "cre" => Some(Self::Cre),
            "cme" => Some(Self::Cme),
            "cce" => Some(Self::Cce),
            "cse" => Some(Self::Cse),
            "pro" => Some(Self::Pro),
            _ => None,
        }
    }

    /// The short code expected by `cmk-dev-install -e <edition>`.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Cee => "cee",
            Self::Cre => "cre",
            Self::Cme => "cme",
            Self::Cce => "cce",
            Self::Cse => "cse",
            Self::Pro => "pro",
        }
    }

    /// Display name for the TUI.
    pub fn display_name(&self) -> &str {
        match self {
            Self::Cee => "Enterprise",
            Self::Cre => "Community",
            Self::Cme => "Managed Services",
            Self::Cce => "Cloud",
            Self::Cse => "SaaS",
            Self::Pro => "Pro",
        }
    }
}

// ── Credentials ──────────────────────────────────────────────────────────────

/// Reads `~/.cmk-credentials` and returns `(username, password)`.
///
/// The file must contain exactly one line in the format `username:password`.
/// This is the same file and format that cmk-dev-site reads.
pub fn read_credentials() -> Result<(String, String)> {
    let path = credentials_path();
    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("Cannot read credentials from {}", path.display()))?;

    // Rust concept: `split_once` splits on the *first* occurrence only,
    // so passwords containing `:` are handled correctly.
    let (user, pass) = contents.trim().split_once(':').with_context(|| {
        format!(
            "Credentials file {} must be in `username:password` format",
            path.display()
        )
    })?;

    Ok((user.to_string(), pass.to_string()))
}

// ── HTML Parsing ─────────────────────────────────────────────────────────────

/// Parses the HTML directory listing returned by the Checkmk download server.
///
/// The server returns something like:
/// ```html
/// <a href="2.4.0-2025.04.07.cee/">2.4.0-2025.04.07.cee/</a>
/// <a href="2.3.0-2025.01.15.cre/">2.3.0-2025.01.15.cre/</a>
/// ```
///
/// The regex pattern mirrors cmk-dev-site's `VersionParser`:
///   group 1 = base version  (e.g. "2.4.0")
///   group 2 = release date  (e.g. "2025.04.07")
///   group 3 = edition       (e.g. "cee")
///
/// Rust concept: `Regex::new` can fail if the pattern is invalid — but since
/// this pattern is a compile-time constant string we use `unwrap()` here.
/// An alternative is the `once_cell` crate to make it a static.
fn parse_packages_from_html(html: &str) -> Vec<Package> {
    // Matches: `2.4.0-2025.04.07.cee` optionally followed by `/`
    let re = Regex::new(
        r#"href="(\d+\.\d+\.\d+)-(\d{4}\.\d{2}\.\d{2})\.([a-z]+)/?"#
    )
    .unwrap();

    re.captures_iter(html)
        .filter_map(|cap| {
            let base_version = cap[1].to_string();
            let release_date = cap[2].to_string();
            let edition_str = &cap[3];
            // Skip unknown edition codes rather than crashing.
            let edition = Edition::from_suffix(edition_str)?;
            Some(Package { base_version, release_date, edition })
        })
        .collect()
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Fetches available packages from the Checkmk download server.
///
/// Uses HTTP Basic Auth with credentials from `~/.cmk-credentials`.
/// Returns packages in listing order (newest first, as the server returns them).
///
/// # Errors
/// - Credentials file missing or malformed
/// - Network or HTTP error
/// - Server returns no recognisable package links
pub async fn fetch_packages(base_url: &str) -> Result<Vec<Package>> {
    let (user, password) = read_credentials()?;

    // Rust concept: `reqwest::Client` is reusable; creating one per call is
    // fine for a TUI but in a long-running service you'd share it.
    let client = reqwest::Client::new();

    let html = client
        .get(base_url)
        .basic_auth(&user, Some(&password))
        .send()
        .await
        .with_context(|| format!("Failed to reach {base_url}"))?
        .error_for_status()
        .context("Server returned an error status")?
        .text()
        .await
        .context("Failed to read response body")?;

    let packages = parse_packages_from_html(&html);

    if packages.is_empty() {
        bail!("No packages found in server response — check the URL or credentials");
    }

    Ok(packages)
}

// ── Unit Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Rust concept: `#[test]` marks a function as a unit test.
    // Run all tests with `cargo test`. No test framework needed — it's built in.
    #[test]
    fn parse_html_extracts_packages() {
        let html = r#"
            <a href="2.4.0-2025.04.07.cee/">2.4.0-2025.04.07.cee/</a>
            <a href="2.3.0-2025.01.15.cre/">2.3.0-2025.01.15.cre/</a>
            <a href="parent-of-page/">Parent</a>
        "#;

        let packages = parse_packages_from_html(html);
        assert_eq!(packages.len(), 2);
        assert_eq!(packages[0].base_version, "2.4.0");
        assert_eq!(packages[0].release_date, "2025.04.07");
        assert_eq!(packages[0].edition, Edition::Cee);
        assert_eq!(packages[1].edition, Edition::Cre);
    }

    #[test]
    fn install_version_arg_converts_dots_to_hyphens() {
        let pkg = Package {
            base_version: "2.4.0".into(),
            release_date: "2025.04.07".into(),
            edition: Edition::Cee,
        };
        assert_eq!(pkg.install_version_arg(), "2.4.0-2025-04-07");
    }
}
