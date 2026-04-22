//! PHP extraction code.
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
    let alias_idx = qwc.capture_index("alias");
    let include_source_idx = qwc.capture_index("include_source");

    let mut imports: Vec<ImportInfo> = Vec::new();
    // Track seen imports by (line, source_text). If we see a later match
    // with an alias on the same line+source, update the existing import.
    let mut seen: Vec<(usize, String)> = Vec::new();

    for m in &matches {
        let mut line = 0usize;

        for &(idx, node) in &m.captures {
            if Some(idx) == stmt_idx {
                line = node.start_position().row + 1;
            }
        }

        // Handle `use` imports (namespace imports)
        if m.has_capture(source_idx) {
            let source_text = m
                .get_capture(source_idx)
                .map(|n| node_text(&n, source).to_string())
                .unwrap_or_default();
            if source_text.is_empty() {
                continue;
            }

            let alias = m
                .get_capture(alias_idx)
                .map(|n| node_text(&n, source).to_string());

            let key = (line, source_text.clone());
            if let Some(pos) = seen.iter().position(|k| *k == key) {
                // If this match has an alias and the existing one doesn't, update it
                if alias.is_some() && imports[pos].names[0].alias.is_none() {
                    imports[pos].names[0].alias = alias;
                }
                continue;
            }
            seen.push(key);

            // Extract the class name (last segment) as the imported name
            let class_name = source_text
                .rsplit('\\')
                .next()
                .unwrap_or(&source_text)
                .to_string();

            // Use backslash-separated namespace path as source
            imports.push(ImportInfo {
                source: source_text,
                names: vec![ImportedName {
                    name: class_name,
                    alias,
                }],
                is_default: false,
                is_namespace: false,
                line,
            });
        }

        // Handle require/include (file inclusion)
        if m.has_capture(include_source_idx) {
            let path_text = m
                .get_capture(include_source_idx)
                .map(|n| node_text(&n, source).to_string())
                .unwrap_or_default();
            if path_text.is_empty() {
                continue;
            }

            let key = (line, path_text.clone());
            if seen.contains(&key) {
                continue;
            }
            seen.push(key);

            imports.push(ImportInfo {
                source: path_text,
                names: vec![],
                is_default: false,
                is_namespace: false,
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
    if extract_definitions_standard(
        matches,
        source,
        qwc,
        definitions,
        seen_nodes,
    ) {
        // Standard path took over.
    } else {
    let method_name_idx = qwc.capture_index("method_name");
    let method_node_idx = qwc.capture_index("method_node");
    let func_name_idx = qwc.capture_index("func_name");
    let func_node_idx = qwc.capture_index("func_node");
    let class_name_idx = qwc.capture_index("class_name");
    let class_node_idx = qwc.capture_index("class_node");
    let iface_name_idx = qwc.capture_index("iface_name");
    let iface_node_idx = qwc.capture_index("iface_node");
    let trait_name_idx = qwc.capture_index("trait_name");
    let trait_node_idx = qwc.capture_index("trait_node");
    let enum_name_idx = qwc.capture_index("enum_name");
    let enum_node_idx = qwc.capture_index("enum_node");
    let const_name_idx = qwc.capture_index("const_name");
    let const_node_idx = qwc.capture_index("const_node");

    let php_def_captures: &[(Option<u32>, Option<u32>, SymbolKind)] = &[
        (method_name_idx, method_node_idx, SymbolKind::Function),
        (func_name_idx, func_node_idx, SymbolKind::Function),
        (class_name_idx, class_node_idx, SymbolKind::Class),
        (iface_name_idx, iface_node_idx, SymbolKind::Interface),
        (trait_name_idx, trait_node_idx, SymbolKind::Interface),
        (enum_name_idx, enum_node_idx, SymbolKind::TypeAlias),
        (const_name_idx, const_node_idx, SymbolKind::Constant),
    ];

    for m in matches {
        for &(name_cap, node_cap, kind) in php_def_captures {
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
    fn test_php_language_detection() {
        assert_eq!(
            Language::from_path("app/Http/Controllers/UserController.php"),
            Language::Php
        );
        assert_eq!(
            Language::from_path("src/Services/UserService.php"),
            Language::Php
        );
        assert_eq!(
            Language::from_path("tests/Unit/UserTest.php"),
            Language::Php
        );
    }

    #[test]
    fn test_php_use_import() {
        let e = engine();
        let result = e
            .parse_file(
                "app/Models/User.php",
                "<?php\n\nnamespace App\\Models;\n\nuse Illuminate\\Database\\Eloquent\\Model;\n\nclass User extends Model {}\n",
            )
            .unwrap();
        assert_eq!(result.language, Language::Php);
        assert_eq!(result.imports.len(), 1);
        assert_eq!(
            result.imports[0].source,
            "Illuminate\\Database\\Eloquent\\Model"
        );
        assert_eq!(result.imports[0].names[0].name, "Model");
    }

    #[test]
    fn test_php_use_import_with_alias() {
        let e = engine();
        let result = e
            .parse_file(
                "app/Services/Service.php",
                "<?php\n\nuse App\\Services\\UserService as US;\n",
            )
            .unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].source, "App\\Services\\UserService");
        assert_eq!(result.imports[0].names[0].name, "UserService");
        assert_eq!(result.imports[0].names[0].alias, Some("US".to_string()));
    }

    #[test]
    fn test_php_multiple_imports() {
        let e = engine();
        let result = e
            .parse_file(
                "app/Http/Controllers/UserController.php",
                "<?php\n\nuse Illuminate\\Http\\Request;\nuse App\\Models\\User;\nuse App\\Services\\UserService;\n",
            )
            .unwrap();
        assert_eq!(result.imports.len(), 3);
        let sources: Vec<&str> = result.imports.iter().map(|i| i.source.as_str()).collect();
        assert!(sources.contains(&"Illuminate\\Http\\Request"));
        assert!(sources.contains(&"App\\Models\\User"));
        assert!(sources.contains(&"App\\Services\\UserService"));
    }

    #[test]
    fn test_php_require_include() {
        let e = engine();
        let result = e
            .parse_file(
                "index.php",
                "<?php\n\nrequire 'config.php';\nrequire_once 'vendor/autoload.php';\ninclude 'helpers.php';\ninclude_once 'utils.php';\n",
            )
            .unwrap();
        assert_eq!(result.imports.len(), 4);
        let sources: Vec<&str> = result.imports.iter().map(|i| i.source.as_str()).collect();
        assert!(sources.contains(&"config.php"));
        assert!(sources.contains(&"vendor/autoload.php"));
        assert!(sources.contains(&"helpers.php"));
        assert!(sources.contains(&"utils.php"));
    }

    #[test]
    fn test_php_function_definition() {
        let e = engine();
        let result = e
            .parse_file(
                "helpers.php",
                "<?php\n\nfunction greet($name) {\n    return \"Hello, \" . $name;\n}\n\nfunction farewell($name) {\n    return \"Goodbye, \" . $name;\n}\n",
            )
            .unwrap();
        let def_names: Vec<&str> = result.definitions.iter().map(|d| d.name.as_str()).collect();
        assert!(def_names.contains(&"greet"));
        assert!(def_names.contains(&"farewell"));
        assert_eq!(result.definitions.len(), 2);
        // Both should be Function kind
        assert!(result
            .definitions
            .iter()
            .all(|d| d.kind == SymbolKind::Function));
    }

    #[test]
    fn test_php_class_definition() {
        let e = engine();
        let result = e
            .parse_file(
                "app/Models/User.php",
                "<?php\n\nclass User {\n    public function getName() {\n        return $this->name;\n    }\n}\n",
            )
            .unwrap();
        let def_names: Vec<&str> = result.definitions.iter().map(|d| d.name.as_str()).collect();
        assert!(def_names.contains(&"User"));
        assert!(def_names.contains(&"getName"));
        let class_def = result
            .definitions
            .iter()
            .find(|d| d.name == "User")
            .unwrap();
        assert_eq!(class_def.kind, SymbolKind::Class);
    }

    #[test]
    fn test_php_interface_definition() {
        let e = engine();
        let result = e
            .parse_file(
                "app/Contracts/Greetable.php",
                "<?php\n\ninterface Greetable {\n    public function greet();\n}\n",
            )
            .unwrap();
        let def_names: Vec<&str> = result.definitions.iter().map(|d| d.name.as_str()).collect();
        assert!(def_names.contains(&"Greetable"));
        let iface_def = result
            .definitions
            .iter()
            .find(|d| d.name == "Greetable")
            .unwrap();
        assert_eq!(iface_def.kind, SymbolKind::Interface);
    }

    #[test]
    fn test_php_trait_definition() {
        let e = engine();
        let result = e
            .parse_file(
                "app/Traits/Loggable.php",
                "<?php\n\ntrait Loggable {\n    public function log($msg) {\n        echo $msg;\n    }\n}\n",
            )
            .unwrap();
        let def_names: Vec<&str> = result.definitions.iter().map(|d| d.name.as_str()).collect();
        assert!(def_names.contains(&"Loggable"));
        assert!(def_names.contains(&"log"));
    }

    #[test]
    fn test_php_enum_definition() {
        let e = engine();
        let result = e
            .parse_file(
                "app/Enums/Color.php",
                "<?php\n\nenum Color {\n    case Red;\n    case Blue;\n}\n",
            )
            .unwrap();
        let def_names: Vec<&str> = result.definitions.iter().map(|d| d.name.as_str()).collect();
        assert!(def_names.contains(&"Color"));
    }

    #[test]
    fn test_php_const_definition() {
        let e = engine();
        let result = e
            .parse_file(
                "config.php",
                "<?php\n\nconst MAX_SIZE = 100;\nconst APP_NAME = 'diffcore';\n",
            )
            .unwrap();
        let def_names: Vec<&str> = result.definitions.iter().map(|d| d.name.as_str()).collect();
        assert!(def_names.contains(&"MAX_SIZE"));
        assert!(def_names.contains(&"APP_NAME"));
        assert!(result
            .definitions
            .iter()
            .all(|d| d.kind == SymbolKind::Constant));
    }

    #[test]
    fn test_php_method_definitions() {
        let e = engine();
        let result = e
            .parse_file(
                "app/Services/UserService.php",
                "<?php\n\nclass UserService {\n    public function findAll() {\n        return [];\n    }\n\n    public function findById($id) {\n        return null;\n    }\n\n    public function create($data) {\n        return $data;\n    }\n}\n",
            )
            .unwrap();
        let method_names: Vec<&str> = result
            .definitions
            .iter()
            .filter(|d| d.kind == SymbolKind::Function)
            .map(|d| d.name.as_str())
            .collect();
        assert!(method_names.contains(&"findAll"));
        assert!(method_names.contains(&"findById"));
        assert!(method_names.contains(&"create"));
    }

    #[test]
    fn test_php_function_call() {
        let e = engine();
        let result = e
            .parse_file(
                "index.php",
                "<?php\n\n$result = greet('World');\necho strlen($result);\n",
            )
            .unwrap();
        let callees: Vec<&str> = result
            .call_sites
            .iter()
            .map(|c| c.callee.as_str())
            .collect();
        assert!(callees.contains(&"greet"));
        assert!(callees.contains(&"strlen"));
    }

    #[test]
    fn test_php_method_call() {
        let e = engine();
        let result = e
            .parse_file(
                "app/Http/Controllers/UserController.php",
                "<?php\n\nclass UserController {\n    public function index() {\n        $users = $this->service->findAll();\n        return response()->json($users);\n    }\n}\n",
            )
            .unwrap();
        let callees: Vec<&str> = result
            .call_sites
            .iter()
            .map(|c| c.callee.as_str())
            .collect();
        assert!(callees.contains(&"findAll"));
        assert!(callees.contains(&"response"));
        assert!(callees.contains(&"json"));
    }

    #[test]
    fn test_php_static_call() {
        let e = engine();
        let result = e
            .parse_file(
                "app/Http/Controllers/UserController.php",
                "<?php\n\nclass UserController {\n    public function index() {\n        $users = User::all();\n        $user = User::find(1);\n        return $users;\n    }\n}\n",
            )
            .unwrap();
        let callees: Vec<&str> = result
            .call_sites
            .iter()
            .map(|c| c.callee.as_str())
            .collect();
        assert!(callees.contains(&"all"));
        assert!(callees.contains(&"find"));
    }

    #[test]
    fn test_php_object_creation() {
        let e = engine();
        let result = e
            .parse_file(
                "index.php",
                "<?php\n\n$service = new UserService();\n$response = new JsonResponse($data);\n",
            )
            .unwrap();
        let callees: Vec<&str> = result
            .call_sites
            .iter()
            .map(|c| c.callee.as_str())
            .collect();
        assert!(callees.contains(&"UserService"));
        assert!(callees.contains(&"JsonResponse"));
    }

    #[test]
    fn test_php_call_containing_function() {
        let e = engine();
        let result = e
            .parse_file(
                "app/Services/UserService.php",
                "<?php\n\nclass UserService {\n    public function create($data) {\n        $user = User::create($data);\n        return $user;\n    }\n}\n",
            )
            .unwrap();
        let create_call = result
            .call_sites
            .iter()
            .find(|c| c.callee == "create")
            .unwrap();
        // The containing function should be the method "create"
        assert!(create_call.containing_function.is_some());
        assert_eq!(create_call.containing_function.as_deref(), Some("create"));
    }

    #[test]
    fn test_php_data_flow_function_assignment() {
        let e = engine();
        let result = e
            .extract_data_flow("index.php", "<?php\n\n$result = greet('World');\n")
            .unwrap();
        assert!(!result.assignments.is_empty());
        let a = &result.assignments[0];
        assert_eq!(a.variable, "result");
        assert_eq!(a.callee, "greet");
    }

    #[test]
    fn test_php_data_flow_method_assignment() {
        let e = engine();
        let result = e
            .extract_data_flow(
                "app/Controllers/UserController.php",
                "<?php\n\nclass UserController {\n    public function index() {\n        $users = $this->service->findAll();\n    }\n}\n",
            )
            .unwrap();
        let assignment = result.assignments.iter().find(|a| a.variable == "users");
        assert!(assignment.is_some(), "should find $users assignment");
        assert_eq!(assignment.unwrap().callee, "findAll");
    }

    #[test]
    fn test_php_data_flow_static_assignment() {
        let e = engine();
        let result = e
            .extract_data_flow(
                "app/Controllers/UserController.php",
                "<?php\n\nclass UserController {\n    public function index() {\n        $users = User::all();\n    }\n}\n",
            )
            .unwrap();
        let assignment = result.assignments.iter().find(|a| a.variable == "users");
        assert!(assignment.is_some(), "should find $users assignment");
        assert_eq!(assignment.unwrap().callee, "all");
    }

    #[test]
    fn test_php_data_flow_constructor_assignment() {
        let e = engine();
        let result = e
            .extract_data_flow("index.php", "<?php\n\n$service = new UserService();\n")
            .unwrap();
        let assignment = result.assignments.iter().find(|a| a.variable == "service");
        assert!(assignment.is_some(), "should find $service assignment");
        assert_eq!(assignment.unwrap().callee, "UserService");
    }

    #[test]
    fn test_php_empty_source() {
        let e = engine();
        let result = e.parse_file("empty.php", "<?php\n").unwrap();
        assert_eq!(result.language, Language::Php);
        assert!(result.definitions.is_empty());
        assert!(result.imports.is_empty());
        assert!(result.call_sites.is_empty());
    }

    #[test]
    fn test_php_full_laravel_controller() {
        let e = engine();
        let source = "<?php\n\nnamespace App\\Http\\Controllers;\n\nuse Illuminate\\Http\\Request;\nuse App\\Models\\User;\nuse App\\Services\\UserService;\n\nclass UserController extends Controller\n{\n    private $service;\n\n    public function __construct(UserService $service)\n    {\n        $this->service = $service;\n    }\n\n    public function index()\n    {\n        $users = User::all();\n        return response()->json($users);\n    }\n\n    public function store(Request $request)\n    {\n        $data = $request->validated();\n        $user = User::create($data);\n        return response()->json($user, 201);\n    }\n\n    public function show(User $user)\n    {\n        return response()->json($user);\n    }\n\n    public function destroy(User $user)\n    {\n        $user->delete();\n        return response()->json(null, 204);\n    }\n}\n";
        let result = e
            .parse_file("app/Http/Controllers/UserController.php", source)
            .unwrap();
        assert_eq!(result.language, Language::Php);

        // Imports
        let import_sources: Vec<&str> = result.imports.iter().map(|i| i.source.as_str()).collect();
        assert!(import_sources.contains(&"Illuminate\\Http\\Request"));
        assert!(import_sources.contains(&"App\\Models\\User"));
        assert!(import_sources.contains(&"App\\Services\\UserService"));

        // Definitions
        let def_names: Vec<&str> = result.definitions.iter().map(|d| d.name.as_str()).collect();
        assert!(def_names.contains(&"UserController"));
        assert!(def_names.contains(&"__construct"));
        assert!(def_names.contains(&"index"));
        assert!(def_names.contains(&"store"));
        assert!(def_names.contains(&"show"));
        assert!(def_names.contains(&"destroy"));

        // Call sites
        let callees: Vec<&str> = result
            .call_sites
            .iter()
            .map(|c| c.callee.as_str())
            .collect();
        assert!(callees.contains(&"all"));
        assert!(callees.contains(&"response"));
        assert!(callees.contains(&"json"));
        assert!(callees.contains(&"validated"));
        assert!(callees.contains(&"create"));
        assert!(callees.contains(&"delete"));
    }

}
