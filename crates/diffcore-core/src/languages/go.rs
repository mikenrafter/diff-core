//! GO extraction code.
//!
//! Moved from `query_engine.rs` during the per-language module split.
//! Pure mechanical move — no logic changes.

use crate::ast::{Definition, ImportInfo, ImportedName};
use crate::languages::common::{
    collect_matches, extract_definitions_standard, hash_str, node_span, node_text, CollectedMatch,
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
    let alias_name_idx = qwc.capture_index("alias_name");
    let dot_import_idx = qwc.capture_index("dot_import");
    let blank_import_idx = qwc.capture_index("blank_import");

    let mut imports = Vec::new();
    let mut seen: Vec<(usize, String)> = Vec::new();

    for m in &matches {
        let mut line = 0usize;
        let mut source_text = String::new();

        for &(idx, node) in &m.captures {
            if Some(idx) == stmt_idx {
                line = node.start_position().row + 1;
            }
            if Some(idx) == source_idx {
                // Strip quotes from Go string literal
                let raw = node_text(&node, source);
                source_text = raw.trim_matches('"').to_string();
                line = node.start_position().row + 1;
            }
        }

        if source_text.is_empty() {
            continue;
        }

        // Dedup: same source at same line
        let key = (line, source_text.clone());
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);

        let pkg_name = source_text
            .rsplit('/')
            .next()
            .unwrap_or(&source_text)
            .to_string();

        if m.has_capture(dot_import_idx) {
            // import . "path" — wildcard import
            imports.push(ImportInfo {
                source: source_text,
                names: vec![ImportedName {
                    name: "*".to_string(),
                    alias: None,
                }],
                is_default: false,
                is_namespace: false,
                line,
            });
        } else if m.has_capture(blank_import_idx) {
            // import _ "path" — side-effect only
            imports.push(ImportInfo {
                source: source_text,
                names: vec![],
                is_default: false,
                is_namespace: false,
                line,
            });
        } else if m.has_capture(alias_name_idx) {
            // import alias "path"
            let mut alias_text = String::new();
            for &(idx, node) in &m.captures {
                if Some(idx) == alias_name_idx {
                    alias_text = node_text(&node, source).to_string();
                }
            }
            imports.push(ImportInfo {
                source: source_text,
                names: vec![ImportedName {
                    name: pkg_name,
                    alias: Some(alias_text),
                }],
                is_default: false,
                is_namespace: true,
                line,
            });
        } else {
            // Simple import "path"
            imports.push(ImportInfo {
                source: source_text,
                names: vec![ImportedName {
                    name: pkg_name,
                    alias: None,
                }],
                is_default: false,
                is_namespace: true,
                line,
            });
        }
    }

    Ok(imports)
}

pub(crate) fn extract_definitions(
    matches: &[CollectedMatch<'_>],
    source: &[u8],
    qwc: &QueryWithCaptures,
    definitions: &mut Vec<Definition>,
    seen_nodes: &mut Vec<(usize, usize)>,
) {
    if extract_definitions_standard(matches, source, qwc, definitions, seen_nodes) {
        // Standard path took over: bespoke fallback skipped.
    } else {
        let fn_name_idx = qwc.capture_index("fn_name");
        let fn_node_idx = qwc.capture_index("fn_node");
        let method_name_idx = qwc.capture_index("method_name");
        let method_node_idx = qwc.capture_index("method_node");
        let struct_name_idx = qwc.capture_index("struct_name");
        let struct_node_idx = qwc.capture_index("struct_node");
        let iface_name_idx = qwc.capture_index("iface_name");
        let iface_node_idx = qwc.capture_index("iface_node");
        let type_name_idx = qwc.capture_index("type_name");
        let type_node_idx = qwc.capture_index("type_node");
        let const_name_idx = qwc.capture_index("const_name");
        let const_node_idx = qwc.capture_index("const_node");
        let var_decl_name_idx = qwc.capture_index("var_decl_name");
        let var_decl_node_idx = qwc.capture_index("var_decl_node");

        let go_def_captures: &[(Option<u32>, Option<u32>, SymbolKind)] = &[
            (fn_name_idx, fn_node_idx, SymbolKind::Function),
            (method_name_idx, method_node_idx, SymbolKind::Function),
            (struct_name_idx, struct_node_idx, SymbolKind::Class),
            (iface_name_idx, iface_node_idx, SymbolKind::Interface),
            (type_name_idx, type_node_idx, SymbolKind::TypeAlias),
            (const_name_idx, const_node_idx, SymbolKind::Constant),
            (var_decl_name_idx, var_decl_node_idx, SymbolKind::Constant),
        ];

        for m in matches {
            for &(name_cap, node_cap, kind) in go_def_captures {
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
