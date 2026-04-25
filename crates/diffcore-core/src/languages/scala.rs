//! Scala extraction code.
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

    let mut imports = Vec::new();

    for m in &matches {
        let stmt_node = match m.get_capture(stmt_idx) {
            Some(n) => n,
            None => continue,
        };
        let line = stmt_node.start_position().row + 1;

        // Walk import_declaration children to reconstruct the dotted path.
        // Children: `import`, identifier, `.`, identifier, `.`, ..., [namespace_selectors | namespace_wildcard]
        let mut path_parts: Vec<String> = Vec::new();
        let mut named_imports: Vec<String> = Vec::new();
        let mut is_wildcard = false;

        let mut child_cursor = stmt_node.walk();
        for child in stmt_node.children(&mut child_cursor) {
            match child.kind() {
                "identifier" => {
                    path_parts.push(node_text(&child, source).to_string());
                }
                "namespace_selectors" => {
                    // { A, B } — collect named imports from inside selectors
                    let mut sel_cursor = child.walk();
                    for sel_child in child.children(&mut sel_cursor) {
                        if sel_child.kind() == "identifier" {
                            named_imports.push(node_text(&sel_child, source).to_string());
                        }
                    }
                }
                "namespace_wildcard" => {
                    is_wildcard = true;
                }
                _ => {} // skip `import`, `.`, etc.
            }
        }

        if path_parts.is_empty() {
            continue;
        }

        if is_wildcard {
            // import com.example._ → source = "com.example", namespace import
            let pkg = path_parts.join(".");
            imports.push(ImportInfo {
                source: pkg,
                names: vec![ImportedName {
                    name: "*".to_string(),
                    alias: None,
                }],
                is_default: false,
                is_namespace: true,
                line,
            });
        } else if !named_imports.is_empty() {
            // import akka.actor.{ActorSystem, Props} → source = "akka.actor"
            let pkg = path_parts.join(".");
            let names = named_imports
                .into_iter()
                .map(|n| ImportedName {
                    name: n,
                    alias: None,
                })
                .collect();
            imports.push(ImportInfo {
                source: pkg,
                names,
                is_default: false,
                is_namespace: false,
                line,
            });
        } else {
            // import com.example.Foo → source = "com.example", name = "Foo"
            let class_name = path_parts.last().cloned().unwrap_or_default();
            let pkg = if path_parts.len() > 1 {
                path_parts[..path_parts.len() - 1].join(".")
            } else {
                class_name.clone()
            };
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
        let func_name_idx = qwc.capture_index("func_name");
        let func_node_idx = qwc.capture_index("func_node");
        let class_name_idx = qwc.capture_index("class_name");
        let class_node_idx = qwc.capture_index("class_node");
        let trait_name_idx = qwc.capture_index("trait_name");
        let trait_node_idx = qwc.capture_index("trait_node");
        let object_name_idx = qwc.capture_index("object_name");
        let object_node_idx = qwc.capture_index("object_node");
        let prop_name_idx = qwc.capture_index("prop_name");
        let prop_node_idx = qwc.capture_index("prop_node");
        let typealias_name_idx = qwc.capture_index("typealias_name");
        let typealias_node_idx = qwc.capture_index("typealias_node");

        let scala_def_captures: &[(Option<u32>, Option<u32>, SymbolKind)] = &[
            (func_name_idx, func_node_idx, SymbolKind::Function),
            (class_name_idx, class_node_idx, SymbolKind::Class),
            (trait_name_idx, trait_node_idx, SymbolKind::Interface),
            (object_name_idx, object_node_idx, SymbolKind::Class),
            (prop_name_idx, prop_node_idx, SymbolKind::Constant),
            (
                typealias_name_idx,
                typealias_node_idx,
                SymbolKind::TypeAlias,
            ),
        ];

        for m in matches {
            for &(name_cap, node_cap, kind) in scala_def_captures {
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
    fn test_scala_language_detection() {
        assert_eq!(
            Language::from_path("src/main/scala/App.scala"),
            Language::Scala
        );
        assert_eq!(Language::from_path("build.sc"), Language::Scala);
        assert_eq!(
            Language::from_path("src/test/scala/AppTest.scala"),
            Language::Scala
        );
    }

    #[test]
    fn test_scala_imports() {
        let e = engine();
        let result = e
            .parse_file(
                "src/main/scala/App.scala",
                r#"import scala.collection.mutable
import akka.actor.{ActorSystem, Props}
import play.api.mvc._
import com.example.UserService

object Main {
  val system = ActorSystem("test")
}
"#,
            )
            .unwrap();
        assert_eq!(result.language, Language::Scala);

        let import_sources: Vec<&str> = result.imports.iter().map(|i| i.source.as_str()).collect();
        assert!(
            import_sources.contains(&"scala.collection"),
            "should extract scala.collection import; got: {:?}",
            import_sources
        );
        assert!(
            import_sources.contains(&"akka.actor"),
            "should extract akka.actor import; got: {:?}",
            import_sources
        );

        // Named imports {ActorSystem, Props}
        let akka_import = result
            .imports
            .iter()
            .find(|i| i.source == "akka.actor")
            .unwrap();
        let names: Vec<&str> = akka_import.names.iter().map(|n| n.name.as_str()).collect();
        assert!(
            names.contains(&"ActorSystem"),
            "should have ActorSystem; got: {:?}",
            names
        );
        assert!(
            names.contains(&"Props"),
            "should have Props; got: {:?}",
            names
        );

        // Wildcard import
        let wildcard_import = result.imports.iter().find(|i| i.source == "play.api.mvc");
        assert!(
            wildcard_import.is_some(),
            "should detect play.api.mvc wildcard import; imports: {:?}",
            result.imports
        );
        assert!(wildcard_import.unwrap().is_namespace);

        // Regular import
        assert!(
            import_sources.contains(&"com.example"),
            "should extract com.example import; got: {:?}",
            import_sources
        );
    }

    #[test]
    fn test_scala_definitions() {
        let e = engine();
        let result = e
            .parse_file(
                "src/main/scala/models/User.scala",
                r#"package com.example

class UserService {
  def getUser(id: String): User = ???
  val maxRetries: Int = 3
}

trait Repository[T] {
  def findById(id: String): Option[T]
}

object UserService {
  def apply(): UserService = new UserService()
}

case class User(name: String, email: String)

sealed trait Shape

type UserId = String
"#,
            )
            .unwrap();
        assert_eq!(result.language, Language::Scala);

        let def_names: Vec<&str> = result.definitions.iter().map(|d| d.name.as_str()).collect();
        assert!(
            def_names.contains(&"UserService"),
            "should detect class; got: {:?}",
            def_names
        );
        assert!(
            def_names.contains(&"getUser"),
            "should detect def; got: {:?}",
            def_names
        );
        assert!(
            def_names.contains(&"maxRetries"),
            "should detect val; got: {:?}",
            def_names
        );
        assert!(
            def_names.contains(&"Repository"),
            "should detect trait; got: {:?}",
            def_names
        );
        assert!(
            def_names.contains(&"findById"),
            "should detect abstract def; got: {:?}",
            def_names
        );
        assert!(
            def_names.contains(&"apply"),
            "should detect object method; got: {:?}",
            def_names
        );
        assert!(
            def_names.contains(&"User"),
            "should detect case class; got: {:?}",
            def_names
        );
        assert!(
            def_names.contains(&"Shape"),
            "should detect sealed trait; got: {:?}",
            def_names
        );
        assert!(
            def_names.contains(&"UserId"),
            "should detect type alias; got: {:?}",
            def_names
        );

        // Verify kinds
        let user_service = result
            .definitions
            .iter()
            .find(|d| d.name == "UserService")
            .unwrap();
        assert_eq!(user_service.kind, SymbolKind::Class);

        let repository = result
            .definitions
            .iter()
            .find(|d| d.name == "Repository")
            .unwrap();
        assert_eq!(repository.kind, SymbolKind::Interface);

        let user_id = result
            .definitions
            .iter()
            .find(|d| d.name == "UserId")
            .unwrap();
        assert_eq!(user_id.kind, SymbolKind::TypeAlias);
    }

    #[test]
    fn test_scala_call_sites() {
        let e = engine();
        let result = e
            .parse_file(
                "src/main/scala/services/UserService.scala",
                r#"import com.example.repositories.UserRepository

class UserService(repo: UserRepository) {
  def findAll(): List[User] = {
    val users = repo.findAll()
    users.map(_.toString())
  }

  def create(name: String): User = {
    val result = repo.save(name)
    println("Created")
    result
  }
}
"#,
            )
            .unwrap();
        assert_eq!(result.language, Language::Scala);

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
    fn test_scala_data_flow() {
        let e = engine();
        let dfi = e
            .extract_data_flow(
                "src/main/scala/services/UserService.scala",
                r#"class UserService(repo: UserRepository) {
  def findAll(): List[User] = {
    val users = repo.findAll()
    users
  }
}
"#,
            )
            .unwrap();

        assert!(
            !dfi.assignments.is_empty(),
            "should detect val assignment from method call; got: {:?}",
            dfi.assignments
        );
        let assign = dfi.assignments.iter().find(|a| a.variable == "users");
        assert!(
            assign.is_some(),
            "should find 'users' assignment; got: {:?}",
            dfi.assignments
        );
    }

    #[test]
    fn test_scala_full_file() {
        let e = engine();
        let source = r#"
import akka.actor.{ActorSystem, Props}
import akka.http.scaladsl.Http
import com.example.services.UserService
import com.example.repositories.UserRepository

object Main {
  def main(args: Array[String]): Unit = {
    val system = ActorSystem("test")
    val repo = UserRepository()
    val service = UserService(repo)
    val result = service.findAll()
    println(result)
    Http().newServerAt("0.0.0.0", 8080).bind(routes)
  }

  def routes(): Route = {
    get(complete("ok"))
  }
}

case class User(id: String, name: String)

type UserId = String
"#;
        let result = e.parse_file("src/main/scala/Main.scala", source).unwrap();

        // Imports
        assert_eq!(result.imports.len(), 4);
        let import_sources: Vec<&str> = result.imports.iter().map(|i| i.source.as_str()).collect();
        assert!(import_sources.contains(&"akka.actor"));
        assert!(import_sources.contains(&"akka.http.scaladsl"));
        assert!(import_sources.contains(&"com.example.services"));
        assert!(import_sources.contains(&"com.example.repositories"));

        // Definitions
        let def_names: Vec<&str> = result.definitions.iter().map(|d| d.name.as_str()).collect();
        assert!(
            def_names.contains(&"Main"),
            "object Main; got: {:?}",
            def_names
        );
        assert!(
            def_names.contains(&"main"),
            "def main; got: {:?}",
            def_names
        );
        assert!(
            def_names.contains(&"routes"),
            "def routes; got: {:?}",
            def_names
        );
        assert!(
            def_names.contains(&"User"),
            "case class User; got: {:?}",
            def_names
        );
        assert!(
            def_names.contains(&"UserId"),
            "type alias UserId; got: {:?}",
            def_names
        );

        // Call sites
        let callees: Vec<&str> = result
            .call_sites
            .iter()
            .map(|c| c.callee.as_str())
            .collect();
        assert!(
            callees.contains(&"ActorSystem"),
            "ActorSystem call; got: {:?}",
            callees
        );
        assert!(
            callees.contains(&"UserRepository"),
            "UserRepository call; got: {:?}",
            callees
        );
        assert!(
            callees.contains(&"UserService"),
            "UserService call; got: {:?}",
            callees
        );
        assert!(
            callees.contains(&"findAll"),
            "findAll call; got: {:?}",
            callees
        );
        assert!(
            callees.contains(&"println"),
            "println call; got: {:?}",
            callees
        );
    }
}
