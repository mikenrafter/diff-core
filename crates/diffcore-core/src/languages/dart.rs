//! Dart extraction tests and language-detection helpers.
//!
//! Dart uses the generic fallback extractors (`extract_definitions_standard`
//! and `extract_minimal_imports`) because its definitions are fully covered
//! by the standard `.scm` tags convention in `queries/dart/definitions.scm`.
//! This module exists purely to house language tests that exercise the query
//! compilation path — catching wrong node-type names at test time rather
//! than silently at runtime.

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr
)]
mod tests {
    use crate::ast::Language;
    use crate::query_engine::shared_test_engine;
    use crate::types::SymbolKind;

    fn e() -> &'static crate::query_engine::QueryEngine {
        shared_test_engine()
    }

    // ── Language detection ────────────────────────────────────────────────

    #[test]
    fn test_dart_language_detection() {
        assert_eq!(Language::from_path("main.dart"), Language::Dart);
        assert_eq!(Language::from_path("lib/src/user.dart"), Language::Dart);
    }

    // ── Query compilation smoke-test ──────────────────────────────────────
    //
    // Parsing an empty file forces the query engine to compile the Dart
    // tree-sitter queries. If any capture name or node type in the .scm
    // files is wrong, this test panics with a QueryEngineError explaining
    // exactly which pattern failed — far better than a silent empty result
    // at runtime.

    #[test]
    fn test_dart_empty_file_parses_without_error() {
        let result = e().parse_file("empty.dart", "").unwrap();
        assert_eq!(result.language, Language::Dart);
        assert!(result.definitions.is_empty());
        assert!(result.imports.is_empty());
    }

    // ── Class definitions ─────────────────────────────────────────────────

    #[test]
    fn test_dart_class_definition() {
        let src = r#"
class User {
  final String name;
  User(this.name);
  void greet() { print(name); }
}
"#;
        let result = e().parse_file("user.dart", src).unwrap();
        let classes: Vec<_> = result
            .definitions
            .iter()
            .filter(|d| d.kind == SymbolKind::Class)
            .collect();
        assert!(
            !classes.is_empty(),
            "expected at least one class definition"
        );
        assert!(
            classes.iter().any(|d| d.name == "User"),
            "expected 'User' class; got: {:?}",
            classes
        );
    }

    // ── Top-level function definitions ────────────────────────────────────

    #[test]
    fn test_dart_top_level_function() {
        let src = "void main() { print('hello'); }\n";
        let result = e().parse_file("main.dart", src).unwrap();
        let fns: Vec<_> = result
            .definitions
            .iter()
            .filter(|d| d.kind == SymbolKind::Function)
            .collect();
        assert!(
            fns.iter().any(|d| d.name == "main"),
            "expected 'main' function; got: {:?}",
            fns
        );
    }

    // ── Enum definitions ──────────────────────────────────────────────────

    #[test]
    fn test_dart_enum_definition() {
        let src = "enum Color { red, green, blue }\n";
        let result = e().parse_file("color.dart", src).unwrap();
        // Enums map to Struct in the standard convention.
        let found = result.definitions.iter().any(|d| d.name == "Color");
        assert!(
            found,
            "expected 'Color' enum; got: {:?}",
            result.definitions
        );
    }

    // ── Imports ───────────────────────────────────────────────────────────

    #[test]
    fn test_dart_import() {
        let src = "import 'dart:core';\nimport 'package:http/http.dart';\n";
        let result = e().parse_file("app.dart", src).unwrap();
        assert_eq!(
            result.imports.len(),
            2,
            "expected 2 imports; got: {:?}",
            result.imports.iter().map(|i| &i.source).collect::<Vec<_>>()
        );
        assert!(result
            .imports
            .iter()
            .any(|i| i.source.contains("dart:core")));
        assert!(result.imports.iter().any(|i| i.source.contains("http")));
    }
}
