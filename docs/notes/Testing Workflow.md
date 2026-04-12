---
title: Testing Workflow
tags: [workflow]
status: completed
priority: 2
publish: true
aliases: [testing, cargo test, unit tests]
---

# Testing Workflow

---

## Running Tests

```bash
cargo test                    # run all non-ignored tests
cargo test -- --ignored       # run only ignored (integration) tests
cargo test -- --include-ignored # run everything
cargo test api                # run tests in the api module only
cargo test parse_daily        # run tests matching "parse_daily"
```

---

## Where Tests Live

Tests live in `#[cfg(test)]` blocks at the bottom of each module file. This is standard Rust convention — tests are in the same file as the code they test.

```rust
// src/api/mod.rs — at the bottom:
#[cfg(test)]
mod tests {
    use super::*;  // bring the module's private items into scope

    #[test]
    fn test_parse_daily_build() {
        let html = r#"<a href="2.5.0-2026.04.03/">..."#;
        let versions = parse_versions_from_html(html).unwrap();
        assert_eq!(versions[0].base, "2.5.0");
        assert!(matches!(
            &versions[0].kind,
            VersionKind::Daily { date } if date == "2026.04.03"
        ));
    }
}
```

---

## Test Types

**Unit tests** — pure function tests, no I/O:
```rust
#[test]
fn test_parse_stable_patch() { ... }
```

**Async unit tests** — for async functions (rare, only in api/):
```rust
#[tokio::test]
async fn test_version_grouping() { ... }
```

**Ignored integration tests** — require network + credentials:
```rust
#[tokio::test]
#[ignore = "requires ~/.cmk-credentials and network access"]
async fn test_live_fetch() { ... }
```

See [[Rust Async Await]] for `#[tokio::test]`.

---

## What's Tested

`src/api/mod.rs` has the most tests:
- Parsing daily builds, stable patches, betas
- Edition detection from filenames
- Version sorting and grouping
- `to_install_arg()` format transformation

`src/ui/mod.rs` and `src/installer/mod.rs` have minimal tests currently. UI behaviour is tested manually.

---

## `matches!` in Tests

The `matches!` macro is convenient for asserting enum variant content:

```rust
assert!(matches!(
    &result.kind,
    VersionKind::Daily { date } if date == "2026.04.03"
));
// Cleaner than:
// match &result.kind {
//     VersionKind::Daily { date } => assert_eq!(date, "2026.04.03"),
//     _ => panic!("Expected Daily"),
// }
```

See [[Rust Pattern Matching]].

---

## Metadata

**Tags:** workflow
**Related:** [[Rust Async Await]], [[Rust Pattern Matching]], [[Git Workflow]], [[Development Workflow]]
