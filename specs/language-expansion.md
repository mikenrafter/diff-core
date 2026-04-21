# Language Expansion

Expand diffcore's tree-sitter language coverage from 13 to ~25+ languages, gate every grammar behind a Cargo feature, and parameterize the Nix flake so consumers can pick which grammars to bundle.

## Goals

- Retain all currently supported languages with no behavior change at the default feature set.
- Add many more common languages via published `tree-sitter-*` crates on crates.io.
- Gate every grammar (existing + new) behind a per-language Cargo feature (`lang-rust`, `lang-haskell`, …) so binary size and compile time scale with intent.
- Default feature set bundles every language for which a crate exists, so `cargo build` is a no-surprise experience.
- Expose a `withLanguages` knob in `flake.nix` that maps a Nix list (e.g. `[ "rust" "python" "bash" ]`) to `--no-default-features --features "lang-rust lang-python lang-bash"`.
- For languages without a maintained published crate (Nim, SQL today), use a `build.rs` fallback that vendors the upstream tree-sitter grammar repo. **Deferred** in the first cut.

## Non-goals (this spec)

- Deep entrypoint detection for new languages on par with TS Effect.ts / FastAPI / Express coverage. Best-effort `.scm` for definitions/imports/calls/assignments only.
- Runtime dynamic loading of `.so` grammars (rejected — fragile cross-platform).
- Full shared-IR refactor — the engine in `query_engine.rs` is already declarative and `.scm`-driven; the work here is *additive* (more grammars + feature gates).

## Inventory

Picks below verified against `cargo search` on 2026-04-20 (tree-sitter 0.26.7 ABI target).

### Already in workspace (retain, gate behind `lang-*` feature)

| Lang | Crate | Version | Feature flag |
|---|---|---|---|
| TypeScript / JavaScript | `tree-sitter-typescript` | 0.23.2 | `lang-typescript` |
| Python | `tree-sitter-python` | 0.25.0 | `lang-python` |
| Go | `tree-sitter-go` | 0.25.0 | `lang-go` |
| Rust | `tree-sitter-rust` | 0.24.1 | `lang-rust` |
| Java | `tree-sitter-java` | 0.23.5 | `lang-java` |
| C# | `tree-sitter-c-sharp` | 0.23.1 | `lang-csharp` |
| PHP | `tree-sitter-php` | 0.24.2 | `lang-php` |
| Ruby | `tree-sitter-ruby` | 0.23.1 | `lang-ruby` |
| Kotlin | `tree-sitter-kotlin-ng` | 1.1.0 | `lang-kotlin` |
| Swift | `tree-sitter-swift` | 0.7.1 | `lang-swift` |
| C | `tree-sitter-c` | 0.24.1 | `lang-c` |
| C++ | `tree-sitter-cpp` | 0.23.4 | `lang-cpp` |
| Scala | `tree-sitter-scala` | 0.25.0 | `lang-scala` |

### New languages (to add)

| Lang | Crate | Version | Feature flag | Risk |
|---|---|---|---|---|
| Bash | `tree-sitter-bash` | 0.25.1 | `lang-bash` | low |
| Haskell | `tree-sitter-haskell` | 0.23.1 | `lang-haskell` | medium (older) |
| Nix | `tree-sitter-nix` | 0.3.0 | `lang-nix` | medium (ABI) |
| Lua | `tree-sitter-lua` | 0.5.0 | `lang-lua` | medium (ABI) |
| Perl | `tree-sitter-perl-next` | 0.1.0 | `lang-perl` | low (explicit 0.25+) |
| Elixir | `tree-sitter-elixir` | 0.3.5 | `lang-elixir` | low |
| Erlang | `tree-sitter-erlang` | 0.15.0 | `lang-erlang` | low |
| Zig | `tree-sitter-zig` | 1.1.2 | `lang-zig` | low |
| OCaml | `tree-sitter-ocaml` | 0.24.2 | `lang-ocaml` | low |
| Julia | `tree-sitter-julia` | 0.23.1 | `lang-julia` | low |
| Dart | `tree-sitter-dart-orchard` | 0.3.2 | `lang-dart` | medium (fork) |
| R | `tree-sitter-r` | 1.2.0 | `lang-r` | low |
| Fish | `tree-sitter-fish` | 3.6.0 | `lang-fish` | low |
| HTML | `tree-sitter-html` | 0.23.2 | `lang-html` | low |
| CSS | `tree-sitter-css` | 0.25.0 | `lang-css` | low |
| SCSS | `tree-sitter-scss` | 1.0.0 | `lang-scss` | low |
| JSON | `tree-sitter-json` | 0.24.8 | `lang-json` | low |
| YAML | `tree-sitter-yaml` | 0.7.2 | `lang-yaml` | medium (older) |
| TOML | `tree-sitter-toml-ng` | 0.7.0 | `lang-toml` | low (modern fork) |
| Markdown | `tree-sitter-md` | 0.5.3 | `lang-markdown` | low |
| GraphQL | `tree-sitter-graphql` | 0.1.0 | `lang-graphql` | high (very old) |
| Vue | `tree-sitter-vue-next` | 0.1.0 | `lang-vue` | high (ABI) |
| Svelte | `tree-sitter-svelte-next` | 0.1.1 | `lang-svelte` | low (explicit 0.25+) |

### Deferred

| Lang | Reason | Future path |
|---|---|---|
| **Nim** | No published `tree-sitter-nim` crate on crates.io | `build.rs` vendor of [`alaviss/tree-sitter-nim`](https://github.com/alaviss/tree-sitter-nim) |
| **SQL** | Only `tree-sitter-sql = 0.0.2` published; abandoned | `build.rs` vendor of [`derekstride/tree-sitter-sql`](https://github.com/DerekStride/tree-sitter-sql) |

If GraphQL or Vue fail to build against tree-sitter 0.26.7 ABI in the first integration pass, they will be moved to **Deferred** with the same `build.rs` follow-up path.

## Implementation Phases

### Phase 1 (current) — Pragmatic gating

The 13 existing languages stay **always-on** as a "core bundle". This is a deliberate scoping choice: `entrypoint.rs` and `flow.rs` contain large amounts of imperative per-language code (HTTP route detection, Effect.ts service detection, CLI command detection, data flow extraction) that depends on `tree_sitter_typescript`, `tree_sitter_python`, and `tree_sitter_go` directly. Gating the core 13 individually requires `#[cfg]` arms across hundreds of lines of detector code; that work is reserved for Phase 2.

1. **Add new crates + features.** Each new language from the table gets `optional = true` plus a `lang-X` feature flag.
2. **Add an `extra-languages` umbrella feature** that activates every new-language flag.
3. **Set `default = ["extra-languages"]`** so a vanilla `cargo build` bundles everything (preserving today's UX).
4. **Extend `Language` enum + `from_path`** with new variants and extensions. The variants are always present in the type (so downstream code compiles regardless of features); only the corresponding `get_lang_queries` arm is `#[cfg]`-gated.
5. **Extend `query_engine.rs`** — new `OnceCell` fields, embedded query strings, and `get_lang_queries` arms, each gated on `#[cfg(feature = "lang-X")]`.
6. **Write best-effort `.scm` query files** for each new language. Source from each upstream repo's `queries/highlights.scm` / `queries/tags.scm` where useful, then narrow to our 4-file structure: `imports.scm`, `definitions.scm`, `calls.scm`, `assignments.scm`. For pure-data languages (JSON/YAML/TOML/Markdown), the `.scm` files capture nothing — those grammars register the language for file counting and tree visualization only.
7. **Build + iterate.** `cargo check --all-features`. Move any crate with broken ABI to **Deferred**.
8. **Parameterize `flake.nix`.** Expose a `withLanguages` knob: `"all"` (default), `"core"` (the 13 only — disables `extra-languages`), or a Nix list like `[ "haskell" "bash" ]` (core + listed extras). Internally maps to `cargoExtraArgs`.
9. **Update `AGENTS.md`** to reflect the expanded language list and document the feature-flag mechanism.

### Phase 2 (future) — Full granularity

After Phase 1 is vetted, gate the core 13 individually. Requires `#[cfg]`-ing per-language code in `ast.rs`, `entrypoint.rs`, `flow.rs`, and their tests. Highest-value targets first (Java/C#/PHP/Ruby — large grammars, low usage in the eval corpus). TS/Python/Go gating last because they are tangled with the entrypoint detection backbone.

## Acceptance

- `cargo build` (no flags) compiles every supported new language and behaves identically to today for the existing 13.
- `cargo build --no-default-features --features "lang-rust lang-python"` produces a working binary that parses only Rust and Python files, treating others as `Language::Unknown`.
- `cargo test` passes (existing language coverage unchanged).
- `nix build .#diffcore-cli` works and bundles all languages.
- `nix build '.#diffcore-cli.override { languages = [ "rust" "bash" ]; }'` (or equivalent invocation) produces a slimmer binary.
- `eval/` suite scores within ±0.02 of baseline on the existing-language corpus.
