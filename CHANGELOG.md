# Changelog

All notable changes to CMK Helm are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
Versioning follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Chores

- Add git-cliff changelog setup and initial CHANGELOG.md


### Documentation

- Update CHANGELOG.md with post-v0.1.0 unreleased changes


### Features

- Show version in tab bar title and splash screen

- Auto-start mock-auth and add Cloud to newer-version editions


## [0.1.0] — 2026-05-20

### Bug Fixes

- Resolve rustc 1.87/1.88 MSRV conflict and Cargo.lock tracking

- Update footer key hints to match all handled keys

- Update stale 30-second comment to 5 minutes


### Documentation

- Add Obsidian Zettelkasten vault

- Add CLAUDE.md project guide

- Restructure vault to Zettelkasten with 40+ atomic notes

- Add Log Panel note, update TUI Layout Design, archive Installing Screen

- Add README with nix run usage, key bindings, and dev setup

- Add Nix install instructions for Ubuntu in README

- Update GitHub path to m3hransh/cmk-helm in README


### Features

- Scaffold Rust TUI with real server/install knowledge

- Rewrite api with correct server model; add edition picker screen

- Tabs by base version, timestamps, installed versions/sites panel

- Async job system, live log panel, and navigation overhaul

- Delete version/site, create site, sudo caching, edition fix

- Bundle cmk-dev-site as Nix runtime dependency

- Animated splash screen with CMK branding

- Modularize ui, add background version refresh

- Log panel job tabs and copy-paste mode

- Update refresh time to 5 mins


### Refactoring

- Replace sequential screens with multi-pane layout

- Rename package and binary from cmk-cockpit to cmk-helm


