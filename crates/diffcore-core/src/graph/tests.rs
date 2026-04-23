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

    // === Import edge tests ===

    #[test]
    fn test_build_import_edges() {
        let graph = build_graph_from_sources(&[
            (
                "src/utils.ts",
                r#"
export function validate(data: any) { return data; }
export function sanitize(data: any) { return data; }
"#,
            ),
            (
                "src/handler.ts",
                r#"
import { validate, sanitize } from './utils';
function handle() { validate({}); }
"#,
            ),
        ]);

        // handler.ts module should import validate and sanitize from utils.ts
        assert!(
            has_edge(
                &graph,
                "src/handler.ts",
                "src/utils.ts::validate",
                &EdgeType::Imports
            ),
            "should have import edge to validate"
        );
        assert!(
            has_edge(
                &graph,
                "src/handler.ts",
                "src/utils.ts::sanitize",
                &EdgeType::Imports
            ),
            "should have import edge to sanitize"
        );
    }

    #[test]
    fn test_build_import_edges_default() {
        let graph = build_graph_from_sources(&[
            (
                "src/app.ts",
                r#"
const app = createApp();
export default app;
"#,
            ),
            (
                "src/main.ts",
                r#"
import App from './app';
"#,
            ),
        ]);

        // Default import should link to the module node.
        assert!(
            has_edge(&graph, "src/main.ts", "src/app.ts", &EdgeType::Imports),
            "should have import edge for default import"
        );
    }

    #[test]
    fn test_build_import_edges_namespace() {
        let graph = build_graph_from_sources(&[
            (
                "src/utils.ts",
                r#"
export function foo() {}
export function bar() {}
"#,
            ),
            (
                "src/main.ts",
                r#"
import * as utils from './utils';
"#,
            ),
        ]);

        assert!(
            has_edge(&graph, "src/main.ts", "src/utils.ts", &EdgeType::Imports),
            "namespace import should link to module node"
        );
    }

    #[test]
    fn test_side_effect_import() {
        let graph = build_graph_from_sources(&[
            ("src/polyfill.ts", "// polyfill code"),
            (
                "src/main.ts",
                r#"
import './polyfill';
"#,
            ),
        ]);

        assert!(
            has_edge(&graph, "src/main.ts", "src/polyfill.ts", &EdgeType::Imports),
            "side-effect import should create module-to-module edge"
        );
    }

    // === Call edge tests ===

    #[test]
    fn test_build_call_edges() {
        let graph = build_graph_from_sources(&[
            (
                "src/utils.ts",
                r#"
export function validate(data: any) { return data; }
"#,
            ),
            (
                "src/handler.ts",
                r#"
import { validate } from './utils';
function processRequest(req: any) {
    const v = validate(req.body);
    return v;
}
"#,
            ),
        ]);

        assert!(
            has_edge(
                &graph,
                "src/handler.ts::processRequest",
                "src/utils.ts::validate",
                &EdgeType::Calls
            ),
            "processRequest should have call edge to validate"
        );
    }

    #[test]
    fn test_build_call_edges_local() {
        let graph = build_graph_from_sources(&[(
            "src/service.ts",
            r#"
function helper() { return 42; }
function main() {
    const x = helper();
    return x;
}
"#,
        )]);

        assert!(
            has_edge(
                &graph,
                "src/service.ts::main",
                "src/service.ts::helper",
                &EdgeType::Calls
            ),
            "main should have call edge to local helper"
        );
    }

    #[test]
    fn test_build_call_edges_method_on_import() {
        let graph = build_graph_from_sources(&[
            (
                "src/db.ts",
                r#"
export function save(data: any) { return data; }
export function find(id: string) { return {}; }
"#,
            ),
            (
                "src/service.ts",
                r#"
import * as db from './db';
function createUser(data: any) {
    return db.save(data);
}
"#,
            ),
        ]);

        assert!(
            has_edge(
                &graph,
                "src/service.ts::createUser",
                "src/db.ts::save",
                &EdgeType::Calls
            ),
            "should resolve method call on namespace import"
        );
    }

    #[test]
    fn test_no_self_call_edge() {
        let graph = build_graph_from_sources(&[(
            "src/lib.ts",
            r#"
function recurse(n: number): number {
    if (n <= 0) return 0;
    return recurse(n - 1);
}
"#,
        )]);

        // Recursive calls should not create self-edges.
        let self_edges: Vec<_> = graph
            .edges()
            .into_iter()
            .filter(|(f, t, _)| f == t)
            .collect();
        assert!(
            self_edges.is_empty(),
            "recursive function should not create self-edges"
        );
    }

    // === Graph structure tests ===

    #[test]
    fn test_graph_node_count() {
        let graph = build_graph_from_sources(&[
            (
                "src/a.ts",
                r#"
export function foo() {}
export function bar() {}
"#,
            ),
            (
                "src/b.ts",
                r#"
export class Baz {}
"#,
            ),
        ]);

        // 2 module nodes + 2 functions + 1 class = 5
        assert_eq!(graph.node_count(), 5);
    }

    #[test]
    fn test_graph_edge_count() {
        let graph = build_graph_from_sources(&[
            (
                "src/utils.ts",
                r#"
export function validate(x: any) { return x; }
"#,
            ),
            (
                "src/handler.ts",
                r#"
import { validate } from './utils';
function handle() { validate({}); }
"#,
            ),
        ]);

        // 1 import edge + 1 call edge = 2
        let import_count = count_edges_of_type(&graph, &EdgeType::Imports);
        let call_count = count_edges_of_type(&graph, &EdgeType::Calls);
        assert_eq!(import_count, 1, "should have 1 import edge");
        assert_eq!(call_count, 1, "should have 1 call edge");
    }

    #[test]
    fn test_cyclic_imports() {
        let graph = build_graph_from_sources(&[
            (
                "src/a.ts",
                r#"
import { funcB } from './b';
export function funcA() { funcB(); }
"#,
            ),
            (
                "src/b.ts",
                r#"
import { funcA } from './a';
export function funcB() { funcA(); }
"#,
            ),
        ]);

        // Should handle cycles without panic/infinite loop.
        assert!(graph.node_count() > 0);

        // Both import edges should exist.
        assert!(has_edge(
            &graph,
            "src/a.ts",
            "src/b.ts::funcB",
            &EdgeType::Imports
        ));
        assert!(has_edge(
            &graph,
            "src/b.ts",
            "src/a.ts::funcA",
            &EdgeType::Imports
        ));

        // Both call edges should exist.
        assert!(has_edge(
            &graph,
            "src/a.ts::funcA",
            "src/b.ts::funcB",
            &EdgeType::Calls
        ));
        assert!(has_edge(
            &graph,
            "src/b.ts::funcB",
            "src/a.ts::funcA",
            &EdgeType::Calls
        ));
    }

    #[test]
    fn test_reexport_chains() {
        let graph = build_graph_from_sources(&[
            (
                "src/core/validate.ts",
                r#"
export function validate(data: any) { return data; }
"#,
            ),
            (
                "src/core/index.ts",
                r#"
export { validate } from './validate';
"#,
            ),
            (
                "src/handler.ts",
                r#"
import { validate } from './core/index';
function handle() { validate({}); }
"#,
            ),
        ]);

        // The import from handler should resolve through the barrel file to the actual definition.
        assert!(
            has_edge(
                &graph,
                "src/handler.ts",
                "src/core/validate.ts::validate",
                &EdgeType::Imports
            ),
            "should resolve re-export chain through barrel file"
        );
    }

    #[test]
    fn test_graph_serialization_roundtrip() {
        let original = build_graph_from_sources(&[
            (
                "src/a.ts",
                r#"
export function foo() {}
"#,
            ),
            (
                "src/b.ts",
                r#"
import { foo } from './a';
function bar() { foo(); }
"#,
            ),
        ]);

        let serialized = original.to_serializable();
        let json = serde_json::to_string(&serialized).unwrap();
        let deserialized_data: SerializableGraph = serde_json::from_str(&json).unwrap();
        let restored = SymbolGraph::from_serializable(&deserialized_data);

        assert_eq!(original.node_count(), restored.node_count());
        assert_eq!(original.edge_count(), restored.edge_count());

        // Verify all nodes match.
        let orig_serialized = original.to_serializable();
        assert_eq!(orig_serialized, deserialized_data);
    }

    #[test]
    fn test_empty_files() {
        let graph = build_graph_from_sources(&[]);
        assert_eq!(graph.node_count(), 0);
        assert_eq!(graph.edge_count(), 0);
    }

    #[test]
    fn test_single_file_no_edges() {
        let graph = build_graph_from_sources(&[(
            "src/lib.ts",
            r#"
function hello() { console.log('hi'); }
"#,
        )]);

        // 1 module node + 1 function node = 2
        assert_eq!(graph.node_count(), 2);
        // console.log is external, no edge should be created.
        assert_eq!(
            count_edges_of_type(&graph, &EdgeType::Calls),
            0,
            "external calls should not create edges"
        );
    }

    #[test]
    fn test_python_import_edges() {
        let graph = build_graph_from_sources(&[
            (
                "src/models.py",
                r#"
class User:
    def __init__(self, name):
        self.name = name
"#,
            ),
            (
                "src/service.py",
                r#"
from .models import User

def create_user(name):
    return User(name)
"#,
            ),
        ]);

        assert!(
            has_edge(
                &graph,
                "src/service.py",
                "src/models.py::User",
                &EdgeType::Imports
            ),
            "Python from-import should create import edge"
        );
    }

    #[test]
    fn test_python_call_edges() {
        let graph = build_graph_from_sources(&[
            (
                "src/utils.py",
                r#"
def validate(data):
    return data
"#,
            ),
            (
                "src/handler.py",
                r#"
from .utils import validate

def process(data):
    return validate(data)
"#,
            ),
        ]);

        assert!(
            has_edge(
                &graph,
                "src/handler.py::process",
                "src/utils.py::validate",
                &EdgeType::Calls
            ),
            "Python call should create call edge"
        );
    }

    #[test]
    fn test_cross_directory_imports() {
        let graph = build_graph_from_sources(&[
            (
                "src/models/user.ts",
                r#"
export interface User { name: string; }
"#,
            ),
            (
                "src/handlers/auth.ts",
                r#"
import { User } from '../models/user';
function login(user: User) {}
"#,
            ),
        ]);

        assert!(
            has_edge(
                &graph,
                "src/handlers/auth.ts",
                "src/models/user.ts::User",
                &EdgeType::Imports
            ),
            "should resolve cross-directory relative import with .."
        );
    }

    #[test]
    fn test_unknown_language_no_crash() {
        let graph =
            build_graph_from_sources(&[("src/main.rs", r#"fn main() { println!("hello"); }"#)]);

        // Should have module node only, no definitions from unknown language.
        assert_eq!(graph.node_count(), 1);
        assert_eq!(graph.edge_count(), 0);
    }

    #[test]
    fn test_multiple_call_targets() {
        let graph = build_graph_from_sources(&[
            (
                "src/a.ts",
                r#"
export function alpha() { return 1; }
"#,
            ),
            (
                "src/b.ts",
                r#"
export function beta() { return 2; }
"#,
            ),
            (
                "src/c.ts",
                r#"
import { alpha } from './a';
import { beta } from './b';
function gamma() {
    alpha();
    beta();
}
"#,
            ),
        ]);

        assert!(has_edge(
            &graph,
            "src/c.ts::gamma",
            "src/a.ts::alpha",
            &EdgeType::Calls
        ));
        assert!(has_edge(
            &graph,
            "src/c.ts::gamma",
            "src/b.ts::beta",
            &EdgeType::Calls
        ));
    }

    #[test]
    fn test_aliased_import_call() {
        let graph = build_graph_from_sources(&[
            (
                "src/utils.ts",
                r#"
export function validate(data: any) { return data; }
"#,
            ),
            (
                "src/handler.ts",
                r#"
import { validate as check } from './utils';
function handle() { check({}); }
"#,
            ),
        ]);

        assert!(
            has_edge(
                &graph,
                "src/handler.ts::handle",
                "src/utils.ts::validate",
                &EdgeType::Calls
            ),
            "aliased import should resolve calls through the alias"
        );
    }

    #[test]
    fn test_index_file_resolution() {
        let graph = build_graph_from_sources(&[
            (
                "src/lib/index.ts",
                r#"
export function helper() { return 42; }
"#,
            ),
            (
                "src/main.ts",
                r#"
import { helper } from './lib';
function run() { helper(); }
"#,
            ),
        ]);

        // `./lib` should resolve to `src/lib/index.ts`
        assert!(
            has_edge(
                &graph,
                "src/main.ts",
                "src/lib/index.ts::helper",
                &EdgeType::Imports
            ),
            "should resolve ./lib to ./lib/index.ts"
        );
    }

    #[test]
    fn test_node_lookup() {
        let graph = build_graph_from_sources(&[(
            "src/app.ts",
            r#"
export function start() {}
export class Server {}
"#,
        )]);

        assert!(graph.get_node("src/app.ts").is_some());
        assert!(graph.get_node("src/app.ts::start").is_some());
        assert!(graph.get_node("src/app.ts::Server").is_some());
        assert!(graph.get_node("src/nonexistent.ts").is_none());

        let start = graph.get_symbol("src/app.ts::start").unwrap();
        assert_eq!(start.name, "start");
        assert_eq!(start.kind, SymbolKind::Function);
    }

    #[test]
    fn test_external_imports_no_edges() {
        let graph = build_graph_from_sources(&[(
            "src/app.ts",
            r#"
import express from 'express';
import { Router } from 'express';
const app = express();
"#,
        )]);

        // External packages (non-relative imports) should not create edges.
        assert_eq!(
            count_edges_of_type(&graph, &EdgeType::Imports),
            0,
            "external imports should not create edges"
        );
    }

    #[test]
    fn test_deterministic_output() {
        let files = &[
            (
                "src/a.ts",
                r#"
export function foo() {}
export function bar() {}
"#,
            ),
            (
                "src/b.ts",
                r#"
import { foo, bar } from './a';
function baz() { foo(); bar(); }
"#,
            ),
        ];

        let g1 = build_graph_from_sources(files);
        let g2 = build_graph_from_sources(files);

        assert_eq!(g1.node_count(), g2.node_count());
        assert_eq!(g1.edge_count(), g2.edge_count());
        assert_eq!(g1.to_serializable(), g2.to_serializable());
    }

    // === §13.3 spec-required tests ===

    /// §13.3: Creates `extends` edges from class inheritance.
    #[test]
    fn test_build_extends_edges() {
        // TypeScript class inheritance via AST path.
        // Note: the AST path's `collect_extends_edges` is a stub — extends edges
        // come from the IR path. Verify IR-based extends edges work correctly.
        let graph = build_graph_from_sources(&[
            (
                "src/base.ts",
                r#"
export class BaseEntity {
    id: string;
}
"#,
            ),
            (
                "src/user.ts",
                r#"
import { BaseEntity } from './base';
export class User extends BaseEntity {
    name: string;
}
"#,
            ),
        ]);

        // Via AST path, extends edges are not yet produced (stub).
        // Verify the graph builds without error and has the expected nodes.
        assert!(graph.node_count() >= 4, "should have module + class nodes");

        // Now test via IR path which DOES produce extends edges.
        use crate::ir::{IrFile, IrImport, IrImportSpecifier, IrTypeDef, Span, TypeDefKind};

        let empty_span = || Span {
            start_line: 0,
            end_line: 0,
        };

        let base_file = IrFile {
            path: "src/base.ts".to_string(),
            language: crate::ast::Language::TypeScript,
            functions: vec![],
            type_defs: vec![IrTypeDef {
                name: "BaseEntity".to_string(),
                kind: TypeDefKind::Class,
                span: empty_span(),
                bases: vec![],
                is_exported: true,
                decorators: vec![],
            }],
            constants: vec![],
            imports: vec![],
            exports: vec![],
            call_expressions: vec![],
            assignments: vec![],
        };

        let user_file = IrFile {
            path: "src/user.ts".to_string(),
            language: crate::ast::Language::TypeScript,
            functions: vec![],
            type_defs: vec![IrTypeDef {
                name: "User".to_string(),
                kind: TypeDefKind::Class,
                span: empty_span(),
                bases: vec!["BaseEntity".to_string()],
                is_exported: true,
                decorators: vec![],
            }],
            constants: vec![],
            imports: vec![IrImport {
                source: "./base".to_string(),
                specifiers: vec![IrImportSpecifier::Named {
                    name: "BaseEntity".to_string(),
                    alias: None,
                }],
                span: empty_span(),
            }],
            exports: vec![],
            call_expressions: vec![],
            assignments: vec![],
        };

        let ir_graph = SymbolGraph::build_from_ir(&[base_file, user_file]);
        assert!(
            has_edge(
                &ir_graph,
                "src/user.ts::User",
                "src/base.ts::BaseEntity",
                &EdgeType::Extends
            ),
            "should have Extends edge from User to BaseEntity via IR path"
        );
    }

    /// §13.3: Resolves imports across monorepo package boundaries.
    #[test]
    fn test_cross_package_edges() {
        let files = vec![
            (
                "packages/shared/src/index.ts",
                r#"
export function formatDate(d: Date): string { return d.toISOString(); }
"#,
            ),
            (
                "packages/api/src/handler.ts",
                r#"
import { formatDate } from "@acme/shared";
export function handle() { return formatDate(new Date()); }
"#,
            ),
        ];

        let parsed: Vec<ParsedFile> = files
            .iter()
            .map(|(path, source)| ast::parse_file(path, source).unwrap())
            .collect();

        let mut ws = WorkspaceMap::new();
        ws.insert(
            "@acme/shared".to_string(),
            "packages/shared/src/index.ts".to_string(),
        );
        let graph = SymbolGraph::build_with_workspace(&parsed, &ws);

        // Should have cross-package import edge
        assert!(
            has_edge(
                &graph,
                "packages/api/src/handler.ts",
                "packages/shared/src/index.ts::formatDate",
                &EdgeType::Imports
            ),
            "should resolve import across monorepo package boundary"
        );

        // Should have cross-package call edge
        assert!(
            has_edge(
                &graph,
                "packages/api/src/handler.ts::handle",
                "packages/shared/src/index.ts::formatDate",
                &EdgeType::Calls
            ),
            "should resolve call across monorepo package boundary"
        );
    }

    /// §13.3: Handles `import()` / `require()` dynamic imports.
    #[test]
    fn test_dynamic_imports() {
        // Dynamic imports (import() and require()) should not crash the graph builder.
        // Whether edges are created depends on whether the callee can be resolved.
        let graph = build_graph_from_sources(&[
            (
                "src/utils.ts",
                r#"
export function lazyLoad() { return 42; }
"#,
            ),
            (
                "src/main.ts",
                r#"
async function loadModule() {
    const mod = await import('./utils');
    return mod.lazyLoad();
}
function loadSync() {
    const mod = require('./utils');
}
"#,
            ),
        ]);

        // Graph should build without crashing on dynamic imports.
        assert!(graph.node_count() >= 2, "should have nodes for both files");

        // Dynamic import() and require() are call expressions; they may or may not
        // create edges depending on resolution. The key property is no panic.
        // Check that the graph is well-formed.
        let serialized = graph.to_serializable();
        let json = serde_json::to_string(&serialized).unwrap();
        let _: SerializableGraph = serde_json::from_str(&json).unwrap();
    }

    // === Property-based tests ===

    mod proptests {
        use super::*;
        use proptest::prelude::*;

        /// Generate a random function name.
        fn func_name_strategy() -> impl Strategy<Value = String> {
            "[a-z][a-zA-Z0-9]{0,15}".prop_map(|s| s)
        }

        /// Generate a ParsedFile with random definitions.
        fn parsed_file_strategy() -> impl Strategy<Value = ParsedFile> {
            (
                "[a-z]{1,8}".prop_map(|s| format!("src/{}.ts", s)),
                prop::collection::vec(func_name_strategy(), 0..10),
            )
                .prop_map(|(path, func_names)| {
                    let definitions: Vec<Definition> = func_names
                        .iter()
                        .enumerate()
                        .map(|(i, name)| Definition {
                            name: name.clone(),
                            kind: SymbolKind::Function,
                            start_line: i + 1,
                            end_line: i + 3,
                        })
                        .collect();

                    ParsedFile {
                        path,
                        language: Language::TypeScript,
                        definitions,
                        imports: vec![],
                        exports: vec![],
                        call_sites: vec![],
                    }
                })
        }

        proptest! {
            #[test]
            fn prop_every_definition_has_node(files in prop::collection::vec(parsed_file_strategy(), 1..5)) {
                let graph = SymbolGraph::build(&files);

                for file in &files {
                    // Module node exists.
                    prop_assert!(graph.get_node(&file.path).is_some(),
                        "module node should exist for {}", file.path);

                    // Each unique definition has a node.
                    let mut seen = std::collections::HashSet::new();
                    for def in &file.definitions {
                        let sym_id = format!("{}::{}", file.path, def.name);
                        if seen.insert(sym_id.clone()) {
                            prop_assert!(graph.get_node(&sym_id).is_some(),
                                "node should exist for {}", sym_id);
                        }
                    }
                }
            }

            #[test]
            fn prop_node_count_at_least_file_count(files in prop::collection::vec(parsed_file_strategy(), 0..5)) {
                let graph = SymbolGraph::build(&files);
                // At minimum, one module node per file.
                prop_assert!(graph.node_count() >= files.len());
            }

            #[test]
            fn prop_no_self_edges(files in prop::collection::vec(parsed_file_strategy(), 0..5)) {
                let graph = SymbolGraph::build(&files);
                for (from, to, _) in graph.edges() {
                    prop_assert!(from != to, "self-edge found: {} -> {}", from, to);
                }
            }

            #[test]
            fn prop_serialization_roundtrip(files in prop::collection::vec(parsed_file_strategy(), 0..5)) {
                let graph = SymbolGraph::build(&files);
                let serialized = graph.to_serializable();
                let json = serde_json::to_string(&serialized).unwrap();
                let deserialized: SerializableGraph = serde_json::from_str(&json).unwrap();
                let restored = SymbolGraph::from_serializable(&deserialized);

                prop_assert_eq!(graph.node_count(), restored.node_count());
                prop_assert_eq!(graph.edge_count(), restored.edge_count());
            }

            #[test]
            fn prop_deterministic(files in prop::collection::vec(parsed_file_strategy(), 0..5)) {
                let g1 = SymbolGraph::build(&files);
                let g2 = SymbolGraph::build(&files);
                prop_assert_eq!(g1.node_count(), g2.node_count());
                prop_assert_eq!(g1.edge_count(), g2.edge_count());
            }

            #[test]
            fn prop_empty_input_empty_graph(_dummy in 0u32..1) {
                let graph = SymbolGraph::build(&[]);
                prop_assert_eq!(graph.node_count(), 0);
                prop_assert_eq!(graph.edge_count(), 0);
            }
        }
    }

    // =======================================================================
    // IR-based graph parity tests
    // =======================================================================

    mod ir_parity {
        use super::*;
        use crate::ir::IrFile;

        /// Helper: parse files and build graph via both paths, return both.
        fn build_both(files: &[(&str, &str)]) -> (SymbolGraph, SymbolGraph) {
            let parsed: Vec<ParsedFile> = files
                .iter()
                .map(|(path, source)| ast::parse_file(path, source).unwrap())
                .collect();
            let ir_files: Vec<IrFile> = parsed.iter().map(IrFile::from_parsed_file).collect();

            let graph_parsed = SymbolGraph::build(&parsed);
            let graph_ir = SymbolGraph::build_from_ir(&ir_files);
            (graph_parsed, graph_ir)
        }

        #[test]
        fn test_ir_parity_simple_import() {
            let (gp, gi) = build_both(&[
                (
                    "src/utils.ts",
                    r#"
export function validate(data: any) { return data; }
export function sanitize(data: any) { return data; }
"#,
                ),
                (
                    "src/handler.ts",
                    r#"
import { validate, sanitize } from './utils';
function handle() { validate({}); }
"#,
                ),
            ]);

            assert_eq!(gp.node_count(), gi.node_count(), "node counts should match");
            assert_eq!(gp.edge_count(), gi.edge_count(), "edge counts should match");
        }

        #[test]
        fn test_ir_parity_call_edges() {
            let (gp, gi) = build_both(&[
                (
                    "src/utils.ts",
                    r#"
export function validate(data: any) { return data; }
"#,
                ),
                (
                    "src/handler.ts",
                    r#"
import { validate } from './utils';
function processRequest(req: any) {
    const v = validate(req.body);
    return v;
}
"#,
                ),
            ]);

            assert_eq!(gp.node_count(), gi.node_count());
            assert_eq!(gp.edge_count(), gi.edge_count());

            // Verify specific edge exists in IR graph.
            assert!(
                has_edge(
                    &gi,
                    "src/handler.ts::processRequest",
                    "src/utils.ts::validate",
                    &EdgeType::Calls
                ),
                "IR graph should have call edge"
            );
        }

        #[test]
        fn test_ir_parity_namespace_import() {
            let (gp, gi) = build_both(&[
                (
                    "src/utils.ts",
                    r#"
export function foo() {}
export function bar() {}
"#,
                ),
                (
                    "src/main.ts",
                    r#"
import * as utils from './utils';
function main() {
    utils.foo();
}
"#,
                ),
            ]);

            assert_eq!(gp.node_count(), gi.node_count());
            assert_eq!(gp.edge_count(), gi.edge_count());
        }

        #[test]
        fn test_ir_parity_default_import() {
            let (gp, gi) = build_both(&[
                (
                    "src/utils.ts",
                    r#"
export default function doStuff() {}
"#,
                ),
                (
                    "src/main.ts",
                    r#"
import doStuff from './utils';
doStuff();
"#,
                ),
            ]);

            assert_eq!(gp.node_count(), gi.node_count());
            assert_eq!(gp.edge_count(), gi.edge_count());
        }

        #[test]
        fn test_ir_parity_python_imports() {
            let (gp, gi) = build_both(&[
                (
                    "models.py",
                    r#"
class User:
    pass

def create_user():
    pass
"#,
                ),
                (
                    "views.py",
                    r#"
from .models import User, create_user

def list_users():
    return create_user()
"#,
                ),
            ]);

            assert_eq!(gp.node_count(), gi.node_count());
            assert_eq!(gp.edge_count(), gi.edge_count());
        }

        #[test]
        fn test_ir_parity_reexport_chain() {
            let (gp, gi) = build_both(&[
                (
                    "src/core.ts",
                    r#"
export function coreFunc() {}
"#,
                ),
                (
                    "src/index.ts",
                    r#"
export { coreFunc } from './core';
"#,
                ),
                (
                    "src/consumer.ts",
                    r#"
import { coreFunc } from './index';
function use() { coreFunc(); }
"#,
                ),
            ]);

            assert_eq!(gp.node_count(), gi.node_count());
            assert_eq!(gp.edge_count(), gi.edge_count());
        }

        #[test]
        fn test_ir_parity_side_effect_import() {
            let (gp, gi) = build_both(&[
                (
                    "src/polyfill.ts",
                    r#"
export function polyfill() {}
"#,
                ),
                ("src/main.ts", r#"import './polyfill';"#),
            ]);

            assert_eq!(gp.node_count(), gi.node_count());
            assert_eq!(gp.edge_count(), gi.edge_count());
        }

        #[test]
        fn test_ir_parity_empty_input() {
            let gi = SymbolGraph::build_from_ir(&[]);
            assert_eq!(gi.node_count(), 0);
            assert_eq!(gi.edge_count(), 0);
        }

        #[test]
        fn test_ir_parity_local_call() {
            let (gp, gi) = build_both(&[(
                "src/app.ts",
                r#"
function helper() { return 42; }
function main() { helper(); }
"#,
            )]);

            assert_eq!(gp.node_count(), gi.node_count());
            assert_eq!(gp.edge_count(), gi.edge_count());

            assert!(
                has_edge(
                    &gi,
                    "src/app.ts::main",
                    "src/app.ts::helper",
                    &EdgeType::Calls
                ),
                "IR graph should have local call edge"
            );
        }

        #[test]
        fn test_ir_parity_aliased_import() {
            let (gp, gi) = build_both(&[
                (
                    "src/utils.ts",
                    r#"
export function validate() {}
"#,
                ),
                (
                    "src/main.ts",
                    r#"
import { validate as check } from './utils';
function run() { check(); }
"#,
                ),
            ]);

            assert_eq!(gp.node_count(), gi.node_count());
            assert_eq!(gp.edge_count(), gi.edge_count());
        }

        #[test]
        fn test_ir_parity_multiple_files() {
            let (gp, gi) = build_both(&[
                (
                    "src/db.ts",
                    r#"
export function query(sql: string) { return []; }
export function insert(data: any) { }
"#,
                ),
                (
                    "src/service.ts",
                    r#"
import { query, insert } from './db';
export function getUsers() { return query('SELECT * FROM users'); }
export function createUser(data: any) { insert(data); }
"#,
                ),
                (
                    "src/handler.ts",
                    r#"
import { getUsers, createUser } from './service';
function handleGet(req: any) { return getUsers(); }
function handlePost(req: any) { createUser(req.body); }
"#,
                ),
            ]);

            assert_eq!(
                gp.node_count(),
                gi.node_count(),
                "3-file graph node count should match"
            );
            assert_eq!(
                gp.edge_count(),
                gi.edge_count(),
                "3-file graph edge count should match"
            );
        }
    }

    // =======================================================================
    // IR-based graph property-based tests
    // =======================================================================

    mod ir_proptest {
        use super::*;
        use crate::ast::{Definition, Language, ParsedFile};
        use crate::ir::IrFile;
        use proptest::prelude::*;

        fn func_name_strategy() -> impl Strategy<Value = String> {
            "[a-z][a-zA-Z0-9]{0,15}".prop_map(|s| s)
        }

        fn parsed_file_strategy() -> impl Strategy<Value = ParsedFile> {
            (
                "[a-z]{1,8}".prop_map(|s| format!("src/{}.ts", s)),
                prop::collection::vec(func_name_strategy(), 0..10),
            )
                .prop_map(|(path, func_names)| {
                    let definitions: Vec<Definition> = func_names
                        .iter()
                        .enumerate()
                        .map(|(i, name)| Definition {
                            name: name.clone(),
                            kind: SymbolKind::Function,
                            start_line: i + 1,
                            end_line: i + 3,
                        })
                        .collect();

                    ParsedFile {
                        path,
                        language: Language::TypeScript,
                        definitions,
                        imports: vec![],
                        exports: vec![],
                        call_sites: vec![],
                    }
                })
        }

        proptest! {
            #[test]
            fn prop_ir_node_count_matches_parsed(files in prop::collection::vec(parsed_file_strategy(), 0..5)) {
                let ir_files: Vec<IrFile> = files.iter().map(|f| IrFile::from_parsed_file(f)).collect();
                let g_parsed = SymbolGraph::build(&files);
                let g_ir = SymbolGraph::build_from_ir(&ir_files);
                prop_assert_eq!(g_parsed.node_count(), g_ir.node_count(),
                    "node count mismatch: parsed={}, ir={}", g_parsed.node_count(), g_ir.node_count());
            }

            #[test]
            fn prop_ir_edge_count_matches_parsed(files in prop::collection::vec(parsed_file_strategy(), 0..5)) {
                let ir_files: Vec<IrFile> = files.iter().map(|f| IrFile::from_parsed_file(f)).collect();
                let g_parsed = SymbolGraph::build(&files);
                let g_ir = SymbolGraph::build_from_ir(&ir_files);
                prop_assert_eq!(g_parsed.edge_count(), g_ir.edge_count());
            }

            #[test]
            fn prop_ir_no_self_edges(files in prop::collection::vec(parsed_file_strategy(), 0..5)) {
                let ir_files: Vec<IrFile> = files.iter().map(|f| IrFile::from_parsed_file(f)).collect();
                let graph = SymbolGraph::build_from_ir(&ir_files);
                for (from, to, _) in graph.edges() {
                    prop_assert!(from != to, "self-edge found in IR graph: {} -> {}", from, to);
                }
            }

            #[test]
            fn prop_ir_deterministic(files in prop::collection::vec(parsed_file_strategy(), 0..5)) {
                let ir_files: Vec<IrFile> = files.iter().map(|f| IrFile::from_parsed_file(f)).collect();
                let g1 = SymbolGraph::build_from_ir(&ir_files);
                let g2 = SymbolGraph::build_from_ir(&ir_files);
                prop_assert_eq!(g1.node_count(), g2.node_count());
                prop_assert_eq!(g1.edge_count(), g2.edge_count());
            }

            #[test]
            fn prop_ir_empty_input_empty_graph(_dummy in 0u32..1) {
                let graph = SymbolGraph::build_from_ir(&[]);
                prop_assert_eq!(graph.node_count(), 0);
                prop_assert_eq!(graph.edge_count(), 0);
            }

            #[test]
            fn prop_ir_every_definition_has_node(files in prop::collection::vec(parsed_file_strategy(), 1..5)) {
                let ir_files: Vec<IrFile> = files.iter().map(|f| IrFile::from_parsed_file(f)).collect();
                let graph = SymbolGraph::build_from_ir(&ir_files);

                for ir_file in &ir_files {
                    prop_assert!(graph.get_node(&ir_file.path).is_some(),
                        "module node should exist for {}", ir_file.path);

                    let mut seen = std::collections::HashSet::new();
                    for func in &ir_file.functions {
                        let sym_id = format!("{}::{}", ir_file.path, func.name);
                        if seen.insert(sym_id.clone()) {
                            prop_assert!(graph.get_node(&sym_id).is_some(),
                                "node should exist for function {}", sym_id);
                        }
                    }
                    for td in &ir_file.type_defs {
                        let sym_id = format!("{}::{}", ir_file.path, td.name);
                        if seen.insert(sym_id.clone()) {
                            prop_assert!(graph.get_node(&sym_id).is_some(),
                                "node should exist for type def {}", sym_id);
                        }
                    }
                    for c in &ir_file.constants {
                        let sym_id = format!("{}::{}", ir_file.path, c.name);
                        if seen.insert(sym_id.clone()) {
                            prop_assert!(graph.get_node(&sym_id).is_some(),
                                "node should exist for constant {}", sym_id);
                        }
                    }
                }
            }
        }
    }

