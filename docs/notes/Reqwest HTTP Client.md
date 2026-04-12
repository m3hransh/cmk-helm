---
title: Reqwest HTTP Client
tags: [tool]
status: completed
priority: 2
publish: true
aliases: [reqwest, HTTP client]
---

# Reqwest

Reqwest is the HTTP client library we use to fetch version listings from the Checkmk download server. It is the most popular async HTTP client in the Rust ecosystem.

---

## Configuration in Cargo.toml

```toml
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls"] }
```

Key choices:
- `default-features = false` — opt out of the default OpenSSL dependency
- `features = ["rustls-tls"]` — use rustls (pure-Rust TLS) instead of OpenSSL

**Why rustls over OpenSSL?**
- No OpenSSL runtime dependency — the binary is self-contained
- Avoids "wrong OpenSSL version" errors on deployment machines
- OpenSSL headers are still needed at *build time* (the Nix build provides `openssl.dev` for this), but not at runtime

---

## How We Use It

```rust
// src/api/mod.rs
async fn fetch_versions(base_url: &str) -> anyhow::Result<Vec<VersionGroup>> {
    let (username, password) = read_credentials()?;

    let client = reqwest::Client::new();
    let html = client
        .get(base_url)
        .basic_auth(&username, Some(&password))  // HTTP Basic Auth
        .send()
        .await
        .with_context(|| format!("Failed to reach {base_url}"))?
        .text()
        .await
        .with_context(|| "Failed to read response body")?;

    parse_versions_from_html(&html)
}
```

The `Client` is created per-call here. For production use you'd share a single `Client` instance (it maintains a connection pool internally), but for a startup-only fetch it doesn't matter.

---

## HTTP Basic Auth

The download server uses HTTP Basic Auth. Reqwest handles this cleanly with `.basic_auth(username, Some(password))`:

```rust
client.get(url)
    .basic_auth("mehran", Some("s3cr3t"))
    .send()
    .await?;
```

This sets the `Authorization: Basic <base64>` header automatically.

---

## Why Async?

Reqwest is async-only — `.send()` returns a future you must `.await`. This is why we need [[Tokio Async Runtime]] even though we only make one HTTP request.

---

## Metadata

**Tags:** tool
**Reference:** https://crates.io/crates/reqwest
**Related:** [[Tokio Async Runtime]], [[Rust Async Await]], [[Data Flow]], [[Credential Auth]]
