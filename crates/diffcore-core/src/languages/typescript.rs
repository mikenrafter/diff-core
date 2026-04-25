//! Typescript extraction code.
//!
//! Moved from `query_engine.rs` during the per-language module split.
//! Pure mechanical move — no logic changes.

use crate::ast::{Definition, ImportInfo, ImportedName};
use crate::languages::common::{
    collect_matches, extract_definitions_standard, get_or_insert_import, hash_str, node_span,
    node_text, CollectedMatch, ImportBuilder,
};
use crate::query_engine::{QueryEngineError, QueryWithCaptures};
use crate::types::SymbolKind;
use tree_sitter::{Node, QueryCursor};

pub(crate) fn extract_imports(
    root: &Node,
    source: &[u8],
    qwc: &QueryWithCaptures,
) -> Result<Vec<ImportInfo>, QueryEngineError> {
    let mut cursor = QueryCursor::new();
    let matches = collect_matches(&mut cursor, &qwc.query, *root, source);

    let stmt_idx = qwc.capture_index("stmt");
    let source_idx = qwc.capture_index("source");
    let default_name_idx = qwc.capture_index("default_name");
    let named_name_idx = qwc.capture_index("named_name");
    let aliased_name_idx = qwc.capture_index("aliased_name");
    let alias_idx = qwc.capture_index("alias");
    let ns_name_idx = qwc.capture_index("ns_name");

    // Ordered map to preserve source order.
    let mut import_map: Vec<(usize, ImportBuilder)> = Vec::new();

    for m in &matches {
        let mut stmt_start = 0usize;
        let mut source_text = String::new();
        let mut line = 0usize;

        for &(idx, node) in &m.captures {
            if Some(idx) == stmt_idx {
                stmt_start = node.start_byte();
                line = node.start_position().row + 1;
            }
            if Some(idx) == source_idx {
                source_text = node_text(&node, source).to_string();
            }
        }

        let entry = get_or_insert_import(&mut import_map, stmt_start, &source_text, line);

        if m.has_capture(default_name_idx) {
            // Default import
            for &(idx, node) in &m.captures {
                if Some(idx) == default_name_idx {
                    entry.is_default = true;
                    entry.names.push(ImportedName {
                        name: node_text(&node, source).to_string(),
                        alias: None,
                    });
                }
            }
        } else if m.has_capture(aliased_name_idx) {
            // Named import with alias (check before named_name since aliased also has named_name)
            let mut name = String::new();
            let mut alias = String::new();
            for &(idx, node) in &m.captures {
                if Some(idx) == aliased_name_idx {
                    name = node_text(&node, source).to_string();
                }
                if Some(idx) == alias_idx {
                    alias = node_text(&node, source).to_string();
                }
            }
            if !name.is_empty() {
                entry.names.retain(|n| n.name != name);
                entry.names.push(ImportedName {
                    name,
                    alias: Some(alias),
                });
            }
        } else if m.has_capture(ns_name_idx) {
            // Namespace import
            for &(idx, node) in &m.captures {
                if Some(idx) == ns_name_idx {
                    entry.is_namespace = true;
                    entry.names.push(ImportedName {
                        name: node_text(&node, source).to_string(),
                        alias: None,
                    });
                }
            }
        } else if m.has_capture(named_name_idx) {
            // Named import
            for &(idx, node) in &m.captures {
                if Some(idx) == named_name_idx {
                    let name = node_text(&node, source).to_string();
                    if !entry.names.iter().any(|n| n.name == name) {
                        entry.names.push(ImportedName { name, alias: None });
                    }
                }
            }
        }
        // else: side-effect import — source already captured, no names
    }

    Ok(import_map.into_iter().map(|(_, b)| b.build()).collect())
}

pub(crate) fn extract_definitions(
    matches: &[CollectedMatch<'_>],
    source: &[u8],
    qwc: &QueryWithCaptures,
    definitions: &mut Vec<Definition>,
    seen_nodes: &mut Vec<(usize, usize)>,
) {
    // Try standard-convention extractor first. When `definitions.scm`
    // has been migrated to use `@definition.<kind>` + `@name`, this
    // takes over and the legacy per-kind dispatch below is skipped.
    if extract_definitions_standard(matches, source, qwc, definitions, seen_nodes) {
        // Standard path emitted at least one definition: bespoke
        // dispatch would only re-discover the same nodes (and our
        // dedup would drop them) so we skip it for clarity.
    } else {
        // TS/JS: each definition kind has a distinct capture name pair
        let fn_name_idx = qwc.capture_index("fn_name");
        let fn_node_idx = qwc.capture_index("fn_node");
        let gen_name_idx = qwc.capture_index("gen_name");
        let gen_node_idx = qwc.capture_index("gen_node");
        let class_name_idx = qwc.capture_index("class_name");
        let class_node_idx = qwc.capture_index("class_node");
        let abstract_name_idx = qwc.capture_index("abstract_name");
        let abstract_node_idx = qwc.capture_index("abstract_node");
        let iface_name_idx = qwc.capture_index("iface_name");
        let iface_node_idx = qwc.capture_index("iface_node");
        let type_name_idx = qwc.capture_index("type_name");
        let type_node_idx = qwc.capture_index("type_node");
        let arrow_name_idx = qwc.capture_index("arrow_name");
        let arrow_node_idx = qwc.capture_index("arrow_node");
        let fn_expr_name_idx = qwc.capture_index("fn_expr_name");
        let fn_expr_node_idx = qwc.capture_index("fn_expr_node");
        let const_name_idx = qwc.capture_index("const_name");
        let const_value_idx = qwc.capture_index("const_value");
        let const_node_idx = qwc.capture_index("const_node");
        let method_name_idx = qwc.capture_index("method_name");
        let method_node_idx = qwc.capture_index("method_node");

        // Ordered list: (name_capture, node_capture, kind).
        // const_name/const_value is special-cased below.
        let ts_def_captures: &[(Option<u32>, Option<u32>, SymbolKind)] = &[
            (fn_name_idx, fn_node_idx, SymbolKind::Function),
            (gen_name_idx, gen_node_idx, SymbolKind::Function),
            (class_name_idx, class_node_idx, SymbolKind::Class),
            (abstract_name_idx, abstract_node_idx, SymbolKind::Class),
            (iface_name_idx, iface_node_idx, SymbolKind::Interface),
            (type_name_idx, type_node_idx, SymbolKind::TypeAlias),
            (arrow_name_idx, arrow_node_idx, SymbolKind::Function),
            (fn_expr_name_idx, fn_expr_node_idx, SymbolKind::Function),
            (method_name_idx, method_node_idx, SymbolKind::Function),
        ];

        for m in matches {
            // Skip the const_name pattern if the value is an arrow/function
            // (those are already captured by arrow_name/fn_expr_name patterns)
            if m.has_capture(const_name_idx) {
                let has_fn_value = m
                    .get_capture(const_value_idx)
                    .map(|n| {
                        let k = n.kind();
                        k == "arrow_function" || k == "function" || k == "function_expression"
                    })
                    .unwrap_or(false);
                if has_fn_value {
                    continue;
                }
                // Non-function constant
                let name_text = m
                    .get_capture(const_name_idx)
                    .map(|n| node_text(&n, source).to_string())
                    .unwrap_or_default();
                let (start_line, end_line, node_start) = node_span(m, const_node_idx);
                if !name_text.is_empty() {
                    let key = (node_start, hash_str(&name_text));
                    if !seen_nodes.contains(&key) {
                        seen_nodes.push(key);
                        definitions.push(Definition {
                            name: name_text,
                            kind: SymbolKind::Constant,
                            start_line,
                            end_line,
                        });
                    }
                }
                continue;
            }

            // Check each distinct definition capture
            for &(name_cap, node_cap, kind) in ts_def_captures {
                if m.has_capture(name_cap) {
                    let name_text = m
                        .get_capture(name_cap)
                        .map(|n| node_text(&n, source).to_string())
                        .unwrap_or_default();
                    let (start_line, end_line, node_start) = node_span(m, node_cap);
                    if !name_text.is_empty() {
                        let key = (node_start, hash_str(&name_text));
                        if !seen_nodes.contains(&key) {
                            seen_nodes.push(key);
                            definitions.push(Definition {
                                name: name_text,
                                kind,
                                start_line,
                                end_line,
                            });
                        }
                    }
                    break;
                }
            }
        }
    } // close `else` opened above for the standard-convention fallback
}

#[cfg(test)]
#[allow(
    unused_imports,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr
)]
mod tests {
    use crate::ast::Language;
    use crate::query_engine::{shared_test_engine, QueryEngine};
    use crate::types::SymbolKind;

    fn engine() -> &'static QueryEngine {
        shared_test_engine()
    }

    #[test]
    fn test_ts_default_import() {
        let e = engine();
        let result = e
            .parse_file("app.ts", "import React from 'react';")
            .unwrap();
        assert_eq!(result.imports.len(), 1);
        assert!(result.imports[0].is_default);
        assert_eq!(result.imports[0].source, "react");
        assert_eq!(result.imports[0].names[0].name, "React");
    }

    #[test]
    fn test_ts_named_imports() {
        let e = engine();
        let result = e
            .parse_file("app.ts", "import { useState, useEffect } from 'react';")
            .unwrap();
        assert_eq!(result.imports.len(), 1);
        assert!(!result.imports[0].is_default);
        assert_eq!(result.imports[0].source, "react");
        let names: Vec<&str> = result.imports[0]
            .names
            .iter()
            .map(|n| n.name.as_str())
            .collect();
        assert!(names.contains(&"useState"));
        assert!(names.contains(&"useEffect"));
    }

    #[test]
    fn test_ts_namespace_import() {
        let e = engine();
        let result = e
            .parse_file("app.ts", "import * as path from 'path';")
            .unwrap();
        assert_eq!(result.imports.len(), 1);
        assert!(result.imports[0].is_namespace);
        assert_eq!(result.imports[0].source, "path");
        assert_eq!(result.imports[0].names[0].name, "path");
    }

    #[test]
    fn test_ts_aliased_import() {
        let e = engine();
        let result = e
            .parse_file("app.ts", "import { foo as bar } from './utils';")
            .unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].source, "./utils");
        assert_eq!(result.imports[0].names[0].name, "foo");
        assert_eq!(result.imports[0].names[0].alias, Some("bar".to_string()));
    }

    #[test]
    fn test_ts_side_effect_import() {
        let e = engine();
        let result = e.parse_file("app.ts", "import './polyfill';").unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].source, "./polyfill");
        assert!(result.imports[0].names.is_empty());
    }

    #[test]
    fn test_ts_combined_default_and_named() {
        let e = engine();
        let result = e
            .parse_file("app.ts", "import React, { useState } from 'react';")
            .unwrap();
        assert_eq!(result.imports.len(), 1);
        assert!(result.imports[0].is_default);
        assert_eq!(result.imports[0].source, "react");
        let names: Vec<&str> = result.imports[0]
            .names
            .iter()
            .map(|n| n.name.as_str())
            .collect();
        assert!(names.contains(&"React"));
        assert!(names.contains(&"useState"));
    }

    #[test]
    fn test_ts_multiple_imports() {
        let e = engine();
        let source = r#"
import React from 'react';
import { useState } from 'react';
import * as path from 'path';
"#;
        let result = e.parse_file("app.ts", source).unwrap();
        assert_eq!(result.imports.len(), 3);
    }

    #[test]
    fn test_ts_exported_function() {
        let e = engine();
        let result = e
            .parse_file("lib.ts", "export function greet() {}")
            .unwrap();
        assert!(result
            .exports
            .iter()
            .any(|e| e.name == "greet" && !e.is_default));
        assert!(result.definitions.iter().any(|d| d.name == "greet"));
    }

    #[test]
    fn test_ts_export_default_function() {
        let e = engine();
        let result = e
            .parse_file("lib.ts", "export default function main() {}")
            .unwrap();
        assert!(result
            .exports
            .iter()
            .any(|e| e.name == "main" && e.is_default));
    }

    #[test]
    fn test_ts_named_exports() {
        let e = engine();
        let result = e.parse_file("lib.ts", "export { foo, bar };").unwrap();
        let names: Vec<&str> = result.exports.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"foo"));
        assert!(names.contains(&"bar"));
    }

    #[test]
    fn test_ts_reexport() {
        let e = engine();
        let result = e
            .parse_file("lib.ts", "export { baz } from './other';")
            .unwrap();
        let exp = result.exports.iter().find(|e| e.name == "baz").unwrap();
        assert!(exp.is_reexport);
        assert_eq!(exp.source, Some("./other".to_string()));
    }

    #[test]
    fn test_ts_wildcard_reexport() {
        let e = engine();
        let result = e.parse_file("lib.ts", "export * from './other';").unwrap();
        let exp = result.exports.iter().find(|e| e.name == "*").unwrap();
        assert!(exp.is_reexport);
        assert_eq!(exp.source, Some("./other".to_string()));
    }

    #[test]
    fn test_ts_export_const() {
        let e = engine();
        let result = e.parse_file("lib.ts", "export const VALUE = 42;").unwrap();
        assert!(result.exports.iter().any(|e| e.name == "VALUE"));
        assert!(result.definitions.iter().any(|d| d.name == "VALUE"));
    }

    #[test]
    fn test_ts_function_definition() {
        let e = engine();
        let result = e.parse_file("lib.ts", "function greet() {}").unwrap();
        let def = result
            .definitions
            .iter()
            .find(|d| d.name == "greet")
            .unwrap();
        assert_eq!(def.kind, SymbolKind::Function);
    }

    #[test]
    fn test_ts_class_definition() {
        let e = engine();
        let result = e
            .parse_file("lib.ts", "class User { getName() {} }")
            .unwrap();
        assert!(result
            .definitions
            .iter()
            .any(|d| d.name == "User" && d.kind == SymbolKind::Class));
        assert!(result
            .definitions
            .iter()
            .any(|d| d.name == "getName" && d.kind == SymbolKind::Function));
    }

    #[test]
    fn test_ts_interface_definition() {
        let e = engine();
        let result = e
            .parse_file("lib.ts", "interface IUser { name: string; }")
            .unwrap();
        assert!(result
            .definitions
            .iter()
            .any(|d| d.name == "IUser" && d.kind == SymbolKind::Interface));
    }

    #[test]
    fn test_ts_type_alias_definition() {
        let e = engine();
        let result = e
            .parse_file("lib.ts", "type Result = { ok: boolean };")
            .unwrap();
        assert!(result
            .definitions
            .iter()
            .any(|d| d.name == "Result" && d.kind == SymbolKind::TypeAlias));
    }

    #[test]
    fn test_ts_arrow_function_def() {
        let e = engine();
        let result = e.parse_file("lib.ts", "const greet = () => {};").unwrap();
        let def = result
            .definitions
            .iter()
            .find(|d| d.name == "greet")
            .unwrap();
        assert_eq!(def.kind, SymbolKind::Function);
    }

    #[test]
    fn test_ts_const_value_def() {
        let e = engine();
        let result = e.parse_file("lib.ts", "const MAX = 100;").unwrap();
        let def = result.definitions.iter().find(|d| d.name == "MAX").unwrap();
        assert_eq!(def.kind, SymbolKind::Constant);
    }

    #[test]
    fn test_ts_call_site() {
        let e = engine();
        let result = e
            .parse_file("lib.ts", "function main() { greet('world'); }")
            .unwrap();
        assert!(result.call_sites.iter().any(|c| c.callee == "greet"));
    }

    #[test]
    fn test_ts_method_call() {
        let e = engine();
        let result = e.parse_file("lib.ts", "console.log('hello');").unwrap();
        assert!(result.call_sites.iter().any(|c| c.callee == "console.log"));
    }

    #[test]
    fn test_ts_call_containing_function() {
        let e = engine();
        let result = e
            .parse_file("lib.ts", "function main() { greet(); }")
            .unwrap();
        let call = result
            .call_sites
            .iter()
            .find(|c| c.callee == "greet")
            .unwrap();
        assert_eq!(call.containing_function, Some("main".to_string()));
    }

    #[test]
    fn test_ts_assignment_from_call() {
        let e = engine();
        let df = e
            .extract_data_flow("lib.ts", "const result = fetchData();")
            .unwrap();
        assert_eq!(df.assignments.len(), 1);
        assert_eq!(df.assignments[0].variable, "result");
        assert_eq!(df.assignments[0].callee, "fetchData");
    }

    #[test]
    fn test_ts_assignment_from_await() {
        let e = engine();
        let df = e
            .extract_data_flow("lib.ts", "const data = await fetchData();")
            .unwrap();
        assert_eq!(df.assignments.len(), 1);
        assert_eq!(df.assignments[0].variable, "data");
        assert_eq!(df.assignments[0].callee, "fetchData");
    }

    #[test]
    fn test_ts_call_with_args() {
        let e = engine();
        let df = e
            .extract_data_flow("lib.ts", "processData(input, config);")
            .unwrap();
        assert_eq!(df.calls_with_args.len(), 1);
        assert_eq!(df.calls_with_args[0].callee, "processData");
        assert_eq!(df.calls_with_args[0].arguments, vec!["input", "config"]);
    }

    #[test]
    fn test_parity_ts_full_file() {
        let source = r#"
import React from 'react';
import { useState, useEffect } from 'react';
import * as path from 'path';
import { foo as bar } from './utils';

export function greet(name: string) {
    console.log(name);
}

export default function main() {}

export class UserService {
    getUser() {}
}

export interface IConfig {
    port: number;
}

export type Result = { ok: boolean };

export const VALUE = 42;

const handler = () => {
    fetchData();
};
"#;
        let e = engine();
        let qe_result = e.parse_file("app.ts", source).unwrap();
        let ast_result = crate::ast::parse_file("app.ts", source).unwrap();

        // Import count should match
        assert_eq!(
            qe_result.imports.len(),
            ast_result.imports.len(),
            "import count mismatch: qe={}, ast={}",
            qe_result.imports.len(),
            ast_result.imports.len()
        );

        // Export count should match
        assert_eq!(
            qe_result.exports.len(),
            ast_result.exports.len(),
            "export count mismatch: qe={}, ast={}",
            qe_result.exports.len(),
            ast_result.exports.len()
        );

        // All definition names from ast should be present in query engine results
        for ast_def in &ast_result.definitions {
            assert!(
                qe_result
                    .definitions
                    .iter()
                    .any(|d| d.name == ast_def.name && d.kind == ast_def.kind),
                "missing definition from query engine: {} ({:?})",
                ast_def.name,
                ast_def.kind,
            );
        }

        // All call site callees from ast should be present
        for ast_call in &ast_result.call_sites {
            assert!(
                qe_result
                    .call_sites
                    .iter()
                    .any(|c| c.callee == ast_call.callee),
                "missing call site from query engine: {}",
                ast_call.callee,
            );
        }
    }

    #[test]
    fn test_ts_abstract_class() {
        let e = engine();
        let source = "abstract class Base { abstract process(): void; helper() {} }";
        let result = e.parse_file("base.ts", source).unwrap();
        assert!(
            result
                .definitions
                .iter()
                .any(|d| d.name == "Base" && d.kind == SymbolKind::Class),
            "abstract classes should be captured"
        );
        assert!(result.definitions.iter().any(|d| d.name == "helper"));
    }

    #[test]
    fn test_ts_generator_function() {
        let e = engine();
        let result = e
            .parse_file("gen.ts", "function* gen() { yield 1; }")
            .unwrap();
        assert!(result
            .definitions
            .iter()
            .any(|d| d.name == "gen" && d.kind == SymbolKind::Function));
    }

    #[test]
    fn test_ts_export_default_expression() {
        // Known limitation: the query engine's exports.scm captures exported
        // declarations (export function/class/const/etc.) but not bare
        // `export default <expression>`. The imperative ast.rs parser handles
        // this via the `value` field fallback. Document the gap here.
        let e = engine();
        let source = "const app = createApp();\nexport default app;\n";
        let _result = e.parse_file("app.ts", source).unwrap();
        // The imperative parser captures this; query engine does not (known gap)
        let ast_result = crate::ast::parse_file("app.ts", source).unwrap();
        assert!(
            ast_result.exports.iter().any(|e| e.is_default),
            "ast.rs should capture export default expression"
        );
        // Query engine may or may not capture it — this is a known gap
        // (exports.scm needs a pattern for `export default <expression>`)
    }

    #[test]
    fn test_ts_multiple_exports_same_line() {
        let e = engine();
        let source = "export { a, b, c };";
        let result = e.parse_file("mod.ts", source).unwrap();
        let names: Vec<&str> = result.exports.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"a"));
        assert!(names.contains(&"b"));
        assert!(names.contains(&"c"));
    }

    #[test]
    fn test_ts_reexport_with_rename() {
        let e = engine();
        let source = "export { foo as bar } from './mod';";
        let result = e.parse_file("index.ts", source).unwrap();
        // The query engine should capture the re-export
        assert!(!result.exports.is_empty());
        // Should have the re-export source
        assert!(result.exports.iter().any(|e| e.is_reexport));
    }

    #[test]
    fn test_ts_unicode_identifiers() {
        let e = engine();
        let source = "function grüßen() {}\nconst αβγ = 42;\n";
        let result = e.parse_file("unicode.ts", source).unwrap();
        assert!(result.definitions.iter().any(|d| d.name == "grüßen"));
        assert!(result.definitions.iter().any(|d| d.name == "αβγ"));
    }

    #[test]
    fn test_ts_syntax_error_partial_results() {
        let e = engine();
        let source = "function valid() {}\nconst x = {{{;\nfunction alsoValid() {}";
        let result = e.parse_file("broken.ts", source).unwrap();
        // tree-sitter does partial parsing — the definition before the error survives
        assert!(result.definitions.iter().any(|d| d.name == "valid"));
        // Recovery after errors is best-effort — later defs may or may not be captured
        assert_eq!(result.language, Language::TypeScript);
    }

    #[test]
    fn test_ts_let_and_var_declarations() {
        let e = engine();
        let source = "let x = 1;\nvar y = 2;\n";
        let result = e.parse_file("vars.ts", source).unwrap();
        // let and var are variable_declarations, should be captured as constants
        assert!(result.definitions.iter().any(|d| d.name == "x"));
        assert!(result.definitions.iter().any(|d| d.name == "y"));
    }

    #[test]
    fn test_ts_deeply_nested_call_containing_function() {
        let e = engine();
        let source = r#"
function outer() {
    function inner() {
        deepCall();
    }
    outerCall();
}
"#;
        let result = e.parse_file("nested.ts", source).unwrap();
        let deep = result
            .call_sites
            .iter()
            .find(|c| c.callee == "deepCall")
            .unwrap();
        assert_eq!(deep.containing_function, Some("inner".to_string()));

        let outer_call = result
            .call_sites
            .iter()
            .find(|c| c.callee == "outerCall")
            .unwrap();
        assert_eq!(outer_call.containing_function, Some("outer".to_string()));
    }

    #[test]
    fn test_ts_arrow_function_containing() {
        let e = engine();
        let source = "const handler = () => { innerCall(); };";
        let result = e.parse_file("fn.ts", source).unwrap();
        let call = result
            .call_sites
            .iter()
            .find(|c| c.callee == "innerCall")
            .unwrap();
        assert_eq!(call.containing_function, Some("handler".to_string()));
    }

    #[test]
    fn test_ts_comments_only_empty_result() {
        let e = engine();
        let source = "// comment\n/* block */\n/** jsdoc */\n";
        let result = e.parse_file("comments.ts", source).unwrap();
        assert!(result.definitions.is_empty());
        assert!(result.imports.is_empty());
        assert!(result.exports.is_empty());
        assert!(result.call_sites.is_empty());
    }

    #[test]
    fn test_ts_export_multiple_const() {
        let e = engine();
        let source = "export const A = 1, B = 2;";
        let result = e.parse_file("consts.ts", source).unwrap();
        assert!(result.exports.iter().any(|ex| ex.name == "A"));
        assert!(result.exports.iter().any(|ex| ex.name == "B"));
    }

    #[test]
    fn test_ts_class_with_multiple_methods() {
        let e = engine();
        let source = "class Router {\n    get() {}\n    post() {}\n    delete() {}\n}";
        let result = e.parse_file("router.ts", source).unwrap();
        assert!(result
            .definitions
            .iter()
            .any(|d| d.name == "Router" && d.kind == SymbolKind::Class));
        assert!(result.definitions.iter().any(|d| d.name == "get"));
        assert!(result.definitions.iter().any(|d| d.name == "post"));
        assert!(result.definitions.iter().any(|d| d.name == "delete"));
    }

    #[test]
    fn test_ts_data_flow_method_chain_assignment() {
        let e = engine();
        let df = e
            .extract_data_flow("app.ts", "const result = db.query.findMany();")
            .unwrap();
        assert_eq!(df.assignments.len(), 1);
        assert_eq!(df.assignments[0].variable, "result");
        assert_eq!(df.assignments[0].callee, "db.query.findMany");
    }

    #[test]
    fn test_parity_ts_exports_match_ast() {
        let e = engine();
        let source = r#"
export function fn1() {}
export default function fn2() {}
export { a, b };
export { c } from './mod';
export * from './all';
export const VAL = 1;
export class Cls {}
export interface IFace {}
export type TAlias = number;
"#;
        let qe = e.parse_file("parity.ts", source).unwrap();
        let ast = crate::ast::parse_file("parity.ts", source).unwrap();

        assert_eq!(
            qe.exports.len(),
            ast.exports.len(),
            "export count mismatch: qe={} vs ast={}",
            qe.exports.len(),
            ast.exports.len()
        );

        // Every export name from ast should be in qe
        for ast_exp in &ast.exports {
            assert!(
                qe.exports.iter().any(|e| e.name == ast_exp.name),
                "missing export '{}' in query engine",
                ast_exp.name
            );
        }
    }

    #[test]
    fn test_parity_ts_data_flow_match_ast() {
        let e = engine();
        let source = r#"
function handler(req: any) {
    const data = parseBody(req);
    const user = await fetchUser(data.id);
    return respond(user);
}
"#;
        let qe_df = e.extract_data_flow("handler.ts", source).unwrap();
        let ast_df = crate::ast::extract_data_flow_info("handler.ts", source).unwrap();

        assert_eq!(
            qe_df.assignments.len(),
            ast_df.assignments.len(),
            "assignment count mismatch"
        );

        for ast_a in &ast_df.assignments {
            assert!(
                qe_df
                    .assignments
                    .iter()
                    .any(|a| a.variable == ast_a.variable && a.callee == ast_a.callee),
                "missing assignment {}.{} in query engine",
                ast_a.variable,
                ast_a.callee
            );
        }
    }

    #[test]
    fn test_ts_enum_not_captured_known_gap() {
        // Known gap: TS enum declarations (enum_declaration) are not in definitions.scm
        let e = engine();
        let source = "enum Direction { Up, Down, Left, Right }";
        let result = e.parse_file("dir.ts", source).unwrap();
        // Document current behavior: enums are NOT captured
        let has_enum = result.definitions.iter().any(|d| d.name == "Direction");
        assert!(
            !has_enum,
            "Enums are not captured (known gap) — if this starts passing, update the .scm file"
        );
    }

    #[test]
    fn test_ts_const_enum_not_captured_known_gap() {
        let e = engine();
        let source = "const enum Status { Active, Inactive }";
        let result = e.parse_file("status.ts", source).unwrap();
        let has_enum = result.definitions.iter().any(|d| d.name == "Status");
        assert!(!has_enum, "Const enums are not captured (known gap)");
    }

    #[test]
    fn test_ts_export_default_identifier_gap() {
        // Known gap: `export default foo` (bare identifier) not captured
        let e = engine();
        let source = "const app = {};\nexport default app;";
        let result = e.parse_file("app.ts", source).unwrap();
        // Query engine cannot capture bare export default expressions
        let has_default = result.exports.iter().any(|e| e.is_default);
        assert!(
            !has_default,
            "Export default identifier is not captured (known gap)"
        );
    }

    #[test]
    fn test_ts_export_default_class_expression_gap() {
        // export default class { } (anonymous class) — another known gap
        let e = engine();
        let source = "export default class { method() {} }";
        let _result = e.parse_file("anon.ts", source).unwrap();
        // Anonymous class export — no name to capture
        // This is acceptable: we can't meaningfully track anonymous exports
    }

    #[test]
    fn test_ts_export_enum_not_captured_known_gap() {
        let e = engine();
        let source = "export enum Color { Red, Green, Blue }";
        let result = e.parse_file("colors.ts", source).unwrap();
        let has_enum_export = result.exports.iter().any(|e| e.name == "Color");
        assert!(!has_enum_export, "Export enum is not captured (known gap)");
    }

    #[test]
    fn test_ts_import_type_captured() {
        // `import type { Foo } from ...` is parsed as a regular import_statement
        // by tree-sitter TS, so our import patterns capture it. This is fine for
        // flow analysis — type imports still create dependency edges.
        let e = engine();
        let source = "import type { User } from './models';";
        let result = e.parse_file("app.ts", source).unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].source, "./models");
        // The "type" keyword gets captured as the first identifier by the default_name pattern
        // This is slightly incorrect but the import source is correct, which is what matters
        // for building dependency edges
    }

    #[test]
    fn test_ts_destructuring_assignment_not_in_assignments() {
        // Destructuring assignments like `const { a, b } = foo()` are not captured
        // by assignments.scm because the LHS is not an `identifier` but an
        // `object_pattern`. The IR layer handles destructuring via IrPattern.
        let e = engine();
        let df = e
            .extract_data_flow("app.ts", "const { a, b } = getData();")
            .unwrap();
        assert!(
            df.assignments.is_empty(),
            "destructuring assignments not in .scm query (handled by IR layer)"
        );
    }

    #[test]
    fn test_ts_array_destructuring_not_in_assignments() {
        let e = engine();
        let df = e
            .extract_data_flow("app.ts", "const [first, ...rest] = getList();")
            .unwrap();
        assert!(
            df.assignments.is_empty(),
            "array destructuring not in .scm query (handled by IR layer)"
        );
    }

    #[test]
    fn test_ts_as_const_assignment() {
        let e = engine();
        let source = "const config = { port: 3000 } as const;";
        let result = e.parse_file("config.ts", source).unwrap();
        assert!(result
            .definitions
            .iter()
            .any(|d| d.name == "config" && d.kind == SymbolKind::Constant));
    }

    #[test]
    fn test_ts_satisfies_expression() {
        let e = engine();
        let source = "const config = { port: 3000 } satisfies Config;";
        let result = e.parse_file("config.ts", source).unwrap();
        assert!(result
            .definitions
            .iter()
            .any(|d| d.name == "config" && d.kind == SymbolKind::Constant));
    }

    #[test]
    fn test_ts_export_default_anonymous_function() {
        let e = engine();
        let source = "export default function() { return 42; }";
        let _result = e.parse_file("anon.ts", source).unwrap();
        // Anonymous function — no name to capture. This is acceptable.
        // The export statement itself may or may not match.
    }

    #[test]
    fn test_ts_complex_type_alias() {
        let e = engine();
        let source = "type EventName = `on${string}`;";
        let result = e.parse_file("events.ts", source).unwrap();
        assert!(result
            .definitions
            .iter()
            .any(|d| d.name == "EventName" && d.kind == SymbolKind::TypeAlias));
    }

    #[test]
    fn test_ts_namespace_not_captured_known_gap() {
        let e = engine();
        let source = "namespace MyApp { export function init() {} }";
        let result = e.parse_file("app.ts", source).unwrap();
        // namespace declarations use `module` node type in tree-sitter TS
        // Our definitions.scm doesn't capture them — acceptable for diff analysis
        let _has_namespace = result.definitions.iter().any(|d| d.name == "MyApp");
        // init() inside should still be captured
        assert!(result.definitions.iter().any(|d| d.name == "init"));
    }

    #[test]
    fn test_ts_export_assignment_not_captured() {
        let e = engine();
        let source = "class Foo {}\nexport = Foo;";
        let _result = e.parse_file("mod.ts", source).unwrap();
        // export = Foo is TypeScript-specific CommonJS compat, rarely used
        // Not captured — acceptable gap
    }

    #[test]
    fn test_ts_require_captured_as_call() {
        // require() is a call expression, not an import statement
        // It should appear in call_sites, which is correct for CJS detection
        let e = engine();
        let source = "const fs = require('fs');";
        let result = e.parse_file("app.ts", source).unwrap();
        assert!(result.call_sites.iter().any(|c| c.callee == "require"));
    }

    #[test]
    fn test_ts_dynamic_import_not_in_imports() {
        // import('module') is a call_expression, not an import_statement
        let e = engine();
        let source = "const mod = await import('./lazy');";
        let result = e.parse_file("app.ts", source).unwrap();
        // Should NOT be in static imports
        assert!(result.imports.is_empty());
        // But should be captured by data flow as an assignment from a call
        let df = e.extract_data_flow("app.ts", source).unwrap();
        assert_eq!(df.assignments.len(), 1);
        assert_eq!(df.assignments[0].variable, "mod");
    }

    #[test]
    fn test_ts_export_arrow_function() {
        let e = engine();
        let source = "export const handler = async (req: Request) => { return new Response(); };";
        let result = e.parse_file("handler.ts", source).unwrap();
        assert!(result.exports.iter().any(|e| e.name == "handler"));
        assert!(result
            .definitions
            .iter()
            .any(|d| d.name == "handler" && d.kind == SymbolKind::Function));
    }

    #[test]
    fn test_ts_namespace_reexport() {
        let e = engine();
        let source = "export * as utils from './utils';";
        let result = e.parse_file("index.ts", source).unwrap();
        // namespace re-export should be captured by the wildcard pattern
        assert!(!result.exports.is_empty());
    }

    #[test]
    fn test_ts_function_expression_containing() {
        let e = engine();
        let source = "const handler = function named() { innerCall(); };";
        let result = e.parse_file("fn.ts", source).unwrap();
        let call = result
            .call_sites
            .iter()
            .find(|c| c.callee == "innerCall")
            .unwrap();
        // function_expression assigned to variable — should find "handler" or "named"
        assert!(
            call.containing_function.is_some(),
            "function expression should have a containing function"
        );
    }

    #[test]
    fn test_ts_computed_property_method() {
        let e = engine();
        let source = "class Router { [Symbol.iterator]() {} }";
        let result = e.parse_file("router.ts", source).unwrap();
        // Computed property methods have non-identifier names — class should still be captured
        assert!(result.definitions.iter().any(|d| d.name == "Router"));
        // The computed method name is captured as the full expression text
        // e.g. "Symbol.iterator" via the (_) capture in method_definition
    }

    #[test]
    fn test_ts_static_method() {
        let e = engine();
        let source = "class Factory { static create() {} }";
        let result = e.parse_file("factory.ts", source).unwrap();
        assert!(result.definitions.iter().any(|d| d.name == "Factory"));
        assert!(result.definitions.iter().any(|d| d.name == "create"));
    }

    #[test]
    fn test_ts_getter_setter() {
        let e = engine();
        let source = "class User { get name() { return ''; } set name(v: string) {} }";
        let result = e.parse_file("user.ts", source).unwrap();
        assert!(result.definitions.iter().any(|d| d.name == "User"));
        // getter/setter are method_definitions — should be captured
        assert!(result.definitions.iter().any(|d| d.name == "name"));
    }

    #[test]
    fn test_all_definition_kinds_ts() {
        let e = engine();
        let source = r#"
function fn() {}
function* gen() {}
class Cls {}
abstract class ACls {}
interface IFace {}
type TAlias = string;
const handler = () => {};
const fnExpr = function() {};
const VAL = 42;
class WithMethod { method() {} }
"#;
        let result = e.parse_file("all.ts", source).unwrap();
        assert!(result
            .definitions
            .iter()
            .any(|d| d.name == "fn" && d.kind == SymbolKind::Function));
        assert!(result
            .definitions
            .iter()
            .any(|d| d.name == "gen" && d.kind == SymbolKind::Function));
        assert!(result
            .definitions
            .iter()
            .any(|d| d.name == "Cls" && d.kind == SymbolKind::Class));
        assert!(result
            .definitions
            .iter()
            .any(|d| d.name == "ACls" && d.kind == SymbolKind::Class));
        assert!(result
            .definitions
            .iter()
            .any(|d| d.name == "IFace" && d.kind == SymbolKind::Interface));
        assert!(result
            .definitions
            .iter()
            .any(|d| d.name == "TAlias" && d.kind == SymbolKind::TypeAlias));
        assert!(result
            .definitions
            .iter()
            .any(|d| d.name == "handler" && d.kind == SymbolKind::Function));
        assert!(result
            .definitions
            .iter()
            .any(|d| d.name == "fnExpr" && d.kind == SymbolKind::Function));
        assert!(result
            .definitions
            .iter()
            .any(|d| d.name == "VAL" && d.kind == SymbolKind::Constant));
        assert!(result
            .definitions
            .iter()
            .any(|d| d.name == "method" && d.kind == SymbolKind::Function));
    }

    #[test]
    fn test_ts_const_assigned_function_expression_not_double_def() {
        // const fnExpr = function() {} should be captured as Function (via fn_expr_name),
        // NOT additionally as a Constant (via const_name). The skip logic checks
        // the value node kind for "arrow_function", "function", and "function_expression".
        let e = engine();
        let source = "const handler = function() { return 42; };";
        let result = e.parse_file("fn.ts", source).unwrap();
        let handler_defs: Vec<_> = result
            .definitions
            .iter()
            .filter(|d| d.name == "handler")
            .collect();
        assert_eq!(
            handler_defs.len(),
            1,
            "function expression should produce exactly 1 definition, got {}",
            handler_defs.len()
        );
        assert_eq!(
            handler_defs[0].kind,
            SymbolKind::Function,
            "should be Function, not Constant"
        );
    }

    #[test]
    fn test_ts_const_assigned_named_function_expression_not_double_def() {
        // const handler = function named() {} — named function expression
        let e = engine();
        let source = "const handler = function named() { return 42; };";
        let result = e.parse_file("fn.ts", source).unwrap();
        // Should have "handler" as Function, not double-counted
        let handler_defs: Vec<_> = result
            .definitions
            .iter()
            .filter(|d| d.name == "handler")
            .collect();
        assert_eq!(
            handler_defs.len(),
            1,
            "named function expression should produce exactly 1 definition for 'handler'"
        );
        assert_eq!(handler_defs[0].kind, SymbolKind::Function);
    }

    #[test]
    fn test_ts_new_expression_not_captured_known_gap() {
        // `new Foo()` uses `new_expression` in tree-sitter TS, not `call_expression`.
        // calls.scm only matches call_expression, so class instantiations are missed.
        // This affects `instantiates` edge construction.
        let e = engine();
        let source = "const user = new User('Alice');";
        let result = e.parse_file("app.ts", source).unwrap();
        let has_user_call = result.call_sites.iter().any(|c| c.callee == "User");
        assert!(
            !has_user_call,
            "new_expression is not captured as a call site (known gap)"
        );
    }
}
