//! CPP extraction code.
//!
//! Moved from `query_engine.rs` during the per-language module split.
//! Pure mechanical move — no logic changes.

use crate::ast::{Definition, ImportInfo};
use crate::languages::common::{
    extract_definitions_standard, hash_str, node_span, node_text, CollectedMatch,
};
use crate::query_engine::{QueryEngineError, QueryWithCaptures};
use crate::types::SymbolKind;
use tree_sitter::Node;

pub(crate) fn extract_imports(
    root: &Node,
    source: &[u8],
    qwc: &QueryWithCaptures,
) -> Result<Vec<ImportInfo>, QueryEngineError> {
    crate::languages::c::extract_imports(root, source, qwc)
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
        let func_name_idx = qwc.capture_index("func_name");
        let func_node_idx = qwc.capture_index("func_node");
        let method_name_idx = qwc.capture_index("method_name");
        let method_node_idx = qwc.capture_index("method_node");
        let class_name_idx = qwc.capture_index("class_name");
        let class_node_idx = qwc.capture_index("class_node");
        let struct_name_idx = qwc.capture_index("struct_name");
        let struct_node_idx = qwc.capture_index("struct_node");
        let enum_name_idx = qwc.capture_index("enum_name");
        let enum_node_idx = qwc.capture_index("enum_node");
        let namespace_name_idx = qwc.capture_index("namespace_name");
        let namespace_node_idx = qwc.capture_index("namespace_node");
        let alias_name_idx = qwc.capture_index("alias_name");
        let alias_node_idx = qwc.capture_index("alias_node");
        let template_func_name_idx = qwc.capture_index("template_func_name");
        let template_func_node_idx = qwc.capture_index("template_func_node");
        let template_class_name_idx = qwc.capture_index("template_class_name");
        let template_class_node_idx = qwc.capture_index("template_class_node");

        let cpp_def_captures: &[(Option<u32>, Option<u32>, SymbolKind)] = &[
            (func_name_idx, func_node_idx, SymbolKind::Function),
            (method_name_idx, method_node_idx, SymbolKind::Function),
            (class_name_idx, class_node_idx, SymbolKind::Class),
            (struct_name_idx, struct_node_idx, SymbolKind::Class),
            (enum_name_idx, enum_node_idx, SymbolKind::Class),
            (namespace_name_idx, namespace_node_idx, SymbolKind::Module),
            (alias_name_idx, alias_node_idx, SymbolKind::TypeAlias),
            (
                template_func_name_idx,
                template_func_node_idx,
                SymbolKind::Function,
            ),
            (
                template_class_name_idx,
                template_class_node_idx,
                SymbolKind::Class,
            ),
        ];

        for m in matches {
            for &(name_cap, node_cap, kind) in cpp_def_captures {
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
    fn test_cpp_system_include() {
        let e = engine();
        let result = e
            .parse_file("main.cpp", "#include <iostream>\n#include <vector>\n")
            .unwrap();
        assert_eq!(result.imports.len(), 2);
        assert_eq!(result.imports[0].source, "iostream");
        assert_eq!(result.imports[1].source, "vector");
    }

    #[test]
    fn test_cpp_local_include() {
        let e = engine();
        let result = e
            .parse_file("main.cpp", "#include \"service.hpp\"\n")
            .unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].source, "service.hpp");
        assert_eq!(result.imports[0].names[0].name, "service");
    }

    #[test]
    fn test_cpp_function_definition() {
        let e = engine();
        let source = r#"
int add(int a, int b) {
    return a + b;
}

std::string greet(const std::string& name) {
    return "Hello, " + name;
}
"#;
        let result = e.parse_file("math.cpp", source).unwrap();
        let def_names: Vec<&str> = result.definitions.iter().map(|d| d.name.as_str()).collect();
        assert!(
            def_names.contains(&"add"),
            "should detect add function; got: {:?}",
            def_names
        );
        assert!(
            def_names.contains(&"greet"),
            "should detect greet function; got: {:?}",
            def_names
        );
    }

    #[test]
    fn test_cpp_class_definition() {
        let e = engine();
        let source = r#"
class UserService {
public:
    void create_user(const std::string& name);
    bool delete_user(int id);
private:
    int count_;
};
"#;
        let result = e.parse_file("service.cpp", source).unwrap();
        let def_names: Vec<&str> = result.definitions.iter().map(|d| d.name.as_str()).collect();
        assert!(
            def_names.contains(&"UserService"),
            "should detect class UserService; got: {:?}",
            def_names
        );
    }

    #[test]
    fn test_cpp_struct_definition() {
        let e = engine();
        let source = r#"
struct Point {
    double x;
    double y;
};
"#;
        let result = e.parse_file("types.cpp", source).unwrap();
        let def_names: Vec<&str> = result.definitions.iter().map(|d| d.name.as_str()).collect();
        assert!(
            def_names.contains(&"Point"),
            "should detect struct Point; got: {:?}",
            def_names
        );
    }

    #[test]
    fn test_cpp_namespace_definition() {
        let e = engine();
        let source = r#"
namespace myapp {
    void init();
}
"#;
        let result = e.parse_file("app.cpp", source).unwrap();
        let def_names: Vec<&str> = result.definitions.iter().map(|d| d.name.as_str()).collect();
        assert!(
            def_names.contains(&"myapp"),
            "should detect namespace myapp; got: {:?}",
            def_names
        );
    }

    #[test]
    fn test_cpp_enum_definition() {
        let e = engine();
        let source = r#"
enum class Status {
    Active,
    Inactive,
    Deleted
};
"#;
        let result = e.parse_file("status.cpp", source).unwrap();
        let def_names: Vec<&str> = result.definitions.iter().map(|d| d.name.as_str()).collect();
        assert!(
            def_names.contains(&"Status"),
            "should detect enum class Status; got: {:?}",
            def_names
        );
    }

    #[test]
    fn test_cpp_using_alias() {
        let e = engine();
        let source = "using StringVec = std::vector<std::string>;\n";
        let result = e.parse_file("types.cpp", source).unwrap();
        let def_names: Vec<&str> = result.definitions.iter().map(|d| d.name.as_str()).collect();
        assert!(
            def_names.contains(&"StringVec"),
            "should detect using alias; got: {:?}",
            def_names
        );
    }

    #[test]
    fn test_cpp_simple_call() {
        let e = engine();
        let source = r#"
void foo() {
    auto result = process();
    std::cout << result << std::endl;
}
"#;
        let result = e.parse_file("main.cpp", source).unwrap();
        let callees: Vec<&str> = result
            .call_sites
            .iter()
            .map(|c| c.callee.as_str())
            .collect();
        assert!(
            callees.contains(&"process"),
            "should detect process call; got: {:?}",
            callees
        );
    }

    #[test]
    fn test_cpp_method_call() {
        let e = engine();
        let source = r#"
void bar(UserService& svc) {
    svc.create_user("Alice");
    svc.delete_user(42);
}
"#;
        let result = e.parse_file("main.cpp", source).unwrap();
        let callees: Vec<&str> = result
            .call_sites
            .iter()
            .map(|c| c.callee.as_str())
            .collect();
        assert!(
            callees.contains(&"create_user"),
            "should detect method call; got: {:?}",
            callees
        );
        assert!(
            callees.contains(&"delete_user"),
            "should detect method call; got: {:?}",
            callees
        );
    }

    #[test]
    fn test_cpp_qualified_call() {
        let e = engine();
        let source = r#"
void foo() {
    std::sort(v.begin(), v.end());
    std::transform(a.begin(), a.end(), b.begin(), op);
}
"#;
        let result = e.parse_file("algo.cpp", source).unwrap();
        let callees: Vec<&str> = result
            .call_sites
            .iter()
            .map(|c| c.callee.as_str())
            .collect();
        assert!(
            callees.contains(&"sort"),
            "should detect std::sort; got: {:?}",
            callees
        );
        assert!(
            callees.contains(&"transform"),
            "should detect std::transform; got: {:?}",
            callees
        );
    }

    #[test]
    fn test_cpp_assignment_from_call() {
        let e = engine();
        let source = r#"
void foo() {
    auto result = compute();
}
"#;
        let df = e.extract_data_flow("main.cpp", source).unwrap();
        assert!(
            !df.assignments.is_empty(),
            "should detect assignment from call"
        );
        assert_eq!(df.assignments[0].variable, "result");
        assert_eq!(df.assignments[0].callee, "compute");
    }

    #[test]
    fn test_cpp_language_detection() {
        assert_eq!(Language::from_path("main.cpp"), Language::Cpp);
        assert_eq!(Language::from_path("main.cc"), Language::Cpp);
        assert_eq!(Language::from_path("main.cxx"), Language::Cpp);
        assert_eq!(Language::from_path("header.hpp"), Language::Cpp);
        assert_eq!(Language::from_path("header.hxx"), Language::Cpp);
        assert_eq!(Language::from_path("header.hh"), Language::Cpp);
        assert_eq!(Language::from_path("header.h++"), Language::Cpp);
    }

    #[test]
    fn test_cpp_full_file() {
        let e = engine();
        let source = r#"
#include <iostream>
#include <vector>
#include "repository.hpp"

namespace api {

class UserService {
public:
    std::vector<User> list_users() {
        return repo_.find_all();
    }

    void create_user(const std::string& name) {
        repo_.insert(name);
    }
private:
    UserRepository repo_;
};

using UserList = std::vector<User>;

}
"#;
        let result = e.parse_file("service.cpp", source).unwrap();

        // Imports
        assert_eq!(result.imports.len(), 3);

        // Definitions
        let def_names: Vec<&str> = result.definitions.iter().map(|d| d.name.as_str()).collect();
        assert!(
            def_names.contains(&"api"),
            "namespace api; got: {:?}",
            def_names
        );
        assert!(
            def_names.contains(&"UserService"),
            "class UserService; got: {:?}",
            def_names
        );
        assert!(
            def_names.contains(&"UserList"),
            "using alias UserList; got: {:?}",
            def_names
        );

        // Call sites
        let callees: Vec<&str> = result
            .call_sites
            .iter()
            .map(|c| c.callee.as_str())
            .collect();
        assert!(
            callees.contains(&"find_all"),
            "find_all method call; got: {:?}",
            callees
        );
        assert!(
            callees.contains(&"insert"),
            "insert method call; got: {:?}",
            callees
        );
    }
}
