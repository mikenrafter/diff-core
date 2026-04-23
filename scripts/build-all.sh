#!/usr/bin/env bash
# Build the diffcore CLI with every supported tree-sitter grammar bundled.
#
# This is the convenience wrapper around `cargo build --release` that maps to
# the flake's "all-cli" / `mkDiffcoreCli "all"` package — it bundles the core 13
# always-on languages plus every extra grammar enabled via the
# `extra-languages` feature umbrella in `crates/diffcore-core/Cargo.toml`.
#
# Usage:
#   ./scripts/build-all.sh                    # debug build of CLI
#   ./scripts/build-all.sh --release          # release build of CLI
#   ./scripts/build-all.sh --tauri            # release build of CLI + Tauri app
#   ./scripts/build-all.sh --debug --tauri    # debug build of CLI + Tauri app
#
# Why `cargo tauri build` instead of `cargo build --package diffcore-tauri`:
#   `cargo tauri build` is the Tauri CLI wrapper that first runs the
#   beforeBuildCommand from tauri.conf.json ("npm run build" in ui/) to produce
#   the JS bundle in ui/dist/, then compiles the Rust binary that embeds it.
#   Plain `cargo build --package diffcore-tauri` skips the npm step entirely,
#   so the binary either embeds a stale JS bundle or fails to link.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

# Flags for plain `cargo build` (CLI).
CARGO_PROFILE_FLAGS=()
# `cargo tauri build` is release by default; pass --debug to build debug.
TAURI_DEBUG_FLAG=()
WITH_TAURI=false

for arg in "$@"; do
  case "$arg" in
    --release|-r)
      CARGO_PROFILE_FLAGS+=("--release")
      # Tauri is release by default — no extra flag needed.
      ;;
    --debug)
      # Tauri release is the default; --debug flips it back.
      TAURI_DEBUG_FLAG+=("--debug")
      ;;
    --tauri) WITH_TAURI=true ;;
    --)      ;;
    *)       CARGO_PROFILE_FLAGS+=("$arg") ;;
  esac
done

echo "==> Building diffcore-cli with ALL languages"
cargo build "${CARGO_PROFILE_FLAGS[@]}" --package diffcore-cli

if $WITH_TAURI; then
  echo "==> Building diffcore-tauri (npm run build → cargo tauri build)"
  cd "$REPO_ROOT/crates/diffcore-tauri"
  # --no-bundle: skip OS packaging (.dmg/.AppImage/.msi) — just produce the binary.
  cargo tauri build "${TAURI_DEBUG_FLAG[@]}" --no-bundle
  cd "$REPO_ROOT"
fi
