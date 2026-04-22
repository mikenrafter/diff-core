//! Declarative tree-sitter query engine.
//!
//! Instead of imperative per-language Rust code, this module uses `.scm` query
//! files (tree-sitter's native query language) to declaratively capture AST
//! patterns. A single generic engine maps query captures to [`ParsedFile`] and
//! [`DataFlowInfo`] types.
//!
//! Adding a new language requires only writing `.scm` query files — zero Rust
//! code changes.

use crate::ast::{
    CallSite, CallWithArgs, DataFlowInfo, Definition, ExportInfo, ImportInfo,
    Language, ParsedFile, VarCallAssignment,
};
use crate::languages::common::{
    collect_matches, extract_arg_texts, extract_definitions_standard,
    find_containing_function, node_text,
};
use crate::types::SymbolKind;
use once_cell::sync::OnceCell;
use std::cell::RefCell;
use std::collections::HashMap;
use thiserror::Error;
use tree_sitter::{Node, Parser, Query, QueryCursor};

// ---------------------------------------------------------------------------
// Thread-local parser pool
// ---------------------------------------------------------------------------
//
// `tree_sitter::Parser` is `Send` but `!Sync`, so we cannot store it inside
// `QueryEngine` (which is shared via `&QueryEngine` across rayon threads).
// Instead, each thread maintains its own set of parsers keyed by `Language`.
// This avoids allocating a new `Parser` on every `parse_tree()` call while
// remaining safe for rayon's work-stealing parallelism.

thread_local! {
    static THREAD_PARSERS: RefCell<HashMap<Language, Parser>> = RefCell::new(HashMap::new());
}

// ---------------------------------------------------------------------------
// Embedded query files (compiled into the binary)
// ---------------------------------------------------------------------------

mod queries {
    pub mod typescript {
        pub const IMPORTS: &str = include_str!("../queries/typescript/imports.scm");
        pub const EXPORTS: &str = include_str!("../queries/typescript/exports.scm");
        pub const DEFINITIONS: &str = include_str!("../queries/typescript/definitions.scm");
        pub const CALLS: &str = include_str!("../queries/typescript/calls.scm");
        pub const ASSIGNMENTS: &str = include_str!("../queries/typescript/assignments.scm");
    }
    pub mod python {
        pub const IMPORTS: &str = include_str!("../queries/python/imports.scm");
        pub const DEFINITIONS: &str = include_str!("../queries/python/definitions.scm");
        pub const CALLS: &str = include_str!("../queries/python/calls.scm");
        pub const ASSIGNMENTS: &str = include_str!("../queries/python/assignments.scm");
    }
    pub mod go {
        pub const IMPORTS: &str = include_str!("../queries/go/imports.scm");
        pub const DEFINITIONS: &str = include_str!("../queries/go/definitions.scm");
        pub const CALLS: &str = include_str!("../queries/go/calls.scm");
        pub const ASSIGNMENTS: &str = include_str!("../queries/go/assignments.scm");
    }
    pub mod rust {
        pub const IMPORTS: &str = include_str!("../queries/rust/imports.scm");
        pub const DEFINITIONS: &str = include_str!("../queries/rust/definitions.scm");
        pub const CALLS: &str = include_str!("../queries/rust/calls.scm");
        pub const ASSIGNMENTS: &str = include_str!("../queries/rust/assignments.scm");
    }
    pub mod java {
        pub const IMPORTS: &str = include_str!("../queries/java/imports.scm");
        pub const DEFINITIONS: &str = include_str!("../queries/java/definitions.scm");
        pub const CALLS: &str = include_str!("../queries/java/calls.scm");
        pub const ASSIGNMENTS: &str = include_str!("../queries/java/assignments.scm");
    }
    pub mod csharp {
        pub const IMPORTS: &str = include_str!("../queries/csharp/imports.scm");
        pub const DEFINITIONS: &str = include_str!("../queries/csharp/definitions.scm");
        pub const CALLS: &str = include_str!("../queries/csharp/calls.scm");
        pub const ASSIGNMENTS: &str = include_str!("../queries/csharp/assignments.scm");
    }
    pub mod php {
        pub const IMPORTS: &str = include_str!("../queries/php/imports.scm");
        pub const DEFINITIONS: &str = include_str!("../queries/php/definitions.scm");
        pub const CALLS: &str = include_str!("../queries/php/calls.scm");
        pub const ASSIGNMENTS: &str = include_str!("../queries/php/assignments.scm");
    }
    pub mod ruby {
        pub const IMPORTS: &str = include_str!("../queries/ruby/imports.scm");
        pub const DEFINITIONS: &str = include_str!("../queries/ruby/definitions.scm");
        pub const CALLS: &str = include_str!("../queries/ruby/calls.scm");
        pub const ASSIGNMENTS: &str = include_str!("../queries/ruby/assignments.scm");
    }
    pub mod kotlin {
        pub const IMPORTS: &str = include_str!("../queries/kotlin/imports.scm");
        pub const DEFINITIONS: &str = include_str!("../queries/kotlin/definitions.scm");
        pub const CALLS: &str = include_str!("../queries/kotlin/calls.scm");
        pub const ASSIGNMENTS: &str = include_str!("../queries/kotlin/assignments.scm");
    }
    pub mod swift {
        pub const IMPORTS: &str = include_str!("../queries/swift/imports.scm");
        pub const DEFINITIONS: &str = include_str!("../queries/swift/definitions.scm");
        pub const CALLS: &str = include_str!("../queries/swift/calls.scm");
        pub const ASSIGNMENTS: &str = include_str!("../queries/swift/assignments.scm");
    }
    pub mod c {
        pub const IMPORTS: &str = include_str!("../queries/c/imports.scm");
        pub const DEFINITIONS: &str = include_str!("../queries/c/definitions.scm");
        pub const CALLS: &str = include_str!("../queries/c/calls.scm");
        pub const ASSIGNMENTS: &str = include_str!("../queries/c/assignments.scm");
    }
    pub mod cpp {
        pub const IMPORTS: &str = include_str!("../queries/cpp/imports.scm");
        pub const DEFINITIONS: &str = include_str!("../queries/cpp/definitions.scm");
        pub const CALLS: &str = include_str!("../queries/cpp/calls.scm");
        pub const ASSIGNMENTS: &str = include_str!("../queries/cpp/assignments.scm");
    }
    pub mod scala {
        pub const IMPORTS: &str = include_str!("../queries/scala/imports.scm");
        pub const DEFINITIONS: &str = include_str!("../queries/scala/definitions.scm");
        pub const CALLS: &str = include_str!("../queries/scala/calls.scm");
        pub const ASSIGNMENTS: &str = include_str!("../queries/scala/assignments.scm");
    }

    // -----------------------------------------------------------------------
    // Extra languages — each gated behind a `lang-<name>` Cargo feature.
    // -----------------------------------------------------------------------
    #[cfg(feature = "lang-bash")]
    pub mod bash {
        pub const IMPORTS: &str = include_str!("../queries/bash/imports.scm");
        pub const DEFINITIONS: &str = include_str!("../queries/bash/definitions.scm");
        pub const CALLS: &str = include_str!("../queries/bash/calls.scm");
        pub const ASSIGNMENTS: &str = include_str!("../queries/bash/assignments.scm");
    }
    #[cfg(feature = "lang-haskell")]
    pub mod haskell {
        pub const IMPORTS: &str = include_str!("../queries/haskell/imports.scm");
        pub const DEFINITIONS: &str = include_str!("../queries/haskell/definitions.scm");
        pub const CALLS: &str = include_str!("../queries/haskell/calls.scm");
        pub const ASSIGNMENTS: &str = include_str!("../queries/haskell/assignments.scm");
    }
    #[cfg(feature = "lang-nix")]
    pub mod nix {
        pub const IMPORTS: &str = include_str!("../queries/nix/imports.scm");
        pub const DEFINITIONS: &str = include_str!("../queries/nix/definitions.scm");
        pub const CALLS: &str = include_str!("../queries/nix/calls.scm");
        pub const ASSIGNMENTS: &str = include_str!("../queries/nix/assignments.scm");
    }
    #[cfg(feature = "lang-lua")]
    pub mod lua {
        pub const IMPORTS: &str = include_str!("../queries/lua/imports.scm");
        pub const DEFINITIONS: &str = include_str!("../queries/lua/definitions.scm");
        pub const CALLS: &str = include_str!("../queries/lua/calls.scm");
        pub const ASSIGNMENTS: &str = include_str!("../queries/lua/assignments.scm");
    }
    #[cfg(feature = "lang-perl")]
    pub mod perl {
        pub const IMPORTS: &str = include_str!("../queries/perl/imports.scm");
        pub const DEFINITIONS: &str = include_str!("../queries/perl/definitions.scm");
        pub const CALLS: &str = include_str!("../queries/perl/calls.scm");
        pub const ASSIGNMENTS: &str = include_str!("../queries/perl/assignments.scm");
    }
    #[cfg(feature = "lang-elixir")]
    pub mod elixir {
        pub const IMPORTS: &str = include_str!("../queries/elixir/imports.scm");
        pub const DEFINITIONS: &str = include_str!("../queries/elixir/definitions.scm");
        pub const CALLS: &str = include_str!("../queries/elixir/calls.scm");
        pub const ASSIGNMENTS: &str = include_str!("../queries/elixir/assignments.scm");
    }
    #[cfg(feature = "lang-erlang")]
    pub mod erlang {
        pub const IMPORTS: &str = include_str!("../queries/erlang/imports.scm");
        pub const DEFINITIONS: &str = include_str!("../queries/erlang/definitions.scm");
        pub const CALLS: &str = include_str!("../queries/erlang/calls.scm");
        pub const ASSIGNMENTS: &str = include_str!("../queries/erlang/assignments.scm");
    }
    #[cfg(feature = "lang-zig")]
    pub mod zig {
        pub const IMPORTS: &str = include_str!("../queries/zig/imports.scm");
        pub const DEFINITIONS: &str = include_str!("../queries/zig/definitions.scm");
        pub const CALLS: &str = include_str!("../queries/zig/calls.scm");
        pub const ASSIGNMENTS: &str = include_str!("../queries/zig/assignments.scm");
    }
    #[cfg(feature = "lang-ocaml")]
    pub mod ocaml {
        pub const IMPORTS: &str = include_str!("../queries/ocaml/imports.scm");
        pub const DEFINITIONS: &str = include_str!("../queries/ocaml/definitions.scm");
        pub const CALLS: &str = include_str!("../queries/ocaml/calls.scm");
        pub const ASSIGNMENTS: &str = include_str!("../queries/ocaml/assignments.scm");
    }
    #[cfg(feature = "lang-julia")]
    pub mod julia {
        pub const IMPORTS: &str = include_str!("../queries/julia/imports.scm");
        pub const DEFINITIONS: &str = include_str!("../queries/julia/definitions.scm");
        pub const CALLS: &str = include_str!("../queries/julia/calls.scm");
        pub const ASSIGNMENTS: &str = include_str!("../queries/julia/assignments.scm");
    }
    #[cfg(feature = "lang-dart")]
    pub mod dart {
        pub const IMPORTS: &str = include_str!("../queries/dart/imports.scm");
        pub const DEFINITIONS: &str = include_str!("../queries/dart/definitions.scm");
        pub const CALLS: &str = include_str!("../queries/dart/calls.scm");
        pub const ASSIGNMENTS: &str = include_str!("../queries/dart/assignments.scm");
    }
    #[cfg(feature = "lang-r")]
    pub mod r {
        pub const IMPORTS: &str = include_str!("../queries/r/imports.scm");
        pub const DEFINITIONS: &str = include_str!("../queries/r/definitions.scm");
        pub const CALLS: &str = include_str!("../queries/r/calls.scm");
        pub const ASSIGNMENTS: &str = include_str!("../queries/r/assignments.scm");
    }
    #[cfg(feature = "lang-fish")]
    pub mod fish {
        pub const IMPORTS: &str = include_str!("../queries/fish/imports.scm");
        pub const DEFINITIONS: &str = include_str!("../queries/fish/definitions.scm");
        pub const CALLS: &str = include_str!("../queries/fish/calls.scm");
        pub const ASSIGNMENTS: &str = include_str!("../queries/fish/assignments.scm");
    }
    #[cfg(feature = "lang-html")]
    pub mod html {
        pub const IMPORTS: &str = include_str!("../queries/html/imports.scm");
        pub const DEFINITIONS: &str = include_str!("../queries/html/definitions.scm");
        pub const CALLS: &str = include_str!("../queries/html/calls.scm");
        pub const ASSIGNMENTS: &str = include_str!("../queries/html/assignments.scm");
    }
    #[cfg(feature = "lang-css")]
    pub mod css {
        pub const IMPORTS: &str = include_str!("../queries/css/imports.scm");
        pub const DEFINITIONS: &str = include_str!("../queries/css/definitions.scm");
        pub const CALLS: &str = include_str!("../queries/css/calls.scm");
        pub const ASSIGNMENTS: &str = include_str!("../queries/css/assignments.scm");
    }
    #[cfg(feature = "lang-scss")]
    pub mod scss {
        pub const IMPORTS: &str = include_str!("../queries/scss/imports.scm");
        pub const DEFINITIONS: &str = include_str!("../queries/scss/definitions.scm");
        pub const CALLS: &str = include_str!("../queries/scss/calls.scm");
        pub const ASSIGNMENTS: &str = include_str!("../queries/scss/assignments.scm");
    }
    #[cfg(feature = "lang-json")]
    pub mod json {
        pub const IMPORTS: &str = include_str!("../queries/json/imports.scm");
        pub const DEFINITIONS: &str = include_str!("../queries/json/definitions.scm");
        pub const CALLS: &str = include_str!("../queries/json/calls.scm");
        pub const ASSIGNMENTS: &str = include_str!("../queries/json/assignments.scm");
    }
    #[cfg(feature = "lang-yaml")]
    pub mod yaml {
        pub const IMPORTS: &str = include_str!("../queries/yaml/imports.scm");
        pub const DEFINITIONS: &str = include_str!("../queries/yaml/definitions.scm");
        pub const CALLS: &str = include_str!("../queries/yaml/calls.scm");
        pub const ASSIGNMENTS: &str = include_str!("../queries/yaml/assignments.scm");
    }
    #[cfg(feature = "lang-toml")]
    pub mod toml {
        pub const IMPORTS: &str = include_str!("../queries/toml/imports.scm");
        pub const DEFINITIONS: &str = include_str!("../queries/toml/definitions.scm");
        pub const CALLS: &str = include_str!("../queries/toml/calls.scm");
        pub const ASSIGNMENTS: &str = include_str!("../queries/toml/assignments.scm");
    }
    #[cfg(feature = "lang-markdown")]
    pub mod markdown {
        pub const IMPORTS: &str = include_str!("../queries/markdown/imports.scm");
        pub const DEFINITIONS: &str = include_str!("../queries/markdown/definitions.scm");
        pub const CALLS: &str = include_str!("../queries/markdown/calls.scm");
        pub const ASSIGNMENTS: &str = include_str!("../queries/markdown/assignments.scm");
    }
    #[cfg(feature = "lang-graphql")]
    pub mod graphql {
        pub const IMPORTS: &str = include_str!("../queries/graphql/imports.scm");
        pub const DEFINITIONS: &str = include_str!("../queries/graphql/definitions.scm");
        pub const CALLS: &str = include_str!("../queries/graphql/calls.scm");
        pub const ASSIGNMENTS: &str = include_str!("../queries/graphql/assignments.scm");
    }
    #[cfg(feature = "lang-vue")]
    pub mod vue {
        pub const IMPORTS: &str = include_str!("../queries/vue/imports.scm");
        pub const DEFINITIONS: &str = include_str!("../queries/vue/definitions.scm");
        pub const CALLS: &str = include_str!("../queries/vue/calls.scm");
        pub const ASSIGNMENTS: &str = include_str!("../queries/vue/assignments.scm");
    }
    #[cfg(feature = "lang-svelte")]
    pub mod svelte {
        pub const IMPORTS: &str = include_str!("../queries/svelte/imports.scm");
        pub const DEFINITIONS: &str = include_str!("../queries/svelte/definitions.scm");
        pub const CALLS: &str = include_str!("../queries/svelte/calls.scm");
        pub const ASSIGNMENTS: &str = include_str!("../queries/svelte/assignments.scm");
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum QueryEngineError {
    #[error("failed to compile query for {language}/{category}: {detail}")]
    QueryCompilation {
        language: String,
        category: String,
        detail: String,
    },
    #[error("failed to set parser language: {0}")]
    LanguageError(String),
    #[error("failed to parse source: {0}")]
    ParseError(String),
}

// ---------------------------------------------------------------------------
// Per-language compiled queries
// ---------------------------------------------------------------------------

struct LanguageQueries {
    language: tree_sitter::Language,
    imports: QueryWithCaptures,
    exports: Option<QueryWithCaptures>,
    definitions: QueryWithCaptures,
    calls: QueryWithCaptures,
    assignments: QueryWithCaptures,
}

/// A compiled query bundled with its capture name -> index mapping.
pub(crate) struct QueryWithCaptures {
    pub(crate) query: Query,
    pub(crate) capture_names: HashMap<String, u32>,
}

impl QueryWithCaptures {
    fn new(
        lang: &tree_sitter::Language,
        source: &str,
        lang_name: &str,
        category: &str,
    ) -> Result<Self, QueryEngineError> {
        let query = Query::new(lang, source).map_err(|e| QueryEngineError::QueryCompilation {
            language: lang_name.to_string(),
            category: category.to_string(),
            detail: e.to_string(),
        })?;
        let mut capture_names = HashMap::new();
        for (i, name) in query.capture_names().iter().enumerate() {
            capture_names.insert(name.to_string(), i as u32);
        }
        Ok(Self {
            query,
            capture_names,
        })
    }

    pub(crate) fn capture_index(&self, name: &str) -> Option<u32> {
        self.capture_names.get(name).copied()
    }
}

// ---------------------------------------------------------------------------
// Collected match data (owns all data extracted from streaming iterator)
// ---------------------------------------------------------------------------

/// Owned representation of a single query match.
/// Extracted from the streaming iterator so we can process after iteration.


/// Collect all matches from a streaming iterator into owned data.

// ---------------------------------------------------------------------------
// QueryEngine
// ---------------------------------------------------------------------------

/// Declarative tree-sitter query engine.
///
/// Query compilation is lazy: `.scm` query files are compiled on first use per
/// language, not upfront. This means `QueryEngine::new()` is near-instant and
/// only the languages actually encountered in a diff pay the compilation cost.
pub struct QueryEngine {
    ts_queries: OnceCell<LanguageQueries>,
    py_queries: OnceCell<LanguageQueries>,
    go_queries: OnceCell<LanguageQueries>,
    rust_queries: OnceCell<LanguageQueries>,
    java_queries: OnceCell<LanguageQueries>,
    csharp_queries: OnceCell<LanguageQueries>,
    php_queries: OnceCell<LanguageQueries>,
    ruby_queries: OnceCell<LanguageQueries>,
    kotlin_queries: OnceCell<LanguageQueries>,
    swift_queries: OnceCell<LanguageQueries>,
    c_queries: OnceCell<LanguageQueries>,
    cpp_queries: OnceCell<LanguageQueries>,
    scala_queries: OnceCell<LanguageQueries>,
    // Extras — present only when their `lang-*` feature is enabled.
    #[cfg(feature = "lang-bash")]     bash_queries:     OnceCell<LanguageQueries>,
    #[cfg(feature = "lang-haskell")]  haskell_queries:  OnceCell<LanguageQueries>,
    #[cfg(feature = "lang-nix")]      nix_queries:      OnceCell<LanguageQueries>,
    #[cfg(feature = "lang-lua")]      lua_queries:      OnceCell<LanguageQueries>,
    #[cfg(feature = "lang-perl")]     perl_queries:     OnceCell<LanguageQueries>,
    #[cfg(feature = "lang-elixir")]   elixir_queries:   OnceCell<LanguageQueries>,
    #[cfg(feature = "lang-erlang")]   erlang_queries:   OnceCell<LanguageQueries>,
    #[cfg(feature = "lang-zig")]      zig_queries:      OnceCell<LanguageQueries>,
    #[cfg(feature = "lang-ocaml")]    ocaml_queries:    OnceCell<LanguageQueries>,
    #[cfg(feature = "lang-julia")]    julia_queries:    OnceCell<LanguageQueries>,
    #[cfg(feature = "lang-dart")]     dart_queries:     OnceCell<LanguageQueries>,
    #[cfg(feature = "lang-r")]        r_queries:        OnceCell<LanguageQueries>,
    #[cfg(feature = "lang-fish")]     fish_queries:     OnceCell<LanguageQueries>,
    #[cfg(feature = "lang-html")]     html_queries:     OnceCell<LanguageQueries>,
    #[cfg(feature = "lang-css")]      css_queries:      OnceCell<LanguageQueries>,
    #[cfg(feature = "lang-scss")]     scss_queries:     OnceCell<LanguageQueries>,
    #[cfg(feature = "lang-json")]     json_queries:     OnceCell<LanguageQueries>,
    #[cfg(feature = "lang-yaml")]     yaml_queries:     OnceCell<LanguageQueries>,
    #[cfg(feature = "lang-toml")]     toml_queries:     OnceCell<LanguageQueries>,
    #[cfg(feature = "lang-markdown")] markdown_queries: OnceCell<LanguageQueries>,
    #[cfg(feature = "lang-graphql")]  graphql_queries:  OnceCell<LanguageQueries>,
    #[cfg(feature = "lang-vue")]      vue_queries:      OnceCell<LanguageQueries>,
    #[cfg(feature = "lang-svelte")]   svelte_queries:   OnceCell<LanguageQueries>,
}

/// Compile all `.scm` queries for a single language into a [`LanguageQueries`].
fn compile_queries(
    ts_lang: tree_sitter::Language,
    lang_name: &str,
    imports_src: &str,
    exports_src: Option<&str>,
    definitions_src: &str,
    calls_src: &str,
    assignments_src: &str,
) -> Result<LanguageQueries, QueryEngineError> {
    Ok(LanguageQueries {
        language: ts_lang.clone(),
        imports: QueryWithCaptures::new(&ts_lang, imports_src, lang_name, "imports")?,
        exports: exports_src
            .map(|src| QueryWithCaptures::new(&ts_lang, src, lang_name, "exports"))
            .transpose()?,
        definitions: QueryWithCaptures::new(&ts_lang, definitions_src, lang_name, "definitions")?,
        calls: QueryWithCaptures::new(&ts_lang, calls_src, lang_name, "calls")?,
        assignments: QueryWithCaptures::new(&ts_lang, assignments_src, lang_name, "assignments")?,
    })
}

impl QueryEngine {
    /// Create a new query engine.
    ///
    /// Construction is near-instant: `.scm` query compilation is deferred to
    /// first use per language via [`OnceCell`].
    pub fn new() -> Result<Self, QueryEngineError> {
        Ok(Self {
            ts_queries: OnceCell::new(),
            py_queries: OnceCell::new(),
            go_queries: OnceCell::new(),
            rust_queries: OnceCell::new(),
            java_queries: OnceCell::new(),
            csharp_queries: OnceCell::new(),
            php_queries: OnceCell::new(),
            ruby_queries: OnceCell::new(),
            kotlin_queries: OnceCell::new(),
            swift_queries: OnceCell::new(),
            c_queries: OnceCell::new(),
            cpp_queries: OnceCell::new(),
            scala_queries: OnceCell::new(),
            #[cfg(feature = "lang-bash")]     bash_queries:     OnceCell::new(),
            #[cfg(feature = "lang-haskell")]  haskell_queries:  OnceCell::new(),
            #[cfg(feature = "lang-nix")]      nix_queries:      OnceCell::new(),
            #[cfg(feature = "lang-lua")]      lua_queries:      OnceCell::new(),
            #[cfg(feature = "lang-perl")]     perl_queries:     OnceCell::new(),
            #[cfg(feature = "lang-elixir")]   elixir_queries:   OnceCell::new(),
            #[cfg(feature = "lang-erlang")]   erlang_queries:   OnceCell::new(),
            #[cfg(feature = "lang-zig")]      zig_queries:      OnceCell::new(),
            #[cfg(feature = "lang-ocaml")]    ocaml_queries:    OnceCell::new(),
            #[cfg(feature = "lang-julia")]    julia_queries:    OnceCell::new(),
            #[cfg(feature = "lang-dart")]     dart_queries:     OnceCell::new(),
            #[cfg(feature = "lang-r")]        r_queries:        OnceCell::new(),
            #[cfg(feature = "lang-fish")]     fish_queries:     OnceCell::new(),
            #[cfg(feature = "lang-html")]     html_queries:     OnceCell::new(),
            #[cfg(feature = "lang-css")]      css_queries:      OnceCell::new(),
            #[cfg(feature = "lang-scss")]     scss_queries:     OnceCell::new(),
            #[cfg(feature = "lang-json")]     json_queries:     OnceCell::new(),
            #[cfg(feature = "lang-yaml")]     yaml_queries:     OnceCell::new(),
            #[cfg(feature = "lang-toml")]     toml_queries:     OnceCell::new(),
            #[cfg(feature = "lang-markdown")] markdown_queries: OnceCell::new(),
            #[cfg(feature = "lang-graphql")]  graphql_queries:  OnceCell::new(),
            #[cfg(feature = "lang-vue")]      vue_queries:      OnceCell::new(),
            #[cfg(feature = "lang-svelte")]   svelte_queries:   OnceCell::new(),
        })
    }

    /// Parse a source file and extract symbols, imports, exports, and call sites.
    ///
    /// This is the declarative equivalent of [`crate::ast::parse_file`].
    pub fn parse_file(&self, path: &str, source: &str) -> Result<ParsedFile, QueryEngineError> {
        let language = Language::from_path(path);
        let Some(lq) = self.get_lang_queries(language)? else {
            return Ok(ParsedFile {
                path: path.to_string(),
                language: Language::Unknown,
                definitions: vec![],
                imports: vec![],
                exports: vec![],
                call_sites: vec![],
            });
        };
        self.parse_with_queries(path, source, language, lq)
    }

    /// Extract data flow information (variable assignments from calls, calls with args).
    ///
    /// This is the declarative equivalent of [`crate::ast::extract_data_flow_info`].
    pub fn extract_data_flow(
        &self,
        path: &str,
        source: &str,
    ) -> Result<DataFlowInfo, QueryEngineError> {
        let language = Language::from_path(path);
        let Some(lq) = self.get_lang_queries(language)? else {
            return Ok(DataFlowInfo {
                assignments: vec![],
                calls_with_args: vec![],
            });
        };

        let tree = self.parse_tree(source, language, &lq.language)?;
        let root = tree.root_node();
        let src = source.as_bytes();

        let assignments = self.extract_assignments(&root, src, &lq.assignments, language)?;
        let calls_with_args = self.extract_calls_with_args(&root, src, &lq.calls, language)?;

        Ok(DataFlowInfo {
            assignments,
            calls_with_args,
        })
    }

    // -----------------------------------------------------------------------
    // Language query resolution
    // -----------------------------------------------------------------------

    /// Resolve a [`Language`] to its compiled query set, compiling on first use.
    /// Returns `Ok(None)` for [`Language::Unknown`].
    fn get_lang_queries(
        &self,
        language: Language,
    ) -> Result<Option<&LanguageQueries>, QueryEngineError> {
        match language {
            Language::TypeScript | Language::JavaScript => self
                .ts_queries
                .get_or_try_init(|| {
                    compile_queries(
                        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
                        "typescript",
                        queries::typescript::IMPORTS,
                        Some(queries::typescript::EXPORTS),
                        queries::typescript::DEFINITIONS,
                        queries::typescript::CALLS,
                        queries::typescript::ASSIGNMENTS,
                    )
                })
                .map(Some),
            Language::Python => self
                .py_queries
                .get_or_try_init(|| {
                    compile_queries(
                        tree_sitter_python::LANGUAGE.into(),
                        "python",
                        queries::python::IMPORTS,
                        None,
                        queries::python::DEFINITIONS,
                        queries::python::CALLS,
                        queries::python::ASSIGNMENTS,
                    )
                })
                .map(Some),
            Language::Go => self
                .go_queries
                .get_or_try_init(|| {
                    compile_queries(
                        tree_sitter_go::LANGUAGE.into(),
                        "go",
                        queries::go::IMPORTS,
                        None,
                        queries::go::DEFINITIONS,
                        queries::go::CALLS,
                        queries::go::ASSIGNMENTS,
                    )
                })
                .map(Some),
            Language::Rust => self
                .rust_queries
                .get_or_try_init(|| {
                    compile_queries(
                        tree_sitter_rust::LANGUAGE.into(),
                        "rust",
                        queries::rust::IMPORTS,
                        None,
                        queries::rust::DEFINITIONS,
                        queries::rust::CALLS,
                        queries::rust::ASSIGNMENTS,
                    )
                })
                .map(Some),
            Language::Java => self
                .java_queries
                .get_or_try_init(|| {
                    compile_queries(
                        tree_sitter_java::LANGUAGE.into(),
                        "java",
                        queries::java::IMPORTS,
                        None,
                        queries::java::DEFINITIONS,
                        queries::java::CALLS,
                        queries::java::ASSIGNMENTS,
                    )
                })
                .map(Some),
            Language::CSharp => self
                .csharp_queries
                .get_or_try_init(|| {
                    compile_queries(
                        tree_sitter_c_sharp::LANGUAGE.into(),
                        "csharp",
                        queries::csharp::IMPORTS,
                        None,
                        queries::csharp::DEFINITIONS,
                        queries::csharp::CALLS,
                        queries::csharp::ASSIGNMENTS,
                    )
                })
                .map(Some),
            Language::Php => self
                .php_queries
                .get_or_try_init(|| {
                    compile_queries(
                        tree_sitter_php::LANGUAGE_PHP.into(),
                        "php",
                        queries::php::IMPORTS,
                        None,
                        queries::php::DEFINITIONS,
                        queries::php::CALLS,
                        queries::php::ASSIGNMENTS,
                    )
                })
                .map(Some),
            Language::Ruby => self
                .ruby_queries
                .get_or_try_init(|| {
                    compile_queries(
                        tree_sitter_ruby::LANGUAGE.into(),
                        "ruby",
                        queries::ruby::IMPORTS,
                        None,
                        queries::ruby::DEFINITIONS,
                        queries::ruby::CALLS,
                        queries::ruby::ASSIGNMENTS,
                    )
                })
                .map(Some),
            Language::Kotlin => self
                .kotlin_queries
                .get_or_try_init(|| {
                    compile_queries(
                        tree_sitter_kotlin_ng::LANGUAGE.into(),
                        "kotlin",
                        queries::kotlin::IMPORTS,
                        None,
                        queries::kotlin::DEFINITIONS,
                        queries::kotlin::CALLS,
                        queries::kotlin::ASSIGNMENTS,
                    )
                })
                .map(Some),
            Language::Swift => self
                .swift_queries
                .get_or_try_init(|| {
                    compile_queries(
                        tree_sitter_swift::LANGUAGE.into(),
                        "swift",
                        queries::swift::IMPORTS,
                        None,
                        queries::swift::DEFINITIONS,
                        queries::swift::CALLS,
                        queries::swift::ASSIGNMENTS,
                    )
                })
                .map(Some),
            Language::C => self
                .c_queries
                .get_or_try_init(|| {
                    compile_queries(
                        tree_sitter_c::LANGUAGE.into(),
                        "c",
                        queries::c::IMPORTS,
                        None,
                        queries::c::DEFINITIONS,
                        queries::c::CALLS,
                        queries::c::ASSIGNMENTS,
                    )
                })
                .map(Some),
            Language::Cpp => self
                .cpp_queries
                .get_or_try_init(|| {
                    compile_queries(
                        tree_sitter_cpp::LANGUAGE.into(),
                        "cpp",
                        queries::cpp::IMPORTS,
                        None,
                        queries::cpp::DEFINITIONS,
                        queries::cpp::CALLS,
                        queries::cpp::ASSIGNMENTS,
                    )
                })
                .map(Some),
            Language::Scala => self
                .scala_queries
                .get_or_try_init(|| {
                    compile_queries(
                        tree_sitter_scala::LANGUAGE.into(),
                        "scala",
                        queries::scala::IMPORTS,
                        None,
                        queries::scala::DEFINITIONS,
                        queries::scala::CALLS,
                        queries::scala::ASSIGNMENTS,
                    )
                })
                .map(Some),

            // ── Extras ────────────────────────────────────────────────
            #[cfg(feature = "lang-bash")]
            Language::Bash => self.bash_queries.get_or_try_init(|| compile_queries(
                tree_sitter_bash::LANGUAGE.into(), "bash",
                queries::bash::IMPORTS, None,
                queries::bash::DEFINITIONS, queries::bash::CALLS, queries::bash::ASSIGNMENTS,
            )).map(Some),
            #[cfg(not(feature = "lang-bash"))] Language::Bash => Ok(None),

            #[cfg(feature = "lang-haskell")]
            Language::Haskell => self.haskell_queries.get_or_try_init(|| compile_queries(
                tree_sitter_haskell::LANGUAGE.into(), "haskell",
                queries::haskell::IMPORTS, None,
                queries::haskell::DEFINITIONS, queries::haskell::CALLS, queries::haskell::ASSIGNMENTS,
            )).map(Some),
            #[cfg(not(feature = "lang-haskell"))] Language::Haskell => Ok(None),

            #[cfg(feature = "lang-nix")]
            Language::Nix => self.nix_queries.get_or_try_init(|| compile_queries(
                tree_sitter_nix::LANGUAGE.into(), "nix",
                queries::nix::IMPORTS, None,
                queries::nix::DEFINITIONS, queries::nix::CALLS, queries::nix::ASSIGNMENTS,
            )).map(Some),
            #[cfg(not(feature = "lang-nix"))] Language::Nix => Ok(None),

            #[cfg(feature = "lang-lua")]
            Language::Lua => self.lua_queries.get_or_try_init(|| compile_queries(
                tree_sitter_lua::LANGUAGE.into(), "lua",
                queries::lua::IMPORTS, None,
                queries::lua::DEFINITIONS, queries::lua::CALLS, queries::lua::ASSIGNMENTS,
            )).map(Some),
            #[cfg(not(feature = "lang-lua"))] Language::Lua => Ok(None),

            #[cfg(feature = "lang-perl")]
            Language::Perl => self.perl_queries.get_or_try_init(|| compile_queries(
                tree_sitter_perl_next::LANGUAGE.into(), "perl",
                queries::perl::IMPORTS, None,
                queries::perl::DEFINITIONS, queries::perl::CALLS, queries::perl::ASSIGNMENTS,
            )).map(Some),
            #[cfg(not(feature = "lang-perl"))] Language::Perl => Ok(None),

            #[cfg(feature = "lang-elixir")]
            Language::Elixir => self.elixir_queries.get_or_try_init(|| compile_queries(
                tree_sitter_elixir::LANGUAGE.into(), "elixir",
                queries::elixir::IMPORTS, None,
                queries::elixir::DEFINITIONS, queries::elixir::CALLS, queries::elixir::ASSIGNMENTS,
            )).map(Some),
            #[cfg(not(feature = "lang-elixir"))] Language::Elixir => Ok(None),

            #[cfg(feature = "lang-erlang")]
            Language::Erlang => self.erlang_queries.get_or_try_init(|| compile_queries(
                tree_sitter_erlang::LANGUAGE.into(), "erlang",
                queries::erlang::IMPORTS, None,
                queries::erlang::DEFINITIONS, queries::erlang::CALLS, queries::erlang::ASSIGNMENTS,
            )).map(Some),
            #[cfg(not(feature = "lang-erlang"))] Language::Erlang => Ok(None),

            #[cfg(feature = "lang-zig")]
            Language::Zig => self.zig_queries.get_or_try_init(|| compile_queries(
                tree_sitter_zig::LANGUAGE.into(), "zig",
                queries::zig::IMPORTS, None,
                queries::zig::DEFINITIONS, queries::zig::CALLS, queries::zig::ASSIGNMENTS,
            )).map(Some),
            #[cfg(not(feature = "lang-zig"))] Language::Zig => Ok(None),

            #[cfg(feature = "lang-ocaml")]
            Language::OCaml => self.ocaml_queries.get_or_try_init(|| compile_queries(
                tree_sitter_ocaml::LANGUAGE_OCAML.into(), "ocaml",
                queries::ocaml::IMPORTS, None,
                queries::ocaml::DEFINITIONS, queries::ocaml::CALLS, queries::ocaml::ASSIGNMENTS,
            )).map(Some),
            #[cfg(not(feature = "lang-ocaml"))] Language::OCaml => Ok(None),

            #[cfg(feature = "lang-julia")]
            Language::Julia => self.julia_queries.get_or_try_init(|| compile_queries(
                tree_sitter_julia::LANGUAGE.into(), "julia",
                queries::julia::IMPORTS, None,
                queries::julia::DEFINITIONS, queries::julia::CALLS, queries::julia::ASSIGNMENTS,
            )).map(Some),
            #[cfg(not(feature = "lang-julia"))] Language::Julia => Ok(None),

            #[cfg(feature = "lang-dart")]
            Language::Dart => self.dart_queries.get_or_try_init(|| compile_queries(
                tree_sitter_dart_orchard::LANGUAGE.into(), "dart",
                queries::dart::IMPORTS, None,
                queries::dart::DEFINITIONS, queries::dart::CALLS, queries::dart::ASSIGNMENTS,
            )).map(Some),
            #[cfg(not(feature = "lang-dart"))] Language::Dart => Ok(None),

            #[cfg(feature = "lang-r")]
            Language::R => self.r_queries.get_or_try_init(|| compile_queries(
                tree_sitter_r::LANGUAGE.into(), "r",
                queries::r::IMPORTS, None,
                queries::r::DEFINITIONS, queries::r::CALLS, queries::r::ASSIGNMENTS,
            )).map(Some),
            #[cfg(not(feature = "lang-r"))] Language::R => Ok(None),

            #[cfg(feature = "lang-fish")]
            Language::Fish => self.fish_queries.get_or_try_init(|| compile_queries(
                tree_sitter_fish::language(), "fish",
                queries::fish::IMPORTS, None,
                queries::fish::DEFINITIONS, queries::fish::CALLS, queries::fish::ASSIGNMENTS,
            )).map(Some),
            #[cfg(not(feature = "lang-fish"))] Language::Fish => Ok(None),

            #[cfg(feature = "lang-html")]
            Language::Html => self.html_queries.get_or_try_init(|| compile_queries(
                tree_sitter_html::LANGUAGE.into(), "html",
                queries::html::IMPORTS, None,
                queries::html::DEFINITIONS, queries::html::CALLS, queries::html::ASSIGNMENTS,
            )).map(Some),
            #[cfg(not(feature = "lang-html"))] Language::Html => Ok(None),

            #[cfg(feature = "lang-css")]
            Language::Css => self.css_queries.get_or_try_init(|| compile_queries(
                tree_sitter_css::LANGUAGE.into(), "css",
                queries::css::IMPORTS, None,
                queries::css::DEFINITIONS, queries::css::CALLS, queries::css::ASSIGNMENTS,
            )).map(Some),
            #[cfg(not(feature = "lang-css"))] Language::Css => Ok(None),

            #[cfg(feature = "lang-scss")]
            Language::Scss => self.scss_queries.get_or_try_init(|| compile_queries(
                tree_sitter_scss::language(), "scss",
                queries::scss::IMPORTS, None,
                queries::scss::DEFINITIONS, queries::scss::CALLS, queries::scss::ASSIGNMENTS,
            )).map(Some),
            #[cfg(not(feature = "lang-scss"))] Language::Scss => Ok(None),

            #[cfg(feature = "lang-json")]
            Language::Json => self.json_queries.get_or_try_init(|| compile_queries(
                tree_sitter_json::LANGUAGE.into(), "json",
                queries::json::IMPORTS, None,
                queries::json::DEFINITIONS, queries::json::CALLS, queries::json::ASSIGNMENTS,
            )).map(Some),
            #[cfg(not(feature = "lang-json"))] Language::Json => Ok(None),

            #[cfg(feature = "lang-yaml")]
            Language::Yaml => self.yaml_queries.get_or_try_init(|| compile_queries(
                tree_sitter_yaml::LANGUAGE.into(), "yaml",
                queries::yaml::IMPORTS, None,
                queries::yaml::DEFINITIONS, queries::yaml::CALLS, queries::yaml::ASSIGNMENTS,
            )).map(Some),
            #[cfg(not(feature = "lang-yaml"))] Language::Yaml => Ok(None),

            #[cfg(feature = "lang-toml")]
            Language::Toml => self.toml_queries.get_or_try_init(|| compile_queries(
                tree_sitter_toml_ng::LANGUAGE.into(), "toml",
                queries::toml::IMPORTS, None,
                queries::toml::DEFINITIONS, queries::toml::CALLS, queries::toml::ASSIGNMENTS,
            )).map(Some),
            #[cfg(not(feature = "lang-toml"))] Language::Toml => Ok(None),

            #[cfg(feature = "lang-markdown")]
            Language::Markdown => self.markdown_queries.get_or_try_init(|| compile_queries(
                tree_sitter_md::LANGUAGE.into(), "markdown",
                queries::markdown::IMPORTS, None,
                queries::markdown::DEFINITIONS, queries::markdown::CALLS, queries::markdown::ASSIGNMENTS,
            )).map(Some),
            #[cfg(not(feature = "lang-markdown"))] Language::Markdown => Ok(None),

            #[cfg(feature = "lang-graphql")]
            Language::GraphQl => self.graphql_queries.get_or_try_init(|| compile_queries(
                tree_sitter_graphql::LANGUAGE.into(), "graphql",
                queries::graphql::IMPORTS, None,
                queries::graphql::DEFINITIONS, queries::graphql::CALLS, queries::graphql::ASSIGNMENTS,
            )).map(Some),
            #[cfg(not(feature = "lang-graphql"))] Language::GraphQl => Ok(None),

            #[cfg(feature = "lang-vue")]
            Language::Vue => self.vue_queries.get_or_try_init(|| compile_queries(
                tree_sitter_vue_next::LANGUAGE.into(), "vue",
                queries::vue::IMPORTS, None,
                queries::vue::DEFINITIONS, queries::vue::CALLS, queries::vue::ASSIGNMENTS,
            )).map(Some),
            #[cfg(not(feature = "lang-vue"))] Language::Vue => Ok(None),

            #[cfg(feature = "lang-svelte")]
            Language::Svelte => self.svelte_queries.get_or_try_init(|| compile_queries(
                tree_sitter_svelte_next::LANGUAGE.into(), "svelte",
                queries::svelte::IMPORTS, None,
                queries::svelte::DEFINITIONS, queries::svelte::CALLS, queries::svelte::ASSIGNMENTS,
            )).map(Some),
            #[cfg(not(feature = "lang-svelte"))] Language::Svelte => Ok(None),

            Language::Unknown => Ok(None),
        }
    }

    // -----------------------------------------------------------------------
    // Tree parsing
    // -----------------------------------------------------------------------

    fn parse_tree(
        &self,
        source: &str,
        language: Language,
        ts_lang: &tree_sitter::Language,
    ) -> Result<tree_sitter::Tree, QueryEngineError> {
        THREAD_PARSERS.with(|parsers| {
            let mut parsers = parsers.borrow_mut();
            if !parsers.contains_key(&language) {
                let mut p = Parser::new();
                p.set_language(ts_lang)
                    .map_err(|e| QueryEngineError::LanguageError(e.to_string()))?;
                parsers.insert(language, p);
            }
            let parser = parsers.get_mut(&language).unwrap();
            parser
                .parse(source, None)
                .ok_or_else(|| QueryEngineError::ParseError("tree-sitter failed to parse".into()))
        })
    }

    /// Parse source into a tree-sitter `Tree` for the given file path.
    ///
    /// Returns the tree and detected language. For unknown languages, returns
    /// `Ok(None)` — callers should produce empty results.
    pub fn parse_tree_for_path(
        &self,
        path: &str,
        source: &str,
    ) -> Result<Option<(tree_sitter::Tree, Language)>, QueryEngineError> {
        let language = Language::from_path(path);
        let Some(lq) = self.get_lang_queries(language)? else {
            return Ok(None);
        };
        let tree = self.parse_tree(source, language, &lq.language)?;
        Ok(Some((tree, language)))
    }

    // -----------------------------------------------------------------------
    // Parse with pre-parsed tree (avoids double parse)
    // -----------------------------------------------------------------------

    /// Like [`parse_file`](Self::parse_file) but reuses an already-parsed tree.
    pub fn parse_file_with_tree(
        &self,
        path: &str,
        source: &str,
        tree: &tree_sitter::Tree,
        language: Language,
    ) -> Result<ParsedFile, QueryEngineError> {
        let Some(lq) = self.get_lang_queries(language)? else {
            return Ok(ParsedFile {
                path: path.to_string(),
                language: Language::Unknown,
                definitions: vec![],
                imports: vec![],
                exports: vec![],
                call_sites: vec![],
            });
        };
        self.parse_with_queries_from_tree(path, source, language, lq, tree)
    }

    /// Like [`extract_data_flow`](Self::extract_data_flow) but reuses an already-parsed tree.
    pub fn extract_data_flow_with_tree(
        &self,
        _path: &str,
        source: &str,
        tree: &tree_sitter::Tree,
        language: Language,
    ) -> Result<DataFlowInfo, QueryEngineError> {
        let Some(lq) = self.get_lang_queries(language)? else {
            return Ok(DataFlowInfo {
                assignments: vec![],
                calls_with_args: vec![],
            });
        };

        let root = tree.root_node();
        let src = source.as_bytes();

        let assignments = self.extract_assignments(&root, src, &lq.assignments, language)?;
        let calls_with_args = self.extract_calls_with_args(&root, src, &lq.calls, language)?;

        Ok(DataFlowInfo {
            assignments,
            calls_with_args,
        })
    }

    // -----------------------------------------------------------------------
    // Internal: generic parse
    // -----------------------------------------------------------------------

    fn parse_with_queries(
        &self,
        path: &str,
        source: &str,
        language: Language,
        lang_queries: &LanguageQueries,
    ) -> Result<ParsedFile, QueryEngineError> {
        let tree = self.parse_tree(source, language, &lang_queries.language)?;
        self.parse_with_queries_from_tree(path, source, language, lang_queries, &tree)
    }

    /// Core extraction logic — shared by both fresh-parse and tree-reuse paths.
    fn parse_with_queries_from_tree(
        &self,
        path: &str,
        source: &str,
        language: Language,
        lang_queries: &LanguageQueries,
        tree: &tree_sitter::Tree,
    ) -> Result<ParsedFile, QueryEngineError> {
        let root = tree.root_node();
        let src = source.as_bytes();

        let imports = self.extract_imports(&root, src, &lang_queries.imports, language)?;
        let exports = if let Some(ref eq) = lang_queries.exports {
            self.extract_exports(&root, src, eq)?
        } else {
            vec![]
        };
        let mut definitions =
            self.extract_definitions(&root, src, &lang_queries.definitions, language)?;

        // For exports that also introduce definitions (exported declarations),
        // merge any definitions extracted from the exports query.
        if let Some(ref eq) = lang_queries.exports {
            let export_defs = self.extract_export_definitions(&root, src, eq)?;
            for def in export_defs {
                if !definitions
                    .iter()
                    .any(|d| d.name == def.name && d.kind == def.kind)
                {
                    definitions.push(def);
                }
            }
        }

        let call_sites = self.extract_call_sites(&root, src, &lang_queries.calls, language)?;

        Ok(ParsedFile {
            path: path.to_string(),
            language,
            definitions,
            imports,
            exports,
            call_sites,
        })
    }

    // -----------------------------------------------------------------------
    // Import extraction
    // -----------------------------------------------------------------------

    fn extract_imports(
        &self,
        root: &Node,
        source: &[u8],
        qwc: &QueryWithCaptures,
        language: Language,
    ) -> Result<Vec<ImportInfo>, QueryEngineError> {
        match language {
            Language::TypeScript | Language::JavaScript => {
                crate::languages::typescript::extract_imports(root, source, qwc)
            }
            Language::Python => crate::languages::python::extract_imports(root, source, qwc),
            Language::Go => crate::languages::go::extract_imports(root, source, qwc),
            Language::Rust => crate::languages::rust::extract_imports(root, source, qwc),
            Language::Java => crate::languages::java::extract_imports(root, source, qwc),
            Language::CSharp => crate::languages::csharp::extract_imports(root, source, qwc),
            Language::Php => crate::languages::php::extract_imports(root, source, qwc),
            Language::Ruby => crate::languages::ruby::extract_imports(root, source, qwc),
            Language::Kotlin => crate::languages::kotlin::extract_imports(root, source, qwc),
            Language::Swift => crate::languages::swift::extract_imports(root, source, qwc),
            Language::C => crate::languages::c::extract_imports(root, source, qwc),
            Language::Cpp => crate::languages::cpp::extract_imports(root, source, qwc),
            Language::Scala => crate::languages::scala::extract_imports(root, source, qwc),
            _ => crate::languages::common::extract_minimal_imports(root, source, qwc),
        }
    }

    /// Generic fallback used by languages that do not (yet) have a bespoke
    /// per-language import extractor. Walks every match in the query, looks
    /// for a `@source` capture (the import path) and an optional `@stmt`
    /// capture (the surrounding statement node, used only for line numbers),
    /// and emits a minimal `ImportInfo` carrying just the source string and
    /// line.
    ///
    /// This is intentionally low-fidelity — `names`, `is_default`, and
    /// `is_namespace` are all left empty/false. Downstream consumers that
    /// only resolve cross-file edges by source path (graph.rs, framework
    /// detection) work correctly; consumers that need named-import detail
    /// will see the language as having no named imports until a bespoke
    /// extractor is wired in.
    ///
    /// Quoting / bracketing is stripped from `@source`: leading/trailing
    /// `"` `'` `` ` `` `<` `>` characters are removed so the resulting
    /// string is a comparable path (e.g. `<stdio.h>` → `stdio.h`,
    /// `"react"` → `react`).













    // -----------------------------------------------------------------------
    // Export extraction (TypeScript only)
    // -----------------------------------------------------------------------

    fn extract_exports(
        &self,
        root: &Node,
        source: &[u8],
        qwc: &QueryWithCaptures,
    ) -> Result<Vec<ExportInfo>, QueryEngineError> {
        let mut cursor = QueryCursor::new();
        let matches = collect_matches(&mut cursor, &qwc.query, *root, source);

        let stmt_idx = qwc.capture_index("stmt");
        let export_name_idx = qwc.capture_index("export_name");
        let reexport_name_idx = qwc.capture_index("reexport_name");
        let reexport_source_idx = qwc.capture_index("reexport_source");
        let wildcard_source_idx = qwc.capture_index("wildcard_source");
        let decl_fn_name_idx = qwc.capture_index("decl_fn_name");
        let decl_gen_name_idx = qwc.capture_index("decl_gen_name");
        let decl_class_name_idx = qwc.capture_index("decl_class_name");
        let decl_abstract_name_idx = qwc.capture_index("decl_abstract_name");
        let decl_iface_name_idx = qwc.capture_index("decl_iface_name");
        let decl_type_name_idx = qwc.capture_index("decl_type_name");
        let decl_var_name_idx = qwc.capture_index("decl_var_name");

        let mut exports = Vec::new();

        // Exported declaration name captures — order doesn't matter,
        // we dispatch by which capture is present.
        let decl_name_captures: &[(Option<u32>, SymbolKind)] = &[
            (decl_fn_name_idx, SymbolKind::Function),
            (decl_gen_name_idx, SymbolKind::Function),
            (decl_class_name_idx, SymbolKind::Class),
            (decl_abstract_name_idx, SymbolKind::Class),
            (decl_iface_name_idx, SymbolKind::Interface),
            (decl_type_name_idx, SymbolKind::TypeAlias),
            (decl_var_name_idx, SymbolKind::Constant),
        ];

        for m in &matches {
            let mut line = 0usize;
            let mut stmt_node: Option<Node> = None;

            for &(idx, node) in &m.captures {
                if Some(idx) == stmt_idx {
                    line = node.start_position().row + 1;
                    stmt_node = Some(node);
                }
            }

            let is_default = stmt_node.map(|n| has_default_keyword(&n)).unwrap_or(false);

            if m.has_capture(reexport_name_idx) {
                // export { baz } from './other'
                let mut reexport_src = String::new();
                for &(idx, node) in &m.captures {
                    if Some(idx) == reexport_source_idx {
                        reexport_src = node_text(&node, source).to_string();
                    }
                }
                for &(idx, node) in &m.captures {
                    if Some(idx) == reexport_name_idx {
                        exports.push(ExportInfo {
                            name: node_text(&node, source).to_string(),
                            is_default: false,
                            is_reexport: true,
                            source: Some(reexport_src.clone()),
                            line,
                        });
                    }
                }
            } else if m.has_capture(export_name_idx) {
                // export { foo, bar }
                for &(idx, node) in &m.captures {
                    if Some(idx) == export_name_idx {
                        exports.push(ExportInfo {
                            name: node_text(&node, source).to_string(),
                            is_default: false,
                            is_reexport: false,
                            source: None,
                            line,
                        });
                    }
                }
            } else if m.has_capture(wildcard_source_idx) {
                // export * from './other'
                // Only treat as wildcard if there's no export_clause child
                // (re-export pattern already handles those).
                let has_export_clause = stmt_node
                    .map(|n| {
                        let mut c = n.walk();
                        let result = n
                            .named_children(&mut c)
                            .any(|ch| ch.kind() == "export_clause");
                        result
                    })
                    .unwrap_or(false);
                if !has_export_clause {
                    for &(idx, node) in &m.captures {
                        if Some(idx) == wildcard_source_idx {
                            exports.push(ExportInfo {
                                name: "*".to_string(),
                                is_default: false,
                                is_reexport: true,
                                source: Some(node_text(&node, source).to_string()),
                                line,
                            });
                        }
                    }
                }
            } else {
                // Exported declarations: function, generator, class, abstract, interface, type, variable
                for &(name_cap_idx, _) in decl_name_captures {
                    if m.has_capture(name_cap_idx) {
                        for &(idx, node) in &m.captures {
                            if Some(idx) == name_cap_idx {
                                exports.push(ExportInfo {
                                    name: node_text(&node, source).to_string(),
                                    is_default,
                                    is_reexport: false,
                                    source: None,
                                    line,
                                });
                            }
                        }
                        break;
                    }
                }
            }
        }

        dedup_exports(&mut exports);

        Ok(exports)
    }

    /// Extract definitions from exported declarations in exports.scm.
    /// Only matches that have a `decl_*_name` capture are declarations.
    fn extract_export_definitions(
        &self,
        root: &Node,
        source: &[u8],
        qwc: &QueryWithCaptures,
    ) -> Result<Vec<Definition>, QueryEngineError> {
        let mut cursor = QueryCursor::new();
        let matches = collect_matches(&mut cursor, &qwc.query, *root, source);

        let decl_fn_name_idx = qwc.capture_index("decl_fn_name");
        let decl_gen_name_idx = qwc.capture_index("decl_gen_name");
        let decl_class_name_idx = qwc.capture_index("decl_class_name");
        let decl_abstract_name_idx = qwc.capture_index("decl_abstract_name");
        let decl_iface_name_idx = qwc.capture_index("decl_iface_name");
        let decl_type_name_idx = qwc.capture_index("decl_type_name");
        let decl_var_name_idx = qwc.capture_index("decl_var_name");
        let stmt_idx = qwc.capture_index("stmt");

        let decl_name_captures: &[(Option<u32>, SymbolKind)] = &[
            (decl_fn_name_idx, SymbolKind::Function),
            (decl_gen_name_idx, SymbolKind::Function),
            (decl_class_name_idx, SymbolKind::Class),
            (decl_abstract_name_idx, SymbolKind::Class),
            (decl_iface_name_idx, SymbolKind::Interface),
            (decl_type_name_idx, SymbolKind::TypeAlias),
            (decl_var_name_idx, SymbolKind::Constant),
        ];

        let mut definitions = Vec::new();

        for m in &matches {
            let mut decl_node: Option<Node> = None;

            for &(idx, node) in &m.captures {
                if Some(idx) == stmt_idx {
                    if let Some(decl) = node.child_by_field_name("declaration") {
                        decl_node = Some(decl);
                    }
                }
            }

            // Find which declaration capture is present
            let mut found = false;
            for &(name_cap_idx, kind) in decl_name_captures {
                if m.has_capture(name_cap_idx) {
                    let mut name_text = String::new();
                    for &(idx, node) in &m.captures {
                        if Some(idx) == name_cap_idx {
                            name_text = node_text(&node, source).to_string();
                        }
                    }
                    if !name_text.is_empty() {
                        let (start_line, end_line) = if let Some(dn) = decl_node {
                            (dn.start_position().row + 1, dn.end_position().row + 1)
                        } else {
                            (0, 0)
                        };
                        definitions.push(Definition {
                            name: name_text,
                            kind,
                            start_line,
                            end_line,
                        });
                    }
                    found = true;
                    break;
                }
            }
            let _ = found; // suppress unused warning
        }

        Ok(definitions)
    }

    // -----------------------------------------------------------------------
    // Definition extraction
    // -----------------------------------------------------------------------

    fn extract_definitions(
        &self,
        root: &Node,
        source: &[u8],
        qwc: &QueryWithCaptures,
        language: Language,
    ) -> Result<Vec<Definition>, QueryEngineError> {
        let mut cursor = QueryCursor::new();
        let matches = collect_matches(&mut cursor, &qwc.query, *root, source);

        let mut definitions = Vec::new();
        let mut seen_nodes: Vec<(usize, usize)> = Vec::new();

        match language {
            Language::TypeScript | Language::JavaScript => {
                crate::languages::typescript::extract_definitions(&matches, source, qwc, &mut definitions, &mut seen_nodes);
            }
            Language::Python => {
                crate::languages::python::extract_definitions(&matches, source, qwc, &mut definitions, &mut seen_nodes);
            }
            Language::Go => {
                crate::languages::go::extract_definitions(&matches, source, qwc, &mut definitions, &mut seen_nodes);
            }
            Language::Java => {
                crate::languages::java::extract_definitions(&matches, source, qwc, &mut definitions, &mut seen_nodes);
            }
            Language::Rust => {
                crate::languages::rust::extract_definitions(&matches, source, qwc, &mut definitions, &mut seen_nodes);
            }
            Language::CSharp => {
                crate::languages::csharp::extract_definitions(&matches, source, qwc, &mut definitions, &mut seen_nodes);
            }
            Language::Php => {
                crate::languages::php::extract_definitions(&matches, source, qwc, &mut definitions, &mut seen_nodes);
            }
            Language::Ruby => {
                crate::languages::ruby::extract_definitions(&matches, source, qwc, &mut definitions, &mut seen_nodes);
            }
            Language::Kotlin => {
                crate::languages::kotlin::extract_definitions(&matches, source, qwc, &mut definitions, &mut seen_nodes);
            }
            Language::Swift => {
                crate::languages::swift::extract_definitions(&matches, source, qwc, &mut definitions, &mut seen_nodes);
            }
            Language::C => {
                crate::languages::c::extract_definitions(&matches, source, qwc, &mut definitions, &mut seen_nodes);
            }
            Language::Cpp => {
                crate::languages::cpp::extract_definitions(&matches, source, qwc, &mut definitions, &mut seen_nodes);
            }
            Language::Scala => {
                crate::languages::scala::extract_definitions(&matches, source, qwc, &mut definitions, &mut seen_nodes);
            }
            // Catch-all for languages without a bespoke per-kind dispatch
            // arm (the 23 newly-added grammars, plus data/markup formats
            // like JSON, Markdown, YAML, TOML — though the latter typically
            // produce no definitions and are no-ops here). Languages whose
            // definitions.scm follows the standard `@name` +
            // `@definition.<kind>` convention are extracted automatically.
            _ => {
                extract_definitions_standard(
                    &matches,
                    source,
                    qwc,
                    &mut definitions,
                    &mut seen_nodes,
                );
            }
        }

        Ok(definitions)
    }

    // -----------------------------------------------------------------------
    // Call site extraction
    // -----------------------------------------------------------------------

    fn extract_call_sites(
        &self,
        root: &Node,
        source: &[u8],
        qwc: &QueryWithCaptures,
        language: Language,
    ) -> Result<Vec<CallSite>, QueryEngineError> {
        let mut cursor = QueryCursor::new();
        let matches = collect_matches(&mut cursor, &qwc.query, *root, source);

        let callee_idx = qwc.capture_index("callee");
        let node_idx = qwc.capture_index("node");

        let mut call_sites = Vec::new();

        for m in &matches {
            let mut callee_text = String::new();
            let mut call_line = 0;
            let mut call_node: Option<Node> = None;

            for &(idx, node) in &m.captures {
                if Some(idx) == callee_idx {
                    callee_text = node_text(&node, source).to_string();
                }
                if Some(idx) == node_idx {
                    call_line = node.start_position().row + 1;
                    call_node = Some(node);
                }
            }

            if !callee_text.is_empty() {
                let containing =
                    call_node.and_then(|n| find_containing_function(&n, source, language));
                call_sites.push(CallSite {
                    callee: callee_text,
                    line: call_line,
                    containing_function: containing,
                });
            }
        }

        Ok(call_sites)
    }

    // -----------------------------------------------------------------------
    // Assignment extraction (data flow)
    // -----------------------------------------------------------------------

    fn extract_assignments(
        &self,
        root: &Node,
        source: &[u8],
        qwc: &QueryWithCaptures,
        language: Language,
    ) -> Result<Vec<VarCallAssignment>, QueryEngineError> {
        let mut cursor = QueryCursor::new();
        let matches = collect_matches(&mut cursor, &qwc.query, *root, source);

        let var_name_idx = qwc.capture_index("var_name");
        let callee_idx = qwc.capture_index("callee");
        let node_idx = qwc.capture_index("node");

        let mut assignments = Vec::new();

        for m in &matches {
            let mut var_name = String::new();
            let mut callee_text = String::new();
            let mut line = 0;
            let mut assign_node: Option<Node> = None;

            for &(idx, node) in &m.captures {
                if Some(idx) == var_name_idx {
                    var_name = node_text(&node, source).to_string();
                }
                if Some(idx) == callee_idx {
                    callee_text = node_text(&node, source).to_string();
                }
                if Some(idx) == node_idx {
                    line = node.start_position().row + 1;
                    assign_node = Some(node);
                }
            }

            if !var_name.is_empty() && !callee_text.is_empty() {
                let containing =
                    assign_node.and_then(|n| find_containing_function(&n, source, language));
                assignments.push(VarCallAssignment {
                    variable: var_name,
                    callee: callee_text,
                    line,
                    containing_function: containing,
                });
            }
        }

        Ok(assignments)
    }

    // -----------------------------------------------------------------------
    // Calls with arguments extraction (data flow)
    // -----------------------------------------------------------------------

    fn extract_calls_with_args(
        &self,
        root: &Node,
        source: &[u8],
        qwc: &QueryWithCaptures,
        language: Language,
    ) -> Result<Vec<CallWithArgs>, QueryEngineError> {
        let mut cursor = QueryCursor::new();
        let matches = collect_matches(&mut cursor, &qwc.query, *root, source);

        let callee_idx = qwc.capture_index("callee");
        let args_idx = qwc.capture_index("args");
        let node_idx = qwc.capture_index("node");

        let mut calls = Vec::new();

        for m in &matches {
            let mut callee_text = String::new();
            let mut arguments = Vec::new();
            let mut line = 0;
            let mut call_node: Option<Node> = None;

            for &(idx, node) in &m.captures {
                if Some(idx) == callee_idx {
                    callee_text = node_text(&node, source).to_string();
                }
                if Some(idx) == args_idx {
                    arguments = extract_arg_texts(&node, source, language);
                }
                if Some(idx) == node_idx {
                    line = node.start_position().row + 1;
                    call_node = Some(node);
                }
            }

            if !callee_text.is_empty() {
                let containing =
                    call_node.and_then(|n| find_containing_function(&n, source, language));
                calls.push(CallWithArgs {
                    callee: callee_text,
                    arguments,
                    line,
                    containing_function: containing,
                });
            }
        }

        Ok(calls)
    }
}

// ---------------------------------------------------------------------------
// Helper types
// ---------------------------------------------------------------------------



// ---------------------------------------------------------------------------
// Free helper functions
// ---------------------------------------------------------------------------


/// Extract (start_line, end_line, start_byte) from a node capture.

// ---------------------------------------------------------------------------
// Standard tree-sitter "tags" capture-name convention
// ---------------------------------------------------------------------------
//
// The community standard for code-navigation queries (used by GitHub's
// universal-ctags adapter, nvim-treesitter, helix, zed, …) tags every
// definition or reference with a structured capture name:
//
//     (function_declaration name: (identifier) @name) @definition.function
//     (class_declaration    name: (identifier) @name) @definition.class
//     (call_expression      function: (_)      @name) @reference.call
//
// Every match contains a single `@name` capture and exactly one
// `@definition.<kind>` (or `@reference.call`) capture that names the
// node and encodes its kind in one place. The kind suffix maps directly
// onto our [`SymbolKind`] enum.
//
// Adopting this convention for `definitions.scm` and `calls.scm` lets us
// drop in upstream `queries/tags.scm` files almost verbatim across the ~25
// new languages we now support — there's no need to invent a unique
// per-kind capture name (`fn_name`, `class_name`, …) for each grammar.
//
// `imports.scm` and `assignments.scm` deliberately keep their bespoke
// per-language capture vocabulary because our IR (`ImportInfo`, `IrPattern`)
// carries strictly more structure than the standard "tags" set provides.

/// Map a `definition.<kind>` suffix onto our [`SymbolKind`].
///
/// Returns `None` for definition kinds we do not currently model (we choose
/// to skip them rather than collapse onto a wrong kind).

/// Run the standard-convention extractor on a set of matches.
///
/// Walks each match looking for a `@definition.<kind>` capture together
/// with the conventional `@name` capture, deduplicating by `(node_start, name)`
/// the same way the bespoke per-language extractors do.
///
/// Returns `Ok(true)` if at least one definition was emitted — callers use
/// this as a signal that the standard path "took" and bespoke fallback can
/// be skipped.

/// Standard-convention extractor for call sites.
///
/// Recognises `(call_expression function: (_) @name) @reference.call` and
/// the closely related `@reference.call.method` / `@reference.call.constructor`
/// variants used by some upstream `tags.scm` files.
///
/// Returns `Ok(true)` if at least one call was emitted.
#[allow(dead_code)] // wired in once a language opts in via its calls.scm


/// Check if an export_statement node has the `default` keyword.
fn has_default_keyword(node: &Node) -> bool {
    let mut cursor = node.walk();
    let result = node.children(&mut cursor).any(|ch| ch.kind() == "default");
    result
}

/// Deduplicate exports: when both a plain export and a re-export match the
/// same specifier, keep only the re-export version.
fn dedup_exports(exports: &mut Vec<ExportInfo>) {
    let mut seen: HashMap<(String, usize), usize> = HashMap::new();
    let mut to_remove = Vec::new();

    for (i, exp) in exports.iter().enumerate() {
        let key = (exp.name.clone(), exp.line);
        if let Some(&prev) = seen.get(&key) {
            if exp.is_reexport && !exports[prev].is_reexport {
                to_remove.push(prev);
                seen.insert(key, i);
            } else {
                to_remove.push(i);
            }
        } else {
            seen.insert(key, i);
        }
    }

    to_remove.sort_unstable();
    to_remove.dedup();
    for i in to_remove.into_iter().rev() {
        exports.remove(i);
    }
}

/// Walk up from a node to find the nearest containing function declaration.

/// Extract argument texts from an arguments/argument_list node.

/// Simple string hash for deduplication keys.

// ---------------------------------------------------------------------------
// Shared test QueryEngine (file-level, reused by all test modules)
// ---------------------------------------------------------------------------

/// Single shared `QueryEngine` across all test modules in this file.
/// Lazy query compilation per language happens once and is reused by every test.
#[cfg(test)]
pub(crate) fn shared_test_engine() -> &'static QueryEngine {
    use std::sync::OnceLock;
    static ENGINE: OnceLock<QueryEngine> = OnceLock::new();
    ENGINE.get_or_init(|| QueryEngine::new().expect("shared test QueryEngine init"))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr
)]
mod tests {
    use super::*;

    fn engine() -> &'static QueryEngine {
        shared_test_engine()
    }

    // === Construction ===

    #[test]
    fn test_engine_construction() {
        let _e = engine();
    }

    // === TypeScript imports ===








    // === TypeScript exports ===







    // === TypeScript definitions ===







    // === TypeScript call sites ===




    // === TypeScript data flow ===




    // === Python imports ===





    // === Python definitions ===




    // === Python call sites ===


    // === Python data flow ===




    // === Unknown language ===

    #[test]
    fn test_unknown_language_returns_empty() {
        let e = engine();
        let result = e.parse_file("data.csv", "a,b,c").unwrap();
        assert_eq!(result.language, Language::Unknown);
        assert!(result.definitions.is_empty());
        assert!(result.imports.is_empty());
    }

    #[test]
    fn test_unknown_language_data_flow() {
        let e = engine();
        let df = e.extract_data_flow("data.csv", "a,b,c").unwrap();
        assert!(df.assignments.is_empty());
        assert!(df.calls_with_args.is_empty());
    }

    // === Parity with ast.rs ===



    // === Determinism ===

    #[test]
    fn test_deterministic_output() {
        let source = r#"
import { a, b, c } from './mod';
export function process(data: string) {
    const result = transform(data);
    return save(result);
}
"#;
        let e = engine();
        let r1 = e.parse_file("app.ts", source).unwrap();
        let r2 = e.parse_file("app.ts", source).unwrap();
        assert_eq!(r1, r2);
    }

    // === Edge cases ===

    #[test]
    fn test_empty_source() {
        let e = engine();
        let result = e.parse_file("app.ts", "").unwrap();
        assert!(result.definitions.is_empty());
        assert!(result.imports.is_empty());
        assert!(result.exports.is_empty());
        assert!(result.call_sites.is_empty());
    }

    #[test]
    fn test_syntax_error_still_parses() {
        let e = engine();
        let result = e.parse_file("app.ts", "import { from 'broken;");
        assert!(result.is_ok());
    }
}

// ---------------------------------------------------------------------------
// Lazy initialization tests (Phase 12.3)
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr
)]
mod lazy_init_tests {
    use super::*;

    #[test]
    fn new_is_instant_no_queries_compiled() {
        // Construction should succeed without compiling any queries.
        let engine = QueryEngine::new().unwrap();
        // All OnceCells should be uninitialized.
        assert!(engine.ts_queries.get().is_none());
        assert!(engine.py_queries.get().is_none());
        assert!(engine.go_queries.get().is_none());
        assert!(engine.rust_queries.get().is_none());
        assert!(engine.java_queries.get().is_none());
        assert!(engine.csharp_queries.get().is_none());
        assert!(engine.php_queries.get().is_none());
        assert!(engine.ruby_queries.get().is_none());
        assert!(engine.kotlin_queries.get().is_none());
        assert!(engine.swift_queries.get().is_none());
        assert!(engine.c_queries.get().is_none());
        assert!(engine.cpp_queries.get().is_none());
        assert!(engine.scala_queries.get().is_none());
    }

    #[test]
    fn first_ts_parse_compiles_only_typescript() {
        let engine = QueryEngine::new().unwrap();
        let _ = engine.parse_file("app.ts", "import x from 'y';").unwrap();
        // Only TypeScript queries should be compiled.
        assert!(engine.ts_queries.get().is_some());
        assert!(engine.py_queries.get().is_none());
        assert!(engine.go_queries.get().is_none());
        assert!(engine.rust_queries.get().is_none());
    }

    #[test]
    fn first_py_parse_compiles_only_python() {
        let engine = QueryEngine::new().unwrap();
        let _ = engine.parse_file("app.py", "import os").unwrap();
        assert!(engine.ts_queries.get().is_none());
        assert!(engine.py_queries.get().is_some());
        assert!(engine.go_queries.get().is_none());
    }

    #[test]
    fn js_and_ts_share_same_queries() {
        let engine = QueryEngine::new().unwrap();
        // Parse a JS file — should compile the ts_queries (shared for TS/JS).
        let _ = engine.parse_file("app.js", "import x from 'y';").unwrap();
        assert!(engine.ts_queries.get().is_some());
        // Parse a TS file — should not trigger a new compilation.
        let _ = engine.parse_file("app.ts", "import x from 'y';").unwrap();
        // Still the same compiled queries.
        assert!(engine.ts_queries.get().is_some());
    }

    #[test]
    fn multiple_languages_compile_independently() {
        let engine = QueryEngine::new().unwrap();
        let _ = engine.parse_file("a.ts", "const x = 1;").unwrap();
        let _ = engine.parse_file("b.py", "x = 1").unwrap();
        let _ = engine.parse_file("c.go", "package main").unwrap();
        assert!(engine.ts_queries.get().is_some());
        assert!(engine.py_queries.get().is_some());
        assert!(engine.go_queries.get().is_some());
        // Unused languages stay uncompiled.
        assert!(engine.rust_queries.get().is_none());
        assert!(engine.java_queries.get().is_none());
        assert!(engine.scala_queries.get().is_none());
    }

    #[test]
    fn unknown_language_does_not_compile_anything() {
        let engine = QueryEngine::new().unwrap();
        let result = engine.parse_file("readme.txt", "hello world").unwrap();
        assert_eq!(result.language, Language::Unknown);
        assert!(result.definitions.is_empty());
        // No queries should be compiled.
        assert!(engine.ts_queries.get().is_none());
        assert!(engine.py_queries.get().is_none());
    }

    #[test]
    fn extract_data_flow_triggers_lazy_init() {
        let engine = QueryEngine::new().unwrap();
        assert!(engine.py_queries.get().is_none());
        let _ = engine.extract_data_flow("main.py", "x = foo(1)").unwrap();
        assert!(engine.py_queries.get().is_some());
    }

    #[test]
    fn parse_tree_for_path_triggers_lazy_init() {
        let engine = QueryEngine::new().unwrap();
        assert!(engine.go_queries.get().is_none());
        let _ = engine
            .parse_tree_for_path("main.go", "package main")
            .unwrap();
        assert!(engine.go_queries.get().is_some());
    }

    #[test]
    fn cached_results_identical_to_fresh_compilation() {
        let engine = QueryEngine::new().unwrap();
        let src = "import { Foo } from './foo';\nfunction bar() { return Foo(); }";

        // First parse: triggers compilation.
        let r1 = engine.parse_file("a.ts", src).unwrap();
        // Second parse: uses cached queries.
        let r2 = engine.parse_file("b.ts", src).unwrap();

        // Results should be structurally identical (paths differ by design).
        assert_eq!(r1.imports.len(), r2.imports.len());
        assert_eq!(r1.definitions.len(), r2.definitions.len());
        assert_eq!(r1.call_sites.len(), r2.call_sites.len());
        for (i1, i2) in r1.imports.iter().zip(r2.imports.iter()) {
            assert_eq!(i1.source, i2.source);
            assert_eq!(i1.names.len(), i2.names.len());
        }
    }
}

// ---------------------------------------------------------------------------
// Thread-local parser reuse tests (Phase 12.7)
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr
)]
mod thread_local_parser_tests {
    use super::*;

    #[test]
    fn parser_reused_across_multiple_parses_same_language() {
        let engine = QueryEngine::new().unwrap();
        let src1 = "function foo() { return 1; }";
        let src2 = "function bar() { return 2; }";

        let r1 = engine.parse_file("a.ts", src1).unwrap();
        let r2 = engine.parse_file("b.ts", src2).unwrap();

        // Both parse successfully, demonstrating parser reuse works.
        assert_eq!(r1.definitions.len(), 1);
        assert_eq!(r2.definitions.len(), 1);
        assert_eq!(r1.definitions[0].name, "foo");
        assert_eq!(r2.definitions[0].name, "bar");
    }

    #[test]
    fn parser_reused_across_different_languages() {
        let engine = QueryEngine::new().unwrap();

        let ts_result = engine.parse_file("app.ts", "function hello() {}").unwrap();
        let py_result = engine
            .parse_file("main.py", "def hello():\n    pass")
            .unwrap();

        assert_eq!(ts_result.definitions.len(), 1);
        assert_eq!(py_result.definitions.len(), 1);
    }

    #[test]
    fn thread_local_parsers_work_with_rayon() {
        use rayon::prelude::*;

        let engine = QueryEngine::new().unwrap();
        let files: Vec<(&str, &str)> = vec![
            ("a.ts", "function a() { return 1; }"),
            ("b.ts", "function b() { return 2; }"),
            ("c.py", "def c():\n    pass"),
            ("d.py", "def d():\n    return 1"),
            ("e.ts", "const e = () => 42;"),
            ("f.go", "package main\nfunc f() {}"),
        ];

        let results: Vec<ParsedFile> = files
            .par_iter()
            .map(|&(path, src)| engine.parse_file(path, src).unwrap())
            .collect();

        assert_eq!(results.len(), 6);
        // All files parsed successfully in parallel with thread-local parsers.
        for r in &results {
            assert!(!r.definitions.is_empty() || r.language == Language::Unknown);
        }
    }

    #[test]
    fn parse_tree_for_path_reuses_parser() {
        let engine = QueryEngine::new().unwrap();

        // First call creates the parser for TypeScript.
        let r1 = engine.parse_tree_for_path("a.ts", "const x = 1;").unwrap();
        assert!(r1.is_some());

        // Second call reuses the same thread-local parser.
        let r2 = engine.parse_tree_for_path("b.ts", "const y = 2;").unwrap();
        assert!(r2.is_some());
    }

    #[test]
    fn extract_data_flow_reuses_parser() {
        let engine = QueryEngine::new().unwrap();

        let df1 = engine
            .extract_data_flow("a.ts", "const x = foo(1);")
            .unwrap();
        let df2 = engine
            .extract_data_flow("b.ts", "const y = bar(2);")
            .unwrap();

        // Both extractions succeed with parser reuse.
        assert!(!df1.assignments.is_empty());
        assert!(!df2.assignments.is_empty());
    }

    #[test]
    fn results_identical_with_reused_vs_fresh_parser() {
        // Parse with a fresh engine (parsers created fresh for first file).
        let engine1 = QueryEngine::new().unwrap();
        let src = "import { Foo } from './foo';\nfunction bar() { return Foo(); }";
        let r_fresh = engine1.parse_file("test.ts", src).unwrap();

        // Parse again on the same engine (parser reused from thread-local).
        let r_reused = engine1.parse_file("test2.ts", src).unwrap();

        // Results must be structurally identical.
        assert_eq!(r_fresh.imports.len(), r_reused.imports.len());
        assert_eq!(r_fresh.definitions.len(), r_reused.definitions.len());
        assert_eq!(r_fresh.call_sites.len(), r_reused.call_sites.len());
        assert_eq!(r_fresh.exports.len(), r_reused.exports.len());

        for (a, b) in r_fresh.imports.iter().zip(r_reused.imports.iter()) {
            assert_eq!(a.source, b.source);
            assert_eq!(a.names.len(), b.names.len());
        }
        for (a, b) in r_fresh.definitions.iter().zip(r_reused.definitions.iter()) {
            assert_eq!(a.name, b.name);
            assert_eq!(a.kind, b.kind);
        }
    }
}

// ---------------------------------------------------------------------------
// Property-based tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr
)]
mod prop_tests {
    use super::*;
    use proptest::prelude::*;

    fn engine() -> &'static QueryEngine {
        shared_test_engine()
    }

    fn ts_import_strategy() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("import React from 'react';".to_string()),
            Just("import { useState } from 'react';".to_string()),
            Just("import * as path from 'path';".to_string()),
            Just("import { foo as bar } from './utils';".to_string()),
            Just("import './polyfill';".to_string()),
            Just("import React, { useState } from 'react';".to_string()),
        ]
    }

    fn ts_definition_strategy() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("function greet() {}".to_string()),
            Just("class User {}".to_string()),
            Just("interface IUser {}".to_string()),
            Just("type Result = number;".to_string()),
            Just("const handler = () => {};".to_string()),
            Just("const MAX = 100;".to_string()),
        ]
    }

    fn python_source_strategy() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("import os".to_string()),
            Just("from os.path import join".to_string()),
            Just("def greet():\n    pass".to_string()),
            Just("class User:\n    pass".to_string()),
            Just("result = fetch()".to_string()),
        ]
    }

    proptest! {
        #[test]
        fn prop_parse_never_panics(source in "[a-zA-Z0-9 (){};='\n./,*_:@#$%^&\\[\\]-]+") {
            let e = engine();
            let _ = e.parse_file("test.ts", &source);
            let _ = e.parse_file("test.py", &source);
            let _ = e.parse_file("test.csv", &source);
        }

        #[test]
        fn prop_data_flow_never_panics(source in "[a-zA-Z0-9 (){};='\n./,*_:@#$%^&\\[\\]-]+") {
            let e = engine();
            let _ = e.extract_data_flow("test.ts", &source);
            let _ = e.extract_data_flow("test.py", &source);
        }

        #[test]
        fn prop_ts_import_always_has_source(import_line in ts_import_strategy()) {
            let e = engine();
            let result = e.parse_file("test.ts", &import_line).unwrap();
            for imp in &result.imports {
                prop_assert!(!imp.source.is_empty(), "import source should not be empty");
            }
        }

        #[test]
        fn prop_ts_definition_always_has_name(def_line in ts_definition_strategy()) {
            let e = engine();
            let result = e.parse_file("test.ts", &def_line).unwrap();
            for def in &result.definitions {
                prop_assert!(!def.name.is_empty(), "definition name should not be empty");
                prop_assert!(def.start_line > 0, "start_line should be > 0");
            }
        }

        #[test]
        fn prop_python_source_has_valid_output(src in python_source_strategy()) {
            let e = engine();
            let result = e.parse_file("test.py", &src).unwrap();
            prop_assert_eq!(result.language, Language::Python);
            for imp in &result.imports {
                prop_assert!(!imp.source.is_empty());
            }
            for def in &result.definitions {
                prop_assert!(!def.name.is_empty());
            }
        }

        #[test]
        fn prop_deterministic(source in "[a-zA-Z0-9 (){};='./,*_\n]+") {
            let e = engine();
            let r1 = e.parse_file("test.ts", &source);
            let r2 = e.parse_file("test.ts", &source);
            match (r1, r2) {
                (Ok(a), Ok(b)) => prop_assert_eq!(a, b),
                (Err(_), Err(_)) => {}
                _ => prop_assert!(false, "inconsistent results"),
            }
        }

        #[test]
        fn prop_unknown_language_always_empty(source in ".*") {
            let e = engine();
            let result = e.parse_file("data.csv", &source).unwrap();
            prop_assert!(result.definitions.is_empty());
            prop_assert!(result.imports.is_empty());
            prop_assert!(result.exports.is_empty());
            prop_assert!(result.call_sites.is_empty());
        }

        #[test]
        fn prop_call_sites_have_callee(source in "[a-zA-Z_][a-zA-Z0-9_]*\\([a-zA-Z0-9_, ]*\\);?") {
            let e = engine();
            let result = e.parse_file("test.ts", &source);
            if let Ok(r) = result {
                for call in &r.call_sites {
                    prop_assert!(!call.callee.is_empty(), "call site callee should not be empty");
                    prop_assert!(call.line > 0, "call site line should be > 0");
                }
            }
        }

        #[test]
        fn prop_export_names_not_empty(idx in 0usize..5) {
            let sources = [
                "export function foo() {}",
                "export class Bar {}",
                "export { baz };",
                "export const X = 1;",
                "export type T = number;",
            ];
            let e = engine();
            let result = e.parse_file("test.ts", sources[idx]).unwrap();
            for exp in &result.exports {
                prop_assert!(!exp.name.is_empty());
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Phase 8 audit: edge case tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr
)]
mod audit_tests {
    use super::*;

    fn engine() -> &'static QueryEngine {
        shared_test_engine()
    }












    #[test]
    fn test_hash_str_no_collision_for_common_names() {
        use crate::languages::common::hash_str;
        // Verify the hash function gives different results for common definition names
        let names = [
            "foo",
            "bar",
            "baz",
            "get",
            "set",
            "create",
            "update",
            "delete",
            "handler",
            "process",
            "validate",
            "transform",
            "save",
            "load",
        ];
        let hashes: Vec<usize> = names.iter().map(|n| hash_str(n)).collect();
        for i in 0..hashes.len() {
            for j in (i + 1)..hashes.len() {
                assert_ne!(
                    hashes[i], hashes[j],
                    "hash collision between '{}' and '{}'",
                    names[i], names[j]
                );
            }
        }
    }









}

// ---------------------------------------------------------------------------
// Phase 8 audit: .scm query file coverage tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr
)]
mod scm_audit_tests {
    use super::*;

    fn engine() -> &'static QueryEngine {
        shared_test_engine()
    }

    // === TS enum declarations ===



    // === TS export default expression ===



    // === TS enum in exports ===


    // === TS import type ===


    // === Python walrus operator ===


    // === TS destructuring assignments ===



    // === Python tuple unpacking ===


    // === Python relative import with alias ===


    // === Python async def ===


    // === TS `as const` / `satisfies` ===



    // === TS `export default function` with no name ===


    // === TS template literal type ===


    // === TS namespace/module declarations ===


    // === TS `export =` (CommonJS-style) ===


    // === Python __all__ ===


    // === TS `require()` calls (CJS imports) ===


    // === TS dynamic import ===


    // === Python decorated method inside decorated class ===


    // === TS exported arrow function ===


    // === Python multiline import ===


    // === Python multiple import on same line ===


    // === TS re-export namespace ===


    // === find_containing_function for function_expression ===


    // === TS computed property method ===


    // === Python nested class ===


    // === TS class with static methods ===


    // === TS getter/setter ===


    // === Verify .scm pattern ordering doesn't matter ===

    #[test]
    fn test_capture_name_dispatch_order_independent() {
        // Verify that the engine dispatches by capture name, not pattern index.
        // This is the key architectural property of the capture-name refactor.
        let e = engine();
        let source = r#"
import React from 'react';
import { useState } from 'react';
import * as path from 'path';
import './polyfill';
"#;
        let result = e.parse_file("app.ts", source).unwrap();
        assert_eq!(result.imports.len(), 4);
        // Verify each import type was correctly dispatched
        assert!(result.imports.iter().any(|i| i.is_default)); // default
        assert!(result
            .imports
            .iter()
            .any(|i| !i.is_default && !i.is_namespace && !i.names.is_empty())); // named
        assert!(result.imports.iter().any(|i| i.is_namespace)); // namespace
        assert!(result.imports.iter().any(|i| i.names.is_empty())); // side-effect
    }

    // === Verify all export types dispatch correctly ===

    #[test]
    fn test_all_export_declaration_types() {
        let e = engine();
        let source = r#"
export function fn() {}
export function* gen() {}
export class Cls {}
export abstract class ACls {}
export interface IFace {}
export type TAlias = string;
export const VAL = 1;
"#;
        let result = e.parse_file("all.ts", source).unwrap();
        assert!(
            result.exports.iter().any(|e| e.name == "fn"),
            "export function"
        );
        assert!(
            result.exports.iter().any(|e| e.name == "gen"),
            "export generator"
        );
        assert!(
            result.exports.iter().any(|e| e.name == "Cls"),
            "export class"
        );
        assert!(
            result.exports.iter().any(|e| e.name == "ACls"),
            "export abstract class"
        );
        assert!(
            result.exports.iter().any(|e| e.name == "IFace"),
            "export interface"
        );
        assert!(
            result.exports.iter().any(|e| e.name == "TAlias"),
            "export type alias"
        );
        assert!(
            result.exports.iter().any(|e| e.name == "VAL"),
            "export const"
        );
    }

    // === Verify definition extraction finds all kinds ===



    // === Verify Python import capture coverage ===


    // === Agent audit Issue 3: Decorated Python function double-counting ===



    // === Agent audit Issue 4: "function" kind string in const skip logic ===



    // === Agent audit Issue 1: TS new_expression not in calls.scm ===


    // =====================================================================
    // Rust language tests
    // =====================================================================


























    // === Java language detection ===


    // === Java imports ===





    // === Java definitions ===







    // === Java call sites ===




    // === Java data flow ===



    // === Java empty / edge cases ===


    // === Java full module ===


    // ===================================================================
    // C# tests
    // ===================================================================


    // === C# imports ===





    // === C# definitions ===











    // === C# call sites ===





    // === C# data flow ===




    // === C# empty / edge cases ===


    // === C# full module ===


    // ===================================================================
    // PHP Tests
    // ===================================================================
























    // -----------------------------------------------------------------------
    // Ruby tests
    // -----------------------------------------------------------------------

















    // Kotlin tests







    // Swift tests






}

// ===========================================================================
// C language tests
// ===========================================================================

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr
)]
mod c_tests {
    use super::*;

    fn engine() -> &'static QueryEngine {
        shared_test_engine()
    }

    // === C imports (#include) ===




    // === C function definitions ===





    // === C call sites ===



    // === C data flow ===


    // === C language detection ===


    // === C full integration ===

}

// ===========================================================================
// C++ language tests
// ===========================================================================

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr
)]
mod cpp_tests {
    use super::*;

    fn engine() -> &'static QueryEngine {
        shared_test_engine()
    }

    // === C++ imports (#include) ===



    // === C++ definitions ===







    // === C++ call sites ===




    // === C++ data flow ===


    // === C++ language detection ===


    // === C++ full integration ===


    // Scala tests






}
