//! Python extraction code.
//!
//! Moved from `query_engine.rs` during the per-language module split.
//! Pure mechanical move — no logic changes.

use crate::ast::{Definition, ImportedName, ImportInfo};
use crate::languages::common::{
    collect_matches, get_or_insert_import, hash_str, node_span, node_text,
    extract_definitions_standard, CollectedMatch, ImportBuilder,
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
    let module_name_idx = qwc.capture_index("module_name");
    let alias_idx = qwc.capture_index("alias");
    let source_cap_idx = qwc.capture_index("source");
    let imported_name_idx = qwc.capture_index("imported_name");
    let aliased_imported_name_idx = qwc.capture_index("aliased_imported_name");
    let imported_alias_idx = qwc.capture_index("imported_alias");
    let wildcard_idx = qwc.capture_index("wildcard");
    let relative_source_idx = qwc.capture_index("relative_source");
    let relative_imported_name_idx = qwc.capture_index("relative_imported_name");
    let relative_aliased_name_idx = qwc.capture_index("relative_aliased_name");
    let relative_alias_idx = qwc.capture_index("relative_alias");

    let mut import_map: Vec<(usize, ImportBuilder)> = Vec::new();

    for m in &matches {
        let mut stmt_start = 0usize;
        let mut line = 0usize;

        for &(idx, node) in &m.captures {
            if Some(idx) == stmt_idx {
                stmt_start = node.start_byte();
                line = node.start_position().row + 1;
            }
        }

        if m.has_capture(relative_aliased_name_idx) {
            // from .models import User as U (relative import with alias)
            let mut src = String::new();
            let mut imported = String::new();
            let mut alias = String::new();
            for &(idx, node) in &m.captures {
                if Some(idx) == relative_source_idx {
                    src = node_text(&node, source).to_string();
                }
                if Some(idx) == relative_aliased_name_idx {
                    imported = node_text(&node, source).to_string();
                }
                if Some(idx) == relative_alias_idx {
                    alias = node_text(&node, source).to_string();
                }
            }
            if !src.is_empty() && !imported.is_empty() {
                let entry = get_or_insert_import(&mut import_map, stmt_start, &src, line);
                entry.names.retain(|n| n.name != imported);
                entry.names.push(ImportedName {
                    name: imported,
                    alias: if alias.is_empty() { None } else { Some(alias) },
                });
            }
        } else if m.has_capture(relative_source_idx) {
            // from .bar import baz (relative import, non-aliased)
            let mut src = String::new();
            let mut imported = String::new();
            for &(idx, node) in &m.captures {
                if Some(idx) == relative_source_idx {
                    src = node_text(&node, source).to_string();
                }
                if Some(idx) == relative_imported_name_idx {
                    imported = node_text(&node, source).to_string();
                }
            }
            if !src.is_empty() {
                let entry = get_or_insert_import(&mut import_map, stmt_start, &src, line);
                if !imported.is_empty() {
                    entry.names.push(ImportedName {
                        name: imported,
                        alias: None,
                    });
                }
            }
        } else if m.has_capture(aliased_imported_name_idx) {
            // from foo import bar as baz
            let mut src = String::new();
            let mut imported = String::new();
            let mut alias = String::new();
            for &(idx, node) in &m.captures {
                if Some(idx) == source_cap_idx {
                    src = node_text(&node, source).to_string();
                }
                if Some(idx) == aliased_imported_name_idx {
                    imported = node_text(&node, source).to_string();
                }
                if Some(idx) == imported_alias_idx {
                    alias = node_text(&node, source).to_string();
                }
            }
            if !src.is_empty() && !imported.is_empty() {
                let entry = get_or_insert_import(&mut import_map, stmt_start, &src, line);
                entry.names.retain(|n| n.name != imported);
                entry.names.push(ImportedName {
                    name: imported,
                    alias: if alias.is_empty() { None } else { Some(alias) },
                });
            }
        } else if m.has_capture(wildcard_idx) {
            // from foo import *
            let mut src = String::new();
            for &(idx, node) in &m.captures {
                if Some(idx) == source_cap_idx {
                    src = node_text(&node, source).to_string();
                }
            }
            if !src.is_empty() {
                let entry = get_or_insert_import(&mut import_map, stmt_start, &src, line);
                entry.names.push(ImportedName {
                    name: "*".to_string(),
                    alias: None,
                });
            }
        } else if m.has_capture(imported_name_idx) {
            // from foo import bar
            let mut src = String::new();
            let mut imported = String::new();
            for &(idx, node) in &m.captures {
                if Some(idx) == source_cap_idx {
                    src = node_text(&node, source).to_string();
                }
                if Some(idx) == imported_name_idx {
                    imported = node_text(&node, source).to_string();
                }
            }
            if !src.is_empty() {
                let entry = get_or_insert_import(&mut import_map, stmt_start, &src, line);
                if !imported.is_empty() && !entry.names.iter().any(|n| n.name == imported) {
                    entry.names.push(ImportedName {
                        name: imported,
                        alias: None,
                    });
                }
            }
        } else if m.has_capture(alias_idx) {
            // import foo as bar
            let mut name = String::new();
            let mut alias = String::new();
            for &(idx, node) in &m.captures {
                if Some(idx) == module_name_idx {
                    name = node_text(&node, source).to_string();
                }
                if Some(idx) == alias_idx {
                    alias = node_text(&node, source).to_string();
                }
            }
            if !name.is_empty() {
                let entry = get_or_insert_import(&mut import_map, stmt_start, &name, line);
                entry.is_namespace = true;
                entry.names.push(ImportedName {
                    name,
                    alias: if alias.is_empty() { None } else { Some(alias) },
                });
            }
        } else if m.has_capture(module_name_idx) {
            // import foo
            for &(idx, node) in &m.captures {
                if Some(idx) == module_name_idx {
                    let name = node_text(&node, source).to_string();
                    let entry = get_or_insert_import(&mut import_map, stmt_start, &name, line);
                    entry.is_namespace = true;
                    entry.names.push(ImportedName {
                        name: name.clone(),
                        alias: None,
                    });
                }
            }
        }
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
    if extract_definitions_standard(
        matches,
        source,
        qwc,
        definitions,
        seen_nodes,
    ) {
        // Standard path took over: bespoke fallback skipped.
    } else {
    // Python: each definition kind has a distinct capture name pair
    let fn_name_idx = qwc.capture_index("fn_name");
    let fn_node_idx = qwc.capture_index("fn_node");
    let class_name_idx = qwc.capture_index("class_name");
    let class_node_idx = qwc.capture_index("class_node");
    let decorated_fn_name_idx = qwc.capture_index("decorated_fn_name");
    let decorated_fn_node_idx = qwc.capture_index("decorated_fn_node");
    let decorated_class_name_idx = qwc.capture_index("decorated_class_name");
    let decorated_class_node_idx = qwc.capture_index("decorated_class_node");
    let method_name_idx = qwc.capture_index("method_name");
    let method_node_idx = qwc.capture_index("method_node");
    let decorated_method_name_idx = qwc.capture_index("decorated_method_name");
    let decorated_method_node_idx = qwc.capture_index("decorated_method_node");

    let py_def_captures: &[(Option<u32>, Option<u32>, SymbolKind)] = &[
        (fn_name_idx, fn_node_idx, SymbolKind::Function),
        (class_name_idx, class_node_idx, SymbolKind::Class),
        (
            decorated_fn_name_idx,
            decorated_fn_node_idx,
            SymbolKind::Function,
        ),
        (
            decorated_class_name_idx,
            decorated_class_node_idx,
            SymbolKind::Class,
        ),
        (method_name_idx, method_node_idx, SymbolKind::Function),
        (
            decorated_method_name_idx,
            decorated_method_node_idx,
            SymbolKind::Function,
        ),
    ];

    for m in matches {
        for &(name_cap, node_cap, kind) in py_def_captures {
            if m.has_capture(name_cap) {
                let name_node = m.get_capture(name_cap);
                let name_text = name_node
                    .map(|n| node_text(&n, source).to_string())
                    .unwrap_or_default();
                let (start_line, end_line, _node_start) = node_span(m, node_cap);
                if !name_text.is_empty() {
                    // Dedup by name node start byte (not outer node) to
                    // prevent decorated functions/classes from being counted
                    // twice — the bare pattern and decorated pattern share
                    // the same inner name identifier node.
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
    fn test_python_simple_import() {
        let e = engine();
        let result = e.parse_file("app.py", "import os").unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].source, "os");
        assert!(result.imports[0].is_namespace);
    }

    #[test]
    fn test_python_aliased_import() {
        let e = engine();
        let result = e.parse_file("app.py", "import numpy as np").unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].source, "numpy");
        assert_eq!(result.imports[0].names[0].alias, Some("np".to_string()));
    }

    #[test]
    fn test_python_from_import() {
        let e = engine();
        let result = e
            .parse_file("app.py", "from os.path import join, exists")
            .unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].source, "os.path");
        let names: Vec<&str> = result.imports[0]
            .names
            .iter()
            .map(|n| n.name.as_str())
            .collect();
        assert!(names.contains(&"join"));
        assert!(names.contains(&"exists"));
    }

    #[test]
    fn test_python_wildcard_import() {
        let e = engine();
        let result = e.parse_file("app.py", "from os.path import *").unwrap();
        assert_eq!(result.imports.len(), 1);
        assert!(result.imports[0].names.iter().any(|n| n.name == "*"));
    }

    #[test]
    fn test_python_function_def() {
        let e = engine();
        let result = e
            .parse_file("app.py", "def greet(name):\n    pass")
            .unwrap();
        assert!(result
            .definitions
            .iter()
            .any(|d| d.name == "greet" && d.kind == SymbolKind::Function));
    }

    #[test]
    fn test_python_class_def() {
        let e = engine();
        let source = "class User:\n    def get_name(self):\n        pass";
        let result = e.parse_file("app.py", source).unwrap();
        assert!(result
            .definitions
            .iter()
            .any(|d| d.name == "User" && d.kind == SymbolKind::Class));
        assert!(result
            .definitions
            .iter()
            .any(|d| d.name == "get_name" && d.kind == SymbolKind::Function));
    }

    #[test]
    fn test_python_decorated_function() {
        let e = engine();
        let source = "@app.route('/hello')\ndef hello():\n    pass";
        let result = e.parse_file("app.py", source).unwrap();
        assert!(result
            .definitions
            .iter()
            .any(|d| d.name == "hello" && d.kind == SymbolKind::Function));
    }

    #[test]
    fn test_python_call_site() {
        let e = engine();
        let result = e
            .parse_file("app.py", "def main():\n    greet('world')")
            .unwrap();
        assert!(result.call_sites.iter().any(|c| c.callee == "greet"));
    }

    #[test]
    fn test_python_assignment_from_call() {
        let e = engine();
        let df = e
            .extract_data_flow("app.py", "result = fetch_data()")
            .unwrap();
        assert_eq!(df.assignments.len(), 1);
        assert_eq!(df.assignments[0].variable, "result");
        assert_eq!(df.assignments[0].callee, "fetch_data");
    }

    #[test]
    fn test_python_call_with_args() {
        let e = engine();
        let df = e
            .extract_data_flow("app.py", "process(data, config)")
            .unwrap();
        assert_eq!(df.calls_with_args.len(), 1);
        assert_eq!(df.calls_with_args[0].callee, "process");
        assert_eq!(df.calls_with_args[0].arguments, vec!["data", "config"]);
    }

    #[test]
    fn test_python_keyword_args() {
        let e = engine();
        let df = e
            .extract_data_flow("app.py", "connect(host='localhost', port=5432)")
            .unwrap();
        assert_eq!(df.calls_with_args.len(), 1);
        assert_eq!(df.calls_with_args[0].arguments, vec!["'localhost'", "5432"]);
    }

    #[test]
    fn test_parity_python_full_file() {
        let source = r#"
import os
import numpy as np
from os.path import join, exists
from typing import List

def greet(name):
    print(name)

class UserService:
    def get_user(self, user_id):
        return self.db.find(user_id)

@app.route('/hello')
def hello():
    data = fetch_data()
    return data
"#;
        let e = engine();
        let qe_result = e.parse_file("app.py", source).unwrap();
        let ast_result = crate::ast::parse_file("app.py", source).unwrap();

        // Import count should match
        assert_eq!(
            qe_result.imports.len(),
            ast_result.imports.len(),
            "import count mismatch: qe={}, ast={}",
            qe_result.imports.len(),
            ast_result.imports.len()
        );

        // All definition names from ast should be present
        for ast_def in &ast_result.definitions {
            assert!(
                qe_result.definitions.iter().any(|d| d.name == ast_def.name),
                "missing definition from query engine: {}",
                ast_def.name,
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
    fn test_python_from_relative_import() {
        let e = engine();
        let result = e.parse_file("pkg/sub.py", "from . import utils").unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].source, ".");
        assert!(result.imports[0].names.iter().any(|n| n.name == "utils"));
    }

    #[test]
    fn test_python_from_import_aliased() {
        let e = engine();
        let result = e
            .parse_file("app.py", "from models import User as U")
            .unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].source, "models");
        assert_eq!(result.imports[0].names[0].name, "User");
        assert_eq!(result.imports[0].names[0].alias, Some("U".to_string()));
    }

    #[test]
    fn test_python_decorated_class() {
        let e = engine();
        let source = "@dataclass\nclass User:\n    name: str\n    def greet(self):\n        pass";
        let result = e.parse_file("models.py", source).unwrap();
        assert!(result
            .definitions
            .iter()
            .any(|d| d.name == "User" && d.kind == SymbolKind::Class));
        assert!(result
            .definitions
            .iter()
            .any(|d| d.name == "greet" && d.kind == SymbolKind::Function));
    }

    #[test]
    fn test_python_method_call_with_args() {
        let e = engine();
        let df = e
            .extract_data_flow("app.py", "db.save(user, commit=True)")
            .unwrap();
        assert_eq!(df.calls_with_args.len(), 1);
        assert_eq!(df.calls_with_args[0].callee, "db.save");
        // keyword arg value should be captured
        assert!(df.calls_with_args[0]
            .arguments
            .contains(&"user".to_string()));
        assert!(df.calls_with_args[0]
            .arguments
            .contains(&"True".to_string()));
    }

    #[test]
    fn test_python_walrus_operator_not_in_assignments() {
        // := (walrus operator / named expression) is not an assignment statement,
        // it's a named_expression. Our assignments.scm only captures assignment
        // statements. This is acceptable because walrus operators are typically
        // used inline (if/while conditions) and rarely represent data flow.
        let e = engine();
        let df = e
            .extract_data_flow("app.py", "if (x := compute()):\n    pass")
            .unwrap();
        assert!(
            df.assignments.is_empty(),
            "walrus operator not captured (acceptable gap)"
        );
    }

    #[test]
    fn test_python_tuple_unpacking_not_in_assignments() {
        // `a, b = foo()` has LHS as `pattern_list`, not `identifier`
        let e = engine();
        let df = e.extract_data_flow("app.py", "a, b = compute()").unwrap();
        assert!(
            df.assignments.is_empty(),
            "tuple unpacking not in .scm query (handled by IR layer)"
        );
    }

    #[test]
    fn test_python_relative_import_with_alias() {
        // `from .models import User as U` — Pattern 6 in python/imports.scm
        let e = engine();
        let result = e
            .parse_file("pkg/app.py", "from .models import User as U")
            .unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].source, ".models");
        assert_eq!(result.imports[0].names.len(), 1);
        assert_eq!(result.imports[0].names[0].name, "User");
        assert_eq!(result.imports[0].names[0].alias, Some("U".to_string()));
    }

    #[test]
    fn test_python_async_def_captured() {
        // async def should be captured — tree-sitter-python uses `function_definition`
        // for both sync and async functions
        let e = engine();
        let source = "async def fetch_data():\n    pass";
        let result = e.parse_file("api.py", source).unwrap();
        assert!(
            result
                .definitions
                .iter()
                .any(|d| d.name == "fetch_data" && d.kind == SymbolKind::Function),
            "async functions should be captured"
        );
    }

    #[test]
    fn test_python_dunder_all_not_captured() {
        // Python uses __all__ for explicit exports, but it's just an assignment
        // statement (not a function/class definition), so it's not captured as
        // a definition. This is correct — Python assignments are not definitions.
        let e = engine();
        let source = "__all__ = ['foo', 'bar']";
        let result = e.parse_file("mod.py", source).unwrap();
        // Python has no const/let/var, so assignments are not captured as definitions
        assert!(
            result.definitions.is_empty(),
            "__all__ is an assignment, not a definition"
        );
    }

    #[test]
    fn test_python_decorated_method_in_decorated_class() {
        let e = engine();
        let source = "@dataclass\nclass Service:\n    @staticmethod\n    def create():\n        pass\n    @classmethod\n    def from_config(cls):\n        pass";
        let result = e.parse_file("svc.py", source).unwrap();
        assert!(result
            .definitions
            .iter()
            .any(|d| d.name == "Service" && d.kind == SymbolKind::Class));
        assert!(result.definitions.iter().any(|d| d.name == "create"));
        assert!(result.definitions.iter().any(|d| d.name == "from_config"));
    }

    #[test]
    fn test_python_multiline_import() {
        let e = engine();
        let source = "from models import (\n    User,\n    Post,\n    Comment\n)";
        let result = e.parse_file("app.py", source).unwrap();
        assert_eq!(result.imports.len(), 1);
        let names: Vec<&str> = result.imports[0]
            .names
            .iter()
            .map(|n| n.name.as_str())
            .collect();
        assert!(names.contains(&"User"));
        assert!(names.contains(&"Post"));
        assert!(names.contains(&"Comment"));
    }

    #[test]
    fn test_python_multiple_import_same_statement() {
        let e = engine();
        let source = "import os, sys, json";
        let result = e.parse_file("app.py", source).unwrap();
        // tree-sitter-python may parse this as separate import nodes
        // or as one import_statement with multiple dotted_names
        assert!(!result.imports.is_empty());
    }

    #[test]
    fn test_python_nested_class() {
        let e = engine();
        let source = "class Outer:\n    class Inner:\n        def method(self):\n            pass";
        let result = e.parse_file("nested.py", source).unwrap();
        assert!(result.definitions.iter().any(|d| d.name == "Outer"));
        // Inner class may or may not be captured depending on query depth
    }

    #[test]
    fn test_all_definition_kinds_python() {
        let e = engine();
        let source = "def fn():\n    pass\n\nclass Cls:\n    def method(self):\n        pass\n\n@deco\ndef decorated():\n    pass\n\n@deco\nclass DCls:\n    @deco\n    def dmethod(self):\n        pass";
        let result = e.parse_file("all.py", source).unwrap();
        assert!(result
            .definitions
            .iter()
            .any(|d| d.name == "fn" && d.kind == SymbolKind::Function));
        assert!(result
            .definitions
            .iter()
            .any(|d| d.name == "Cls" && d.kind == SymbolKind::Class));
        assert!(result
            .definitions
            .iter()
            .any(|d| d.name == "method" && d.kind == SymbolKind::Function));
        assert!(result
            .definitions
            .iter()
            .any(|d| d.name == "decorated" && d.kind == SymbolKind::Function));
        assert!(result
            .definitions
            .iter()
            .any(|d| d.name == "DCls" && d.kind == SymbolKind::Class));
        assert!(result
            .definitions
            .iter()
            .any(|d| d.name == "dmethod" && d.kind == SymbolKind::Function));
    }

    #[test]
    fn test_all_python_import_variants() {
        let e = engine();
        let source = r#"import os
import numpy as np
from os.path import join
from typing import List
from models import User as U
from os.path import *
from . import utils
"#;
        let result = e.parse_file("all.py", source).unwrap();
        // Should capture all 7 imports (though some may be combined)
        assert!(
            result.imports.len() >= 6,
            "expected at least 6 imports, got {}",
            result.imports.len()
        );
    }

    #[test]
    fn test_python_decorated_function_not_double_counted() {
        // A decorated function fires both the bare `function_definition` pattern
        // and the `decorated_definition > function_definition` pattern.
        // Verify they are deduplicated (same name should appear only once).
        let e = engine();
        let source =
            "@app.route('/hello')\ndef hello():\n    pass\n\n@cache\ndef cached_func():\n    pass";
        let result = e.parse_file("app.py", source).unwrap();
        let hello_count = result
            .definitions
            .iter()
            .filter(|d| d.name == "hello")
            .count();
        let cached_count = result
            .definitions
            .iter()
            .filter(|d| d.name == "cached_func")
            .count();
        assert_eq!(
            hello_count, 1,
            "decorated function 'hello' should appear exactly once, got {}",
            hello_count
        );
        assert_eq!(
            cached_count, 1,
            "decorated function 'cached_func' should appear exactly once, got {}",
            cached_count
        );
    }

    #[test]
    fn test_python_decorated_class_not_double_counted() {
        let e = engine();
        let source = "@dataclass\nclass User:\n    name: str";
        let result = e.parse_file("models.py", source).unwrap();
        let user_count = result
            .definitions
            .iter()
            .filter(|d| d.name == "User")
            .count();
        assert_eq!(
            user_count, 1,
            "decorated class 'User' should appear exactly once, got {}",
            user_count
        );
    }

}
