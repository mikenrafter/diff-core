{
  description = "diffcore — semantic diff layer for code review";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    crane.url = "github:ipetkov/crane";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, crane }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" "clippy" ];
        };

        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        # Common native build inputs needed by vendored C libraries
        # (tree-sitter grammars, libgit2, openssl).
        nativeBuildDeps = with pkgs; [
          pkg-config
          cmake # needed by some tree-sitter grammars and libgit2
          perl  # needed by vendored openssl build
        ];

        # Runtime / link-time libraries.
        # libgit2 and openssl are vendored via cargo features, but the
        # compiler still needs a C toolchain and sometimes system headers.
        commonBuildInputs = with pkgs; [
          openssl
        ] ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
          # Tauri 2 on Linux requires these for the webview.
          webkitgtk_4_1
          gtk3
          glib
          glib-networking
          libsoup_3
          cairo
          pango
          gdk-pixbuf
          atk
          harfbuzz
          librsvg
        ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin (with pkgs.darwin.apple_sdk.frameworks; [
          Security
          SystemConfiguration
          CoreFoundation
          CoreServices
          AppKit
          WebKit
        ]);

        # Shared source filtering — only keep Rust sources, query files, and
        # Cargo manifests so that rebuilds are minimal.
        src = let
          # Accept tree-sitter .scm query files alongside standard Rust/Cargo sources.
          queryFilter = path: _type: builtins.match ".*\\.scm$" path != null;
          tomlFilter = path: _type: builtins.match ".*\\.toml$" path != null;
          jsonFilter = path: _type: builtins.match ".*\\.json$" path != null;
          combinedFilter = path: type:
            (craneLib.filterCargoSources path type)
            || (queryFilter path type)
            || (tomlFilter path type)
            || (jsonFilter path type);
        in
          pkgs.lib.cleanSourceWith {
            src = craneLib.path ./.;
            filter = combinedFilter;
          };

        # Build the workspace deps once, then reuse for each package.
        commonArgs = {
          inherit src;
          strictDeps = true;
          nativeBuildInputs = nativeBuildDeps;
          buildInputs = commonBuildInputs;

          # Ensure vendored openssl is used (matches Cargo.toml feature flag).
          OPENSSL_NO_VENDOR = 0;
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        # --- Language selection --------------------------------------------
        #
        # Every tree-sitter grammar lives behind a per-language Cargo feature
        # (`lang-bash`, `lang-haskell`, …). The 13 core languages are always
        # compiled in; everything else is gated.
        #
        # `mkFeatureArgs` translates a Nix-level `languages` selector into the
        # `--features` portion of `cargo build`:
        #
        #   "all"             → default features (core 13 + extra-languages
        #                       umbrella → every supported grammar bundled).
        #   "core"            → only the always-on core 13.
        #   [ "haskell" "bash" "nix" ]
        #                     → core 13 + the listed extras (each name is
        #                       prefixed with `lang-` and joined into the
        #                       Cargo feature list).
        #
        # The "all" alias compiles to an empty string because the default
        # feature set in `Cargo.toml` already equals "everything" — paying the
        # extra `--no-default-features --features extra-languages` round-trip
        # is wasted work.
        mkFeatureArgs = languages:
          if languages == "all" then ""
          else if languages == "core" then "--no-default-features"
          else if builtins.isList languages then
            (if builtins.elem "all" languages then ""
             else
               let
                 langFlags = pkgs.lib.concatStringsSep ","
                   (map (l: "lang-${l}") languages);
               in
                 "--no-default-features --features \"${langFlags}\"")
          else
            throw "diffcore: `languages` must be \"all\", \"core\", or a list of language names";

        # --- Packages -------------------------------------------------------

        # Generic builder for the CLI. Pass a `languages` selector (see above).
        mkDiffcoreCli = languages: craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
          pname = "diffcore";
          cargoExtraArgs = "--package diffcore-cli ${mkFeatureArgs languages}";
          meta.mainProgram = "diffcore";
        });

        # The Tauri package requires a pre-built frontend.
        tauriFrontend = pkgs.buildNpmPackage {
          pname = "diffcore-tauri-ui";
          version = "0.5.7";
          src = ./crates/diffcore-tauri/ui;
          npmDepsHash = ""; # Set after first build, see comment below.
          buildPhase = ''
            npm run build
          '';
          installPhase = ''
            cp -r dist $out
          '';

          # NOTE: On first use, run:
          #   nix build .#diffcore-tauri 2>&1 | grep 'got:'
          # then paste the hash into npmDepsHash above.
          # Left empty so the flake parses; the CLI package works without it.
        };

        # Generic builder for the desktop app. Same `languages` selector as CLI.
        mkDiffcoreTauri = languages: craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
          pname = "diffcore-tauri";
          cargoExtraArgs = "--package diffcore-tauri ${mkFeatureArgs languages}";

          # Point Tauri build at the pre-built frontend dist.
          preBuild = ''
            cp -r ${tauriFrontend} crates/diffcore-tauri/ui/dist
          '';

          buildInputs = commonArgs.buildInputs ++ (with pkgs; [
            nodejs
          ]);
        });

        # Convenience handles: the "all languages" variant is the default; a
        # core-only variant is exposed for fast / slim builds.
        diffcore-cli        = mkDiffcoreCli "all";
        diffcore-cli-core   = mkDiffcoreCli "core";
        diffcore-tauri      = mkDiffcoreTauri "all";
        diffcore-tauri-core = mkDiffcoreTauri "core";

      in {
        packages = {
          default = diffcore-cli;
          inherit diffcore-cli diffcore-tauri
                  diffcore-cli-core diffcore-tauri-core;

          # Aliases requested for clarity ("all" intent in the package name).
          all-cli = diffcore-cli;
          all-gui = diffcore-tauri;
        };

        # Expose the builders so a downstream flake can import this one and
        # build a slimmer variant, e.g.
        #   diffcore.lib.${system}.mkDiffcoreCli [ "haskell" "bash" "nix" ]
        lib = {
          inherit mkDiffcoreCli mkDiffcoreTauri mkFeatureArgs;
        };

        devShells.default = craneLib.devShell {
          # Inherit everything the packages need, plus dev-time extras.
          inputsFrom = [ diffcore-cli ];

          packages = with pkgs; [
            # Rust (provided by craneLib.devShell via the toolchain)
            cargo-watch
            cargo-insta    # snapshot testing

            # Node.js for Tauri frontend development
            nodejs_22
            eslint
            eslint_d

            # Tauri CLI
            cargo-tauri

            # Useful dev utilities
            git
          ] ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
            # Tauri dev on Linux
            webkitgtk_4_1
            gtk3
            glib
            glib-networking
            libsoup_3
          ];

          # Ensure pkg-config can find the libraries.
          shellHook = ''
            export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath commonBuildInputs}:$LD_LIBRARY_PATH"
          '';
        };
      });
}
