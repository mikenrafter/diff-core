#!/usr/bin/env bash
# Build the diffcore CLI with only the core 13 always-on tree-sitter grammars.
#
# This is the convenience wrapper around `cargo build` that maps to the flake's
# `diffcore-cli-core` / `mkDiffcoreCli "core"` package — it disables every
# extra language grammar (Bash, Haskell, Nix, Lua, …) by passing
# `--no-default-features` to cargo. The resulting binary is faster to compile
# and smaller on disk; non-core file types are reported as `Language::Unknown`.
#
# Usage:
#   ./scripts/build-core.sh             # debug build
#   ./scripts/build-core.sh --release   # release build
#   ./scripts/build-core.sh --tauri     # also build the desktop app

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

echo "==> Building diffcore-cli with CORE languages only (--no-default-features)"
cargo build "${PROFILE[@]}" --package diffcore-cli --no-default-features "${EXTRA_PKGS[@]}"

if $WITH_TAURI; then
  echo "==> Building diffcore-tauri with CORE languages only"
  cargo build "${PROFILE[@]}" --package diffcore-tauri --no-default-features "${EXTRA_PKGS[@]}"
fi

echo "==> Done."
