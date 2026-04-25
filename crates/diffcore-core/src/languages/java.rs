//! Java extraction code.
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
    let static_source_idx = qwc.capture_index("static_source");
    let wildcard_idx = qwc.capture_index("wildcard");
    let wildcard_source_idx = qwc.capture_index("wildcard_source");

    let mut imports = Vec::new();
    let mut seen: Vec<(usize, String)> = Vec::new();

    // First pass: collect lines that have wildcard imports so we skip
    // the regular source match for those lines.
    let mut wildcard_lines: Vec<usize> = Vec::new();
    for m in &matches {
        if m.has_capture(wildcard_idx) {
            let line = m
                .get_capture(stmt_idx)
                .map(|n| n.start_position().row + 1)
                .unwrap_or(0);
            wildcard_lines.push(line);
        }
    }

    for m in &matches {
        let mut line = 0usize;

        for &(idx, node) in &m.captures {
            if Some(idx) == stmt_idx {
                line = node.start_position().row + 1;
            }
        }

        if m.has_capture(wildcard_idx) {
            // import com.example.*; — wildcard import
            let pkg = m
                .get_capture(wildcard_source_idx)
                .map(|n| node_text(&n, source).to_string())
                .unwrap_or_default();
            if !pkg.is_empty() {
                let key = (line, pkg.clone());
                if !seen.contains(&key) {
                    seen.push(key);
                    imports.push(ImportInfo {
                        source: pkg,
                        names: vec![ImportedName {
                            name: "*".to_string(),
                            alias: None,
                        }],
                        is_default: false,
                        is_namespace: false,
                        line,
                    });
                }
            }
        } else if m.has_capture(static_source_idx) {
            // import static com.example.Foo.bar;
            let source_text = m
                .get_capture(static_source_idx)
                .map(|n| node_text(&n, source).to_string())
                .unwrap_or_default();
            if !source_text.is_empty() {
                // Split into package and member: com.example.Foo.bar -> source=com.example.Foo, name=bar
                let (pkg, member) = if let Some(dot_pos) = source_text.rfind('.') {
                    (
                        source_text[..dot_pos].to_string(),
                        source_text[dot_pos + 1..].to_string(),
                    )
                } else {
                    (source_text.clone(), source_text.clone())
                };
                let key = (line, source_text.clone());
                if !seen.contains(&key) {
                    seen.push(key);
                    imports.push(ImportInfo {
                        source: pkg,
                        names: vec![ImportedName {
                            name: member,
                            alias: None,
                        }],
                        is_default: false,
                        is_namespace: false,
                        line,
                    });
                }
            }
        } else if m.has_capture(source_idx) && !wildcard_lines.contains(&line) {
            // import com.example.Foo; — skip if this line is a wildcard import
            let source_text = m
                .get_capture(source_idx)
                .map(|n| node_text(&n, source).to_string())
                .unwrap_or_default();
            if !source_text.is_empty() {
                // Extract class name from fully qualified path
                let class_name = source_text
                    .rsplit('.')
                    .next()
                    .unwrap_or(&source_text)
                    .to_string();
                // Use the package (everything before the last dot) as source
                let pkg = if let Some(dot_pos) = source_text.rfind('.') {
                    source_text[..dot_pos].to_string()
                } else {
                    source_text.clone()
                };
                let key = (line, source_text.clone());
                if !seen.contains(&key) {
                    seen.push(key);
                    imports.push(ImportInfo {
                        source: pkg,
                        names: vec![ImportedName {
                            name: class_name,
                            alias: None,
                        }],
                        is_default: false,
                        is_namespace: false,
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
    if extract_definitions_standard(matches, source, qwc, definitions, seen_nodes) {
        // Standard path took over.
    } else {
        let method_name_idx = qwc.capture_index("method_name");
        let method_node_idx = qwc.capture_index("method_node");
        let ctor_name_idx = qwc.capture_index("ctor_name");
        let ctor_node_idx = qwc.capture_index("ctor_node");
        let class_name_idx = qwc.capture_index("class_name");
        let class_node_idx = qwc.capture_index("class_node");
        let iface_name_idx = qwc.capture_index("iface_name");
        let iface_node_idx = qwc.capture_index("iface_node");
        let enum_name_idx = qwc.capture_index("enum_name");
        let enum_node_idx = qwc.capture_index("enum_node");
        let annotation_name_idx = qwc.capture_index("annotation_name");
        let annotation_node_idx = qwc.capture_index("annotation_node");
        let field_name_idx = qwc.capture_index("field_name");
        let field_node_idx = qwc.capture_index("field_node");

        let java_def_captures: &[(Option<u32>, Option<u32>, SymbolKind)] = &[
            (method_name_idx, method_node_idx, SymbolKind::Function),
            (ctor_name_idx, ctor_node_idx, SymbolKind::Function),
            (class_name_idx, class_node_idx, SymbolKind::Class),
            (iface_name_idx, iface_node_idx, SymbolKind::Interface),
            (enum_name_idx, enum_node_idx, SymbolKind::Class),
            (
                annotation_name_idx,
                annotation_node_idx,
                SymbolKind::Interface,
            ),
            (field_name_idx, field_node_idx, SymbolKind::Constant),
        ];

        for m in matches {
            for &(name_cap, node_cap, kind) in java_def_captures {
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
    fn test_java_language_detection() {
        assert_eq!(Language::from_path("Main.java"), Language::Java);
        assert_eq!(
            Language::from_path("src/com/example/App.java"),
            Language::Java
        );
        assert_eq!(
            Language::from_path("src/main/java/UserController.java"),
            Language::Java
        );
    }

    #[test]
    fn test_java_simple_import() {
        let e = engine();
        let result = e.parse_file("App.java", "import java.util.List;").unwrap();
        assert_eq!(result.language, Language::Java);
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].source, "java.util");
        assert_eq!(result.imports[0].names[0].name, "List");
    }

    #[test]
    fn test_java_wildcard_import() {
        let e = engine();
        let result = e.parse_file("App.java", "import java.util.*;").unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].source, "java.util");
        assert_eq!(result.imports[0].names[0].name, "*");
    }

    #[test]
    fn test_java_static_import() {
        let e = engine();
        let result = e
            .parse_file("App.java", "import static org.junit.Assert.assertEquals;")
            .unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].source, "org.junit.Assert");
        assert_eq!(result.imports[0].names[0].name, "assertEquals");
    }

    #[test]
    fn test_java_multiple_imports() {
        let e = engine();
        let source = r#"
import java.util.List;
import java.util.Map;
import org.springframework.web.bind.annotation.RestController;
"#;
        let result = e.parse_file("App.java", source).unwrap();
        assert_eq!(result.imports.len(), 3);
    }

    #[test]
    fn test_java_class_definition() {
        let e = engine();
        let source = r#"
public class UserController {
}
"#;
        let result = e.parse_file("UserController.java", source).unwrap();
        let class_defs: Vec<&str> = result
            .definitions
            .iter()
            .filter(|d| d.kind == SymbolKind::Class)
            .map(|d| d.name.as_str())
            .collect();
        assert!(class_defs.contains(&"UserController"));
    }

    #[test]
    fn test_java_interface_definition() {
        let e = engine();
        let source = r#"
public interface UserRepository {
    User findById(Long id);
}
"#;
        let result = e.parse_file("UserRepository.java", source).unwrap();
        let iface_defs: Vec<&str> = result
            .definitions
            .iter()
            .filter(|d| d.kind == SymbolKind::Interface)
            .map(|d| d.name.as_str())
            .collect();
        assert!(iface_defs.contains(&"UserRepository"));
    }

    #[test]
    fn test_java_method_definitions() {
        let e = engine();
        let source = r#"
public class UserService {
    public User findUser(Long id) {
        return null;
    }

    public void createUser(String name) {
    }
}
"#;
        let result = e.parse_file("UserService.java", source).unwrap();
        let fn_defs: Vec<&str> = result
            .definitions
            .iter()
            .filter(|d| d.kind == SymbolKind::Function)
            .map(|d| d.name.as_str())
            .collect();
        assert!(fn_defs.contains(&"findUser"));
        assert!(fn_defs.contains(&"createUser"));
    }

    #[test]
    fn test_java_enum_definition() {
        let e = engine();
        let source = r#"
public enum Status {
    ACTIVE,
    INACTIVE
}
"#;
        let result = e.parse_file("Status.java", source).unwrap();
        let class_defs: Vec<&str> = result
            .definitions
            .iter()
            .filter(|d| d.kind == SymbolKind::Class)
            .map(|d| d.name.as_str())
            .collect();
        assert!(class_defs.contains(&"Status"));
    }

    #[test]
    fn test_java_constructor_definition() {
        let e = engine();
        let source = r#"
public class UserService {
    public UserService(UserRepository repo) {
    }
}
"#;
        let result = e.parse_file("UserService.java", source).unwrap();
        let fn_defs: Vec<&str> = result
            .definitions
            .iter()
            .filter(|d| d.kind == SymbolKind::Function)
            .map(|d| d.name.as_str())
            .collect();
        assert!(fn_defs.contains(&"UserService"));
    }

    #[test]
    fn test_java_field_definition() {
        let e = engine();
        let source = r#"
public class Config {
    private String apiKey;
    public static final int MAX_RETRIES = 3;
}
"#;
        let result = e.parse_file("Config.java", source).unwrap();
        let field_defs: Vec<&str> = result
            .definitions
            .iter()
            .filter(|d| d.kind == SymbolKind::Constant)
            .map(|d| d.name.as_str())
            .collect();
        assert!(field_defs.contains(&"apiKey"));
        assert!(field_defs.contains(&"MAX_RETRIES"));
    }

    #[test]
    fn test_java_method_call() {
        let e = engine();
        let source = r#"
public class App {
    public void run() {
        userService.findUser(42);
    }
}
"#;
        let result = e.parse_file("App.java", source).unwrap();
        let callees: Vec<&str> = result
            .call_sites
            .iter()
            .map(|c| c.callee.as_str())
            .collect();
        assert!(callees.contains(&"findUser"));
    }

    #[test]
    fn test_java_constructor_call() {
        let e = engine();
        let source = r#"
public class App {
    public void run() {
        User user = new User("Alice");
    }
}
"#;
        let result = e.parse_file("App.java", source).unwrap();
        let callees: Vec<&str> = result
            .call_sites
            .iter()
            .map(|c| c.callee.as_str())
            .collect();
        assert!(callees.contains(&"User"));
    }

    #[test]
    fn test_java_call_containing_function() {
        let e = engine();
        let source = r#"
public class App {
    public void run() {
        doSomething();
    }
}
"#;
        let result = e.parse_file("App.java", source).unwrap();
        let call = result
            .call_sites
            .iter()
            .find(|c| c.callee == "doSomething")
            .unwrap();
        assert_eq!(call.containing_function.as_deref(), Some("run"));
    }

    #[test]
    fn test_java_data_flow_assignment() {
        let e = engine();
        let source = r#"
public class App {
    public void run() {
        User user = findUser(1);
    }
}
"#;
        let df = e.extract_data_flow("App.java", source).unwrap();
        assert!(!df.assignments.is_empty());
        assert_eq!(df.assignments[0].variable, "user");
        assert_eq!(df.assignments[0].callee, "findUser");
    }

    #[test]
    fn test_java_data_flow_constructor_assignment() {
        let e = engine();
        let source = r#"
public class App {
    public void run() {
        UserService svc = new UserService();
    }
}
"#;
        let df = e.extract_data_flow("App.java", source).unwrap();
        assert!(!df.assignments.is_empty());
        assert_eq!(df.assignments[0].variable, "svc");
        assert_eq!(df.assignments[0].callee, "UserService");
    }

    #[test]
    fn test_java_empty_source() {
        let e = engine();
        let result = e.parse_file("App.java", "").unwrap();
        assert_eq!(result.language, Language::Java);
        assert!(result.definitions.is_empty());
        assert!(result.imports.is_empty());
    }

    #[test]
    fn test_java_full_spring_controller() {
        let e = engine();
        let source = r#"
package com.example.demo;

import java.util.List;
import org.springframework.web.bind.annotation.RestController;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;

@RestController
public class UserController {

    private final UserService userService;

    public UserController(UserService userService) {
        this.userService = userService;
    }

    @GetMapping("/users")
    public List<User> getUsers() {
        return userService.findAll();
    }

    @PostMapping("/users")
    public User createUser(@RequestBody User user) {
        return userService.save(user);
    }
}
"#;
        let result = e.parse_file("UserController.java", source).unwrap();

        // Imports
        assert!(result.imports.len() >= 4);
        let import_sources: Vec<&str> = result.imports.iter().map(|i| i.source.as_str()).collect();
        assert!(import_sources.iter().any(|s| s.contains("java.util")));
        assert!(import_sources
            .iter()
            .any(|s| s.contains("org.springframework")));

        // Definitions
        let def_names: Vec<&str> = result.definitions.iter().map(|d| d.name.as_str()).collect();
        assert!(def_names.contains(&"UserController"));
        assert!(def_names.contains(&"getUsers"));
        assert!(def_names.contains(&"createUser"));

        // Call sites
        let callees: Vec<&str> = result
            .call_sites
            .iter()
            .map(|c| c.callee.as_str())
            .collect();
        assert!(callees.contains(&"findAll"));
        assert!(callees.contains(&"save"));
    }
}
