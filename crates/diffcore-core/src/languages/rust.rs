//! Rust extraction code.
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
    let source_idx = qwc.capture_index("source");
    let alias_name_idx = qwc.capture_index("alias_name");
    let named_name_idx = qwc.capture_index("named_name");
    let aliased_name_idx = qwc.capture_index("aliased_name");
    let alias_idx = qwc.capture_index("alias");

    // Use the same ordered-map/builder pattern as TS imports to aggregate
    // multiple matches for the same use statement (e.g. use list items).
    let mut import_map: Vec<(usize, ImportBuilder)> = Vec::new();

    for m in &matches {
        let mut line = 0usize;
        let mut source_text = String::new();
        let mut stmt_start = 0usize;

        for &(idx, node) in &m.captures {
            if Some(idx) == stmt_idx {
                stmt_start = node.start_byte();
                line = node.start_position().row + 1;
            }
            if Some(idx) == source_idx {
                source_text = node_text(&node, source).to_string();
                if line == 0 {
                    line = node.start_position().row + 1;
                }
            }
        }

        if source_text.is_empty() {
            continue;
        }

        let module_path = source_text;

        if m.has_capture(aliased_name_idx) {
            // use std::io::{Read as R};
            let mut imported = String::new();
            let mut alias_val = String::new();
            for &(idx, node) in &m.captures {
                if Some(idx) == aliased_name_idx {
                    imported = node_text(&node, source).to_string();
                }
                if Some(idx) == alias_idx {
                    alias_val = node_text(&node, source).to_string();
                }
            }
            let entry = get_or_insert_import(&mut import_map, stmt_start, &module_path, line);
            entry.names.retain(|n| n.name != imported);
            entry.names.push(ImportedName {
                name: imported,
                alias: if alias_val.is_empty() {
                    None
                } else {
                    Some(alias_val)
                },
            });
        } else if m.has_capture(alias_name_idx) {
            // use std::io as stdio;
            let mut alias_text = String::new();
            for &(idx, node) in &m.captures {
                if Some(idx) == alias_name_idx {
                    alias_text = node_text(&node, source).to_string();
                }
            }
            let name = module_path
                .rsplit("::")
                .next()
                .unwrap_or(&module_path)
                .to_string();
            let entry = get_or_insert_import(&mut import_map, stmt_start, &module_path, line);
            entry.is_namespace = true;
            entry.names.push(ImportedName {
                name,
                alias: Some(alias_text),
            });
        } else if m.has_capture(named_name_idx) {
            // use std::io::{Read, Write}; — may be called multiple times per use stmt
            let entry = get_or_insert_import(&mut import_map, stmt_start, &module_path, line);
            for &(idx, node) in &m.captures {
                if Some(idx) == named_name_idx {
                    let name = node_text(&node, source).to_string();
                    if !entry.names.iter().any(|n| n.name == name) {
                        entry.names.push(ImportedName { name, alias: None });
                    }
                }
            }
        } else {
            // Simple use: use std::io; or use serde;
            // Also handles glob: use std::io::*; (source is "std::io")
            let name = module_path
                .rsplit("::")
                .next()
                .unwrap_or(&module_path)
                .to_string();
            let entry = get_or_insert_import(&mut import_map, stmt_start, &module_path, line);
            entry.is_namespace = true;
            if !entry.names.iter().any(|n| n.name == name) {
                entry.names.push(ImportedName { name, alias: None });
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
        // Standard path took over.
    } else {
    let fn_name_idx = qwc.capture_index("fn_name");
    let fn_node_idx = qwc.capture_index("fn_node");
    let struct_name_idx = qwc.capture_index("struct_name");
    let struct_node_idx = qwc.capture_index("struct_node");
    let enum_name_idx = qwc.capture_index("enum_name");
    let enum_node_idx = qwc.capture_index("enum_node");
    let trait_name_idx = qwc.capture_index("trait_name");
    let trait_node_idx = qwc.capture_index("trait_node");
    let type_name_idx = qwc.capture_index("type_name");
    let type_node_idx = qwc.capture_index("type_node");
    let const_name_idx = qwc.capture_index("const_name");
    let const_node_idx = qwc.capture_index("const_node");
    let static_name_idx = qwc.capture_index("static_name");
    let static_node_idx = qwc.capture_index("static_node");
    let method_name_idx = qwc.capture_index("method_name");
    let method_node_idx = qwc.capture_index("method_node");
    let macro_name_idx = qwc.capture_index("macro_name");
    let macro_node_idx = qwc.capture_index("macro_node");

    let rust_def_captures: &[(Option<u32>, Option<u32>, SymbolKind)] = &[
        (fn_name_idx, fn_node_idx, SymbolKind::Function),
        (method_name_idx, method_node_idx, SymbolKind::Function),
        (struct_name_idx, struct_node_idx, SymbolKind::Class),
        (enum_name_idx, enum_node_idx, SymbolKind::Class),
        (trait_name_idx, trait_node_idx, SymbolKind::Interface),
        (type_name_idx, type_node_idx, SymbolKind::TypeAlias),
        (const_name_idx, const_node_idx, SymbolKind::Constant),
        (static_name_idx, static_node_idx, SymbolKind::Constant),
        (macro_name_idx, macro_node_idx, SymbolKind::Function),
    ];

    for m in matches {
        for &(name_cap, node_cap, kind) in rust_def_captures {
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
    fn test_rust_language_detection() {
        assert_eq!(Language::from_path("main.rs"), Language::Rust);
        assert_eq!(Language::from_path("src/lib.rs"), Language::Rust);
        assert_eq!(Language::from_path("src/handlers/auth.rs"), Language::Rust);
    }

    #[test]
    fn test_rust_simple_use() {
        let e = engine();
        let result = e.parse_file("main.rs", "use std::io;").unwrap();
        assert_eq!(result.language, Language::Rust);
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].source, "std::io");
    }

    #[test]
    fn test_rust_use_list() {
        let e = engine();
        let source = "use std::collections::{HashMap, BTreeMap};";
        let result = e.parse_file("main.rs", source).unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].source, "std::collections");
        let names: Vec<&str> = result.imports[0]
            .names
            .iter()
            .map(|n| n.name.as_str())
            .collect();
        assert!(names.contains(&"HashMap"));
        assert!(names.contains(&"BTreeMap"));
    }

    #[test]
    fn test_rust_use_alias() {
        let e = engine();
        let source = "use std::io as stdio;";
        let result = e.parse_file("main.rs", source).unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].source, "std::io");
        assert_eq!(result.imports[0].names[0].alias.as_deref(), Some("stdio"));
    }

    #[test]
    fn test_rust_use_crate() {
        let e = engine();
        let source = "use crate::handlers::auth;";
        let result = e.parse_file("main.rs", source).unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].source, "crate::handlers::auth");
    }

    #[test]
    fn test_rust_use_self() {
        let e = engine();
        let source = "use self::models::User;";
        let result = e.parse_file("main.rs", source).unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].source, "self::models::User");
    }

    #[test]
    fn test_rust_use_glob() {
        let e = engine();
        let source = "use std::io::*;";
        let result = e.parse_file("main.rs", source).unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].source, "std::io");
    }

    #[test]
    fn test_rust_multiple_uses() {
        let e = engine();
        let source = r#"
use std::io;
use serde::{Serialize, Deserialize};
use tokio;
"#;
        let result = e.parse_file("main.rs", source).unwrap();
        assert!(result.imports.len() >= 3);
    }

    #[test]
    fn test_rust_function_definitions() {
        let e = engine();
        let source = r#"
fn hello() {
    println!("hello");
}

pub fn greet(name: &str) -> String {
    format!("Hello, {}", name)
}

async fn fetch_data() -> Result<(), Error> {
    Ok(())
}
"#;
        let result = e.parse_file("main.rs", source).unwrap();
        let fn_names: Vec<&str> = result
            .definitions
            .iter()
            .filter(|d| d.kind == SymbolKind::Function)
            .map(|d| d.name.as_str())
            .collect();
        assert!(fn_names.contains(&"hello"));
        assert!(fn_names.contains(&"greet"));
        assert!(fn_names.contains(&"fetch_data"));
    }

    #[test]
    fn test_rust_struct_definitions() {
        let e = engine();
        let source = r#"
pub struct User {
    pub name: String,
    pub email: String,
}

struct Config {
    port: u16,
}
"#;
        let result = e.parse_file("main.rs", source).unwrap();
        let struct_names: Vec<&str> = result
            .definitions
            .iter()
            .filter(|d| d.kind == SymbolKind::Class)
            .map(|d| d.name.as_str())
            .collect();
        assert!(struct_names.contains(&"User"));
        assert!(struct_names.contains(&"Config"));
    }

    #[test]
    fn test_rust_enum_definitions() {
        let e = engine();
        let source = r#"
pub enum Status {
    Active,
    Inactive,
    Suspended,
}
"#;
        let result = e.parse_file("main.rs", source).unwrap();
        let enum_names: Vec<&str> = result
            .definitions
            .iter()
            .filter(|d| d.kind == SymbolKind::Class)
            .map(|d| d.name.as_str())
            .collect();
        assert!(enum_names.contains(&"Status"));
    }

    #[test]
    fn test_rust_trait_definitions() {
        let e = engine();
        let source = r#"
pub trait Repository {
    fn find_by_id(&self, id: u64) -> Option<User>;
    fn save(&self, user: &User) -> Result<(), Error>;
}
"#;
        let result = e.parse_file("main.rs", source).unwrap();
        let trait_names: Vec<&str> = result
            .definitions
            .iter()
            .filter(|d| d.kind == SymbolKind::Interface)
            .map(|d| d.name.as_str())
            .collect();
        assert!(trait_names.contains(&"Repository"));
    }

    #[test]
    fn test_rust_type_alias() {
        let e = engine();
        let source = r#"
type Result<T> = std::result::Result<T, AppError>;
type UserId = u64;
"#;
        let result = e.parse_file("main.rs", source).unwrap();
        let type_names: Vec<&str> = result
            .definitions
            .iter()
            .filter(|d| d.kind == SymbolKind::TypeAlias)
            .map(|d| d.name.as_str())
            .collect();
        assert!(type_names.contains(&"Result"));
        assert!(type_names.contains(&"UserId"));
    }

    #[test]
    fn test_rust_const_and_static() {
        let e = engine();
        let source = r#"
const MAX_RETRIES: u32 = 3;
static DB_URL: &str = "postgres://localhost/mydb";
"#;
        let result = e.parse_file("main.rs", source).unwrap();
        let const_names: Vec<&str> = result
            .definitions
            .iter()
            .filter(|d| d.kind == SymbolKind::Constant)
            .map(|d| d.name.as_str())
            .collect();
        assert!(const_names.contains(&"MAX_RETRIES"));
        assert!(const_names.contains(&"DB_URL"));
    }

    #[test]
    fn test_rust_impl_methods() {
        let e = engine();
        let source = r#"
struct UserService;

impl UserService {
    fn new() -> Self {
        UserService
    }

    pub fn find_user(&self, id: u64) -> Option<User> {
        None
    }
}
"#;
        let result = e.parse_file("main.rs", source).unwrap();
        let fn_names: Vec<&str> = result
            .definitions
            .iter()
            .filter(|d| d.kind == SymbolKind::Function)
            .map(|d| d.name.as_str())
            .collect();
        assert!(fn_names.contains(&"new"));
        assert!(fn_names.contains(&"find_user"));
    }

    #[test]
    fn test_rust_call_sites() {
        let e = engine();
        let source = r#"
fn main() {
    let service = UserService::new();
    let user = service.find_user(42);
    println!("User: {:?}", user);
}
"#;
        let result = e.parse_file("main.rs", source).unwrap();
        let callees: Vec<&str> = result
            .call_sites
            .iter()
            .map(|c| c.callee.as_str())
            .collect();
        assert!(callees.contains(&"UserService::new"));
        assert!(callees.contains(&"service.find_user"));
    }

    #[test]
    fn test_rust_call_sites_containing_function() {
        let e = engine();
        let source = r#"
fn process() {
    let data = fetch_data();
    save(data);
}
"#;
        let result = e.parse_file("main.rs", source).unwrap();
        for call in &result.call_sites {
            assert_eq!(call.containing_function.as_deref(), Some("process"));
        }
    }

    #[test]
    fn test_rust_data_flow_let_binding() {
        let e = engine();
        let source = r#"
fn handler() {
    let user = find_user(42);
    let result = save_user(user);
}
"#;
        let result = e.extract_data_flow("main.rs", source).unwrap();
        let vars: Vec<&str> = result
            .assignments
            .iter()
            .map(|a| a.variable.as_str())
            .collect();
        assert!(vars.contains(&"user"));
        assert!(vars.contains(&"result"));
    }

    #[test]
    fn test_rust_data_flow_call_with_args() {
        let e = engine();
        let source = r#"
fn handler(req: Request) {
    let body = parse_body(req);
    let user = create_user(body);
}
"#;
        let result = e.extract_data_flow("main.rs", source).unwrap();
        let call = result
            .calls_with_args
            .iter()
            .find(|c| c.callee == "parse_body");
        assert!(call.is_some());
        assert!(call.unwrap().arguments.contains(&"req".to_string()));
    }

    #[test]
    fn test_rust_macro_definition() {
        let e = engine();
        let source = r#"
macro_rules! my_macro {
    ($x:expr) => {
        println!("{}", $x);
    };
}
"#;
        let result = e.parse_file("main.rs", source).unwrap();
        let macro_names: Vec<&str> = result
            .definitions
            .iter()
            .filter(|d| d.name == "my_macro")
            .map(|d| d.name.as_str())
            .collect();
        assert!(macro_names.contains(&"my_macro"));
    }

    #[test]
    fn test_rust_empty_source() {
        let e = engine();
        let result = e.parse_file("main.rs", "").unwrap();
        assert_eq!(result.language, Language::Rust);
        assert!(result.definitions.is_empty());
        assert!(result.imports.is_empty());
    }

    #[test]
    fn test_rust_complex_use_patterns() {
        let e = engine();
        let source = r#"
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use crate::models::User;
use super::config::Config;
"#;
        let result = e.parse_file("main.rs", source).unwrap();
        assert!(result.imports.len() >= 4);
        // Verify serde has named imports
        let serde_import = result.imports.iter().find(|i| i.source == "serde");
        assert!(serde_import.is_some());
        let serde_names: Vec<&str> = serde_import
            .unwrap()
            .names
            .iter()
            .map(|n| n.name.as_str())
            .collect();
        assert!(serde_names.contains(&"Serialize"));
        assert!(serde_names.contains(&"Deserialize"));
    }

    #[test]
    fn test_rust_use_from_crate_root() {
        // use axum::{Router, routing::get};  — source should be "axum"
        let e = engine();
        let source = "use axum::{Router, routing};";
        let result = e.parse_file("main.rs", source).unwrap();
        assert!(
            !result.imports.is_empty(),
            "should have at least one import"
        );
        // The source should be "axum" (the crate root)
        assert!(
            result.imports.iter().any(|i| i.source == "axum"),
            "should detect axum import; got: {:?}",
            result.imports.iter().map(|i| &i.source).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_rust_async_main_detection() {
        let e = engine();
        let source = r#"
use axum::{Router, routing::get};

#[tokio::main]
async fn main() {
    let app = Router::new();
    axum::serve(app).await.unwrap();
}
"#;
        let result = e.parse_file("src/main.rs", source).unwrap();
        let fn_names: Vec<&str> = result
            .definitions
            .iter()
            .filter(|d| d.kind == SymbolKind::Function)
            .map(|d| d.name.as_str())
            .collect();
        assert!(
            fn_names.contains(&"main"),
            "should detect async fn main(); got defs: {:?}",
            fn_names
        );
    }

    #[test]
    fn test_rust_full_module() {
        // Test a realistic Rust file with mixed definitions
        let e = engine();
        let source = r#"
use std::sync::Arc;
use serde::{Serialize, Deserialize};

const DEFAULT_PORT: u16 = 8080;

#[derive(Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub port: u16,
    pub host: String,
}

pub trait Service {
    fn start(&self) -> Result<(), Box<dyn std::error::Error>>;
}

impl AppConfig {
    pub fn new() -> Self {
        AppConfig {
            port: DEFAULT_PORT,
            host: "localhost".to_string(),
        }
    }
}

pub fn run_server(config: AppConfig) {
    let addr = format!("{}:{}", config.host, config.port);
    start_listener(addr);
}

fn start_listener(addr: String) {
    println!("Listening on {}", addr);
}
"#;
        let result = e.parse_file("lib.rs", source).unwrap();
        assert_eq!(result.language, Language::Rust);

        // Check imports
        assert!(result.imports.len() >= 2);

        // Check definitions
        let def_names: Vec<&str> = result.definitions.iter().map(|d| d.name.as_str()).collect();
        assert!(def_names.contains(&"DEFAULT_PORT"));
        assert!(def_names.contains(&"AppConfig"));
        assert!(def_names.contains(&"Service"));
        assert!(def_names.contains(&"new"));
        assert!(def_names.contains(&"run_server"));
        assert!(def_names.contains(&"start_listener"));

        // Check call sites (format! is a macro, not a call_expression — won't appear)
        let callees: Vec<&str> = result
            .call_sites
            .iter()
            .map(|c| c.callee.as_str())
            .collect();
        assert!(callees.contains(&"start_listener"));
    }

}
