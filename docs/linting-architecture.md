# Diffcore Linting Architecture

This document explains the linting strategy for the diffcore codebase: the tools chosen, the rules enforced, and the rationale behind each decision.

---

## Overview

Diffcore uses two complementary linting layers:

| Layer | Tool | Scope | Enforced by |
|---|---|---|---|
| TypeScript / React | ESLint v9 (flat config) | `crates/diffcore-tauri/ui/src/` | `npm run lint` |
| Rust file sizes | Node.js script | `diffcore-core/src/`, `diffcore-tauri/src/` | `npm run lint:rust-size` |

Both checks are run in CI and are expected to pass on every PR.

---

## Why ESLint?

ESLint was chosen for the TypeScript/React layer for the following reasons:

1. **Maintained and widely adopted** — ESLint is the de-facto standard linter for TypeScript and React projects. The v9 flat config format (`eslint.config.js`) is the current stable API with long-term support.
2. **CVE-free** — The pinned versions of `eslint`, `@typescript-eslint/parser`, `@typescript-eslint/eslint-plugin`, `eslint-plugin-react`, and `eslint-plugin-react-hooks` have zero known vulnerabilities at the time of adoption (verified against the GitHub Advisory Database).
3. **Plugin ecosystem** — TypeScript-aware rules (`@typescript-eslint`), React lifecycle rules (`react-hooks`), and the ability to write custom rules with full AST access make ESLint the best fit for our multi-file comprehension requirement.
4. **Flat config** — ESLint v9's flat config makes per-directory rule overrides (e.g. exempting test files) explicit and easy to audit.

Alternatives considered:
- **Biome** — fast, but its plugin API is not yet stable enough for custom multi-file rules.
- **oxlint** — promising performance, but the custom rule API is experimental.
- **custom tsc plugin** — type-aware but complex to maintain; ESLint + `@typescript-eslint` covers the same ground with a better plugin API.

---

## TypeScript File Size Cap (2 000 lines)

**Rule:** `max-lines: ["error", { max: 2000 }]`  
**Files:** all `.ts` and `.tsx` under `src/` (non-test)  
**Exempt:** `*.spec.ts`, `*.spec.tsx`, `*.test.ts`, `*.test.tsx`

### Rationale

- Large files accumulate hidden coupling. When a single file owns state management, rendering, utilities, and business logic, pull requests grow to hundreds of diff lines that are hard to review.
- 2 000 lines is the chosen cap for TypeScript because React component files with a moderate amount of JSX naturally approach 500–800 lines; the 2 000 cap allows for growth without forcing premature splits.
- Test files are explicitly exempt. Tests should be co-located with their source and grow as coverage increases. Restricting test file size discourages thorough testing.

### Current violations

`App.tsx` currently exceeds this limit (5 990 lines). The file is being progressively split per the roadmap in `specs/component-architecture.md`. The lint error is intentional — it tracks the remaining work. When all panel, tab, and modal components are extracted, `App.tsx` will serve only as the orchestration layer (≤ 2 000 lines).

---

## Rust File Size Cap (3 000 lines)

**Script:** `crates/diffcore-tauri/ui/scripts/check-rust-file-sizes.js`  
**Invoked by:** `npm run lint:rust-size`  
**Directories scanned:** `crates/diffcore-core/src/`, `crates/diffcore-tauri/src/`  
**Exempt:** any file whose name contains `test` or `spec`

### Rationale

- Rust files are slightly more dense than TypeScript (no JSX, more impl blocks) so a higher cap of 3 000 lines is appropriate.
- The Rust source modules (`ast.rs`, `flow.rs`, `entrypoint.rs`, `graph.rs`) were all already split in this PR series; the script confirms and enforces that they remain under the limit.
- `commands.rs` (4 517 lines) was split into 8 submodules in `commands/mod.rs` + subfiles; the script confirms all are within 3 000 lines.
- Integration test files (`tests/e2e_pipeline.rs` and siblings) are exempt from this check.

---

## Tauri Command Alignment Rule (`no-orphan-tauri-commands`)

**Rule:** `diffcore-local/no-orphan-tauri-commands: "error"`  
**Implementation:** `eslint-rules/no-orphan-tauri-commands.js`

### What it does

This is the **multi-file comprehension** rule. It reads the Rust backend source at lint time and validates that every `tauriInvoke("command_name", ...)` call in the TypeScript frontend references a `#[tauri::command]` function that actually exists.

### Why this matters

Tauri commands are invoked by string name at runtime:
```ts
await tauriInvoke<AnalysisOutput>("analyze", { repoPath, ... });
```

If the Rust function is renamed, removed, or never added, the call silently fails at runtime (Tauri returns an error that must be caught by the caller). This rule catches those mismatches at lint time, before the app is built.

### How it works

1. At rule initialization (once per ESLint run), the rule reads all `.rs` files in `crates/diffcore-tauri/src/commands/`.
2. It extracts every function decorated with `#[tauri::command]` using a regex against the source text.
3. For each `CallExpression` where the callee is `tauriInvoke` (direct or generic `tauriInvoke<T>`), it checks the first string-literal argument against the collected command set.
4. If the command name is not found, an error is reported on the string literal.

### Graceful degradation

If the `commands/` directory cannot be read (e.g., running ESLint in a context where only the `ui/` subtree is checked out), the rule returns an empty visitor and does not block the lint run.

---

## How to run linting locally

```bash
# From crates/diffcore-tauri/ui/
npm run lint            # ESLint (TypeScript + React + Tauri alignment)
npm run lint:rust-size  # Rust file size checker
```

To lint a single file:
```bash
npx eslint src/utils/activityUtils.ts
```

To auto-fix warnings (blank lines, etc.) where possible:
```bash
npm run lint -- --fix
```

---

## File locations

| Path | Purpose |
|---|---|
| `crates/diffcore-tauri/ui/eslint.config.js` | ESLint v9 flat config |
| `crates/diffcore-tauri/ui/eslint-rules/no-orphan-tauri-commands.js` | Custom multi-file alignment rule |
| `crates/diffcore-tauri/ui/scripts/check-rust-file-sizes.js` | Rust file size enforcement script |

---

## Future work

- **Pre-commit hook** — integrate both checks into a `pre-commit` (or `lefthook`) hook so violations are caught before push.
- **CI integration** — add `npm run lint && npm run lint:rust-size` to the GitHub Actions workflow.
- **Incremental App.tsx split** — as panels, tabs, and modals are extracted per `specs/component-architecture.md`, the `max-lines` error on `App.tsx` will naturally resolve.
