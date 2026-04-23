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
#   ./scripts/build-core.sh                    # debug build of CLI
#   ./scripts/build-core.sh --release          # release build of CLI
#   ./scripts/build-core.sh --tauri            # release build of CLI + Tauri app
#   ./scripts/build-core.sh --debug --tauri    # debug build of CLI + Tauri app
#
# See build-all.sh for the explanation of why --tauri uses `cargo tauri build`
# rather than `cargo build --package diffcore-tauri`.

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

echo "==> Building diffcore-cli with CORE languages only (--no-default-features)"
cargo build "${CARGO_PROFILE_FLAGS[@]}" --package diffcore-cli --no-default-features

if $WITH_TAURI; then
  echo "==> Building diffcore-tauri with CORE languages only (npm run build → cargo tauri build)"
  cd "$REPO_ROOT/crates/diffcore-tauri"
  # --no-bundle: skip OS packaging (.dmg/.AppImage/.msi) — just produce the binary.
  # --no-default-features disables the extra language grammars in the Rust layer.
  cargo tauri build "${TAURI_DEBUG_FLAG[@]}" --no-bundle -- --no-default-features
  cd "$REPO_ROOT"
fi

echo "==> Done."
