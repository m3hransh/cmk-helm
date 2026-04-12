---
title: Credential Auth
tags: [feature]
status: completed
priority: 2
publish: true
aliases: [credentials, auth, .cmk-credentials, HTTP Basic Auth]
---

# Credential Auth

CMK Cockpit reads credentials from `~/.cmk-credentials` and uses them for HTTP Basic Auth against the Checkmk download server.

---

## Credentials File Format

```
username:password
```

One line, colon-separated. Same file and format as the `cmk-dev-site` Python toolchain. Example:

```
john.doe:mysecretpassword
```

---

## How It's Read

```rust
// src/api/mod.rs
fn read_credentials() -> anyhow::Result<(String, String)> {
    let home = std::env::var("HOME").context("HOME env var not set")?;
    let path = PathBuf::from(home).join(".cmk-credentials");

    let content = fs::read_to_string(&path)
        .with_context(|| format!("Could not read {}", path.display()))?;

    let (user, pass) = content
        .trim()
        .split_once(':')
        .context("Credentials file must contain 'username:password'")?;

    Ok((user.to_string(), pass.to_string()))
}
```

`split_once(':')` returns `Option<(&str, &str)>` — `None` if there's no colon. The `.context()` turns that into a descriptive error.

---

## How It's Used

```rust
// In fetch_versions():
let (username, password) = read_credentials()?;

let html = reqwest::Client::new()
    .get(base_url)
    .basic_auth(&username, Some(&password))  // sets Authorization: Basic header
    .send()
    .await?
    .text()
    .await?;
```

HTTP Basic Auth base64-encodes `username:password` into the `Authorization` header. Reqwest handles this via `.basic_auth()`.

---

## Security Notes

- Credentials are read fresh on every app start — they are never cached in memory between sessions
- The file should have `chmod 600` (owner-read only) — the app does not enforce this
- Credentials are in plaintext — this is the same model as the Python `cmk-dev-site` toolchain

---

## Metadata

**Tags:** feature
**Related:** [[Data Flow]], [[Reqwest HTTP Client]], [[Getting Started]]
