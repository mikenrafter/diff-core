//! Swift extraction code.
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
            // import Foundation / import Vapor
            // The identifier node contains the full module path as text
            let source_text = m
                .get_capture(source_idx)
                .map(|n| node_text(&n, source).to_string())
                .unwrap_or_default();
            if !source_text.is_empty() {
                let key = (line, source_text.clone());
                if !seen.contains(&key) {
                    seen.push(key);
                    // Swift imports are module-level: `import Foundation`
                    // The module name is the source, imported name is the module itself
                    let module_name = source_text
                        .rsplit('.')
                        .next()
                        .unwrap_or(&source_text)
                        .to_string();
                    imports.push(ImportInfo {
                        source: source_text,
                        names: vec![ImportedName {
                            name: module_name,
                            alias: None,
                        }],
                        is_default: false,
                        is_namespace: true, // Swift imports bring the whole module into scope
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
    let class_name_idx = qwc.capture_index("class_name");
    let class_node_idx = qwc.capture_index("class_node");
    let protocol_name_idx = qwc.capture_index("protocol_name");
    let protocol_node_idx = qwc.capture_index("protocol_node");
    let proto_func_name_idx = qwc.capture_index("proto_func_name");
    let proto_func_node_idx = qwc.capture_index("proto_func_node");
    let prop_name_idx = qwc.capture_index("prop_name");
    let prop_node_idx = qwc.capture_index("prop_node");
    let typealias_name_idx = qwc.capture_index("typealias_name");
    let typealias_node_idx = qwc.capture_index("typealias_node");

    let swift_def_captures: &[(Option<u32>, Option<u32>, SymbolKind)] = &[
        (func_name_idx, func_node_idx, SymbolKind::Function),
        (class_name_idx, class_node_idx, SymbolKind::Class),
        (protocol_name_idx, protocol_node_idx, SymbolKind::Interface),
        (
            proto_func_name_idx,
            proto_func_node_idx,
            SymbolKind::Function,
        ),
        (prop_name_idx, prop_node_idx, SymbolKind::Constant),
        (
            typealias_name_idx,
            typealias_node_idx,
            SymbolKind::TypeAlias,
        ),
    ];

    for m in matches {
        for &(name_cap, node_cap, kind) in swift_def_captures {
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
    fn test_swift_language_detection() {
        assert_eq!(
            Language::from_path("Sources/App/main.swift"),
            Language::Swift
        );
        assert_eq!(
            Language::from_path("Tests/AppTests/AppTests.swift"),
            Language::Swift
        );
        assert_eq!(Language::from_path("Package.swift"), Language::Swift);
    }

    #[test]
    fn test_swift_imports() {
        let e = engine();
        let result = e
            .parse_file(
                "Sources/App/Controllers/UserController.swift",
                r#"import Foundation
import Vapor

class UserController {
    func index(req: Request) throws -> EventLoopFuture<[User]> {
        return User.query(on: req.db).all()
    }
}
"#,
            )
            .unwrap();
        assert_eq!(result.language, Language::Swift);

        let import_sources: Vec<&str> = result.imports.iter().map(|i| i.source.as_str()).collect();
        assert!(
            import_sources.contains(&"Foundation"),
            "should detect Foundation import; got: {:?}",
            import_sources
        );
        assert!(
            import_sources.contains(&"Vapor"),
            "should detect Vapor import; got: {:?}",
            import_sources
        );
    }

    #[test]
    fn test_swift_definitions() {
        let e = engine();
        let result = e
            .parse_file(
                "Sources/App/Models/User.swift",
                r#"import Foundation

struct User: Content {
    var id: UUID?
    var name: String
}

class UserService {
    func findAll() -> [User] {
        return []
    }
}

protocol UserRepository {
    func findById(id: UUID) -> User?
}

enum UserRole {
    case admin
    case regular
}

let defaultName = "World"

typealias UserID = UUID

func greet(name: String) -> String {
    return "Hello, \(name)!"
}
"#,
            )
            .unwrap();
        assert_eq!(result.language, Language::Swift);

        let def_names: Vec<&str> = result.definitions.iter().map(|d| d.name.as_str()).collect();
        assert!(
            def_names.contains(&"User"),
            "should detect struct; got: {:?}",
            def_names
        );
        assert!(
            def_names.contains(&"UserService"),
            "should detect class; got: {:?}",
            def_names
        );
        assert!(
            def_names.contains(&"findAll"),
            "should detect method; got: {:?}",
            def_names
        );
        assert!(
            def_names.contains(&"UserRepository"),
            "should detect protocol; got: {:?}",
            def_names
        );
        assert!(
            def_names.contains(&"findById"),
            "should detect protocol function; got: {:?}",
            def_names
        );
        assert!(
            def_names.contains(&"UserRole"),
            "should detect enum; got: {:?}",
            def_names
        );
        assert!(
            def_names.contains(&"defaultName"),
            "should detect let property; got: {:?}",
            def_names
        );
        assert!(
            def_names.contains(&"UserID"),
            "should detect typealias; got: {:?}",
            def_names
        );
        assert!(
            def_names.contains(&"greet"),
            "should detect top-level function; got: {:?}",
            def_names
        );
    }

    #[test]
    fn test_swift_call_sites() {
        let e = engine();
        let result = e
            .parse_file(
                "Sources/App/Controllers/UserController.swift",
                r#"import Vapor

class UserController {
    func index(req: Request) throws -> [User] {
        let users = service.findAll()
        return users.map { $0.toDTO() }
    }

    func create(req: Request) throws -> User {
        let dto = try req.content.decode(UserDTO.self)
        let user = User(name: dto.name)
        try user.save(on: req.db)
        return user
    }
}
"#,
            )
            .unwrap();
        assert_eq!(result.language, Language::Swift);

        let callees: Vec<&str> = result
            .call_sites
            .iter()
            .map(|c| c.callee.as_str())
            .collect();
        assert!(
            callees.contains(&"findAll"),
            "should detect method call; got: {:?}",
            callees
        );
        assert!(
            callees.contains(&"decode"),
            "should detect decode call; got: {:?}",
            callees
        );
        assert!(
            callees.contains(&"save"),
            "should detect save call; got: {:?}",
            callees
        );
    }

    #[test]
    fn test_swift_data_flow() {
        let e = engine();
        let dfi = e
            .extract_data_flow(
                "Sources/App/Controllers/UserController.swift",
                r#"import Vapor

func index(req: Request) throws -> [User] {
    let users = service.findAll()
    let result = transform(users)
    return result
}
"#,
            )
            .unwrap();

        let assign_vars: Vec<&str> = dfi
            .assignments
            .iter()
            .map(|a| a.variable.as_str())
            .collect();
        assert!(
            assign_vars.contains(&"users"),
            "should detect let assignment from method call; got: {:?}",
            assign_vars
        );
        assert!(
            assign_vars.contains(&"result"),
            "should detect let assignment from function call; got: {:?}",
            assign_vars
        );
    }

    #[test]
    fn test_swift_full_parsing() {
        let e = engine();
        let source = r#"import Foundation
import Vapor
import Fluent

struct UserController: RouteCollection {
    func boot(routes: RoutesBuilder) throws {
        let users = routes.grouped("users")
        users.get(use: index)
        users.post(use: create)
    }

    func index(req: Request) throws -> EventLoopFuture<[User]> {
        return User.query(on: req.db).all()
    }

    func create(req: Request) throws -> EventLoopFuture<User> {
        let user = try req.content.decode(User.self)
        return user.save(on: req.db).map { user }
    }
}

typealias UserID = UUID
"#;
        let result = e
            .parse_file("Sources/App/Controllers/UserController.swift", source)
            .unwrap();
        assert_eq!(result.language, Language::Swift);

        // Imports
        let import_sources: Vec<&str> = result.imports.iter().map(|i| i.source.as_str()).collect();
        assert!(import_sources.contains(&"Foundation"));
        assert!(import_sources.contains(&"Vapor"));
        assert!(import_sources.contains(&"Fluent"));

        // Definitions
        let def_names: Vec<&str> = result.definitions.iter().map(|d| d.name.as_str()).collect();
        assert!(
            def_names.contains(&"UserController"),
            "should detect struct; got: {:?}",
            def_names
        );
        assert!(
            def_names.contains(&"boot"),
            "should detect boot function; got: {:?}",
            def_names
        );
        assert!(
            def_names.contains(&"index"),
            "should detect index function; got: {:?}",
            def_names
        );
        assert!(
            def_names.contains(&"create"),
            "should detect create function; got: {:?}",
            def_names
        );
        assert!(
            def_names.contains(&"UserID"),
            "should detect typealias; got: {:?}",
            def_names
        );

        // Call sites
        let callees: Vec<&str> = result
            .call_sites
            .iter()
            .map(|c| c.callee.as_str())
            .collect();
        assert!(
            callees.contains(&"grouped"),
            "should detect grouped call; got: {:?}",
            callees
        );
        assert!(
            callees.contains(&"get"),
            "should detect get route; got: {:?}",
            callees
        );
        assert!(
            callees.contains(&"post"),
            "should detect post route; got: {:?}",
            callees
        );
        assert!(
            callees.contains(&"all"),
            "should detect query().all(); got: {:?}",
            callees
        );
    }

}
