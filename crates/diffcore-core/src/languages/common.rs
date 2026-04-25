//! Shared helpers used by per-language extraction modules.
//!
//! Moved from `query_engine.rs` during the per-language module split.
//! Pure mechanical move — no logic changes.

use crate::ast::{CallSite, Definition, ImportInfo, ImportedName, Language};
use crate::query_engine::{QueryEngineError, QueryWithCaptures};
use crate::types::SymbolKind;
use tree_sitter::{Node, Query, QueryCursor, StreamingIterator};

// ---------------------------------------------------------------------------
// CollectedMatch
// ---------------------------------------------------------------------------

/// Owned representation of a single query match.
/// Extracted from the streaming iterator so we can process after iteration.
pub(crate) struct CollectedMatch<'tree> {
    pub(crate) captures: Vec<(u32, Node<'tree>)>,
}

impl<'tree> CollectedMatch<'tree> {
    /// Check whether this match contains a capture with the given index.
    pub(crate) fn has_capture(&self, idx: Option<u32>) -> bool {
        match idx {
            Some(i) => self.captures.iter().any(|&(ci, _)| ci == i),
            None => false,
        }
    }

    /// Get the first node captured at the given index.
    pub(crate) fn get_capture(&self, idx: Option<u32>) -> Option<Node<'tree>> {
        idx.and_then(|i| {
            self.captures
                .iter()
                .find(|&&(ci, _)| ci == i)
                .map(|&(_, n)| n)
        })
    }
}

/// Collect all matches from a streaming iterator into owned data.
pub(crate) fn collect_matches<'tree>(
    cursor: &mut QueryCursor,
    query: &Query,
    root: Node<'tree>,
    source: &'tree [u8],
) -> Vec<CollectedMatch<'tree>> {
    let mut result = Vec::new();
    let mut matches = cursor.matches(query, root, source);
    loop {
        matches.advance();
        match matches.get() {
            Some(m) => {
                let caps: Vec<(u32, Node)> = m.captures.iter().map(|c| (c.index, c.node)).collect();
                result.push(CollectedMatch { captures: caps });
            }
            None => break,
        }
    }
    result
}

// ---------------------------------------------------------------------------
// ImportBuilder
// ---------------------------------------------------------------------------

pub(crate) struct ImportBuilder {
    pub(crate) source: String,
    pub(crate) names: Vec<ImportedName>,
    pub(crate) is_default: bool,
    pub(crate) is_namespace: bool,
    pub(crate) line: usize,
}

impl ImportBuilder {
    pub(crate) fn new(source: &str, line: usize) -> Self {
        Self {
            source: source.to_string(),
            names: Vec::new(),
            is_default: false,
            is_namespace: false,
            line,
        }
    }

    pub(crate) fn build(self) -> ImportInfo {
        ImportInfo {
            source: self.source,
            names: self.names,
            is_default: self.is_default,
            is_namespace: self.is_namespace,
            line: self.line,
        }
    }
}

pub(crate) fn get_or_insert_import<'a>(
    map: &'a mut Vec<(usize, ImportBuilder)>,
    key: usize,
    source: &str,
    line: usize,
) -> &'a mut ImportBuilder {
    if let Some(pos) = map.iter().position(|(k, _)| *k == key) {
        &mut map[pos].1
    } else {
        map.push((key, ImportBuilder::new(source, line)));
        let len = map.len();
        &mut map[len - 1].1
    }
}

// ---------------------------------------------------------------------------
// Node helpers
// ---------------------------------------------------------------------------

pub(crate) fn node_text<'a>(node: &Node, source: &'a [u8]) -> &'a str {
    node.utf8_text(source).unwrap_or("")
}

/// Extract (start_line, end_line, start_byte) from a node capture.
pub(crate) fn node_span(m: &CollectedMatch, node_cap: Option<u32>) -> (usize, usize, usize) {
    m.get_capture(node_cap)
        .map(|n| {
            (
                n.start_position().row + 1,
                n.end_position().row + 1,
                n.start_byte(),
            )
        })
        .unwrap_or((0, 0, 0))
}

/// Simple string hash for deduplication keys.
pub(crate) fn hash_str(s: &str) -> usize {
    let mut h: usize = 0;
    for b in s.bytes() {
        h = h.wrapping_mul(31).wrapping_add(b as usize);
    }
    h
}

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
pub(crate) fn standard_kind_to_symbol_kind(suffix: &str) -> Option<SymbolKind> {
    Some(match suffix {
        // Universal-ctags / nvim-treesitter "tags" vocabulary.
        "function" | "method" | "macro" | "operator" => SymbolKind::Function,
        // C# `record` and Java `enum` map to Class because our IR doesn't
        // distinguish "value-type wrapper" from a plain class. Both still
        // expose fields/methods that downstream consumers reason about.
        "class" | "struct" | "enum" | "union" | "type" | "record" => SymbolKind::Class,
        // Java `@interface` and C# `delegate` are both contract types — they
        // declare a callable shape rather than a concrete implementation, so
        // they slot in alongside interfaces / traits / protocols.
        "interface" | "trait" | "protocol" | "annotation" | "delegate" => SymbolKind::Interface,
        // We collapse "type" aliases to TypeAlias when the grammar makes the
        // distinction explicit; otherwise the "type" arm above wins.
        "type_alias" | "typealias" | "alias" => SymbolKind::TypeAlias,
        "constant" | "const" | "static" | "variable" | "field" | "property" | "enum_member"
        | "enumerator" => SymbolKind::Constant,
        // Ruby `module Foo … end` and C++ `namespace foo { … }` are
        // first-class symbols in our IR (Symbol::Module). Note that
        // upstream `tags.scm` files sometimes use these for *scoping
        // containers* (patterns whose only purpose is hosting nested
        // definitions) which would over-emit. Languages whose .scm
        // declares \`@definition.module\` / \`@definition.namespace\`
        // should mean "this IS a module/namespace definition".
        "module" | "namespace" => SymbolKind::Module,
        // Package containers are skipped — Java/Go etc. don't model them
        // as standalone symbols in our IR.
        "package" => return None,
        _ => return None,
    })
}

/// Run the standard-convention extractor on a set of matches.
///
/// Walks each match looking for a `@definition.<kind>` capture together
/// with the conventional `@name` capture, deduplicating by `(node_start, name)`
/// the same way the bespoke per-language extractors do.
///
/// Returns `true` if at least one definition was emitted — callers use
/// this as a signal that the standard path "took" and bespoke fallback can
/// be skipped.
pub(crate) fn extract_definitions_standard(
    matches: &[CollectedMatch<'_>],
    source: &[u8],
    qwc: &QueryWithCaptures,
    out_definitions: &mut Vec<Definition>,
    seen_nodes: &mut Vec<(usize, usize)>,
) -> bool {
    let name_idx = qwc.capture_index("name");
    if name_idx.is_none() {
        // No `@name` capture in this query → it is not a standard-style query.
        return false;
    }

    // Pre-resolve the indexes for every `definition.<suffix>` capture the
    // query declared, so we can recognise them in O(captures-per-match).
    let definition_captures: Vec<(u32, SymbolKind)> = qwc
        .capture_names
        .iter()
        .filter_map(|(name, &idx)| {
            name.strip_prefix("definition.")
                .and_then(standard_kind_to_symbol_kind)
                .map(|kind| (idx, kind))
        })
        .collect();
    if definition_captures.is_empty() {
        return false;
    }

    let mut emitted_any = false;
    for m in matches {
        let Some(name_node) = m.get_capture(name_idx) else {
            continue;
        };
        // Find which definition.* capture this match carried.
        let Some(&(def_idx, kind)) = definition_captures
            .iter()
            .find(|(idx, _)| m.has_capture(Some(*idx)))
        else {
            continue;
        };
        let name_text = node_text(&name_node, source).to_string();
        if name_text.is_empty() {
            continue;
        }
        let (start_line, end_line, _outer_start) = node_span(m, Some(def_idx));
        // Dedup by the name identifier's start byte (not the outer node's),
        // so that "wrapping" patterns like Python's decorated_definition do
        // not produce a duplicate of the inner function/class — both patterns
        // capture the same `@name` node.
        let name_start = name_node.start_byte();
        let key = (name_start, hash_str(&name_text));
        if !seen_nodes.contains(&key) {
            seen_nodes.push(key);
            out_definitions.push(Definition {
                name: name_text,
                kind,
                start_line,
                end_line,
            });
            emitted_any = true;
        }
    }
    emitted_any
}

/// Standard-convention extractor for call sites.
///
/// Recognises `(call_expression function: (_) @name) @reference.call` and
/// the closely related `@reference.call.method` / `@reference.call.constructor`
/// variants used by some upstream `tags.scm` files.
///
/// Returns `true` if at least one call was emitted.
#[allow(dead_code)] // wired in once a language opts in via its calls.scm
pub(crate) fn extract_calls_standard(
    matches: &[CollectedMatch<'_>],
    source: &[u8],
    qwc: &QueryWithCaptures,
    out_calls: &mut Vec<CallSite>,
) -> bool {
    let name_idx = qwc.capture_index("name");
    if name_idx.is_none() {
        return false;
    }
    // Any capture whose name starts with `reference.call` counts.
    let ref_indexes: Vec<u32> = qwc
        .capture_names
        .iter()
        .filter_map(|(n, &i)| n.starts_with("reference.call").then_some(i))
        .collect();
    if ref_indexes.is_empty() {
        return false;
    }

    let mut emitted = false;
    for m in matches {
        let Some(name_node) = m.get_capture(name_idx) else {
            continue;
        };
        if !ref_indexes.iter().any(|&i| m.has_capture(Some(i))) {
            continue;
        }
        let callee = node_text(&name_node, source).to_string();
        if callee.is_empty() {
            continue;
        }
        out_calls.push(CallSite {
            callee,
            line: name_node.start_position().row + 1,
            containing_function: None, // computed by caller after extraction
        });
        emitted = true;
    }
    emitted
}

// ---------------------------------------------------------------------------
// Minimal imports fallback
// ---------------------------------------------------------------------------

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
pub(crate) fn extract_minimal_imports(
    root: &Node,
    source: &[u8],
    qwc: &QueryWithCaptures,
) -> Result<Vec<ImportInfo>, QueryEngineError> {
    let source_idx = qwc.capture_index("source");
    if source_idx.is_none() {
        return Ok(vec![]);
    }
    let stmt_idx = qwc.capture_index("stmt");

    let mut cursor = QueryCursor::new();
    let matches = collect_matches(&mut cursor, &qwc.query, *root, source);

    // Dedup by (source-string, line) so that languages whose .scm has
    // multiple overlapping patterns for the same import statement (e.g.
    // a "side-effect" pattern and a "named-imports" pattern that both
    // match `import 'x'`) don't double-emit.
    let mut seen: Vec<(String, usize)> = Vec::new();
    let mut imports: Vec<ImportInfo> = Vec::new();

    for m in matches {
        let mut source_text = String::new();
        let mut line = 0usize;
        for &(idx, node) in &m.captures {
            if Some(idx) == source_idx {
                source_text = node_text(&node, source).to_string();
                if line == 0 {
                    line = node.start_position().row + 1;
                }
            }
            if Some(idx) == stmt_idx {
                line = node.start_position().row + 1;
            }
        }
        let trimmed = source_text
            .trim_matches(|c: char| matches!(c, '"' | '\'' | '`' | '<' | '>'))
            .to_string();
        if trimmed.is_empty() {
            continue;
        }
        let key = (trimmed.clone(), line);
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        imports.push(ImportInfo {
            source: trimmed,
            names: Vec::new(),
            is_default: false,
            is_namespace: false,
            line,
        });
    }
    Ok(imports)
}

// ---------------------------------------------------------------------------
// find_containing_function
// ---------------------------------------------------------------------------

/// Walk up from a node to find the nearest containing function declaration.
pub(crate) fn find_containing_function(
    node: &Node,
    source: &[u8],
    language: Language,
) -> Option<String> {
    let fn_kinds: &[&str] = match language {
        Language::TypeScript | Language::JavaScript => &[
            "function_declaration",
            "generator_function_declaration",
            "method_definition",
        ],
        Language::Python => &["function_definition"],
        Language::Go => &["function_declaration", "method_declaration"],
        Language::Rust => &["function_item"],
        Language::Java => &["method_declaration", "constructor_declaration"],
        Language::CSharp => &["method_declaration", "constructor_declaration"],
        Language::Php => &["method_declaration", "function_definition"],
        Language::Ruby => &["method", "singleton_method"],
        Language::Kotlin => &["function_declaration"],
        Language::Swift => &["function_declaration"],
        Language::C => &["function_definition"],
        Language::Cpp => &["function_definition"],
        Language::Scala => &["function_definition", "function_declaration"],
        // Extras: containing-function lookup not yet implemented.
        Language::Bash
        | Language::Haskell
        | Language::Nix
        | Language::Lua
        | Language::Perl
        | Language::Elixir
        | Language::Erlang
        | Language::Zig
        | Language::OCaml
        | Language::Julia
        | Language::Dart
        | Language::R
        | Language::Fish
        | Language::Html
        | Language::Css
        | Language::Scss
        | Language::Json
        | Language::Yaml
        | Language::Toml
        | Language::Markdown
        | Language::GraphQl
        | Language::Vue
        | Language::Svelte => return None,
        Language::Unknown => return None,
    };

    let mut current = node.parent();
    while let Some(parent) = current {
        if fn_kinds.contains(&parent.kind()) {
            return parent
                .child_by_field_name("name")
                .map(|n| node_text(&n, source).to_string());
        }
        // Also check for arrow function / function expression assigned to a variable
        if parent.kind() == "variable_declarator" {
            let is_fn = parent
                .child_by_field_name("value")
                .map(|v| {
                    v.kind() == "arrow_function"
                        || v.kind() == "function"
                        || v.kind() == "function_expression"
                })
                .unwrap_or(false);
            if is_fn {
                return parent
                    .child_by_field_name("name")
                    .filter(|n| n.kind() == "identifier")
                    .map(|n| node_text(&n, source).to_string());
            }
        }
        current = parent.parent();
    }
    None
}

/// Extract argument texts from an arguments/argument_list node.
pub(crate) fn extract_arg_texts(
    args_node: &Node,
    source: &[u8],
    language: Language,
) -> Vec<String> {
    let mut args = Vec::new();
    let mut cursor = args_node.walk();
    for child in args_node.named_children(&mut cursor) {
        if language == Language::Python && child.kind() == "keyword_argument" {
            if let Some(val) = child.child_by_field_name("value") {
                let text = node_text(&val, source).to_string();
                if !text.is_empty() {
                    args.push(text);
                }
            }
            continue;
        }
        let text = node_text(&child, source).to_string();
        if !text.is_empty() {
            args.push(text);
        }
    }
    args
}
