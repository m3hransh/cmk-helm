{
  description = "CMK Helm — interactive TUI for cmk-dev-site";

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

        # ── Runtime Python toolchain ─────────────────────────────────────────
        #
        # cmk-helm shells out to cmk-dev-install / cmk-dev-site at runtime.
        # We package them here from PyPI wheels so users get everything with a
        # single `nix run` — no separate pipx/pip step needed.
        #
        # Rust concept (build systems): `format = "wheel"` tells
        # buildPythonPackage to install the pre-built wheel archive directly
        # instead of compiling from source. Pure-Python packages (tagged
        # py3-none-any) ship as wheels that work on any platform, so this is
        # both faster and simpler — no compiler, no build backend needed.
        #
        # Three packages are absent from nixpkgs and must be built here;
        # the rest come from pkgs.python3Packages.

        trickkiste = pkgs.python3Packages.buildPythonPackage {
          pname = "trickkiste";
          version = "0.3.7";
          format = "wheel";
          src = pkgs.fetchurl {
            url = "https://files.pythonhosted.org/packages/7f/fb/b8862d2e799f0927c39a377393c2587d44e3161304895b9972f8393faaa2/trickkiste-0.3.7-py3-none-any.whl";
            hash = "sha256-8C95pZ65dW6XxUvVMbpBpzaVOhj5SAm9BaEIaRhu+e0=";
          };
          dependencies = with pkgs.python3Packages; [ python-dateutil ];
          doCheck = false;
        };

        python-jenkins = pkgs.python3Packages.buildPythonPackage {
          pname = "python-jenkins";
          version = "1.8.3";
          format = "wheel";
          src = pkgs.fetchurl {
            url = "https://files.pythonhosted.org/packages/84/78/2105fc1fc43257057bf687429af3114f16b0574f905dd4840eabd40585ed/python_jenkins-1.8.3-py3-none-any.whl";
            hash = "sha256-LhdmslPjsvKPUqv9DeRf/qoRkfmFIMyNdNLDF3huWdA=";
          };
          dependencies = with pkgs.python3Packages; [ pbr multi-key-dict requests ];
          doCheck = false;
        };

        checkmk-dev-tools = pkgs.python3Packages.buildPythonPackage {
          pname = "checkmk-dev-tools";
          version = "2.2.0";
          format = "wheel";
          src = pkgs.fetchurl {
            url = "https://files.pythonhosted.org/packages/6b/3f/896e2d62450d9a900b905d33ff4dcdf10b7696525920759e3acc4658b8cd/checkmk_dev_tools-2.2.0-py3-none-any.whl";
            hash = "sha256-zfQXEzM4zWcLXqFcLs9kv3ekHv95dWKkSe0+/CsOrfk=";
          };
          dependencies = with pkgs.python3Packages; [
            pydantic rich influxdb-client
          ] ++ [ python-jenkins trickkiste ];
          doCheck = false;
        };

        cmk-dev-site-pkg = pkgs.python3Packages.buildPythonPackage {
          pname = "cmk-dev-site";
          version = "1.15.2";
          format = "wheel";
          src = pkgs.fetchurl {
            url = "https://files.pythonhosted.org/packages/e8/1b/441d3a55cf7893608292444d1bed08d5f8be1f57f6a7dbd6c75736693ad6/cmk_dev_site-1.15.2-py3-none-any.whl";
            hash = "sha256-JK2eg0/1qGUm4yGHqZ9LA8IpiaSR4h0XMv3TnDVo9X4=";
          };
          dependencies = with pkgs.python3Packages; [
            requests pyjwt cryptography fastapi uvicorn python-multipart
          ] ++ [ checkmk-dev-tools ];
          # The published 1.15.2 wheel declares checkmk-dev-tools<1 (stale).
          # The constraint has been fixed upstream in cmk-dev-site's pyproject.toml;
          # remove this once a new release is published to PyPI.
          pythonRelaxDeps = [ "checkmk-dev-tools" ];
          doCheck = false;
        };

        # The Rust binary, wrapped so cmk-dev-site tools land on PATH.
        #
        # Rust concept (linking vs wrapping): we can't embed Python scripts
        # into a Rust binary, so instead makeWrapper rewrites the installed
        # $out/bin/cmk-helm script to prepend cmk-dev-site's bin/ to PATH
        # before exec-ing the real binary. Anyone who runs the Nix-built binary
        # gets cmk-dev-install / cmk-dev-site automatically — no separate install.
        cmk-helm = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
          pname = "cmk-helm";
          nativeBuildInputs = commonArgs.nativeBuildInputs ++ [ pkgs.makeWrapper ];
          postInstall = ''
            wrapProgram $out/bin/cmk-helm \
              --prefix PATH : ${pkgs.lib.makeBinPath [ cmk-dev-site-pkg ]}
          '';
        });

      in {
        # ── Outputs ──────────────────────────────────────────────────────────

        # `nix build .`            →  result/bin/cmk-helm (with bundled toolchain)
        # `nix run .`              →  build & execute immediately
        # `nix run github:you/repo` →  works for any user, no prior installs needed
        # `nix profile install .`  →  installs to user profile
        packages = {
          default = cmk-helm;
          inherit cmk-helm;
        };

        apps.default = flake-utils.lib.mkApp { drv = cmk-helm; };

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
            git-cliff      # `git cliff`            — generate CHANGELOG.md from commits
          ];

          shellHook = ''
            export PKG_CONFIG_PATH="${pkgs.openssl.dev}/lib/pkgconfig:$PKG_CONFIG_PATH"
            echo ""
            echo "  CMK Helm dev shell ready."
            echo "  cargo run           — start the TUI"
            echo "  cargo watch -x run  — auto-restart on save"
            echo "  git cliff -o CHANGELOG.md  — regenerate changelog"
            echo "  cargo clippy       — lint"
            echo ""
          '';
        };

        # `nix flake check`  →  run clippy + fmt + full build
        checks = {
          inherit cmk-helm;

          clippy = craneLib.cargoClippy (commonArgs // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets -- --deny warnings";
          });

          fmt = craneLib.cargoFmt { inherit src; };
        };
      }
    );
}
