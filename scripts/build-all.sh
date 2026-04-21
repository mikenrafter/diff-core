#!/usr/bin/env bash
# Build the diffcore CLI with every supported tree-sitter grammar bundled.
#
# This is the convenience wrapper around `cargo build --release` that maps to
# the flake's "all-cli" / `mkDiffcoreCli "all"` package — it bundles the core 13
# always-on languages plus every extra grammar enabled via the
# `extra-languages` feature umbrella in `crates/diffcore-core/Cargo.toml`.
#
# Usage:
#   ./scripts/build-all.sh             # debug build
#   ./scripts/build-all.sh --release   # release build
#   ./scripts/build-all.sh -- --tauri  # also build the desktop app

set -euo pipefail

cd "$(dirname "$0")/.."

PROFILE=()
EXTRA_PKGS=()
WITH_TAURI=false

for arg in "$@"; do
  case "$arg" in
    --release|-r) PROFILE+=("--release") ;;
    --tauri)      WITH_TAURI=true ;;
    --)           ;;
    *)            EXTRA_PKGS+=("$arg") ;;
  esac
done

echo "==> Building diffcore-cli with ALL languages"
cargo build "${PROFILE[@]}" --package diffcore-cli "${EXTRA_PKGS[@]}"

if $WITH_TAURI; then
  echo "==> Building diffcore-tauri with ALL languages"
  cargo build "${PROFILE[@]}" --package diffcore-tauri "${EXTRA_PKGS[@]}"
fi

echo "==> Done."
