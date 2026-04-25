use super::*;
use crate::ast::{self, ParsedFile};
use crate::types::SymbolKind;

/// Helper: parse multiple files and build a graph.
fn build_graph_from_sources(files: &[(&str, &str)]) -> SymbolGraph {
    let parsed: Vec<ParsedFile> = files
        .iter()
        .map(|(path, source)| ast::parse_file(path, source).unwrap())
        .collect();
    SymbolGraph::build(&parsed)
}

/// Helper: check if an edge exists between two symbol ids with a given type.
fn has_edge(graph: &SymbolGraph, from: &str, to: &str, edge_type: &EdgeType) -> bool {
    graph
        .edges()
        .iter()
        .any(|(f, t, et)| *f == from && *t == to && *et == edge_type)
}

/// Helper: count edges of a specific type.
fn count_edges_of_type(graph: &SymbolGraph, edge_type: &EdgeType) -> usize {
    graph
        .edges()
        .iter()
        .filter(|(_, _, et)| *et == edge_type)
        .count()
}

    // =======================================================================
    // Helper function unit tests
    // =======================================================================

    mod helper_tests {
        use super::*;

        // --- normalize_path ---

        #[test]
        fn test_normalize_path_simple() {
            assert_eq!(normalize_path("src/utils.ts"), "src/utils.ts");
        }

        #[test]
        fn test_normalize_path_dot_segments() {
            assert_eq!(normalize_path("src/./utils.ts"), "src/utils.ts");
        }

        #[test]
        fn test_normalize_path_dotdot_segments() {
            assert_eq!(normalize_path("src/handlers/../utils.ts"), "src/utils.ts");
        }

        #[test]
        fn test_normalize_path_multiple_dotdot() {
            assert_eq!(normalize_path("src/a/b/../../utils.ts"), "src/utils.ts");
        }

        #[test]
        fn test_normalize_path_leading_dotdot() {
            // More `..` than components — pops everything available.
            assert_eq!(normalize_path("../utils.ts"), "utils.ts");
        }

        #[test]
        fn test_normalize_path_empty_segments() {
            assert_eq!(normalize_path("src//utils.ts"), "src/utils.ts");
        }

        #[test]
        fn test_normalize_path_only_dot() {
            assert_eq!(normalize_path("."), "");
        }

        #[test]
        fn test_normalize_path_trailing_slash() {
            assert_eq!(normalize_path("src/lib/"), "src/lib");
        }

        // --- normalize_python_import ---

        #[test]
        fn test_python_import_single_dot() {
            assert_eq!(normalize_python_import(".models"), "./models");
        }

        #[test]
        fn test_python_import_double_dot() {
            assert_eq!(normalize_python_import("..models"), "../models");
        }

        #[test]
        fn test_python_import_triple_dot() {
            assert_eq!(
                normalize_python_import("...utils.helpers"),
                "../../utils/helpers"
            );
        }

        #[test]
        fn test_python_import_dot_only() {
            assert_eq!(normalize_python_import("."), ".");
        }

        #[test]
        fn test_python_import_dotdot_only() {
            assert_eq!(normalize_python_import(".."), "..");
        }

        #[test]
        fn test_python_import_no_dots() {
            assert_eq!(normalize_python_import("os.path"), "os.path");
        }

        #[test]
        fn test_python_import_dotted_remainder() {
            assert_eq!(
                normalize_python_import(".models.user.schema"),
                "./models/user/schema"
            );
        }

        // --- parent_dir ---

        #[test]
        fn test_parent_dir_nested() {
            assert_eq!(parent_dir("src/handlers/auth.ts"), "src/handlers");
        }

        #[test]
        fn test_parent_dir_single_level() {
            assert_eq!(parent_dir("src/app.ts"), "src");
        }

        #[test]
        fn test_parent_dir_no_slash() {
            assert_eq!(parent_dir("app.ts"), ".");
        }

        // --- file_stem ---

        #[test]
        fn test_file_stem_simple() {
            assert_eq!(file_stem("src/utils.ts"), "utils");
        }

        #[test]
        fn test_file_stem_no_extension() {
            assert_eq!(file_stem("src/Makefile"), "Makefile");
        }

        #[test]
        fn test_file_stem_multiple_dots() {
            assert_eq!(file_stem("src/utils.test.ts"), "utils");
        }

        #[test]
        fn test_file_stem_no_directory() {
            assert_eq!(file_stem("app.ts"), "app");
        }

        // --- resolve_import_path ---

        #[test]
        fn test_resolve_import_exact_match() {
            // Note: resolve_import_path normalizes Python-style dots, so
            // explicit extensions like `./utils.ts` get mangled. Use
            // extension-less import sources (the normal JS/TS convention).
            let known = vec!["src/utils.ts"];
            let result = resolve_import_path("./utils", "src/handler.ts", &known);
            assert_eq!(result, Some("src/utils.ts".to_string()));
        }

        #[test]
        fn test_resolve_import_ts_extension() {
            let known = vec!["src/utils.ts"];
            let result = resolve_import_path("./utils", "src/handler.ts", &known);
            assert_eq!(result, Some("src/utils.ts".to_string()));
        }

        #[test]
        fn test_resolve_import_tsx_extension() {
            let known = vec!["src/Button.tsx"];
            let result = resolve_import_path("./Button", "src/App.tsx", &known);
            assert_eq!(result, Some("src/Button.tsx".to_string()));
        }

        #[test]
        fn test_resolve_import_index_file() {
            let known = vec!["src/lib/index.ts"];
            let result = resolve_import_path("./lib", "src/main.ts", &known);
            assert_eq!(result, Some("src/lib/index.ts".to_string()));
        }

        #[test]
        fn test_resolve_import_parent_dir() {
            let known = vec!["src/utils.ts"];
            let result = resolve_import_path("../utils", "src/handlers/auth.ts", &known);
            assert_eq!(result, Some("src/utils.ts".to_string()));
        }

        #[test]
        fn test_resolve_import_nonrelative_ignored() {
            let known = vec!["node_modules/express/index.js"];
            let result = resolve_import_path("express", "src/app.ts", &known);
            assert_eq!(result, None);
        }

        #[test]
        fn test_resolve_import_not_found() {
            let known = vec!["src/app.ts"];
            let result = resolve_import_path("./nonexistent", "src/main.ts", &known);
            assert_eq!(result, None);
        }

        #[test]
        fn test_resolve_import_python_style() {
            let known = vec!["models.py"];
            let result = resolve_import_path(".models", "views.py", &known);
            assert_eq!(result, Some("models.py".to_string()));
        }

        #[test]
        fn test_resolve_import_js_extension() {
            let known = vec!["src/helper.js"];
            let result = resolve_import_path("./helper", "src/main.ts", &known);
            assert_eq!(result, Some("src/helper.js".to_string()));
        }

        #[test]
        fn test_resolve_import_priority_exact_over_extension() {
            // If both exact match and .ts exist, exact match wins.
            let known = vec!["src/utils", "src/utils.ts"];
            let result = resolve_import_path("./utils", "src/main.ts", &known);
            assert_eq!(result, Some("src/utils".to_string()));
        }

        // --- resolve_workspace_import ---

        #[test]
        fn test_workspace_exact_package_match() {
            let known = vec!["packages/shared/src/index.ts"];
            let mut ws = WorkspaceMap::new();
            ws.insert(
                "@mono/shared".to_string(),
                "packages/shared/src/index.ts".to_string(),
            );
            let result = resolve_workspace_import("@mono/shared", &known, &ws);
            assert_eq!(result, Some("packages/shared/src/index.ts".to_string()));
        }

        #[test]
        fn test_workspace_package_not_in_known_files() {
            let known: Vec<&str> = vec!["src/app.ts"];
            let mut ws = WorkspaceMap::new();
            ws.insert(
                "@mono/shared".to_string(),
                "packages/shared/src/index.ts".to_string(),
            );
            let result = resolve_workspace_import("@mono/shared", &known, &ws);
            assert_eq!(result, None);
        }

        #[test]
        fn test_workspace_deep_import() {
            // @mono/shared/utils → packages/shared/utils.ts
            let known = vec!["packages/shared/utils.ts"];
            let mut ws = WorkspaceMap::new();
            ws.insert(
                "@mono/shared".to_string(),
                "packages/shared/src/index.ts".to_string(),
            );
            let result = resolve_workspace_import("@mono/shared/utils", &known, &ws);
            assert_eq!(result, Some("packages/shared/utils.ts".to_string()));
        }

        #[test]
        fn test_workspace_deep_import_with_extension() {
            let known = vec!["packages/shared/models/user.ts"];
            let mut ws = WorkspaceMap::new();
            ws.insert(
                "@mono/shared".to_string(),
                "packages/shared/src/index.ts".to_string(),
            );
            let result = resolve_workspace_import("@mono/shared/models/user", &known, &ws);
            assert_eq!(result, Some("packages/shared/models/user.ts".to_string()));
        }

        #[test]
        fn test_workspace_relative_import_skipped() {
            let known = vec!["packages/shared/src/index.ts"];
            let mut ws = WorkspaceMap::new();
            ws.insert(
                "@mono/shared".to_string(),
                "packages/shared/src/index.ts".to_string(),
            );
            let result = resolve_workspace_import("./utils", &known, &ws);
            assert_eq!(result, None);
        }

        #[test]
        fn test_workspace_no_match() {
            let known = vec!["src/app.ts"];
            let ws = WorkspaceMap::new();
            let result = resolve_workspace_import("@mono/shared", &known, &ws);
            assert_eq!(result, None);
        }

        #[test]
        fn test_workspace_longest_prefix_match() {
            // @mono/shared/sub should match @mono/shared, not @mono
            let known = vec!["packages/shared/sub.ts"];
            let mut ws = WorkspaceMap::new();
            ws.insert(
                "@mono".to_string(),
                "packages/mono/src/index.ts".to_string(),
            );
            ws.insert(
                "@mono/shared".to_string(),
                "packages/shared/src/index.ts".to_string(),
            );
            let result = resolve_workspace_import("@mono/shared/sub", &known, &ws);
            assert_eq!(result, Some("packages/shared/sub.ts".to_string()));
        }

        // --- resolve_import_or_workspace ---

        #[test]
        fn test_resolve_or_workspace_prefers_relative() {
            // Relative import should still work, even with workspace map.
            let known = vec!["src/utils.ts", "packages/utils/src/index.ts"];
            let mut ws = WorkspaceMap::new();
            ws.insert(
                "utils".to_string(),
                "packages/utils/src/index.ts".to_string(),
            );
            let result = resolve_import_or_workspace("./utils", "src/handler.ts", &known, &ws);
            assert_eq!(result, Some("src/utils.ts".to_string()));
        }

        #[test]
        fn test_resolve_or_workspace_falls_back_to_workspace() {
            let known = vec!["packages/types/src/index.ts"];
            let mut ws = WorkspaceMap::new();
            ws.insert(
                "@app/types".to_string(),
                "packages/types/src/index.ts".to_string(),
            );
            let result = resolve_import_or_workspace("@app/types", "src/handler.ts", &known, &ws);
            assert_eq!(result, Some("packages/types/src/index.ts".to_string()));
        }
    }

    // =======================================================================
    // IR extends edge tests
    // =======================================================================

    mod ir_extends_tests {
        use super::*;
        use crate::ast::Language;
        use crate::ir::{IrFile, IrImport, IrImportSpecifier, IrTypeDef, Span, TypeDefKind};

        fn empty_span() -> Span {
            Span::new(1, 1)
        }

        fn make_ir_file(path: &str, language: Language) -> IrFile {
            IrFile {
                path: path.to_string(),
                language,
                functions: vec![],
                type_defs: vec![],
                constants: vec![],
                imports: vec![],
                exports: vec![],
                call_expressions: vec![],
                assignments: vec![],
            }
        }

        #[test]
        fn test_ir_extends_local_class() {
            let mut file = make_ir_file("src/models.ts", Language::TypeScript);
            file.type_defs.push(IrTypeDef {
                name: "BaseModel".to_string(),
                kind: TypeDefKind::Class,
                span: empty_span(),
                bases: vec![],
                is_exported: true,
                decorators: vec![],
            });
            file.type_defs.push(IrTypeDef {
                name: "User".to_string(),
                kind: TypeDefKind::Class,
                span: empty_span(),
                bases: vec!["BaseModel".to_string()],
                is_exported: true,
                decorators: vec![],
            });

            let graph = SymbolGraph::build_from_ir(&[file]);

            assert!(
                has_edge(
                    &graph,
                    "src/models.ts::User",
                    "src/models.ts::BaseModel",
                    &EdgeType::Extends
                ),
                "should have extends edge from User to BaseModel"
            );
        }

        #[test]
        fn test_ir_extends_imported_class() {
            let mut base_file = make_ir_file("src/base.ts", Language::TypeScript);
            base_file.type_defs.push(IrTypeDef {
                name: "Entity".to_string(),
                kind: TypeDefKind::Class,
                span: empty_span(),
                bases: vec![],
                is_exported: true,
                decorators: vec![],
            });

            let mut child_file = make_ir_file("src/user.ts", Language::TypeScript);
            child_file.imports.push(IrImport {
                source: "./base".to_string(),
                specifiers: vec![IrImportSpecifier::Named {
                    name: "Entity".to_string(),
                    alias: None,
                }],
                span: empty_span(),
            });
            child_file.type_defs.push(IrTypeDef {
                name: "User".to_string(),
                kind: TypeDefKind::Class,
                span: empty_span(),
                bases: vec!["Entity".to_string()],
                is_exported: true,
                decorators: vec![],
            });

            let graph = SymbolGraph::build_from_ir(&[base_file, child_file]);

            assert!(
                has_edge(
                    &graph,
                    "src/user.ts::User",
                    "src/base.ts::Entity",
                    &EdgeType::Extends
                ),
                "should have extends edge to imported base class"
            );
        }

        #[test]
        fn test_ir_extends_multiple_bases() {
            let mut file = make_ir_file("src/mixin.ts", Language::TypeScript);
            file.type_defs.push(IrTypeDef {
                name: "Serializable".to_string(),
                kind: TypeDefKind::Interface,
                span: empty_span(),
                bases: vec![],
                is_exported: false,
                decorators: vec![],
            });
            file.type_defs.push(IrTypeDef {
                name: "Loggable".to_string(),
                kind: TypeDefKind::Interface,
                span: empty_span(),
                bases: vec![],
                is_exported: false,
                decorators: vec![],
            });
            file.type_defs.push(IrTypeDef {
                name: "UserService".to_string(),
                kind: TypeDefKind::Class,
                span: empty_span(),
                bases: vec!["Serializable".to_string(), "Loggable".to_string()],
                is_exported: true,
                decorators: vec![],
            });

            let graph = SymbolGraph::build_from_ir(&[file]);

            assert!(
                has_edge(
                    &graph,
                    "src/mixin.ts::UserService",
                    "src/mixin.ts::Serializable",
                    &EdgeType::Extends
                ),
                "should have extends edge to Serializable"
            );
            assert!(
                has_edge(
                    &graph,
                    "src/mixin.ts::UserService",
                    "src/mixin.ts::Loggable",
                    &EdgeType::Extends
                ),
                "should have extends edge to Loggable"
            );
        }

        #[test]
        fn test_ir_extends_no_self_edge() {
            let mut file = make_ir_file("src/app.ts", Language::TypeScript);
            file.type_defs.push(IrTypeDef {
                name: "App".to_string(),
                kind: TypeDefKind::Class,
                span: empty_span(),
                bases: vec!["App".to_string()],
                is_exported: false,
                decorators: vec![],
            });

            let graph = SymbolGraph::build_from_ir(&[file]);

            let self_edges: Vec<_> = graph
                .edges()
                .into_iter()
                .filter(|(f, t, _)| f == t)
                .collect();
            assert!(
                self_edges.is_empty(),
                "self-referencing base should not create self-edge"
            );
        }

        #[test]
        fn test_ir_extends_missing_base_no_panic() {
            let mut file = make_ir_file("src/app.ts", Language::TypeScript);
            file.type_defs.push(IrTypeDef {
                name: "App".to_string(),
                kind: TypeDefKind::Class,
                span: empty_span(),
                bases: vec!["NonExistent".to_string()],
                is_exported: false,
                decorators: vec![],
            });

            let graph = SymbolGraph::build_from_ir(&[file]);

            assert!(graph.get_node("src/app.ts::App").is_some());
            assert_eq!(
                count_edges_of_type(&graph, &EdgeType::Extends),
                0,
                "missing base should not create extends edge"
            );
        }

        #[test]
        fn test_ir_extends_empty_bases() {
            let mut file = make_ir_file("src/app.ts", Language::TypeScript);
            file.type_defs.push(IrTypeDef {
                name: "PlainClass".to_string(),
                kind: TypeDefKind::Class,
                span: empty_span(),
                bases: vec![],
                is_exported: false,
                decorators: vec![],
            });

            let graph = SymbolGraph::build_from_ir(&[file]);
            assert_eq!(count_edges_of_type(&graph, &EdgeType::Extends), 0);
        }

        #[test]
        fn test_ir_extends_cross_file_chain() {
            let mut file_a = make_ir_file("src/a.ts", Language::TypeScript);
            file_a.type_defs.push(IrTypeDef {
                name: "GrandParent".to_string(),
                kind: TypeDefKind::Class,
                span: empty_span(),
                bases: vec![],
                is_exported: true,
                decorators: vec![],
            });

            let mut file_b = make_ir_file("src/b.ts", Language::TypeScript);
            file_b.imports.push(IrImport {
                source: "./a".to_string(),
                specifiers: vec![IrImportSpecifier::Named {
                    name: "GrandParent".to_string(),
                    alias: None,
                }],
                span: empty_span(),
            });
            file_b.type_defs.push(IrTypeDef {
                name: "Parent".to_string(),
                kind: TypeDefKind::Class,
                span: empty_span(),
                bases: vec!["GrandParent".to_string()],
                is_exported: true,
                decorators: vec![],
            });

            let mut file_c = make_ir_file("src/c.ts", Language::TypeScript);
            file_c.imports.push(IrImport {
                source: "./b".to_string(),
                specifiers: vec![IrImportSpecifier::Named {
                    name: "Parent".to_string(),
                    alias: None,
                }],
                span: empty_span(),
            });
            file_c.type_defs.push(IrTypeDef {
                name: "Child".to_string(),
                kind: TypeDefKind::Class,
                span: empty_span(),
                bases: vec!["Parent".to_string()],
                is_exported: false,
                decorators: vec![],
            });

            let graph = SymbolGraph::build_from_ir(&[file_a, file_b, file_c]);

            assert!(has_edge(
                &graph,
                "src/b.ts::Parent",
                "src/a.ts::GrandParent",
                &EdgeType::Extends
            ));
            assert!(has_edge(
                &graph,
                "src/c.ts::Child",
                "src/b.ts::Parent",
                &EdgeType::Extends
            ));
        }
    }

    // =======================================================================
    // IR-specific node type tests
    // =======================================================================

    mod ir_node_type_tests {
        use super::*;
        use crate::ast::Language;
        use crate::ir::FunctionKind;
        use crate::ir::{
            IrConstant, IrFile, IrFunctionDef, IrImport, IrImportSpecifier, IrTypeDef, Span,
            TypeDefKind,
        };

        fn empty_span() -> Span {
            Span::new(1, 1)
        }

        fn make_ir_file(path: &str) -> IrFile {
            IrFile {
                path: path.to_string(),
                language: Language::TypeScript,
                functions: vec![],
                type_defs: vec![],
                constants: vec![],
                imports: vec![],
                exports: vec![],
                call_expressions: vec![],
                assignments: vec![],
            }
        }

        #[test]
        fn test_ir_class_node_kind() {
            let mut file = make_ir_file("src/app.ts");
            file.type_defs.push(IrTypeDef {
                name: "AppServer".to_string(),
                kind: TypeDefKind::Class,
                span: empty_span(),
                bases: vec![],
                is_exported: false,
                decorators: vec![],
            });

            let graph = SymbolGraph::build_from_ir(&[file]);
            let sym = graph.get_symbol("src/app.ts::AppServer").unwrap();
            assert_eq!(sym.kind, SymbolKind::Class);
        }

        #[test]
        fn test_ir_struct_node_kind() {
            let mut file = make_ir_file("src/data.ts");
            file.type_defs.push(IrTypeDef {
                name: "Point".to_string(),
                kind: TypeDefKind::Struct,
                span: empty_span(),
                bases: vec![],
                is_exported: false,
                decorators: vec![],
            });

            let graph = SymbolGraph::build_from_ir(&[file]);
            let sym = graph.get_symbol("src/data.ts::Point").unwrap();
            assert_eq!(sym.kind, SymbolKind::Struct);
        }

        #[test]
        fn test_ir_interface_node_kind() {
            let mut file = make_ir_file("src/types.ts");
            file.type_defs.push(IrTypeDef {
                name: "Serializable".to_string(),
                kind: TypeDefKind::Interface,
                span: empty_span(),
                bases: vec![],
                is_exported: false,
                decorators: vec![],
            });

            let graph = SymbolGraph::build_from_ir(&[file]);
            let sym = graph.get_symbol("src/types.ts::Serializable").unwrap();
            assert_eq!(sym.kind, SymbolKind::Interface);
        }

        #[test]
        fn test_ir_type_alias_node_kind() {
            let mut file = make_ir_file("src/types.ts");
            file.type_defs.push(IrTypeDef {
                name: "UserId".to_string(),
                kind: TypeDefKind::TypeAlias,
                span: empty_span(),
                bases: vec![],
                is_exported: false,
                decorators: vec![],
            });

            let graph = SymbolGraph::build_from_ir(&[file]);
            let sym = graph.get_symbol("src/types.ts::UserId").unwrap();
            assert_eq!(sym.kind, SymbolKind::TypeAlias);
        }

        #[test]
        fn test_ir_enum_node_kind() {
            let mut file = make_ir_file("src/status.ts");
            file.type_defs.push(IrTypeDef {
                name: "Status".to_string(),
                kind: TypeDefKind::Enum,
                span: empty_span(),
                bases: vec![],
                is_exported: false,
                decorators: vec![],
            });

            let graph = SymbolGraph::build_from_ir(&[file]);
            let sym = graph.get_symbol("src/status.ts::Status").unwrap();
            assert_eq!(sym.kind, SymbolKind::Class);
        }

        #[test]
        fn test_ir_constant_node() {
            let mut file = make_ir_file("src/config.ts");
            file.constants.push(IrConstant {
                name: "MAX_RETRIES".to_string(),
                span: empty_span(),
                is_exported: true,
            });

            let graph = SymbolGraph::build_from_ir(&[file]);
            let sym = graph.get_symbol("src/config.ts::MAX_RETRIES").unwrap();
            assert_eq!(sym.kind, SymbolKind::Constant);
            assert_eq!(sym.file, "src/config.ts");
        }

        #[test]
        fn test_ir_function_node() {
            let mut file = make_ir_file("src/utils.ts");
            file.functions.push(IrFunctionDef {
                name: "helper".to_string(),
                kind: FunctionKind::Function,
                span: empty_span(),
                parameters: vec![],
                is_async: false,
                is_exported: false,
                decorators: vec![],
            });

            let graph = SymbolGraph::build_from_ir(&[file]);
            let sym = graph.get_symbol("src/utils.ts::helper").unwrap();
            assert_eq!(sym.kind, SymbolKind::Function);
        }

        #[test]
        fn test_ir_mixed_definitions() {
            let mut file = make_ir_file("src/app.ts");
            file.functions.push(IrFunctionDef {
                name: "start".to_string(),
                kind: FunctionKind::Function,
                span: empty_span(),
                parameters: vec![],
                is_async: false,
                is_exported: true,
                decorators: vec![],
            });
            file.type_defs.push(IrTypeDef {
                name: "Config".to_string(),
                kind: TypeDefKind::Interface,
                span: empty_span(),
                bases: vec![],
                is_exported: true,
                decorators: vec![],
            });
            file.constants.push(IrConstant {
                name: "VERSION".to_string(),
                span: empty_span(),
                is_exported: true,
            });

            let graph = SymbolGraph::build_from_ir(&[file]);

            assert_eq!(graph.node_count(), 4);
            assert!(graph.get_node("src/app.ts").is_some());
            assert!(graph.get_node("src/app.ts::start").is_some());
            assert!(graph.get_node("src/app.ts::Config").is_some());
            assert!(graph.get_node("src/app.ts::VERSION").is_some());
        }

        #[test]
        fn test_ir_duplicate_definition_name_across_files() {
            let mut file_a = make_ir_file("src/a.ts");
            file_a.functions.push(IrFunctionDef {
                name: "validate".to_string(),
                kind: FunctionKind::Function,
                span: empty_span(),
                parameters: vec![],
                is_async: false,
                is_exported: true,
                decorators: vec![],
            });

            let mut file_b = make_ir_file("src/b.ts");
            file_b.functions.push(IrFunctionDef {
                name: "validate".to_string(),
                kind: FunctionKind::Function,
                span: empty_span(),
                parameters: vec![],
                is_async: false,
                is_exported: true,
                decorators: vec![],
            });

            let graph = SymbolGraph::build_from_ir(&[file_a, file_b]);

            assert!(graph.get_node("src/a.ts::validate").is_some());
            assert!(graph.get_node("src/b.ts::validate").is_some());
            assert_eq!(graph.node_count(), 4);
        }

        #[test]
        fn test_ir_duplicate_name_within_file_skipped() {
            let mut file = make_ir_file("src/lib.ts");
            file.functions.push(IrFunctionDef {
                name: "config".to_string(),
                kind: FunctionKind::Function,
                span: empty_span(),
                parameters: vec![],
                is_async: false,
                is_exported: false,
                decorators: vec![],
            });
            file.constants.push(IrConstant {
                name: "config".to_string(),
                span: empty_span(),
                is_exported: false,
            });

            let graph = SymbolGraph::build_from_ir(&[file]);
            assert_eq!(graph.node_count(), 2);
            let sym = graph.get_symbol("src/lib.ts::config").unwrap();
            assert_eq!(sym.kind, SymbolKind::Function);
        }

        #[test]
        fn test_ir_call_edges_with_containing_function() {
            use crate::ir::IrCallExpression;

            let mut utils = make_ir_file("src/utils.ts");
            utils.functions.push(IrFunctionDef {
                name: "validate".to_string(),
                kind: FunctionKind::Function,
                span: empty_span(),
                parameters: vec![],
                is_async: false,
                is_exported: true,
                decorators: vec![],
            });

            let mut handler = make_ir_file("src/handler.ts");
            handler.functions.push(IrFunctionDef {
                name: "process".to_string(),
                kind: FunctionKind::Function,
                span: empty_span(),
                parameters: vec![],
                is_async: false,
                is_exported: false,
                decorators: vec![],
            });
            handler.imports.push(IrImport {
                source: "./utils".to_string(),
                specifiers: vec![IrImportSpecifier::Named {
                    name: "validate".to_string(),
                    alias: None,
                }],
                span: empty_span(),
            });
            handler.call_expressions.push(IrCallExpression {
                callee: "validate".to_string(),
                arguments: vec!["data".to_string()],
                span: empty_span(),
                containing_function: Some("process".to_string()),
            });

            let graph = SymbolGraph::build_from_ir(&[utils, handler]);

            assert!(has_edge(
                &graph,
                "src/handler.ts::process",
                "src/utils.ts::validate",
                &EdgeType::Calls
            ));
        }

        #[test]
        fn test_ir_module_level_call() {
            use crate::ir::IrCallExpression;

            let mut utils = make_ir_file("src/utils.ts");
            utils.functions.push(IrFunctionDef {
                name: "init".to_string(),
                kind: FunctionKind::Function,
                span: empty_span(),
                parameters: vec![],
                is_async: false,
                is_exported: true,
                decorators: vec![],
            });

            let mut main_file = make_ir_file("src/main.ts");
            main_file.imports.push(IrImport {
                source: "./utils".to_string(),
                specifiers: vec![IrImportSpecifier::Named {
                    name: "init".to_string(),
                    alias: None,
                }],
                span: empty_span(),
            });
            main_file.call_expressions.push(IrCallExpression {
                callee: "init".to_string(),
                arguments: vec![],
                span: empty_span(),
                containing_function: None,
            });

            let graph = SymbolGraph::build_from_ir(&[utils, main_file]);

            assert!(has_edge(
                &graph,
                "src/main.ts",
                "src/utils.ts::init",
                &EdgeType::Calls
            ));
        }
    }

    // =======================================================================
    // Edge case tests
    // =======================================================================

    mod edge_case_tests {
        use super::*;
        use crate::ast::Language;
        use crate::ir::{IrFile, Span};

        fn make_empty_ir(path: &str) -> IrFile {
            IrFile {
                path: path.to_string(),
                language: Language::TypeScript,
                functions: vec![],
                type_defs: vec![],
                constants: vec![],
                imports: vec![],
                exports: vec![],
                call_expressions: vec![],
                assignments: vec![],
            }
        }

        #[test]
        fn test_unicode_file_path() {
            let file = make_empty_ir("src/日本語/コンポーネント.ts");
            let graph = SymbolGraph::build_from_ir(&[file]);
            assert!(graph.get_node("src/日本語/コンポーネント.ts").is_some());
            let sym = graph.get_symbol("src/日本語/コンポーネント.ts").unwrap();
            assert_eq!(sym.name, "コンポーネント");
        }

        #[test]
        fn test_unicode_symbol_name() {
            use crate::ir::{FunctionKind, IrFunctionDef};
            let mut file = make_empty_ir("src/utils.ts");
            file.functions.push(IrFunctionDef {
                name: "überprüfen".to_string(),
                kind: FunctionKind::Function,
                span: Span::new(1, 1),
                parameters: vec![],
                is_async: false,
                is_exported: false,
                decorators: vec![],
            });
            let graph = SymbolGraph::build_from_ir(&[file]);
            assert!(graph.get_node("src/utils.ts::überprüfen").is_some());
        }

        #[test]
        fn test_deeply_nested_path() {
            let path = "src/a/b/c/d/e/f/g/h/i/j/deep.ts";
            let file = make_empty_ir(path);
            let graph = SymbolGraph::build_from_ir(&[file]);
            assert!(graph.get_node(path).is_some());
            let sym = graph.get_symbol(path).unwrap();
            assert_eq!(sym.name, "deep");
        }

        #[test]
        fn test_file_only_imports_no_definitions() {
            use crate::ir::{IrImport, IrImportSpecifier};
            let mut file = make_empty_ir("src/init.ts");
            file.imports.push(IrImport {
                source: "./polyfill".to_string(),
                specifiers: vec![IrImportSpecifier::SideEffect],
                span: Span::new(1, 1),
            });
            let graph = SymbolGraph::build_from_ir(&[file]);
            assert_eq!(graph.node_count(), 1);
        }

        #[test]
        fn test_many_files_scale() {
            use crate::ir::{FunctionKind, IrFunctionDef};
            let files: Vec<IrFile> = (0..50)
                .map(|i| {
                    let mut f = make_empty_ir(&format!("src/file_{}.ts", i));
                    for j in 0..5 {
                        f.functions.push(IrFunctionDef {
                            name: format!("func_{}", j),
                            kind: FunctionKind::Function,
                            span: Span::new(1, 1),
                            parameters: vec![],
                            is_async: false,
                            is_exported: true,
                            decorators: vec![],
                        });
                    }
                    f
                })
                .collect();

            let graph = SymbolGraph::build_from_ir(&files);
            assert_eq!(graph.node_count(), 300);
        }

        #[test]
        fn test_edges_on_empty_graph() {
            let graph = SymbolGraph::build_from_ir(&[]);
            assert!(graph.edges().is_empty());
            assert!(graph.node_ids().is_empty());
        }

        #[test]
        fn test_node_ids_contains_all() {
            use crate::ir::{FunctionKind, IrFunctionDef};
            let mut file = make_empty_ir("src/lib.ts");
            file.functions.push(IrFunctionDef {
                name: "alpha".to_string(),
                kind: FunctionKind::Function,
                span: Span::new(1, 1),
                parameters: vec![],
                is_async: false,
                is_exported: false,
                decorators: vec![],
            });

            let graph = SymbolGraph::build_from_ir(&[file]);
            let ids = graph.node_ids();
            assert!(ids.contains(&"src/lib.ts"));
            assert!(ids.contains(&"src/lib.ts::alpha"));
            assert_eq!(ids.len(), 2);
        }

        #[test]
        fn test_get_symbol_returns_none_for_missing() {
            let graph = SymbolGraph::build_from_ir(&[make_empty_ir("src/a.ts")]);
            assert!(graph.get_symbol("nonexistent").is_none());
            assert!(graph.get_symbol("src/a.ts::nonexistent").is_none());
        }

        #[test]
        fn test_add_edge_directly() {
            let files = vec![make_empty_ir("src/x.ts"), make_empty_ir("src/y.ts")];
            let mut graph = SymbolGraph::build_from_ir(&files);
            let x_idx = graph.get_node("src/x.ts").unwrap();
            let y_idx = graph.get_node("src/y.ts").unwrap();

            graph.add_edge(
                x_idx,
                y_idx,
                GraphEdge {
                    edge_type: EdgeType::Calls,
                },
            );
            assert_eq!(graph.edge_count(), 1);
            assert!(has_edge(&graph, "src/x.ts", "src/y.ts", &EdgeType::Calls));
        }

        #[test]
        fn test_from_serializable_invalid_edge_endpoint() {
            let sg = SerializableGraph {
                nodes: vec![SymbolNode {
                    id: "a.ts".to_string(),
                    name: "a".to_string(),
                    file: "a.ts".to_string(),
                    kind: SymbolKind::Module,
                }],
                edges: vec![SerializableEdge {
                    from: "a.ts".to_string(),
                    to: "nonexistent.ts".to_string(),
                    edge_type: EdgeType::Imports,
                }],
            };

            let graph = SymbolGraph::from_serializable(&sg);
            assert_eq!(graph.node_count(), 1);
            assert_eq!(
                graph.edge_count(),
                0,
                "edge with invalid endpoint should be skipped"
            );
        }

        #[test]
        fn test_from_serializable_both_endpoints_invalid() {
            let sg = SerializableGraph {
                nodes: vec![],
                edges: vec![SerializableEdge {
                    from: "x.ts".to_string(),
                    to: "y.ts".to_string(),
                    edge_type: EdgeType::Calls,
                }],
            };

            let graph = SymbolGraph::from_serializable(&sg);
            assert_eq!(graph.node_count(), 0);
            assert_eq!(graph.edge_count(), 0);
        }

        #[test]
        fn test_serializable_preserves_all_edge_types() {
            let nodes = vec![
                SymbolNode {
                    id: "a.ts".to_string(),
                    name: "a".to_string(),
                    file: "a.ts".to_string(),
                    kind: SymbolKind::Module,
                },
                SymbolNode {
                    id: "b.ts".to_string(),
                    name: "b".to_string(),
                    file: "b.ts".to_string(),
                    kind: SymbolKind::Module,
                },
            ];
            let edge_types = vec![
                EdgeType::Imports,
                EdgeType::Calls,
                EdgeType::Extends,
                EdgeType::Instantiates,
                EdgeType::Reads,
                EdgeType::Writes,
                EdgeType::Emits,
                EdgeType::Handles,
            ];
            let edges: Vec<SerializableEdge> = edge_types
                .iter()
                .map(|et| SerializableEdge {
                    from: "a.ts".to_string(),
                    to: "b.ts".to_string(),
                    edge_type: et.clone(),
                })
                .collect();
            let sg = SerializableGraph { nodes, edges };

            let json = serde_json::to_string(&sg).unwrap();
            let restored: SerializableGraph = serde_json::from_str(&json).unwrap();
            assert_eq!(sg, restored);
            assert_eq!(restored.edges.len(), 8);
        }

        #[test]
        fn test_same_name_different_directories() {
            use crate::ir::{FunctionKind, IrFunctionDef};
            let mut file_a = make_empty_ir("src/auth/utils.ts");
            file_a.functions.push(IrFunctionDef {
                name: "validate".to_string(),
                kind: FunctionKind::Function,
                span: Span::new(1, 1),
                parameters: vec![],
                is_async: false,
                is_exported: true,
                decorators: vec![],
            });

            let mut file_b = make_empty_ir("src/data/utils.ts");
            file_b.functions.push(IrFunctionDef {
                name: "validate".to_string(),
                kind: FunctionKind::Function,
                span: Span::new(1, 1),
                parameters: vec![],
                is_async: false,
                is_exported: true,
                decorators: vec![],
            });

            let graph = SymbolGraph::build_from_ir(&[file_a, file_b]);
            assert!(graph.get_node("src/auth/utils.ts::validate").is_some());
            assert!(graph.get_node("src/data/utils.ts::validate").is_some());
            assert_eq!(graph.node_count(), 4);
        }

        #[test]
        fn test_multiple_importers_of_same_symbol() {
            let graph = build_graph_from_sources(&[
                (
                    "src/shared.ts",
                    r#"
export function log(msg: string) {}
"#,
                ),
                (
                    "src/a.ts",
                    r#"
import { log } from './shared';
function doA() { log("a"); }
"#,
                ),
                (
                    "src/b.ts",
                    r#"
import { log } from './shared';
function doB() { log("b"); }
"#,
                ),
            ]);

            assert!(has_edge(
                &graph,
                "src/a.ts",
                "src/shared.ts::log",
                &EdgeType::Imports
            ));
            assert!(has_edge(
                &graph,
                "src/b.ts",
                "src/shared.ts::log",
                &EdgeType::Imports
            ));
            assert!(has_edge(
                &graph,
                "src/a.ts::doA",
                "src/shared.ts::log",
                &EdgeType::Calls
            ));
            assert!(has_edge(
                &graph,
                "src/b.ts::doB",
                "src/shared.ts::log",
                &EdgeType::Calls
            ));
        }
    }

    // =======================================================================
    // Additional property-based tests
    // =======================================================================

    mod extended_proptests {
        use super::*;
        use crate::ast::Language;
        use crate::ir::{
            FunctionKind, IrConstant, IrFile, IrFunctionDef, IrTypeDef, Span, TypeDefKind,
        };
        use proptest::prelude::*;

        fn symbol_kind_strategy() -> impl Strategy<Value = SymbolKind> {
            prop_oneof![
                Just(SymbolKind::Function),
                Just(SymbolKind::Class),
                Just(SymbolKind::Interface),
                Just(SymbolKind::TypeAlias),
                Just(SymbolKind::Constant),
                Just(SymbolKind::Module),
                Just(SymbolKind::Struct),
            ]
        }

        fn edge_type_strategy() -> impl Strategy<Value = EdgeType> {
            prop_oneof![
                Just(EdgeType::Imports),
                Just(EdgeType::Calls),
                Just(EdgeType::Extends),
                Just(EdgeType::Instantiates),
                Just(EdgeType::Reads),
                Just(EdgeType::Writes),
                Just(EdgeType::Emits),
                Just(EdgeType::Handles),
            ]
        }

        fn ir_file_strategy() -> impl Strategy<Value = IrFile> {
            (
                "[a-z]{1,6}".prop_map(|s| format!("src/{}.ts", s)),
                prop::collection::vec("[a-z][a-zA-Z0-9]{0,10}", 0..8),
                prop::collection::vec("[A-Z][a-zA-Z0-9]{0,10}", 0..4),
                prop::collection::vec("[A-Z_][A-Z_0-9]{0,10}", 0..3),
            )
                .prop_map(|(path, func_names, type_names, const_names)| {
                    let functions: Vec<IrFunctionDef> = func_names
                        .into_iter()
                        .map(|name| IrFunctionDef {
                            name,
                            kind: FunctionKind::Function,
                            span: Span::new(1, 1),
                            parameters: vec![],
                            is_async: false,
                            is_exported: false,
                            decorators: vec![],
                        })
                        .collect();
                    let type_defs: Vec<IrTypeDef> = type_names
                        .into_iter()
                        .map(|name| IrTypeDef {
                            name,
                            kind: TypeDefKind::Class,
                            span: Span::new(1, 1),
                            bases: vec![],
                            is_exported: false,
                            decorators: vec![],
                        })
                        .collect();
                    let constants: Vec<IrConstant> = const_names
                        .into_iter()
                        .map(|name| IrConstant {
                            name,
                            span: Span::new(1, 1),
                            is_exported: false,
                        })
                        .collect();
                    IrFile {
                        path,
                        language: Language::TypeScript,
                        functions,
                        type_defs,
                        constants,
                        imports: vec![],
                        exports: vec![],
                        call_expressions: vec![],
                        assignments: vec![],
                    }
                })
        }

        proptest! {
            #[test]
            fn prop_all_edges_reference_valid_nodes(
                files in prop::collection::vec(ir_file_strategy(), 1..6)
            ) {
                let graph = SymbolGraph::build_from_ir(&files);
                let all_ids: std::collections::HashSet<&str> =
                    graph.node_ids().into_iter().collect();

                for (from, to, _) in graph.edges() {
                    prop_assert!(
                        all_ids.contains(from),
                        "edge source {} not in graph nodes", from
                    );
                    prop_assert!(
                        all_ids.contains(to),
                        "edge target {} not in graph nodes", to
                    );
                }
            }

            #[test]
            fn prop_module_node_id_equals_file_path(
                files in prop::collection::vec(ir_file_strategy(), 1..6)
            ) {
                let graph = SymbolGraph::build_from_ir(&files);

                for file in &files {
                    if let Some(sym) = graph.get_symbol(&file.path) {
                        prop_assert_eq!(&sym.id, &file.path);
                        prop_assert_eq!(&sym.file, &file.path);
                        prop_assert_eq!(sym.kind, SymbolKind::Module);
                    }
                }
            }

            #[test]
            fn prop_serializable_roundtrip_preserves_edge_types(
                edge_type in edge_type_strategy()
            ) {
                let sg = SerializableGraph {
                    nodes: vec![
                        SymbolNode {
                            id: "a.ts".to_string(),
                            name: "a".to_string(),
                            file: "a.ts".to_string(),
                            kind: SymbolKind::Module,
                        },
                        SymbolNode {
                            id: "b.ts".to_string(),
                            name: "b".to_string(),
                            file: "b.ts".to_string(),
                            kind: SymbolKind::Module,
                        },
                    ],
                    edges: vec![SerializableEdge {
                        from: "a.ts".to_string(),
                        to: "b.ts".to_string(),
                        edge_type: edge_type.clone(),
                    }],
                };

                let graph = SymbolGraph::from_serializable(&sg);
                let restored = graph.to_serializable();
                prop_assert_eq!(restored.edges.len(), 1);
                prop_assert_eq!(&restored.edges[0].edge_type, &edge_type);
            }

            #[test]
            fn prop_serializable_roundtrip_preserves_symbol_kinds(
                kind in symbol_kind_strategy()
            ) {
                let sg = SerializableGraph {
                    nodes: vec![SymbolNode {
                        id: "test::sym".to_string(),
                        name: "sym".to_string(),
                        file: "test".to_string(),
                        kind: kind.clone(),
                    }],
                    edges: vec![],
                };

                let graph = SymbolGraph::from_serializable(&sg);
                let restored = graph.to_serializable();
                prop_assert_eq!(restored.nodes.len(), 1);
                prop_assert_eq!(&restored.nodes[0].kind, &kind);
            }

            #[test]
            fn prop_node_count_equals_unique_defs_plus_modules(
                files in prop::collection::vec(ir_file_strategy(), 1..6)
            ) {
                // Deduplicate files by path to avoid the edge case where
                // duplicate paths create phantom graph nodes (the graph
                // unconditionally adds module nodes without checking for
                // duplicates — a known characteristic of the current impl).
                let mut seen_paths = std::collections::HashSet::new();
                let unique_files: Vec<&IrFile> = files
                    .iter()
                    .filter(|f| seen_paths.insert(f.path.clone()))
                    .collect();

                let graph = SymbolGraph::build_from_ir(
                    &unique_files.iter().cloned().cloned().collect::<Vec<_>>(),
                );

                let mut expected_ids = std::collections::HashSet::new();
                for file in &unique_files {
                    expected_ids.insert(file.path.clone());
                    for func in &file.functions {
                        expected_ids.insert(format!("{}::{}", file.path, func.name));
                    }
                    for td in &file.type_defs {
                        expected_ids.insert(format!("{}::{}", file.path, td.name));
                    }
                    for c in &file.constants {
                        expected_ids.insert(format!("{}::{}", file.path, c.name));
                    }
                }

                prop_assert_eq!(
                    graph.node_count(),
                    expected_ids.len(),
                    "node count should equal unique definition ids"
                );
            }

            #[test]
            fn prop_graph_error_display(msg in "[a-zA-Z0-9 ]{1,50}") {
                let err = GraphError::SerializationError(msg.clone());
                let display = format!("{}", err);
                prop_assert!(display.contains(&msg));
            }

            #[test]
            fn prop_normalize_path_no_panic(path in "[a-z./]{0,30}") {
                let _ = normalize_path(&path);
            }

            #[test]
            fn prop_normalize_python_import_no_panic(input in "[a-z.]{0,20}") {
                let _ = normalize_python_import(&input);
            }

            #[test]
            fn prop_file_stem_no_panic(path in "[a-zA-Z0-9/._-]{0,30}") {
                let _ = file_stem(&path);
            }

            #[test]
            fn prop_resolve_import_never_resolves_absolute(
                source in "[a-z]{1,10}",
                importer in "[a-z/]{1,15}\\.ts"
            ) {
                let known = vec!["anything.ts"];
                let result = resolve_import_path(&source, &importer, &known);
                prop_assert!(result.is_none(),
                    "absolute import '{}' should not resolve", source);
            }
        }
    }

    // =======================================================================
    // Workspace graph integration tests
    // =======================================================================

    mod workspace_graph_tests {
        use super::*;
        use crate::ast;

        #[test]
        fn test_workspace_cross_package_import_edges() {
            // Simulate a monorepo: shared-types exports User, backend imports it.
            let files = vec![
                (
                    "packages/shared-types/src/index.ts",
                    r#"
export interface User { id: string; name: string; }
export function validateUser(user: User): boolean { return true; }
"#,
                ),
                (
                    "packages/backend/src/routes/users.ts",
                    r#"
import { User, validateUser } from "@monorepo/shared-types";
export function handleRequest(user: User) { return validateUser(user); }
"#,
                ),
            ];

            let parsed: Vec<ast::ParsedFile> = files
                .iter()
                .map(|(path, source)| ast::parse_file(path, source).unwrap())
                .collect();

            // Without workspace map: no cross-package edges.
            let graph_no_ws = SymbolGraph::build(&parsed);
            let edges_no_ws = graph_no_ws.edges();
            let cross_pkg_edges: Vec<_> = edges_no_ws
                .iter()
                .filter(|(f, t, _)| f.contains("backend") && t.contains("shared-types"))
                .collect();
            assert!(
                cross_pkg_edges.is_empty(),
                "without workspace map, no cross-package edges should exist"
            );

            // With workspace map: cross-package edges appear.
            let mut ws = WorkspaceMap::new();
            ws.insert(
                "@monorepo/shared-types".to_string(),
                "packages/shared-types/src/index.ts".to_string(),
            );
            let graph_ws = SymbolGraph::build_with_workspace(&parsed, &ws);
            let edges_ws = graph_ws.edges();
            let cross_pkg_edges: Vec<_> = edges_ws
                .iter()
                .filter(|(f, t, _)| f.contains("backend") && t.contains("shared-types"))
                .collect();
            assert!(
                cross_pkg_edges.len() >= 2,
                "with workspace map, cross-package import edges should exist, got: {:?}",
                cross_pkg_edges
            );

            // Verify specific edges.
            assert!(
                cross_pkg_edges
                    .iter()
                    .any(|(_, t, et)| { t.contains("validateUser") && **et == EdgeType::Imports }),
                "should have import edge to validateUser"
            );
        }

        #[test]
        fn test_workspace_cross_package_call_edges() {
            let files = vec![
                (
                    "packages/utils/src/index.ts",
                    r#"
export function formatName(name: string): string { return name.trim(); }
"#,
                ),
                (
                    "packages/app/src/handler.ts",
                    r#"
import { formatName } from "@my/utils";
export function handle(name: string) { return formatName(name); }
"#,
                ),
            ];

            let parsed: Vec<ast::ParsedFile> = files
                .iter()
                .map(|(path, source)| ast::parse_file(path, source).unwrap())
                .collect();

            let mut ws = WorkspaceMap::new();
            ws.insert(
                "@my/utils".to_string(),
                "packages/utils/src/index.ts".to_string(),
            );
            let graph = SymbolGraph::build_with_workspace(&parsed, &ws);
            let edges = graph.edges();

            // Should have both import and call edges.
            let import_edge = edges.iter().any(|(f, t, et)| {
                f.contains("handler") && t.contains("formatName") && **et == EdgeType::Imports
            });
            let call_edge = edges.iter().any(|(f, t, et)| {
                f.contains("handler") && t.contains("formatName") && **et == EdgeType::Calls
            });
            assert!(import_edge, "should have import edge to formatName");
            assert!(call_edge, "should have call edge to formatName");
        }

        #[test]
        fn test_workspace_empty_map_same_as_build() {
            let files = vec![
                ("src/handler.ts", r#"import { foo } from './utils'; foo();"#),
                ("src/utils.ts", r#"export function foo() {}"#),
            ];

            let parsed: Vec<ast::ParsedFile> = files
                .iter()
                .map(|(path, source)| ast::parse_file(path, source).unwrap())
                .collect();

            let g1 = SymbolGraph::build(&parsed);
            let g2 = SymbolGraph::build_with_workspace(&parsed, &WorkspaceMap::new());
            assert_eq!(g1.node_count(), g2.node_count());
            assert_eq!(g1.edge_count(), g2.edge_count());
        }
    }
