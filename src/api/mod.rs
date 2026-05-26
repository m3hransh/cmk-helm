// API module — fetches the version list from the Checkmk download server.
//
// Server structure (discovered by live testing):
//
//   Root URL: https://download.checkmk.com/checkmk/
//   Returns an Apache autoindex HTML page, one <tr> per version directory.
//
//   Directory formats:
//     2.5.0-2026.04.03/   — daily build  (base + YYYY.MM.DD)
//     2.4.0p24/           — stable patch (base + p<n>)
//     2.5.0b2/            — beta release (base + b<n>)
//
//   Each <tr> has the timestamp in the adjacent <td>:
//     <a href="2.5.0-2026.04.03/">…</a></td><td …>2026-04-03 13:50  </td>
//
// We parse both the version and the timestamp from the same row with one regex.
// Versions are then grouped by base version (e.g. "2.5.0") for tab navigation.

use anyhow::{bail, Context, Result};
use regex::Regex;
use std::{path::PathBuf, sync::LazyLock};

// ── Constants ─────────────────────────────────────────────────────────────────

pub const CMK_DOWNLOAD_URL: &str = "https://download.checkmk.com/checkmk";

fn credentials_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
    PathBuf::from(home).join(".cmk-credentials")
}

// ── Static Regexes ────────────────────────────────────────────────────────────
//
// Rust concept: `std::sync::LazyLock` (stable since 1.80) initialises a value
// the first time it is accessed and reuses it forever. Compiling a `Regex` is
// expensive — doing it inside a loop would be wasteful. LazyLock gives us
// "compile once, use many times" without any runtime overhead after the first call.

/// Matches a version directory row in the Apache autoindex HTML.
/// Captures: (1) version string without trailing slash, (2) timestamp.
/// Only matches strings that start with a semver base (digits.digits.digits).
static ROW_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"href="(\d+\.\d+\.\d+[^"/]*)/">[^<]*</a></td><td[^>]*>[ ]*(\d{4}-\d{2}-\d{2} \d{2}:\d{2})"#,
    )
    .unwrap()
});

static DAILY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d+\.\d+\.\d+)-(\d{4}\.\d{2}\.\d{2})$").unwrap());
static STABLE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d+\.\d+\.\d+)p(\d+)$").unwrap());
static BETA_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d+\.\d+\.\d+)b(\d+)$").unwrap());

// ── Data Types ────────────────────────────────────────────────────────────────

/// A single version entry from the root directory listing.
#[derive(Debug, Clone)]
pub struct Version {
    pub base: String, // "2.5.0"
    pub kind: VersionKind,
    /// Server modification time — "2026-04-03 13:50"
    pub timestamp: String,
}

#[derive(Debug, Clone)]
pub enum VersionKind {
    Daily { date: String }, // date = "2026.04.03" (dots, server format)
    StablePatch { patch: u32 },
    Beta { num: u32 },
}

impl Version {
    /// Version directory name on the server.
    #[allow(dead_code)]
    pub fn dir_name(&self) -> String {
        match &self.kind {
            VersionKind::Daily { date } => format!("{}-{}", self.base, date),
            VersionKind::StablePatch { patch } => format!("{}p{}", self.base, patch),
            VersionKind::Beta { num } => format!("{}b{}", self.base, num),
        }
    }

    /// Version argument for `cmk-dev-install`.
    /// Daily builds need dots → hyphens in the date part.
    pub fn install_arg(&self) -> String {
        let arg = match &self.kind {
            VersionKind::Daily { date } => {
                format!("{}-{}", self.base, date.replace('.', "-"))
            }
            VersionKind::StablePatch { patch } => format!("{}p{}", self.base, patch),
            VersionKind::Beta { num } => format!("{}b{}", self.base, num),
        };
        crate::debug::log(&format!(
            "install_arg: base={} kind={:?} → {arg}",
            self.base, self.kind
        ));
        arg
    }

    pub fn kind_label(&self) -> &str {
        match &self.kind {
            VersionKind::Daily { .. } => "daily",
            VersionKind::StablePatch { .. } => "stable",
            VersionKind::Beta { .. } => "beta",
        }
    }

    /// Short detail string for the table (date, patch number, or beta number).
    pub fn detail(&self) -> String {
        match &self.kind {
            VersionKind::Daily { date } => date.clone(),
            VersionKind::StablePatch { patch } => format!("p{patch}"),
            VersionKind::Beta { num } => format!("b{num}"),
        }
    }

    /// Editions that make sense for this version's base branch.
    pub fn available_editions(&self) -> &'static [Edition] {
        let minor: u32 = self
            .base
            .split('.')
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        if minor >= 5 {
            &[
                Edition::Community,
                Edition::Pro,
                Edition::Ultimate,
                Edition::Ultimatemt,
                Edition::Cloud,
            ]
        } else {
            &[Edition::Cee, Edition::Cre, Edition::Cloud, Edition::Cme]
        }
    }
}

// ── Version Groups ────────────────────────────────────────────────────────────

/// Versions sharing the same base version, e.g. all "2.5.0" builds.
/// Used to populate the tab bar.
#[derive(Debug, Clone)]
pub struct VersionGroup {
    pub base: String,
    pub versions: Vec<Version>, // newest first
}

/// Groups `versions` by base version string, sorted newest base first.
///
/// Rust concept: `.windows(1)` / sorting by a key — here we parse the semver
/// base into `(major, minor, patch)` tuples so "2.10.0" sorts after "2.9.0"
/// (lexicographic order would give the wrong result).
pub fn group_by_base(versions: Vec<Version>) -> Vec<VersionGroup> {
    let mut groups: Vec<VersionGroup> = Vec::new();

    // Versions arrive newest-first; preserve that order within each group.
    for v in versions {
        match groups.iter_mut().find(|g| g.base == v.base) {
            Some(g) => g.versions.push(v),
            None => groups.push(VersionGroup {
                base: v.base.clone(),
                versions: vec![v],
            }),
        }
    }

    // Sort groups so the newest base version is the first tab.
    groups.sort_by(|a, b| {
        parse_semver(&b.base).cmp(&parse_semver(&a.base)) // descending
    });

    groups
}

fn parse_semver(s: &str) -> (u32, u32, u32) {
    let mut parts = s.split('.').flat_map(|p| p.parse::<u32>().ok());
    (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    )
}

// ── Edition ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edition {
    // v2.5+ editions
    Community,
    Pro,
    Ultimate,
    Ultimatemt,
    // v2.4- editions
    Cee,
    Cre,
    Cce,
    Cme,
    // v2.5+ cloud (different package name from cce)
    Cloud,
}

impl Edition {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Community => "community",
            Self::Pro => "pro",
            Self::Ultimate => "ultimate",
            Self::Ultimatemt => "ultimatemt",
            Self::Cee => "cee",
            Self::Cre => "cre",
            Self::Cce => "cce",
            Self::Cme => "cme",
            Self::Cloud => "cloud",
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            Self::Community => "Community (free)",
            Self::Pro => "Pro",
            Self::Ultimate => "Ultimate",
            Self::Ultimatemt => "Ultimate Multitenant",
            Self::Cee => "Enterprise (cee)",
            Self::Cre => "Community Raw (cre)",
            Self::Cce => "Cloud (cce)",
            Self::Cme => "Managed Services (cme)",
            Self::Cloud => "Cloud",
        }
    }
}

// ── Credentials ───────────────────────────────────────────────────────────────

pub fn read_credentials() -> Result<(String, String)> {
    let path = credentials_path();
    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("Cannot read credentials from {}", path.display()))?;
    let (user, pass) = contents.trim().split_once(':').with_context(|| {
        format!(
            "Credentials file {} must be `username:password`",
            path.display()
        )
    })?;
    Ok((user.to_string(), pass.to_string()))
}

// ── HTML Parsing ──────────────────────────────────────────────────────────────

fn parse_version_from_str(s: &str, timestamp: String) -> Option<Version> {
    if let Some(cap) = DAILY_RE.captures(s) {
        return Some(Version {
            base: cap[1].to_string(),
            kind: VersionKind::Daily {
                date: cap[2].to_string(),
            },
            timestamp,
        });
    }
    if let Some(cap) = STABLE_RE.captures(s) {
        let patch = cap[2].parse().ok()?;
        return Some(Version {
            base: cap[1].to_string(),
            kind: VersionKind::StablePatch { patch },
            timestamp,
        });
    }
    if let Some(cap) = BETA_RE.captures(s) {
        let num = cap[2].parse().ok()?;
        return Some(Version {
            base: cap[1].to_string(),
            kind: VersionKind::Beta { num },
            timestamp,
        });
    }
    None // bare "2.1.0" style — old releases, skip
}

fn parse_versions_from_html(html: &str) -> Vec<Version> {
    ROW_RE
        .captures_iter(html)
        .filter_map(|cap| {
            let version_str = &cap[1];
            let timestamp = cap[2].trim().to_string();
            parse_version_from_str(version_str, timestamp)
        })
        .collect()
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Fetches available versions from the server.
/// Returns versions newest-first, grouped by base version.
pub async fn fetch_versions(base_url: &str) -> Result<Vec<VersionGroup>> {
    let (user, password) = read_credentials()?;

    let html = reqwest::Client::new()
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
        bail!("No versions found — check URL or credentials");
    }

    // Reverse so newest entries come first (Apache lists oldest first).
    versions.reverse();

    Ok(group_by_base(versions))
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_row(href: &str, ts: &str) -> String {
        format!(r#"<a href="{href}/">{href}/</a></td><td align="right">{ts}  </td>"#)
    }

    #[test]
    fn parse_daily_with_timestamp() {
        let html = make_row("2.5.0-2026.04.03", "2026-04-03 13:50");
        let versions = parse_versions_from_html(&html);
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].base, "2.5.0");
        assert_eq!(versions[0].timestamp, "2026-04-03 13:50");
        assert!(matches!(&versions[0].kind, VersionKind::Daily { date } if date == "2026.04.03"));
    }

    #[test]
    fn parse_stable_with_timestamp() {
        let html = make_row("2.4.0p24", "2026-03-16 10:49");
        let versions = parse_versions_from_html(&html);
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].timestamp, "2026-03-16 10:49");
        assert!(matches!(
            &versions[0].kind,
            VersionKind::StablePatch { patch: 24 }
        ));
    }

    #[test]
    fn parse_beta_with_timestamp() {
        let html = make_row("2.5.0b2", "2026-03-31 11:17");
        let versions = parse_versions_from_html(&html);
        assert_eq!(versions.len(), 1);
        assert!(matches!(&versions[0].kind, VersionKind::Beta { num: 2 }));
    }

    #[test]
    fn bare_versions_are_skipped() {
        // "2.1.0/" has no patch/daily/beta suffix — should be silently ignored.
        let html = make_row("2.1.0", "2023-05-05 11:06");
        assert!(parse_versions_from_html(&html).is_empty());
    }

    #[test]
    fn install_arg_daily_uses_hyphens() {
        let v = Version {
            base: "2.5.0".into(),
            kind: VersionKind::Daily {
                date: "2026.04.03".into(),
            },
            timestamp: "".into(),
        };
        assert_eq!(v.install_arg(), "2.5.0-2026-04-03");
    }

    #[test]
    fn install_arg_stable_unchanged() {
        let v = Version {
            base: "2.4.0".into(),
            kind: VersionKind::StablePatch { patch: 24 },
            timestamp: "".into(),
        };
        assert_eq!(v.install_arg(), "2.4.0p24");
    }

    #[test]
    fn group_by_base_sorts_newest_first() {
        let versions = vec![
            Version {
                base: "2.4.0".into(),
                kind: VersionKind::StablePatch { patch: 1 },
                timestamp: "".into(),
            },
            Version {
                base: "2.6.0".into(),
                kind: VersionKind::Daily { date: "x".into() },
                timestamp: "".into(),
            },
            Version {
                base: "2.5.0".into(),
                kind: VersionKind::Beta { num: 1 },
                timestamp: "".into(),
            },
        ];
        let groups = group_by_base(versions);
        assert_eq!(groups[0].base, "2.6.0");
        assert_eq!(groups[1].base, "2.5.0");
        assert_eq!(groups[2].base, "2.4.0");
    }

    #[test]
    fn available_editions_splits_on_minor_version() {
        let v25 = Version {
            base: "2.5.0".into(),
            kind: VersionKind::Daily { date: "x".into() },
            timestamp: "".into(),
        };
        let v24 = Version {
            base: "2.4.0".into(),
            kind: VersionKind::StablePatch { patch: 1 },
            timestamp: "".into(),
        };
        assert!(v25.available_editions().contains(&Edition::Pro));
        assert!(v24.available_editions().contains(&Edition::Cee));
        assert!(!v25.available_editions().contains(&Edition::Cee));
        assert!(!v24.available_editions().contains(&Edition::Pro));
    }

    // ── Integration test ──────────────────────────────────────────────────────
    // cargo test -- --ignored --nocapture

    #[tokio::test]
    #[ignore = "requires ~/.cmk-credentials and network access"]
    async fn fetch_versions_live() {
        let groups = fetch_versions(CMK_DOWNLOAD_URL)
            .await
            .expect("fetch failed");

        assert!(!groups.is_empty());

        // Newest base version should be first.
        let bases: Vec<&str> = groups.iter().map(|g| g.base.as_str()).collect();
        println!("\nBase version tabs: {:?}", bases);

        // Every version must have a timestamp and a valid install_arg.
        for g in &groups {
            for v in &g.versions {
                assert!(!v.timestamp.is_empty(), "timestamp must not be empty");
                assert!(
                    v.timestamp.contains('-'),
                    "timestamp must be YYYY-MM-DD ..."
                );
                if let VersionKind::Daily { .. } = &v.kind {
                    assert!(
                        !v.install_arg().contains('.') || v.install_arg().starts_with(&v.base),
                        "daily install_arg date part must use hyphens: {}",
                        v.install_arg()
                    );
                }
            }
            println!(
                "  {} ({} entries, newest: {})",
                g.base,
                g.versions.len(),
                g.versions
                    .first()
                    .map(|v| format!("{} @ {}", v.detail(), v.timestamp))
                    .unwrap_or_default()
            );
        }
    }
}
