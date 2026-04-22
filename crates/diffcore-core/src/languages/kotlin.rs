//! Kotlin extraction code.
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
    let alias_source_idx = qwc.capture_index("alias_source");
    let alias_name_idx = qwc.capture_index("alias_name");
    let wildcard_idx = qwc.capture_index("wildcard");
    let wildcard_source_idx = qwc.capture_index("wildcard_source");

    let mut imports = Vec::new();
    let mut seen: Vec<(usize, String)> = Vec::new();

    // First pass: collect lines that have wildcard or aliased imports
    let mut wildcard_lines: Vec<usize> = Vec::new();
    let mut alias_lines: Vec<usize> = Vec::new();
    for m in &matches {
        let line = m
            .get_capture(stmt_idx)
            .map(|n| n.start_position().row + 1)
            .unwrap_or(0);
        if m.has_capture(wildcard_idx) {
            wildcard_lines.push(line);
        }
        if m.has_capture(alias_name_idx) {
            alias_lines.push(line);
        }
    }

    for m in &matches {
        let line = m
            .get_capture(stmt_idx)
            .map(|n| n.start_position().row + 1)
            .unwrap_or(0);

        if m.has_capture(wildcard_idx) {
            // import com.example.*
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
        } else if m.has_capture(alias_name_idx) {
            // import com.example.Foo as Bar
            let source_text = m
                .get_capture(alias_source_idx)
                .map(|n| node_text(&n, source).to_string())
                .unwrap_or_default();
            let alias_text = m
                .get_capture(alias_name_idx)
                .map(|n| node_text(&n, source).to_string())
                .unwrap_or_default();
            if !source_text.is_empty() {
                let class_name = source_text
                    .rsplit('.')
                    .next()
                    .unwrap_or(&source_text)
                    .to_string();
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
                            alias: if alias_text.is_empty() {
                                None
                            } else {
                                Some(alias_text)
                            },
                        }],
                        is_default: false,
                        is_namespace: false,
                        line,
                    });
                }
            }
        } else if m.has_capture(source_idx)
            && !wildcard_lines.contains(&line)
            && !alias_lines.contains(&line)
        {
            // import com.example.Foo
            let source_text = m
                .get_capture(source_idx)
                .map(|n| node_text(&n, source).to_string())
                .unwrap_or_default();
            if !source_text.is_empty() {
                let class_name = source_text
                    .rsplit('.')
                    .next()
                    .unwrap_or(&source_text)
                    .to_string();
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
    let object_name_idx = qwc.capture_index("object_name");
    let object_node_idx = qwc.capture_index("object_node");
    let prop_name_idx = qwc.capture_index("prop_name");
    let prop_node_idx = qwc.capture_index("prop_node");
    let typealias_name_idx = qwc.capture_index("typealias_name");
    let typealias_node_idx = qwc.capture_index("typealias_node");

    let kotlin_def_captures: &[(Option<u32>, Option<u32>, SymbolKind)] = &[
        (func_name_idx, func_node_idx, SymbolKind::Function),
        (class_name_idx, class_node_idx, SymbolKind::Class),
        (object_name_idx, object_node_idx, SymbolKind::Class),
        (prop_name_idx, prop_node_idx, SymbolKind::Constant),
        (
            typealias_name_idx,
            typealias_node_idx,
            SymbolKind::TypeAlias,
        ),
    ];

    for m in matches {
        for &(name_cap, node_cap, kind) in kotlin_def_captures {
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
    fn test_kotlin_language_detection() {
        assert_eq!(
            Language::from_path("src/main/kotlin/App.kt"),
            Language::Kotlin
        );
        assert_eq!(Language::from_path("build.gradle.kts"), Language::Kotlin);
        assert_eq!(
            Language::from_path("src/test/kotlin/AppTest.kt"),
            Language::Kotlin
        );
    }

    #[test]
    fn test_kotlin_imports() {
        let e = engine();
        let result = e
            .parse_file(
                "src/main/kotlin/App.kt",
                r#"import io.ktor.server.routing.get
import com.example.services.UserService
import com.example.models.*
import org.jetbrains.exposed.sql.Database as DB

fun main() {
    val service = UserService()
}
"#,
            )
            .unwrap();
        assert_eq!(result.language, Language::Kotlin);

        let import_sources: Vec<&str> = result.imports.iter().map(|i| i.source.as_str()).collect();
        assert!(
            import_sources.contains(&"io.ktor.server.routing"),
            "should extract Ktor import package; got: {:?}",
            import_sources
        );
        assert!(
            import_sources.contains(&"com.example.services"),
            "should extract service import package; got: {:?}",
            import_sources
        );

        // Wildcard import
        let wildcard_import = result
            .imports
            .iter()
            .find(|i| i.names.iter().any(|n| n.name == "*"));
        assert!(
            wildcard_import.is_some(),
            "should detect wildcard import; imports: {:?}",
            result.imports
        );

        // Aliased import
        let aliased_import = result
            .imports
            .iter()
            .find(|i| i.names.iter().any(|n| n.alias.as_deref() == Some("DB")));
        assert!(
            aliased_import.is_some(),
            "should detect aliased import (Database as DB); imports: {:?}",
            result.imports
        );
    }

    #[test]
    fn test_kotlin_definitions() {
        let e = engine();
        let result = e
            .parse_file(
                "src/main/kotlin/models/User.kt",
                r#"import kotlinx.serialization.Serializable

data class User(
    val id: String,
    val name: String
)

object UserFactory {
    fun create(name: String): User {
        return User("1", name)
    }
}

fun greet(user: User): String {
    return "Hello, ${user.name}"
}

val DEFAULT_NAME = "World"

typealias UserId = String
"#,
            )
            .unwrap();
        assert_eq!(result.language, Language::Kotlin);

        let def_names: Vec<&str> = result.definitions.iter().map(|d| d.name.as_str()).collect();
        assert!(
            def_names.contains(&"User"),
            "should detect data class; got: {:?}",
            def_names
        );
        assert!(
            def_names.contains(&"UserFactory"),
            "should detect object; got: {:?}",
            def_names
        );
        assert!(
            def_names.contains(&"create"),
            "should detect function in object; got: {:?}",
            def_names
        );
        assert!(
            def_names.contains(&"greet"),
            "should detect top-level function; got: {:?}",
            def_names
        );
        assert!(
            def_names.contains(&"DEFAULT_NAME"),
            "should detect val property; got: {:?}",
            def_names
        );
        assert!(
            def_names.contains(&"UserId"),
            "should detect typealias; got: {:?}",
            def_names
        );
    }

    #[test]
    fn test_kotlin_call_sites() {
        let e = engine();
        let result = e
            .parse_file(
                "src/main/kotlin/services/UserService.kt",
                r#"import com.example.repositories.UserRepository

class UserService(private val repo: UserRepository) {
    fun findAll(): List<String> {
        val users = repo.findAll()
        return users.map { it.toString() }
    }

    fun create(name: String): String {
        val result = repo.save(name)
        println("Created: $result")
        return result
    }
}
"#,
            )
            .unwrap();
        assert_eq!(result.language, Language::Kotlin);

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
            callees.contains(&"save"),
            "should detect repo.save call; got: {:?}",
            callees
        );
        assert!(
            callees.contains(&"println"),
            "should detect println call; got: {:?}",
            callees
        );
    }

    #[test]
    fn test_kotlin_data_flow() {
        let e = engine();
        let dfi = e
            .extract_data_flow(
                "src/main/kotlin/services/UserService.kt",
                r#"import com.example.repositories.UserRepository

class UserService(private val repo: UserRepository) {
    fun findAll(): List<String> {
        val users = repo.findAll()
        return users
    }
}
"#,
            )
            .unwrap();

        // Should detect val users = repo.findAll() as an assignment
        let assign_vars: Vec<&str> = dfi
            .assignments
            .iter()
            .map(|a| a.variable.as_str())
            .collect();
        assert!(
            assign_vars.contains(&"users"),
            "should detect val assignment from call; got: {:?}",
            assign_vars
        );
    }

    #[test]
    fn test_kotlin_full_parsing() {
        let e = engine();
        let source = r#"import io.ktor.server.routing.get
import io.ktor.server.routing.post
import io.ktor.server.response.respond
import com.example.services.UserService

fun Route.userRoutes(userService: UserService) {
    get("/users") {
        val users = userService.findAll()
        call.respond(users)
    }

    post("/users") {
        val user = userService.create(call)
        call.respond(user)
    }
}
"#;
        let result = e
            .parse_file("src/main/kotlin/routes/UserRoutes.kt", source)
            .unwrap();
        assert_eq!(result.language, Language::Kotlin);

        // Imports
        let import_sources: Vec<&str> = result.imports.iter().map(|i| i.source.as_str()).collect();
        assert!(import_sources.contains(&"io.ktor.server.routing"));
        assert!(import_sources.contains(&"io.ktor.server.response"));
        assert!(import_sources.contains(&"com.example.services"));

        // Definitions
        let def_names: Vec<&str> = result.definitions.iter().map(|d| d.name.as_str()).collect();
        assert!(
            def_names.contains(&"userRoutes"),
            "should detect extension function; got: {:?}",
            def_names
        );

        // Call sites
        let callees: Vec<&str> = result
            .call_sites
            .iter()
            .map(|c| c.callee.as_str())
            .collect();
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
            callees.contains(&"findAll"),
            "should detect service call; got: {:?}",
            callees
        );
        assert!(
            callees.contains(&"respond"),
            "should detect respond call; got: {:?}",
            callees
        );
    }

}
