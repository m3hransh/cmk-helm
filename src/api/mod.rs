// API module — fetches the version list from the Checkmk download server.
//
// The server at https://download.checkmk.com/checkmk/ returns an Apache
// autoindex HTML page listing version directories. Two directory formats exist:
//
//   Daily builds:    2.5.0-2026.04.03/   (base-YYYY.MM.DD)
//   Stable patches:  2.4.0p24/           (baseP<n>)
//   Beta releases:   2.5.0b1/            (baseB<n>)
//
// Inside each directory, .deb files are named:
//   check-mk-{edition}-{version}_{pkg_rev}.{distro}_{arch}.deb
// e.g.:
//   check-mk-pro-2.5.0-2026.04.03_0.noble_amd64.deb
//   check-mk-cloud-2.4.0p24_0.jammy_amd64.deb
//
// We only parse the root listing — one HTTP request is enough to show the
// version picker. Editions are a fixed list; if a combination is invalid,
// cmk-dev-install reports it cleanly.
//
// Auth: HTTP Basic Auth, credentials from ~/.cmk-credentials (user:password).

use anyhow::{bail, Context, Result};
use regex::Regex;
use std::path::PathBuf;

// ── Constants ────────────────────────────────────────────────────────────────

pub const CMK_DOWNLOAD_URL: &str = "https://download.checkmk.com/checkmk";

fn credentials_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
    PathBuf::from(home).join(".cmk-credentials")
}

// ── Data Types ───────────────────────────────────────────────────────────────

/// A version entry parsed from the server's root directory listing.
///
/// Rust concept: enum variants carrying data ("algebraic data types") let us
/// express that a Daily version has a `date` while a StablePatch has a `patch`
/// number — both in a single type without any null/Option juggling.
#[derive(Debug, Clone)]
pub struct Version {
    /// The base semver string, e.g. "2.5.0" or "2.4.0"
    pub base: String,
    pub kind: VersionKind,
}

#[derive(Debug, Clone)]
pub enum VersionKind {
    /// Directory like `2.5.0-2026.04.03/`
    Daily { date: String }, // "2026.04.03"
    /// Directory like `2.4.0p24/`
    StablePatch { patch: u32 },
    /// Directory like `2.5.0b1/`
    Beta { num: u32 },
}

impl Version {
    /// The directory name on the server (used to construct sub-URLs if needed).
    pub fn dir_name(&self) -> String {
        match &self.kind {
            VersionKind::Daily { date } => format!("{}-{}", self.base, date),
            VersionKind::StablePatch { patch } => format!("{}p{}", self.base, patch),
            VersionKind::Beta { num } => format!("{}b{}", self.base, num),
        }
    }

    /// The version argument for `cmk-dev-install`.
    /// Daily builds: dots in date become hyphens ("2026.04.03" → "2026-04-03")
    /// Stable/beta: identical to the directory name.
    pub fn install_arg(&self) -> String {
        match &self.kind {
            VersionKind::Daily { date } => {
                format!("{}-{}", self.base, date.replace('.', "-"))
            }
            VersionKind::StablePatch { patch } => format!("{}p{}", self.base, patch),
            VersionKind::Beta { num } => format!("{}b{}", self.base, num),
        }
    }

    /// Short kind label for the TUI table ("daily", "stable", "beta").
    pub fn kind_label(&self) -> &str {
        match &self.kind {
            VersionKind::Daily { .. } => "daily",
            VersionKind::StablePatch { .. } => "stable",
            VersionKind::Beta { .. } => "beta",
        }
    }

    /// The date/patch info column for the TUI table.
    pub fn detail(&self) -> String {
        match &self.kind {
            VersionKind::Daily { date } => date.clone(),
            VersionKind::StablePatch { patch } => format!("patch {}", patch),
            VersionKind::Beta { num } => format!("beta {}", num),
        }
    }

    /// Editions that make sense for this version's base branch.
    /// 2.5.0+ uses new edition names; 2.4.x and older use the legacy codes.
    pub fn available_editions(&self) -> &'static [Edition] {
        // Rust concept: `&'static [T]` is a reference to a slice that lives for
        // the entire program ("static lifetime"). Safe for compile-time constants.
        let major: u32 = self.base
            .split('.')
            .next()
            .and_then(|s| s.parse().ok())
            .unwrap_or(2);
        let minor: u32 = self.base
            .split('.')
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        if major > 2 || (major == 2 && minor >= 5) {
            // 2.5.0+: new edition names
            &[Edition::Community, Edition::Pro, Edition::Ultimate, Edition::Ultimatemt]
        } else {
            // ≤2.4.x: legacy edition codes
            &[Edition::Cre, Edition::Cee, Edition::Cloud, Edition::Cme]
        }
    }
}

// ── Edition ──────────────────────────────────────────────────────────────────

/// Checkmk edition.
///
/// The edition is NOT encoded in the root version directory — it lives inside
/// the directory as part of each .deb filename
/// (e.g. `check-mk-pro-2.5.0-2026.04.03_0.noble_amd64.deb`).
///
/// `cmk-dev-install -e <code>` accepts both old codes and new names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edition {
    // 2.5.0+ names (used in .deb filenames and -e flag)
    Community,   // check-mk-community → -e community
    Pro,         // check-mk-pro       → -e pro
    Ultimate,    // check-mk-ultimate  → -e ultimate
    Ultimatemt,  // check-mk-ultimatemt→ -e ultimatemt
    // ≤2.4.x legacy codes
    Cee,         // check-mk-enterprise→ -e cee
    Cre,         // check-mk-raw       → -e cre
    Cloud,       // check-mk-cloud     → -e cloud
    Cme,         // check-mk-managed   → -e cme
}

impl Edition {
    /// The code passed to `cmk-dev-install -e <code>`.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Community  => "community",
            Self::Pro        => "pro",
            Self::Ultimate   => "ultimate",
            Self::Ultimatemt => "ultimatemt",
            Self::Cee        => "cee",
            Self::Cre        => "cre",
            Self::Cloud      => "cloud",
            Self::Cme        => "cme",
        }
    }

    /// Human-readable label for the TUI.
    pub fn display_name(&self) -> &str {
        match self {
            Self::Community  => "Community (free)",
            Self::Pro        => "Pro",
            Self::Ultimate   => "Ultimate",
            Self::Ultimatemt => "Ultimate Multitenant",
            Self::Cee        => "Enterprise (cee)",
            Self::Cre        => "Community Raw (cre)",
            Self::Cloud      => "Cloud (cce)",
            Self::Cme        => "Managed Services (cme)",
        }
    }
}

// ── Credentials ──────────────────────────────────────────────────────────────

pub fn read_credentials() -> Result<(String, String)> {
    let path = credentials_path();
    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("Cannot read credentials from {}", path.display()))?;

    let (user, pass) = contents.trim().split_once(':').with_context(|| {
        format!(
            "Credentials file {} must be `username:password` on one line",
            path.display()
        )
    })?;

    Ok((user.to_string(), pass.to_string()))
}

// ── HTML Parsing ─────────────────────────────────────────────────────────────

/// Parses the root directory listing HTML and returns all version entries.
///
/// The server returns Apache autoindex HTML. We match two `href` patterns:
///
///   Daily:  `href="2.5.0-2026.04.03/"`
///   Stable: `href="2.4.0p24/"`
///   Beta:   `href="2.5.0b1/"`
///
/// Rust concept: `once_cell::sync::Lazy` (or `std::sync::LazyLock` in ≥1.80)
/// is used to compile a regex only once instead of on every call.
/// Here we use local `Regex::new().unwrap()` for simplicity — the patterns are
/// compile-time constants so `unwrap()` cannot fail.
fn parse_versions_from_html(html: &str) -> Vec<Version> {
    let daily_re = Regex::new(
        r#"href="(\d+\.\d+\.\d+)-(\d{4}\.\d{2}\.\d{2})/""#
    ).unwrap();
    let stable_re = Regex::new(
        r#"href="(\d+\.\d+\.\d+)p(\d+)/""#
    ).unwrap();
    let beta_re = Regex::new(
        r#"href="(\d+\.\d+\.\d+)b(\d+)/""#
    ).unwrap();

    let mut versions = Vec::new();

    for cap in daily_re.captures_iter(html) {
        versions.push(Version {
            base: cap[1].to_string(),
            kind: VersionKind::Daily { date: cap[2].to_string() },
        });
    }
    for cap in stable_re.captures_iter(html) {
        if let Ok(patch) = cap[2].parse::<u32>() {
            versions.push(Version {
                base: cap[1].to_string(),
                kind: VersionKind::StablePatch { patch },
            });
        }
    }
    for cap in beta_re.captures_iter(html) {
        if let Ok(num) = cap[2].parse::<u32>() {
            versions.push(Version {
                base: cap[1].to_string(),
                kind: VersionKind::Beta { num },
            });
        }
    }

    versions
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Fetches available versions from the Checkmk download server.
///
/// Returns versions in the order the server lists them (oldest first by default;
/// the caller may reverse or sort as needed).
pub async fn fetch_versions(base_url: &str) -> Result<Vec<Version>> {
    let (user, password) = read_credentials()?;

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

    let mut versions = parse_versions_from_html(&html);

    if versions.is_empty() {
        bail!("No versions found in server response — check the URL or credentials");
    }

    // Reverse so newest entries appear first in the TUI.
    // The server lists oldest first (Apache autoindex default).
    versions.reverse();

    Ok(versions)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Unit tests ────────────────────────────────────────────────────────────

    #[test]
    fn parse_daily_version() {
        let html = r#"<a href="2.5.0-2026.04.03/">2.5.0-2026.04.03/</a>"#;
        let versions = parse_versions_from_html(html);
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].base, "2.5.0");
        assert!(matches!(
            &versions[0].kind,
            VersionKind::Daily { date } if date == "2026.04.03"
        ));
    }

    #[test]
    fn parse_stable_patch_version() {
        let html = r#"<a href="2.4.0p24/">2.4.0p24/</a>"#;
        let versions = parse_versions_from_html(html);
        assert_eq!(versions.len(), 1);
        assert!(matches!(
            &versions[0].kind,
            VersionKind::StablePatch { patch: 24 }
        ));
    }

    #[test]
    fn parse_beta_version() {
        let html = r#"<a href="2.5.0b2/">2.5.0b2/</a>"#;
        let versions = parse_versions_from_html(html);
        assert_eq!(versions.len(), 1);
        assert!(matches!(&versions[0].kind, VersionKind::Beta { num: 2 }));
    }

    #[test]
    fn parse_mixed_listing() {
        let html = r#"
            <a href="/">Parent Directory</a>
            <a href="2.4.0p24/">2.4.0p24/</a>
            <a href="2.5.0-2026.04.03/">2.5.0-2026.04.03/</a>
            <a href="2.5.0b1/">2.5.0b1/</a>
            <a href="?C=N;O=D">Name</a>
        "#;
        let versions = parse_versions_from_html(html);
        assert_eq!(versions.len(), 3);
    }

    #[test]
    fn install_arg_daily_uses_hyphens_in_date() {
        let v = Version {
            base: "2.5.0".into(),
            kind: VersionKind::Daily { date: "2026.04.03".into() },
        };
        assert_eq!(v.install_arg(), "2.5.0-2026-04-03");
    }

    #[test]
    fn install_arg_stable_unchanged() {
        let v = Version {
            base: "2.4.0".into(),
            kind: VersionKind::StablePatch { patch: 24 },
        };
        assert_eq!(v.install_arg(), "2.4.0p24");
    }

    #[test]
    fn available_editions_splits_on_version_25() {
        let v25 = Version { base: "2.5.0".into(), kind: VersionKind::Daily { date: "x".into() } };
        let v24 = Version { base: "2.4.0".into(), kind: VersionKind::StablePatch { patch: 1 } };
        assert!(v25.available_editions().contains(&Edition::Pro));
        assert!(v24.available_editions().contains(&Edition::Cee));
        assert!(!v25.available_editions().contains(&Edition::Cee));
        assert!(!v24.available_editions().contains(&Edition::Pro));
    }

    #[test]
    fn edition_as_str_round_trips() {
        for ed in [
            Edition::Community, Edition::Pro, Edition::Ultimate, Edition::Ultimatemt,
            Edition::Cee, Edition::Cre, Edition::Cloud, Edition::Cme,
        ] {
            assert!(!ed.as_str().is_empty());
            assert!(!ed.display_name().is_empty());
        }
    }

    // ── Integration test ──────────────────────────────────────────────────────
    //
    // Run with: cargo test -- --ignored --nocapture

    #[tokio::test]
    #[ignore = "requires ~/.cmk-credentials and network access"]
    async fn fetch_versions_returns_real_data() {
        let versions = fetch_versions(CMK_DOWNLOAD_URL)
            .await
            .expect("fetch_versions failed");

        assert!(!versions.is_empty(), "Expected at least one version");

        // Verify structural integrity of every parsed entry.
        for v in &versions {
            assert!(!v.base.is_empty(), "base must not be empty");
            assert!(!v.dir_name().is_empty(), "dir_name must not be empty");
            assert!(!v.install_arg().is_empty(), "install_arg must not be empty");

            // Daily install_arg must not keep dots in the date part.
            if let VersionKind::Daily { .. } = &v.kind {
                let arg = v.install_arg();
                let date_part = arg.trim_start_matches(&v.base).trim_start_matches('-');
                assert!(
                    !date_part.contains('.'),
                    "Daily install_arg date must use hyphens, got: {arg}"
                );
            }
        }

        // Show a sample so we can eyeball the parsing.
        println!("\nNewest 10 versions from server:");
        for v in versions.iter().take(10) {
            println!(
                "  [{:6}] {} {}  →  cmk-dev-install {} -e ...",
                v.kind_label(),
                v.base,
                v.detail(),
                v.install_arg(),
            );
        }
        println!("  … {} total", versions.len());
    }
}
