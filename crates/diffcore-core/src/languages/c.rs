//! C extraction code.
//!
//! Moved from `query_engine.rs` during the per-language module split.
//! Pure mechanical move — no logic changes.

use crate::ast::{Definition, ImportedName, ImportInfo};
use crate::languages::common::{
    collect_matches, hash_str, node_span, node_text,
    extract_definitions_standard, CollectedMatch,
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

    let mut imports = Vec::new();
    let mut seen: Vec<(usize, String)> = Vec::new();

    for m in &matches {
        let line = m
            .get_capture(stmt_idx)
            .map(|n| n.start_position().row + 1)
            .unwrap_or(0);

        if m.has_capture(source_idx) {
            let raw = m
                .get_capture(source_idx)
                .map(|n| node_text(&n, source).to_string())
                .unwrap_or_default();
            // Strip quotes and angle brackets: "foo.h" -> foo.h, <stdio.h> -> stdio.h
            let source_text = raw
                .trim_start_matches('"')
                .trim_end_matches('"')
                .trim_start_matches('<')
                .trim_end_matches('>')
                .to_string();
            if !source_text.is_empty() {
                let key = (line, source_text.clone());
                if !seen.contains(&key) {
                    seen.push(key);
                    // Extract the header name without path for the imported name
                    let header_name = source_text
                        .rsplit('/')
                        .next()
                        .unwrap_or(&source_text)
                        .trim_end_matches(".h")
                        .trim_end_matches(".hpp")
                        .trim_end_matches(".hxx")
                        .to_string();
                    imports.push(ImportInfo {
                        source: source_text,
                        names: vec![ImportedName {
                            name: header_name,
                            alias: None,
                        }],
                        is_default: false,
                        is_namespace: true, // #include brings everything into scope
                        line,
                    });
                }
            }
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
    if extract_definitions_standard(
        matches,
        source,
        qwc,
        definitions,
        seen_nodes,
    ) {
        // Standard path took over.
    } else {
    let func_name_idx = qwc.capture_index("func_name");
    let func_node_idx = qwc.capture_index("func_node");
    let struct_name_idx = qwc.capture_index("struct_name");
    let struct_node_idx = qwc.capture_index("struct_node");
    let enum_name_idx = qwc.capture_index("enum_name");
    let enum_node_idx = qwc.capture_index("enum_node");
    let union_name_idx = qwc.capture_index("union_name");
    let union_node_idx = qwc.capture_index("union_node");
    let typedef_name_idx = qwc.capture_index("typedef_name");
    let typedef_node_idx = qwc.capture_index("typedef_node");
    let global_name_idx = qwc.capture_index("global_name");
    let global_node_idx = qwc.capture_index("global_node");

    let c_def_captures: &[(Option<u32>, Option<u32>, SymbolKind)] = &[
        (func_name_idx, func_node_idx, SymbolKind::Function),
        (struct_name_idx, struct_node_idx, SymbolKind::Class),
        (enum_name_idx, enum_node_idx, SymbolKind::Class),
        (union_name_idx, union_node_idx, SymbolKind::Class),
        (typedef_name_idx, typedef_node_idx, SymbolKind::TypeAlias),
        (global_name_idx, global_node_idx, SymbolKind::Constant),
    ];

    for m in matches {
        for &(name_cap, node_cap, kind) in c_def_captures {
            if m.has_capture(name_cap) {
                let name_node = m.get_capture(name_cap);
                let name_text = name_node
                    .map(|n| node_text(&n, source).to_string())
                    .unwrap_or_default();
                let (start_line, end_line, _node_start) = node_span(m, node_cap);
                if !name_text.is_empty() {
                    let name_start = name_node.map(|n| n.start_byte()).unwrap_or(0);
                    let key = (name_start, hash_str(&name_text));
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
    fn test_c_system_include() {
        let e = engine();
        let result = e
            .parse_file("main.c", "#include <stdio.h>\n#include <stdlib.h>\n")
            .unwrap();
        assert_eq!(result.imports.len(), 2);
        assert_eq!(result.imports[0].source, "stdio.h");
        assert_eq!(result.imports[0].names[0].name, "stdio");
        assert_eq!(result.imports[1].source, "stdlib.h");
        assert_eq!(result.imports[1].names[0].name, "stdlib");
    }

    #[test]
    fn test_c_local_include() {
        let e = engine();
        let result = e.parse_file("main.c", "#include \"myheader.h\"\n").unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].source, "myheader.h");
        assert_eq!(result.imports[0].names[0].name, "myheader");
    }

    #[test]
    fn test_c_include_with_path() {
        let e = engine();
        let result = e.parse_file("main.c", "#include <curl/curl.h>\n").unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].source, "curl/curl.h");
        assert_eq!(result.imports[0].names[0].name, "curl");
    }

    #[test]
    fn test_c_function_definition() {
        let e = engine();
        let source = r#"
int add(int a, int b) {
    return a + b;
}

void print_hello() {
    printf("Hello\n");
}
"#;
        let result = e.parse_file("math.c", source).unwrap();
        let def_names: Vec<&str> = result.definitions.iter().map(|d| d.name.as_str()).collect();
        assert!(
            def_names.contains(&"add"),
            "should detect add function; got: {:?}",
            def_names
        );
        assert!(
            def_names.contains(&"print_hello"),
            "should detect print_hello function; got: {:?}",
            def_names
        );
    }

    #[test]
    fn test_c_struct_definition() {
        let e = engine();
        let source = r#"
struct Point {
    int x;
    int y;
};
"#;
        let result = e.parse_file("types.c", source).unwrap();
        let def_names: Vec<&str> = result.definitions.iter().map(|d| d.name.as_str()).collect();
        assert!(
            def_names.contains(&"Point"),
            "should detect struct Point; got: {:?}",
            def_names
        );
    }

    #[test]
    fn test_c_enum_definition() {
        let e = engine();
        let source = r#"
enum Color {
    RED,
    GREEN,
    BLUE
};
"#;
        let result = e.parse_file("color.c", source).unwrap();
        let def_names: Vec<&str> = result.definitions.iter().map(|d| d.name.as_str()).collect();
        assert!(
            def_names.contains(&"Color"),
            "should detect enum Color; got: {:?}",
            def_names
        );
    }

    #[test]
    fn test_c_typedef() {
        let e = engine();
        // typedef struct X NewName; — struct typedef has type_identifier as declarator
        let source = "typedef struct Config AppConfig;\n";
        let result = e.parse_file("types.c", source).unwrap();
        let def_names: Vec<&str> = result.definitions.iter().map(|d| d.name.as_str()).collect();
        assert!(
            def_names.contains(&"AppConfig"),
            "should detect typedef; got: {:?}",
            def_names
        );
    }

    #[test]
    fn test_c_simple_call() {
        let e = engine();
        let source = r#"
void foo() {
    int x = bar(42);
    printf("result: %d\n", x);
}
"#;
        let result = e.parse_file("main.c", source).unwrap();
        let callees: Vec<&str> = result
            .call_sites
            .iter()
            .map(|c| c.callee.as_str())
            .collect();
        assert!(
            callees.contains(&"bar"),
            "should detect bar call; got: {:?}",
            callees
        );
        assert!(
            callees.contains(&"printf"),
            "should detect printf call; got: {:?}",
            callees
        );
    }

    #[test]
    fn test_c_member_call() {
        let e = engine();
        let source = r#"
void process(struct Obj *obj) {
    obj->init();
}
"#;
        let result = e.parse_file("main.c", source).unwrap();
        let callees: Vec<&str> = result
            .call_sites
            .iter()
            .map(|c| c.callee.as_str())
            .collect();
        assert!(
            callees.contains(&"init"),
            "should detect member call via ->; got: {:?}",
            callees
        );
    }

    #[test]
    fn test_c_assignment_from_call() {
        let e = engine();
        let source = r#"
void foo() {
    int result = process_data();
}
"#;
        let df = e.extract_data_flow("main.c", source).unwrap();
        assert!(
            !df.assignments.is_empty(),
            "should detect assignment from call"
        );
        assert_eq!(df.assignments[0].variable, "result");
        assert_eq!(df.assignments[0].callee, "process_data");
    }

    #[test]
    fn test_c_language_detection() {
        assert_eq!(Language::from_path("main.c"), Language::C);
        assert_eq!(Language::from_path("header.h"), Language::C);
    }

    #[test]
    fn test_c_full_file() {
        let e = engine();
        let source = r#"
#include <stdio.h>
#include "service.h"

struct Config {
    int port;
    char* host;
};

typedef struct Config AppConfig;

int process_request(int fd) {
    char* data = read_data(fd);
    int result = handle(data);
    printf("Done: %d\n", result);
    return result;
}

int main() {
    int server = init_server();
    process_request(server);
    return 0;
}
"#;
        let result = e.parse_file("server.c", source).unwrap();

        // Imports
        assert_eq!(result.imports.len(), 2);

        // Definitions
        let def_names: Vec<&str> = result.definitions.iter().map(|d| d.name.as_str()).collect();
        assert!(
            def_names.contains(&"Config"),
            "struct Config; got: {:?}",
            def_names
        );
        assert!(
            def_names.contains(&"AppConfig"),
            "typedef AppConfig; got: {:?}",
            def_names
        );
        assert!(
            def_names.contains(&"process_request"),
            "process_request fn; got: {:?}",
            def_names
        );
        assert!(def_names.contains(&"main"), "main fn; got: {:?}", def_names);

        // Call sites
        let callees: Vec<&str> = result
            .call_sites
            .iter()
            .map(|c| c.callee.as_str())
            .collect();
        assert!(
            callees.contains(&"read_data"),
            "read_data call; got: {:?}",
            callees
        );
        assert!(
            callees.contains(&"handle"),
            "handle call; got: {:?}",
            callees
        );
        assert!(
            callees.contains(&"printf"),
            "printf call; got: {:?}",
            callees
        );
        assert!(
            callees.contains(&"init_server"),
            "init_server call; got: {:?}",
            callees
        );
        assert!(
            callees.contains(&"process_request"),
            "process_request call; got: {:?}",
            callees
        );
    }

}
