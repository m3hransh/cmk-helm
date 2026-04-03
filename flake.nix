{
  description = "CMK Cockpit — interactive TUI for cmk-dev-site";

  inputs = {
    # The main Nix package set — we follow unstable for fresh Rust support
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    # Provides a pinned, reproducible Rust toolchain (replaces rustup in Nix)
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs"; # share the same nixpkgs, no duplicates
    };

    # Crane: the recommended Nix build helper for Cargo projects.
    # Key benefit: it pre-builds dependencies in a separate derivation, so
    # only your code (not all of crates.io) rebuilds on each change.
    crane.url = "github:ipetkov/crane";

    # Reduces boilerplate for multi-platform (x86_64-linux, aarch64-darwin …)
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, crane, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };

        # Pin Rust to 1.88.0 explicitly rather than `stable.latest`.
        # Reason: ratatui's `instability` transitive dep requires rustc ≥ 1.88.
        # Using a fixed version ensures the dev shell and CI always agree
        # on the toolchain, regardless of when `nix flake update` was last run.
        # Bump this when upgrading dependencies that need a newer compiler.
        rustToolchain = pkgs.rust-bin.stable."1.88.0".default.override {
          extensions = [ "rust-src" "rust-analyzer" "clippy" "rustfmt" ];
        };

        # Wire crane to our pinned toolchain instead of whatever is in nixpkgs
        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        # cleanCargoSource strips docs, scripts, etc. so Nix hash stays stable
        # even when you edit non-Rust files (avoids unnecessary rebuilds).
        src = craneLib.cleanCargoSource ./.;

        # Shared arguments used by every crane derivation below.
        # reqwest needs OpenSSL headers at *build* time (nativeBuildInputs)
        # and the library at *link* time (buildInputs).
        commonArgs = {
          inherit src;
          strictDeps = true;

          buildInputs = with pkgs;
            lib.optionals stdenv.isDarwin [
              libiconv
              darwin.apple_sdk.frameworks.Security
              darwin.apple_sdk.frameworks.SystemConfiguration
            ];

          nativeBuildInputs = with pkgs; [
            pkg-config
          ];

          # reqwest's rustls-tls feature doesn't need OpenSSL at runtime,
          # but cargo still needs pkg-config to find it during linking on Linux.
          OPENSSL_NO_VENDOR = "1";
        };

        # Build only the dependencies (Cargo.lock → all transitive crates).
        # This derivation is cached separately: editing your source files does
        # NOT invalidate it, so incremental builds stay fast.
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        # The actual application binary, built on top of cached deps.
        cmk-cockpit = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
          pname = "cmk-cockpit";
        });

      in {
        # ── Outputs ──────────────────────────────────────────────────────────

        # `nix build .`  →  result/bin/cmk-cockpit
        # `nix profile install .`  →  installs to user profile
        packages = {
          default = cmk-cockpit;
          inherit cmk-cockpit;
        };

        # `nix run .`  →  build & execute immediately
        apps.default = flake-utils.lib.mkApp { drv = cmk-cockpit; };

        # `nix develop`  →  enter the dev shell (also used by direnv)
        devShells.default = craneLib.devShell {
          checks = self.checks.${system} or {};

          packages = with pkgs; [
            rustToolchain
            pkg-config
            openssl.dev    # headers for building reqwest

            # Developer ergonomics
            cargo-watch    # `cargo watch -x run`  — auto-restart on save
            cargo-expand   # `cargo expand`        — unfold macros (great for learning)
          ];

          shellHook = ''
            export PKG_CONFIG_PATH="${pkgs.openssl.dev}/lib/pkgconfig:$PKG_CONFIG_PATH"
            echo ""
            echo "  CMK Cockpit dev shell ready."
            echo "  cargo run          — start the TUI"
            echo "  cargo watch -x run — auto-restart on save"
            echo "  cargo clippy       — lint"
            echo ""
          '';
        };

        # `nix flake check`  →  run clippy + fmt + full build
        checks = {
          inherit cmk-cockpit;

          clippy = craneLib.cargoClippy (commonArgs // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets -- --deny warnings";
          });

          fmt = craneLib.cargoFmt { inherit src; };
        };
      }
    );
}
