//! AST parsing module using tree-sitter for extracting symbols, imports, exports, and call sites.

use crate::types::SymbolKind;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tree_sitter::{Node, Parser};

/// Errors from AST parsing operations.
#[derive(Debug, Error)]
pub enum AstError {
    #[error("failed to set parser language: {0}")]
    LanguageError(String),
    #[error("failed to parse source: {0}")]
    ParseError(String),
}

/// Detected programming language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    // ── Core 13 (always-on) ──────────────────────────────────────────
    TypeScript,
    JavaScript,
    Python,
    Go,
    Rust,
    Java,
    CSharp,
    Php,
    Ruby,
    Kotlin,
    Swift,
    C,
    Cpp,
    Scala,
    // ── Extras (each gated behind a `lang-<name>` Cargo feature) ─────
    Bash,
    Haskell,
    Nix,
    Lua,
    Perl,
    Elixir,
    Erlang,
    Zig,
    OCaml,
    Julia,
    Dart,
    R,
    Fish,
    Html,
    Css,
    Scss,
    Json,
    Yaml,
    Toml,
    Markdown,
    GraphQl,
    Vue,
    Svelte,
    Unknown,
}

impl Language {
    /// Detect language from file path extension.
    pub fn from_path(path: &str) -> Self {
        match path.rsplit('.').next().unwrap_or("") {
            // ── Core 13 ─────────────────────────────────────────────
            "ts" | "tsx" => Language::TypeScript,
            "js" | "jsx" | "mjs" | "cjs" => Language::JavaScript,
            "py" | "pyi" => Language::Python,
            "go" => Language::Go,
            "rs" => Language::Rust,
            "java" => Language::Java,
            "cs" => Language::CSharp,
            "php" => Language::Php,
            "rb" => Language::Ruby,
            "kt" | "kts" => Language::Kotlin,
            "swift" => Language::Swift,
            "c" => Language::C,
            "h" => Language::C,
            "cpp" | "cc" | "cxx" | "c++" => Language::Cpp,
            "hpp" | "hxx" | "h++" | "hh" => Language::Cpp,
            "scala" | "sc" => Language::Scala,
            // ── Extras ──────────────────────────────────────────────
            "sh" | "bash" => Language::Bash,
            "hs" | "lhs" => Language::Haskell,
            "nix" => Language::Nix,
            "lua" => Language::Lua,
            "pl" | "pm" | "t" | "perl" => Language::Perl,
            "ex" | "exs" => Language::Elixir,
            "erl" | "hrl" => Language::Erlang,
            "zig" | "zon" => Language::Zig,
            "ml" | "mli" => Language::OCaml,
            "jl" => Language::Julia,
            "dart" => Language::Dart,
            "r" | "R" => Language::R,
            "fish" => Language::Fish,
            "html" | "htm" => Language::Html,
            "css" => Language::Css,
            "scss" | "sass" => Language::Scss,
            "json" | "jsonc" => Language::Json,
            "yaml" | "yml" => Language::Yaml,
            "toml" => Language::Toml,
            "md" | "markdown" | "mdx" => Language::Markdown,
            "graphql" | "gql" => Language::GraphQl,
            "vue" => Language::Vue,
            "svelte" => Language::Svelte,
            _ => Language::Unknown,
        }
    }
}

/// A symbol definition extracted from source code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Definition {
    pub name: String,
    pub kind: SymbolKind,
    pub start_line: usize,
    pub end_line: usize,
}

/// An import statement extracted from source code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportInfo {
    pub source: String,
    pub names: Vec<ImportedName>,
    pub is_default: bool,
    pub is_namespace: bool,
    pub line: usize,
}

/// A single imported name, optionally aliased.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportedName {
    pub name: String,
    pub alias: Option<String>,
}

/// An export declaration extracted from source code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportInfo {
    pub name: String,
    pub is_default: bool,
    pub is_reexport: bool,
    pub source: Option<String>,
    pub line: usize,
}

/// A function call site.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallSite {
    pub callee: String,
    pub line: usize,
    pub containing_function: Option<String>,
}

/// Result of parsing a single source file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedFile {
    pub path: String,
    pub language: Language,
    pub definitions: Vec<Definition>,
    pub imports: Vec<ImportInfo>,
    pub exports: Vec<ExportInfo>,
    pub call_sites: Vec<CallSite>,
}

/// Represents a change to a symbol between old and new versions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolChange {
    Added(Definition),
    Removed(Definition),
    Modified { old: Definition, new: Definition },
}

/// A local variable assigned from a function call return value.
/// Captures patterns like `const x = funcA()` or `x = func_a()`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VarCallAssignment {
    /// Name of the variable being assigned.
    pub variable: String,
    /// The callee expression (function being called).
    pub callee: String,
    /// Line number of the assignment.
    pub line: usize,
    /// Function containing this assignment.
    pub containing_function: Option<String>,
}

/// A function call with its resolved argument expressions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallWithArgs {
    /// The callee expression.
    pub callee: String,
    /// Argument expression texts (variable names, literals, nested calls, etc.).
    pub arguments: Vec<String>,
    /// Line number.
    pub line: usize,
    /// Function containing this call.
    pub containing_function: Option<String>,
}

/// Data flow information extracted from a source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataFlowInfo {
    /// Variables assigned from function call return values.
    pub assignments: Vec<VarCallAssignment>,
    /// Function calls with their argument expressions.
    pub calls_with_args: Vec<CallWithArgs>,
}

/// Parse a source file and extract symbols, imports, exports, and call sites.
pub fn parse_file(path: &str, source: &str) -> Result<ParsedFile, AstError> {
    let language = Language::from_path(path);
    match language {
        Language::TypeScript | Language::JavaScript => parse_typescript(path, source, language),
        Language::Python => parse_python(path, source),
        Language::Go => parse_go(path, source),
        Language::Rust
        | Language::Java
        | Language::CSharp
        | Language::Php
        | Language::Ruby
        | Language::Kotlin
        | Language::Swift
        | Language::C
        | Language::Cpp
        | Language::Scala
        | Language::Bash
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
        | Language::Svelte
        | Language::Unknown => Ok(ParsedFile {
            path: path.to_string(),
            language,
            definitions: vec![],
            imports: vec![],
            exports: vec![],
            call_sites: vec![],
        }),
    }
}

/// Detect which symbols were added, removed, or modified between old and new versions.
pub fn detect_changed_symbols(old: &ParsedFile, new: &ParsedFile) -> Vec<SymbolChange> {
    let mut changes = Vec::new();

    // Find removed and modified.
    // Compare span (body size) rather than absolute line positions,
    // so that symbols merely relocated by surrounding edits are not flagged.
    for old_def in &old.definitions {
        match new
            .definitions
            .iter()
            .find(|d| d.name == old_def.name && d.kind == old_def.kind)
        {
            None => changes.push(SymbolChange::Removed(old_def.clone())),
            Some(new_def) => {
                let old_span = old_def.end_line.saturating_sub(old_def.start_line);
                let new_span = new_def.end_line.saturating_sub(new_def.start_line);
                if old_span != new_span {
                    changes.push(SymbolChange::Modified {
                        old: old_def.clone(),
                        new: new_def.clone(),
                    });
                }
            }
        }
    }

    // Find added
    for new_def in &new.definitions {
        if !old
            .definitions
            .iter()
            .any(|d| d.name == new_def.name && d.kind == new_def.kind)
        {
            changes.push(SymbolChange::Added(new_def.clone()));
        }
    }

    changes
}

/// Extract base class names from a Python class definition.
pub fn get_python_class_bases(source: &str, class_name: &str) -> Result<Vec<String>, AstError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_python::LANGUAGE.into())
        .map_err(|e| AstError::LanguageError(e.to_string()))?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| AstError::ParseError("failed to parse".into()))?;

    let root = tree.root_node();
    let src = source.as_bytes();
    let mut bases = Vec::new();
    find_class_bases(&root, src, class_name, &mut bases);
    Ok(bases)
}

/// Extract data flow information from source code for tracking how data moves
/// through variable assignments and function calls within function bodies.
pub fn extract_data_flow_info(path: &str, source: &str) -> Result<DataFlowInfo, AstError> {
    let language = Language::from_path(path);
    match language {
        Language::TypeScript | Language::JavaScript => extract_ts_data_flow(source),
        Language::Python => extract_python_data_flow(source),
        Language::Go => extract_go_data_flow(source),
        Language::Rust
        | Language::Java
        | Language::CSharp
        | Language::Php
        | Language::Ruby
        | Language::Kotlin
        | Language::Swift
        | Language::C
        | Language::Cpp
        | Language::Scala
        | Language::Bash
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
        | Language::Svelte
        | Language::Unknown => Ok(DataFlowInfo {
            assignments: vec![],
            calls_with_args: vec![],
        }),
    }
}

// ---------------------------------------------------------------------------
// Helper: node text
// ---------------------------------------------------------------------------

fn node_text<'a>(node: &Node, source: &'a [u8]) -> &'a str {
    node.utf8_text(source).unwrap_or("")
}

fn extract_string_content(node: &Node, source: &[u8]) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "string_fragment" {
            return Some(node_text(&child, source).to_string());
        }
    }
    // Fallback: strip quotes
    let text = node_text(node, source);
    if (text.starts_with('"') && text.ends_with('"'))
        || (text.starts_with('\'') && text.ends_with('\''))
    {
        Some(text[1..text.len() - 1].to_string())
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// TypeScript / JavaScript parsing
// ---------------------------------------------------------------------------

fn parse_typescript(path: &str, source: &str, lang: Language) -> Result<ParsedFile, AstError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
        .map_err(|e| AstError::LanguageError(e.to_string()))?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| AstError::ParseError("tree-sitter failed to parse".into()))?;

    let root = tree.root_node();
    let src = source.as_bytes();
    let mut definitions = Vec::new();
    let mut imports = Vec::new();
    let mut exports = Vec::new();
    let mut call_sites = Vec::new();

    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        match child.kind() {
            "import_statement" => {
                if let Some(imp) = extract_ts_import(&child, src) {
                    imports.push(imp);
                }
            }
            "export_statement" => {
                extract_ts_export(&child, src, &mut exports, &mut definitions);
            }
            "function_declaration" | "generator_function_declaration" => {
                if let Some(def) = extract_definition_with_name(&child, src, SymbolKind::Function) {
                    definitions.push(def);
                }
            }
            "class_declaration" | "abstract_class_declaration" => {
                if let Some(def) = extract_definition_with_name(&child, src, SymbolKind::Class) {
                    definitions.push(def);
                }
                extract_ts_methods(&child, src, &mut definitions);
            }
            "interface_declaration" => {
                if let Some(def) = extract_definition_with_name(&child, src, SymbolKind::Interface)
                {
                    definitions.push(def);
                }
            }
            "type_alias_declaration" => {
                if let Some(def) = extract_definition_with_name(&child, src, SymbolKind::TypeAlias)
                {
                    definitions.push(def);
                }
            }
            "lexical_declaration" | "variable_declaration" => {
                extract_ts_variable_defs(&child, src, &mut definitions);
            }
            _ => {}
        }
    }

    collect_call_sites(&root, src, &mut call_sites, &None, "call_expression");

    Ok(ParsedFile {
        path: path.to_string(),
        language: lang,
        definitions,
        imports,
        exports,
        call_sites,
    })
}

fn extract_ts_import(node: &Node, source: &[u8]) -> Option<ImportInfo> {
    // Get source module path — try `source` field first, then any string child
    let source_str = node
        .child_by_field_name("source")
        .or_else(|| {
            let mut c = node.walk();
            let found = node.named_children(&mut c).find(|ch| ch.kind() == "string");
            found
        })
        .and_then(|s| extract_string_content(&s, source))?;

    let line = node.start_position().row + 1;
    let mut names = Vec::new();
    let mut is_default = false;
    let mut is_namespace = false;

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "import_clause" {
            let mut clause_cursor = child.walk();
            for clause_child in child.named_children(&mut clause_cursor) {
                match clause_child.kind() {
                    "identifier" => {
                        is_default = true;
                        names.push(ImportedName {
                            name: node_text(&clause_child, source).to_string(),
                            alias: None,
                        });
                    }
                    "named_imports" => {
                        let mut named_cursor = clause_child.walk();
                        for spec in clause_child.named_children(&mut named_cursor) {
                            if spec.kind() == "import_specifier" {
                                if let Some(name_node) = spec.child_by_field_name("name") {
                                    let alias = spec
                                        .child_by_field_name("alias")
                                        .map(|a| node_text(&a, source).to_string());
                                    names.push(ImportedName {
                                        name: node_text(&name_node, source).to_string(),
                                        alias,
                                    });
                                }
                            }
                        }
                    }
                    "namespace_import" => {
                        is_namespace = true;
                        let mut ns_cursor = clause_child.walk();
                        for ns_child in clause_child.named_children(&mut ns_cursor) {
                            if ns_child.kind() == "identifier" {
                                names.push(ImportedName {
                                    name: node_text(&ns_child, source).to_string(),
                                    alias: None,
                                });
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    Some(ImportInfo {
        source: source_str,
        names,
        is_default,
        is_namespace,
        line,
    })
}

fn extract_ts_export(
    node: &Node,
    source: &[u8],
    exports: &mut Vec<ExportInfo>,
    definitions: &mut Vec<Definition>,
) {
    let line = node.start_position().row + 1;

    // Check for "default" keyword among all children (including anonymous)
    let is_default = {
        let mut c = node.walk();
        let result = node.children(&mut c).any(|ch| ch.kind() == "default");
        result
    };

    // Check for re-export source
    let reexport_source = node
        .child_by_field_name("source")
        .and_then(|s| extract_string_content(&s, source));
    let is_reexport = reexport_source.is_some();

    // export { a, b } or export { a } from 'mod' or export * from 'mod'
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "export_clause" {
            let mut spec_cursor = child.walk();
            for spec in child.named_children(&mut spec_cursor) {
                if spec.kind() == "export_specifier" {
                    if let Some(name_node) = spec.child_by_field_name("name") {
                        exports.push(ExportInfo {
                            name: node_text(&name_node, source).to_string(),
                            is_default: false,
                            is_reexport,
                            source: reexport_source.clone(),
                            line,
                        });
                    }
                }
            }
            return;
        }
        if child.kind() == "namespace_export" {
            exports.push(ExportInfo {
                name: "*".to_string(),
                is_default: false,
                is_reexport: true,
                source: reexport_source.clone(),
                line,
            });
            return;
        }
    }

    // Wildcard re-export without namespace_export node: export * from 'mod'
    {
        let mut c2 = node.walk();
        let has_star = node.children(&mut c2).any(|ch| ch.kind() == "*");
        if has_star && is_reexport {
            exports.push(ExportInfo {
                name: "*".to_string(),
                is_default: false,
                is_reexport: true,
                source: reexport_source.clone(),
                line,
            });
            return;
        }
    }

    // Exported declaration: export function foo, export class Bar, etc.
    if let Some(decl) = node.child_by_field_name("declaration") {
        match decl.kind() {
            "function_declaration" | "generator_function_declaration" => {
                if let Some(def) = extract_definition_with_name(&decl, source, SymbolKind::Function)
                {
                    let name = def.name.clone();
                    definitions.push(def);
                    exports.push(ExportInfo {
                        name,
                        is_default,
                        is_reexport: false,
                        source: None,
                        line,
                    });
                }
            }
            "class_declaration" | "abstract_class_declaration" => {
                if let Some(def) = extract_definition_with_name(&decl, source, SymbolKind::Class) {
                    let name = def.name.clone();
                    definitions.push(def);
                    extract_ts_methods(&decl, source, definitions);
                    exports.push(ExportInfo {
                        name,
                        is_default,
                        is_reexport: false,
                        source: None,
                        line,
                    });
                }
            }
            "interface_declaration" => {
                if let Some(def) =
                    extract_definition_with_name(&decl, source, SymbolKind::Interface)
                {
                    let name = def.name.clone();
                    definitions.push(def);
                    exports.push(ExportInfo {
                        name,
                        is_default,
                        is_reexport: false,
                        source: None,
                        line,
                    });
                }
            }
            "type_alias_declaration" => {
                if let Some(def) =
                    extract_definition_with_name(&decl, source, SymbolKind::TypeAlias)
                {
                    let name = def.name.clone();
                    definitions.push(def);
                    exports.push(ExportInfo {
                        name,
                        is_default,
                        is_reexport: false,
                        source: None,
                        line,
                    });
                }
            }
            "lexical_declaration" | "variable_declaration" => {
                let start_idx = definitions.len();
                extract_ts_variable_defs(&decl, source, definitions);
                for def in &definitions[start_idx..] {
                    exports.push(ExportInfo {
                        name: def.name.clone(),
                        is_default,
                        is_reexport: false,
                        source: None,
                        line,
                    });
                }
            }
            _ => {}
        }
        return;
    }

    // export default <expression>
    if is_default {
        if let Some(val) = node.child_by_field_name("value") {
            let name = if val.kind() == "identifier" {
                node_text(&val, source).to_string()
            } else {
                "default".to_string()
            };
            exports.push(ExportInfo {
                name,
                is_default: true,
                is_reexport: false,
                source: None,
                line,
            });
        }
    }
}

fn extract_definition_with_name(
    node: &Node,
    source: &[u8],
    kind: SymbolKind,
) -> Option<Definition> {
    let name_node = node.child_by_field_name("name")?;
    Some(Definition {
        name: node_text(&name_node, source).to_string(),
        kind,
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
    })
}

fn extract_ts_methods(class_node: &Node, source: &[u8], definitions: &mut Vec<Definition>) {
    let body = match class_node.child_by_field_name("body") {
        Some(b) => b,
        None => return,
    };
    let mut cursor = body.walk();
    for child in body.named_children(&mut cursor) {
        if child.kind() == "method_definition" {
            if let Some(def) = extract_definition_with_name(&child, source, SymbolKind::Function) {
                definitions.push(def);
            }
        }
    }
}

fn extract_ts_variable_defs(node: &Node, source: &[u8], definitions: &mut Vec<Definition>) {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "variable_declarator" {
            let name_node = match child.child_by_field_name("name") {
                Some(n) if n.kind() == "identifier" => n,
                _ => continue,
            };
            let value = child.child_by_field_name("value");
            let kind = match value.as_ref().map(|v| v.kind()) {
                Some("arrow_function") | Some("function") => SymbolKind::Function,
                _ => SymbolKind::Constant,
            };
            definitions.push(Definition {
                name: node_text(&name_node, source).to_string(),
                kind,
                start_line: child.start_position().row + 1,
                end_line: child.end_position().row + 1,
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Python parsing
// ---------------------------------------------------------------------------

fn parse_python(path: &str, source: &str) -> Result<ParsedFile, AstError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_python::LANGUAGE.into())
        .map_err(|e| AstError::LanguageError(e.to_string()))?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| AstError::ParseError("tree-sitter failed to parse".into()))?;

    let root = tree.root_node();
    let src = source.as_bytes();
    let mut definitions = Vec::new();
    let mut imports = Vec::new();
    let mut call_sites = Vec::new();

    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        match child.kind() {
            "import_statement" => {
                if let Some(imp) = extract_python_import(&child, src) {
                    imports.push(imp);
                }
            }
            "import_from_statement" => {
                if let Some(imp) = extract_python_import_from(&child, src) {
                    imports.push(imp);
                }
            }
            "function_definition" => {
                if let Some(def) = extract_definition_with_name(&child, src, SymbolKind::Function) {
                    definitions.push(def);
                }
            }
            "class_definition" => {
                if let Some(def) = extract_definition_with_name(&child, src, SymbolKind::Class) {
                    definitions.push(def);
                }
                extract_python_methods(&child, src, &mut definitions);
            }
            "decorated_definition" => {
                if let Some(inner) = child.child_by_field_name("definition") {
                    match inner.kind() {
                        "function_definition" => {
                            if let Some(def) =
                                extract_definition_with_name(&inner, src, SymbolKind::Function)
                            {
                                definitions.push(def);
                            }
                        }
                        "class_definition" => {
                            if let Some(def) =
                                extract_definition_with_name(&inner, src, SymbolKind::Class)
                            {
                                definitions.push(def);
                            }
                            extract_python_methods(&inner, src, &mut definitions);
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    collect_call_sites(&root, src, &mut call_sites, &None, "call");

    Ok(ParsedFile {
        path: path.to_string(),
        language: Language::Python,
        definitions,
        imports,
        exports: vec![], // Python has no explicit export syntax
        call_sites,
    })
}

fn extract_python_import(node: &Node, source: &[u8]) -> Option<ImportInfo> {
    let line = node.start_position().row + 1;
    let mut names = Vec::new();

    let mut cursor = node.walk();
    for child in node.children_by_field_name("name", &mut cursor) {
        match child.kind() {
            "dotted_name" => {
                names.push(ImportedName {
                    name: node_text(&child, source).to_string(),
                    alias: None,
                });
            }
            "aliased_import" => {
                let name_node = child.child_by_field_name("name");
                let alias_node = child.child_by_field_name("alias");
                if let Some(n) = name_node {
                    names.push(ImportedName {
                        name: node_text(&n, source).to_string(),
                        alias: alias_node.map(|a| node_text(&a, source).to_string()),
                    });
                }
            }
            _ => {}
        }
    }

    if names.is_empty() {
        return None;
    }

    let source_str = names.first().map(|n| n.name.clone()).unwrap_or_default();

    Some(ImportInfo {
        source: source_str,
        names,
        is_default: false,
        is_namespace: true, // `import x` imports the whole module
        line,
    })
}

fn extract_python_import_from(node: &Node, source: &[u8]) -> Option<ImportInfo> {
    let line = node.start_position().row + 1;
    let module_node = node.child_by_field_name("module_name")?;
    let source_str = node_text(&module_node, source).to_string();

    let mut names = Vec::new();

    // Check for wildcard import
    let mut wc_cursor = node.walk();
    let has_wildcard = node
        .named_children(&mut wc_cursor)
        .any(|c| c.kind() == "wildcard_import");

    if has_wildcard {
        names.push(ImportedName {
            name: "*".to_string(),
            alias: None,
        });
    } else {
        let mut name_cursor = node.walk();
        for child in node.children_by_field_name("name", &mut name_cursor) {
            match child.kind() {
                "dotted_name" => {
                    names.push(ImportedName {
                        name: node_text(&child, source).to_string(),
                        alias: None,
                    });
                }
                "aliased_import" => {
                    let name_n = child.child_by_field_name("name");
                    let alias_n = child.child_by_field_name("alias");
                    if let Some(n) = name_n {
                        names.push(ImportedName {
                            name: node_text(&n, source).to_string(),
                            alias: alias_n.map(|a| node_text(&a, source).to_string()),
                        });
                    }
                }
                _ => {}
            }
        }
    }

    Some(ImportInfo {
        source: source_str,
        names,
        is_default: false,
        is_namespace: false,
        line,
    })
}

fn extract_python_methods(class_node: &Node, source: &[u8], definitions: &mut Vec<Definition>) {
    let body = match class_node.child_by_field_name("body") {
        Some(b) => b,
        None => return,
    };
    let mut cursor = body.walk();
    for child in body.named_children(&mut cursor) {
        match child.kind() {
            "function_definition" => {
                if let Some(def) =
                    extract_definition_with_name(&child, source, SymbolKind::Function)
                {
                    definitions.push(def);
                }
            }
            "decorated_definition" => {
                if let Some(inner) = child.child_by_field_name("definition") {
                    if inner.kind() == "function_definition" {
                        if let Some(def) =
                            extract_definition_with_name(&inner, source, SymbolKind::Function)
                        {
                            definitions.push(def);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn find_class_bases(node: &Node, source: &[u8], target_class: &str, bases: &mut Vec<String>) {
    match node.kind() {
        "class_definition" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                if node_text(&name_node, source) == target_class {
                    if let Some(supers) = node.child_by_field_name("superclasses") {
                        let mut cursor = supers.walk();
                        for child in supers.named_children(&mut cursor) {
                            match child.kind() {
                                "identifier" | "attribute" => {
                                    bases.push(node_text(&child, source).to_string());
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
        "decorated_definition" => {
            if let Some(inner) = node.child_by_field_name("definition") {
                find_class_bases(&inner, source, target_class, bases);
            }
        }
        _ => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                find_class_bases(&child, source, target_class, bases);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Common: call site extraction (recursive tree walk)
// ---------------------------------------------------------------------------

fn collect_call_sites(
    node: &Node,
    source: &[u8],
    calls: &mut Vec<CallSite>,
    containing: &Option<String>,
    call_kind: &str,
) {
    // Update containing function context
    let new_containing = match node.kind() {
        "function_declaration"
        | "generator_function_declaration"
        | "function_definition"
        | "method_definition"
        | "method_declaration" => node
            .child_by_field_name("name")
            .map(|n| node_text(&n, source).to_string()),
        "variable_declarator" => {
            let is_fn = node
                .child_by_field_name("value")
                .map(|v| v.kind() == "arrow_function" || v.kind() == "function")
                .unwrap_or(false);
            if is_fn {
                node.child_by_field_name("name")
                    .filter(|n| n.kind() == "identifier")
                    .map(|n| node_text(&n, source).to_string())
            } else {
                None
            }
        }
        _ => None,
    };

    let effective = if new_containing.is_some() {
        &new_containing
    } else {
        containing
    };

    if node.kind() == call_kind {
        if let Some(callee) = extract_callee(node, source) {
            calls.push(CallSite {
                callee,
                line: node.start_position().row + 1,
                containing_function: effective.clone(),
            });
        }
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_call_sites(&child, source, calls, effective, call_kind);
    }
}

fn extract_callee(node: &Node, source: &[u8]) -> Option<String> {
    let func = node.child_by_field_name("function")?;
    Some(node_text(&func, source).to_string())
}

// ---------------------------------------------------------------------------
// Data flow extraction: TypeScript / JavaScript
// ---------------------------------------------------------------------------

fn extract_ts_data_flow(source: &str) -> Result<DataFlowInfo, AstError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
        .map_err(|e| AstError::LanguageError(e.to_string()))?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| AstError::ParseError("tree-sitter failed to parse".into()))?;

    let root = tree.root_node();
    let src = source.as_bytes();
    let mut assignments = Vec::new();
    let mut calls_with_args = Vec::new();

    collect_ts_data_flow(&root, src, &mut assignments, &mut calls_with_args, &None);

    Ok(DataFlowInfo {
        assignments,
        calls_with_args,
    })
}

fn collect_ts_data_flow(
    node: &Node,
    source: &[u8],
    assignments: &mut Vec<VarCallAssignment>,
    calls: &mut Vec<CallWithArgs>,
    containing: &Option<String>,
) {
    // Update containing function context (same logic as collect_call_sites).
    let new_containing = match node.kind() {
        "function_declaration"
        | "generator_function_declaration"
        | "function_definition"
        | "method_definition"
        | "method_declaration" => node
            .child_by_field_name("name")
            .map(|n| node_text(&n, source).to_string()),
        "variable_declarator" => {
            let is_fn = node
                .child_by_field_name("value")
                .map(|v| v.kind() == "arrow_function" || v.kind() == "function")
                .unwrap_or(false);
            if is_fn {
                node.child_by_field_name("name")
                    .filter(|n| n.kind() == "identifier")
                    .map(|n| node_text(&n, source).to_string())
            } else {
                None
            }
        }
        _ => None,
    };

    let effective = if new_containing.is_some() {
        &new_containing
    } else {
        containing
    };

    // Detect variable assignment from a call: `const x = funcA()` or `const x = await funcA()`
    if node.kind() == "variable_declarator" {
        if let (Some(name_node), Some(value_node)) = (
            node.child_by_field_name("name"),
            node.child_by_field_name("value"),
        ) {
            if name_node.kind() == "identifier" {
                if let Some(callee) = extract_call_from_value(&value_node, source) {
                    assignments.push(VarCallAssignment {
                        variable: node_text(&name_node, source).to_string(),
                        callee,
                        line: node.start_position().row + 1,
                        containing_function: effective.clone(),
                    });
                }
            }
        }
    }

    // Detect call expression with arguments.
    if node.kind() == "call_expression" {
        if let Some(callee) = extract_callee(node, source) {
            let arguments = extract_argument_texts(node, source);
            calls.push(CallWithArgs {
                callee,
                arguments,
                line: node.start_position().row + 1,
                containing_function: effective.clone(),
            });
        }
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_ts_data_flow(&child, source, assignments, calls, effective);
    }
}

/// Extract the callee from a value that might be a call or an await wrapping a call.
fn extract_call_from_value(node: &Node, source: &[u8]) -> Option<String> {
    match node.kind() {
        "call_expression" => extract_callee(node, source),
        "await_expression" => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                if child.kind() == "call_expression" {
                    return extract_callee(&child, source);
                }
            }
            None
        }
        _ => None,
    }
}

/// Extract the text of each argument in a call expression's argument list.
fn extract_argument_texts(call_node: &Node, source: &[u8]) -> Vec<String> {
    let args_node = match call_node.child_by_field_name("arguments") {
        Some(n) => n,
        None => return vec![],
    };

    let mut args = Vec::new();
    let mut cursor = args_node.walk();
    for child in args_node.named_children(&mut cursor) {
        let text = node_text(&child, source).to_string();
        if !text.is_empty() {
            args.push(text);
        }
    }
    args
}

// ---------------------------------------------------------------------------
// Data flow extraction: Python
// ---------------------------------------------------------------------------

fn extract_python_data_flow(source: &str) -> Result<DataFlowInfo, AstError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_python::LANGUAGE.into())
        .map_err(|e| AstError::LanguageError(e.to_string()))?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| AstError::ParseError("tree-sitter failed to parse".into()))?;

    let root = tree.root_node();
    let src = source.as_bytes();
    let mut assignments = Vec::new();
    let mut calls_with_args = Vec::new();

    collect_python_data_flow(&root, src, &mut assignments, &mut calls_with_args, &None);

    Ok(DataFlowInfo {
        assignments,
        calls_with_args,
    })
}

fn collect_python_data_flow(
    node: &Node,
    source: &[u8],
    assignments: &mut Vec<VarCallAssignment>,
    calls: &mut Vec<CallWithArgs>,
    containing: &Option<String>,
) {
    // Update containing function context.
    let new_containing = match node.kind() {
        "function_definition" => node
            .child_by_field_name("name")
            .map(|n| node_text(&n, source).to_string()),
        _ => None,
    };

    let effective = if new_containing.is_some() {
        &new_containing
    } else {
        containing
    };

    // Detect assignment from a call: `x = func_a()` or `x = await func_a()`
    if node.kind() == "assignment" {
        if let (Some(left), Some(right)) = (
            node.child_by_field_name("left"),
            node.child_by_field_name("right"),
        ) {
            if left.kind() == "identifier" {
                let callee = match right.kind() {
                    "call" => extract_callee(&right, source),
                    "await" => {
                        let mut c = right.walk();
                        let call_node = right.named_children(&mut c).find(|ch| ch.kind() == "call");
                        call_node.and_then(|call| extract_callee(&call, source))
                    }
                    _ => None,
                };
                if let Some(callee) = callee {
                    assignments.push(VarCallAssignment {
                        variable: node_text(&left, source).to_string(),
                        callee,
                        line: node.start_position().row + 1,
                        containing_function: effective.clone(),
                    });
                }
            }
        }
    }

    // Detect call with arguments.
    if node.kind() == "call" {
        if let Some(callee) = extract_callee(node, source) {
            let arguments = extract_python_argument_texts(node, source);
            calls.push(CallWithArgs {
                callee,
                arguments,
                line: node.start_position().row + 1,
                containing_function: effective.clone(),
            });
        }
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_python_data_flow(&child, source, assignments, calls, effective);
    }
}

/// Extract argument texts from a Python call's argument_list.
fn extract_python_argument_texts(call_node: &Node, source: &[u8]) -> Vec<String> {
    let args_node = match call_node.child_by_field_name("arguments") {
        Some(n) => n,
        None => return vec![],
    };

    let mut args = Vec::new();
    let mut cursor = args_node.walk();
    for child in args_node.named_children(&mut cursor) {
        // Skip keyword argument names (only get the value)
        if child.kind() == "keyword_argument" {
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

// ---------------------------------------------------------------------------
// Go parsing
// ---------------------------------------------------------------------------

fn parse_go(path: &str, source: &str) -> Result<ParsedFile, AstError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_go::LANGUAGE.into())
        .map_err(|e| AstError::LanguageError(e.to_string()))?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| AstError::ParseError("tree-sitter failed to parse".into()))?;

    let root = tree.root_node();
    let src = source.as_bytes();
    let mut definitions = Vec::new();
    let mut imports = Vec::new();
    let mut call_sites = Vec::new();
    let mut exports = Vec::new();

    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        match child.kind() {
            "function_declaration" => {
                if let Some(def) = extract_definition_with_name(&child, src, SymbolKind::Function) {
                    if def.name.chars().next().map_or(false, |c| c.is_uppercase()) {
                        exports.push(ExportInfo {
                            name: def.name.clone(),
                            is_default: false,
                            is_reexport: false,
                            source: None,
                            line: def.start_line,
                        });
                    }
                    definitions.push(def);
                }
            }
            "method_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = node_text(&name_node, src).to_string();
                    let def = Definition {
                        name: name.clone(),
                        kind: SymbolKind::Function,
                        start_line: child.start_position().row + 1,
                        end_line: child.end_position().row + 1,
                    };
                    if name.chars().next().map_or(false, |c| c.is_uppercase()) {
                        exports.push(ExportInfo {
                            name: name.clone(),
                            is_default: false,
                            is_reexport: false,
                            source: None,
                            line: def.start_line,
                        });
                    }
                    definitions.push(def);
                }
            }
            "type_declaration" => {
                extract_go_type_defs(&child, src, &mut definitions, &mut exports);
            }
            "const_declaration" => {
                extract_go_const_defs(&child, src, &mut definitions, &mut exports);
            }
            "var_declaration" => {
                extract_go_var_defs(&child, src, &mut definitions);
            }
            "import_declaration" => {
                extract_go_imports(&child, src, &mut imports);
            }
            _ => {}
        }
    }

    collect_call_sites(&root, src, &mut call_sites, &None, "call_expression");

    Ok(ParsedFile {
        path: path.to_string(),
        language: Language::Go,
        definitions,
        imports,
        exports,
        call_sites,
    })
}

fn extract_go_imports(node: &Node, source: &[u8], imports: &mut Vec<ImportInfo>) {
    let line = node.start_position().row + 1;
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "import_spec" => {
                if let Some(imp) = extract_single_go_import(&child, source, line) {
                    imports.push(imp);
                }
            }
            "import_spec_list" => {
                let mut list_cursor = child.walk();
                for spec in child.named_children(&mut list_cursor) {
                    if spec.kind() == "import_spec" {
                        let spec_line = spec.start_position().row + 1;
                        if let Some(imp) = extract_single_go_import(&spec, source, spec_line) {
                            imports.push(imp);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn extract_single_go_import(node: &Node, source: &[u8], line: usize) -> Option<ImportInfo> {
    let path_node = node.child_by_field_name("path")?;
    let raw = node_text(&path_node, source);
    // Strip quotes from Go string literal: "fmt" → fmt
    let source_str = raw.trim_matches('"').to_string();

    let alias_node = node.child_by_field_name("name");
    let mut names = Vec::new();
    let mut is_namespace = false;

    match alias_node.map(|n| n.kind()) {
        Some("package_identifier") => {
            // Aliased import: import alias "path"
            let alias_text = node_text(&alias_node.unwrap(), source).to_string();
            names.push(ImportedName {
                name: go_package_name(&source_str),
                alias: Some(alias_text),
            });
            is_namespace = true;
        }
        Some("dot") => {
            // Dot import: import . "path" — imports all exported names
            names.push(ImportedName {
                name: "*".to_string(),
                alias: None,
            });
        }
        Some("blank_identifier") => {
            // Blank import: import _ "path" — side-effect only
            // No names imported
        }
        _ => {
            // Simple import: import "path" — imports the package name
            let pkg = go_package_name(&source_str);
            names.push(ImportedName {
                name: pkg,
                alias: None,
            });
            is_namespace = true;
        }
    }

    Some(ImportInfo {
        source: source_str,
        names,
        is_default: false,
        is_namespace,
        line,
    })
}

/// Extract Go package name from import path (last path segment).
/// e.g., "net/http" → "http", "github.com/gin-gonic/gin" → "gin"
fn go_package_name(import_path: &str) -> String {
    import_path
        .rsplit('/')
        .next()
        .unwrap_or(import_path)
        .to_string()
}

fn extract_go_type_defs(
    node: &Node,
    source: &[u8],
    definitions: &mut Vec<Definition>,
    exports: &mut Vec<ExportInfo>,
) {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "type_spec" {
            if let Some(name_node) = child.child_by_field_name("name") {
                let name = node_text(&name_node, source).to_string();
                let type_node = child.child_by_field_name("type");
                let kind = match type_node.as_ref().map(|n| n.kind()) {
                    Some("interface_type") => SymbolKind::Interface,
                    Some("struct_type") => SymbolKind::Class, // Use Class for structs
                    _ => SymbolKind::TypeAlias,
                };
                let def = Definition {
                    name: name.clone(),
                    kind,
                    start_line: child.start_position().row + 1,
                    end_line: child.end_position().row + 1,
                };
                if name.chars().next().map_or(false, |c| c.is_uppercase()) {
                    exports.push(ExportInfo {
                        name: name.clone(),
                        is_default: false,
                        is_reexport: false,
                        source: None,
                        line: def.start_line,
                    });
                }
                definitions.push(def);
            }
        }
        // type_alias is handled by same pattern (type_spec fallthrough above)
        if child.kind() == "type_alias" {
            if let Some(name_node) = child.child_by_field_name("name") {
                let name = node_text(&name_node, source).to_string();
                let def = Definition {
                    name: name.clone(),
                    kind: SymbolKind::TypeAlias,
                    start_line: child.start_position().row + 1,
                    end_line: child.end_position().row + 1,
                };
                if name.chars().next().map_or(false, |c| c.is_uppercase()) {
                    exports.push(ExportInfo {
                        name: name.clone(),
                        is_default: false,
                        is_reexport: false,
                        source: None,
                        line: def.start_line,
                    });
                }
                definitions.push(def);
            }
        }
    }
}

fn extract_go_const_defs(
    node: &Node,
    source: &[u8],
    definitions: &mut Vec<Definition>,
    exports: &mut Vec<ExportInfo>,
) {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "const_spec" {
            // const_spec can have multiple names: const A, B = 1, 2
            let mut name_cursor = child.walk();
            for name_child in child.children_by_field_name("name", &mut name_cursor) {
                if name_child.kind() == "identifier" {
                    let name = node_text(&name_child, source).to_string();
                    let def = Definition {
                        name: name.clone(),
                        kind: SymbolKind::Constant,
                        start_line: child.start_position().row + 1,
                        end_line: child.end_position().row + 1,
                    };
                    if name.chars().next().map_or(false, |c| c.is_uppercase()) {
                        exports.push(ExportInfo {
                            name: name.clone(),
                            is_default: false,
                            is_reexport: false,
                            source: None,
                            line: def.start_line,
                        });
                    }
                    definitions.push(def);
                }
            }
        }
    }
}

fn extract_go_var_defs(node: &Node, source: &[u8], definitions: &mut Vec<Definition>) {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "var_spec" => {
                let mut name_cursor = child.walk();
                for name_child in child.children_by_field_name("name", &mut name_cursor) {
                    if name_child.kind() == "identifier" {
                        definitions.push(Definition {
                            name: node_text(&name_child, source).to_string(),
                            kind: SymbolKind::Constant, // Use Constant for package-level vars
                            start_line: child.start_position().row + 1,
                            end_line: child.end_position().row + 1,
                        });
                    }
                }
            }
            "var_spec_list" => {
                let mut list_cursor = child.walk();
                for spec in child.named_children(&mut list_cursor) {
                    if spec.kind() == "var_spec" {
                        let mut name_cursor = spec.walk();
                        for name_child in spec.children_by_field_name("name", &mut name_cursor) {
                            if name_child.kind() == "identifier" {
                                definitions.push(Definition {
                                    name: node_text(&name_child, source).to_string(),
                                    kind: SymbolKind::Constant,
                                    start_line: spec.start_position().row + 1,
                                    end_line: spec.end_position().row + 1,
                                });
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Data flow extraction: Go
// ---------------------------------------------------------------------------

fn extract_go_data_flow(source: &str) -> Result<DataFlowInfo, AstError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_go::LANGUAGE.into())
        .map_err(|e| AstError::LanguageError(e.to_string()))?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| AstError::ParseError("tree-sitter failed to parse".into()))?;

    let root = tree.root_node();
    let src = source.as_bytes();
    let mut assignments = Vec::new();
    let mut calls_with_args = Vec::new();

    collect_go_data_flow(&root, src, &mut assignments, &mut calls_with_args, &None);

    Ok(DataFlowInfo {
        assignments,
        calls_with_args,
    })
}

fn collect_go_data_flow(
    node: &Node,
    source: &[u8],
    assignments: &mut Vec<VarCallAssignment>,
    calls: &mut Vec<CallWithArgs>,
    containing: &Option<String>,
) {
    // Update containing function context.
    let new_containing = match node.kind() {
        "function_declaration" => node
            .child_by_field_name("name")
            .map(|n| node_text(&n, source).to_string()),
        "method_declaration" => node
            .child_by_field_name("name")
            .map(|n| node_text(&n, source).to_string()),
        _ => None,
    };

    let effective = if new_containing.is_some() {
        &new_containing
    } else {
        containing
    };

    // Detect short variable declaration from call: `x := foo()`
    if node.kind() == "short_var_declaration" {
        if let (Some(left), Some(right)) = (
            node.child_by_field_name("left"),
            node.child_by_field_name("right"),
        ) {
            // Get the first identifier in the expression_list
            if left.kind() == "expression_list" {
                let first_id = left.named_child(0);
                if let Some(first_id) = first_id {
                    if first_id.kind() == "identifier" {
                        // Check if right side is a call
                        if right.kind() == "expression_list" {
                            let first_val = right.named_child(0);
                            if let Some(first_val) = first_val {
                                if first_val.kind() == "call_expression" {
                                    if let Some(callee) = extract_callee(&first_val, source) {
                                        assignments.push(VarCallAssignment {
                                            variable: node_text(&first_id, source).to_string(),
                                            callee,
                                            line: node.start_position().row + 1,
                                            containing_function: effective.clone(),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Detect call expressions with arguments
    if node.kind() == "call_expression" {
        if let Some(callee) = extract_callee(node, source) {
            let arguments = extract_go_argument_texts(node, source);
            calls.push(CallWithArgs {
                callee,
                arguments,
                line: node.start_position().row + 1,
                containing_function: effective.clone(),
            });
        }
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_go_data_flow(&child, source, assignments, calls, effective);
    }
}

/// Extract argument texts from a Go call's argument_list.
fn extract_go_argument_texts(call_node: &Node, source: &[u8]) -> Vec<String> {
    let args_node = match call_node.child_by_field_name("arguments") {
        Some(n) => n,
        None => return vec![],
    };

    let mut args = Vec::new();
    let mut cursor = args_node.walk();
    for child in args_node.named_children(&mut cursor) {
        let text = node_text(&child, source).to_string();
        if !text.is_empty() {
            args.push(text);
        }
    }
    args
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
mod tests;
