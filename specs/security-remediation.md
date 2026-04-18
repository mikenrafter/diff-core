# Security Remediation Plan

**Date:** 2026-04-16
**Status:** Active
**Severity range:** High → Low

## Summary

Static analysis and dependency audit of the diffcore workspace identified 6 security findings
ranked by exploitability and real-world impact. This spec documents each finding with evidence
and prescribes concrete fixes.

## Findings (ranked by exploitability)

### 1. HIGH — Repo-local config can drive shell execution via `key_cmd`

**Evidence:**
- Repo config is loaded from `.diffcore.toml` in the target repo (`config.rs:256`).
- `key_cmd` is executed via `sh -c` (`llm/mod.rs:430`).
- `apply_global_llm_defaults` merges repo-local `key_cmd` with global defaults —
  repo-local wins (`config.rs:424`).
- There is a metacharacter filter (`llm/mod.rs:404`), but even `op read malicious-vault`
  can exfiltrate data if the user has `op` installed.

**Impact:** A malicious `.diffcore.toml` in a cloned repo can execute arbitrary simple
commands when the user runs `diffcore analyze` with LLM features.

**Fix:** Ignore repo-local `key_cmd` by default. Only allow `key_cmd` from the global
config (`~/.diffcore/config.toml`) or environment variables. Add a log warning when a
repo-local `key_cmd` is present but ignored.

### 2. MEDIUM — `git2` version below patched advisory threshold

**Evidence:**
- Lockfile pins `git2 0.19.0` (Cargo.lock:2034).
- RUSTSEC-2026-0008 reports UB in `Buf` struct, patched `>= 0.20.4`.
- Production code calls `revparse_single` extensively (`git.rs:99, 213, 517`).

**Fix:** Bump `git2` to `^0.20` in all three crate Cargo.toml files.

### 3. MEDIUM — Desktop CSP permits `unsafe-eval`

**Evidence:**
- `tauri.conf.json:13` CSP includes `script-src 'self' 'unsafe-eval'`.
- This broadens XSS blast radius if any dynamic content injection appears.

**Fix:** Remove `'unsafe-eval'` from the CSP. If Monaco editor requires it, scope the
exception to a documented comment explaining why.

### 4. MEDIUM — Local SSE endpoint uses permissive CORS

**Evidence:**
- `activity_stream.rs:219` sets `CorsLayer::new().allow_origin(Any)`.
- While bound to localhost, any browser tab on the same machine can read the SSE stream.

**Fix:** Replace `Any` origin with the specific Tauri app origin(s).

### 5. LOW — `bincode` is unmaintained (RUSTSEC-2025-0141)

**Evidence:**
- Lockfile pins `bincode 1.3.3` (Cargo.lock:392).
- Used only for local IR disk cache (`pipeline.rs:233, 292`).

**Fix:** Replace bincode with `serde_json` for the IR cache format. This also makes
the cache human-inspectable for debugging.

### 6. LOW — Comment-code drift in CLI path validation

**Evidence:**
- Comment at `main.rs:888` claims "also validate src stays within workdir" but
  no additional check follows in that branch — only the prior traversal rejection
  at `main.rs:856` enforces this.

**Fix:** Add explicit canonicalization + starts-with check, or correct the misleading
comment to describe what actually happens.

### 7. MEDIUM — `@dagrejs/dagre` shares CVE-2025-57347 prototype pollution pattern

**Evidence:**
- CVE-2025-57347 (CVSS 9.8) targets `dagre-d3-es` `addConflict` in `bk.js`, but
  `@dagrejs/dagre` (used here at ^2.0.4) shares the same vulnerable `addConflict`
  function from the common dagre heritage.
- Node IDs are file paths from the Rust backend. A malicious repo with a file named
  `__proto__` could theoretically trigger prototype pollution during graph layout.
- No upstream fix exists for `@dagrejs/dagre` as of 2026-04-16.

**Fix:** Sanitize node IDs passed to dagre's `setNode`/`setEdge` by remapping
prototype-poisoning keys (`__proto__`, `constructor`, `prototype`) to safe prefixed
variants before layout.
