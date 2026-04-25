    use super::*;

    // === TypeScript imports ===

    #[test]
    fn test_parse_ts_imports() {
        let source = r#"
import React from 'react';
import { useState, useEffect } from 'react';
import * as path from 'path';
import { foo as bar } from './utils';
"#;
        let result = parse_file("app.ts", source).unwrap();
        assert_eq!(result.imports.len(), 4);

        // Default import
        assert_eq!(result.imports[0].source, "react");
        assert!(result.imports[0].is_default);
        assert!(!result.imports[0].is_namespace);
        assert_eq!(result.imports[0].names.len(), 1);
        assert_eq!(result.imports[0].names[0].name, "React");

        // Named imports
        assert_eq!(result.imports[1].source, "react");
        assert!(!result.imports[1].is_default);
        assert_eq!(result.imports[1].names.len(), 2);
        assert_eq!(result.imports[1].names[0].name, "useState");
        assert_eq!(result.imports[1].names[1].name, "useEffect");

        // Namespace import
        assert_eq!(result.imports[2].source, "path");
        assert!(result.imports[2].is_namespace);
        assert_eq!(result.imports[2].names[0].name, "path");

        // Aliased import
        assert_eq!(result.imports[3].source, "./utils");
        assert_eq!(result.imports[3].names[0].name, "foo");
        assert_eq!(result.imports[3].names[0].alias, Some("bar".to_string()));
    }

    #[test]
    fn test_parse_ts_default_and_named_import() {
        let source = r#"import React, { useState } from 'react';"#;
        let result = parse_file("app.ts", source).unwrap();
        assert_eq!(result.imports.len(), 1);
        let imp = &result.imports[0];
        assert!(imp.is_default);
        assert_eq!(imp.names.len(), 2);
        assert_eq!(imp.names[0].name, "React");
        assert_eq!(imp.names[1].name, "useState");
    }

    #[test]
    fn test_parse_ts_side_effect_import() {
        let source = r#"import './polyfill';"#;
        let result = parse_file("app.ts", source).unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].source, "./polyfill");
        assert!(result.imports[0].names.is_empty());
    }

    // === TypeScript exports ===

    #[test]
    fn test_parse_ts_exports() {
        let source = r#"
export function greet() {}
export default function main() {}
export { foo, bar };
export { baz } from './other';
export const VALUE = 42;
"#;
        let result = parse_file("lib.ts", source).unwrap();

        // export function greet
        let greet_export = result.exports.iter().find(|e| e.name == "greet").unwrap();
        assert!(!greet_export.is_default);
        assert!(!greet_export.is_reexport);

        // export default function main
        let main_export = result.exports.iter().find(|e| e.name == "main").unwrap();
        assert!(main_export.is_default);

        // export { foo, bar }
        let foo_export = result.exports.iter().find(|e| e.name == "foo").unwrap();
        assert!(!foo_export.is_default);
        assert!(!foo_export.is_reexport);

        let bar_export = result.exports.iter().find(|e| e.name == "bar").unwrap();
        assert!(!bar_export.is_reexport);

        // export { baz } from './other'
        let baz_export = result.exports.iter().find(|e| e.name == "baz").unwrap();
        assert!(baz_export.is_reexport);
        assert_eq!(baz_export.source, Some("./other".to_string()));

        // export const VALUE
        let val_export = result.exports.iter().find(|e| e.name == "VALUE").unwrap();
        assert!(!val_export.is_default);
    }

    #[test]
    fn test_parse_ts_wildcard_reexport() {
        let source = r#"export * from './all';"#;
        let result = parse_file("index.ts", source).unwrap();
        assert_eq!(result.exports.len(), 1);
        assert_eq!(result.exports[0].name, "*");
        assert!(result.exports[0].is_reexport);
        assert_eq!(result.exports[0].source, Some("./all".to_string()));
    }

    #[test]
    fn test_parse_ts_export_default_expression() {
        let source = r#"
const app = createApp();
export default app;
"#;
        let result = parse_file("app.ts", source).unwrap();
        let default_export = result.exports.iter().find(|e| e.is_default).unwrap();
        assert_eq!(default_export.name, "app");
    }

    // === TypeScript definitions ===

    #[test]
    fn test_parse_ts_functions() {
        let source = r#"
function greet(name: string): string {
    return `Hello ${name}`;
}

const double = (x: number) => x * 2;

class Calculator {
    add(a: number, b: number): number {
        return a + b;
    }
    subtract(a: number, b: number): number {
        return a - b;
    }
}
"#;
        let result = parse_file("math.ts", source).unwrap();

        // function declaration
        let greet = result
            .definitions
            .iter()
            .find(|d| d.name == "greet")
            .unwrap();
        assert_eq!(greet.kind, SymbolKind::Function);

        // arrow function
        let double = result
            .definitions
            .iter()
            .find(|d| d.name == "double")
            .unwrap();
        assert_eq!(double.kind, SymbolKind::Function);

        // class
        let calc = result
            .definitions
            .iter()
            .find(|d| d.name == "Calculator")
            .unwrap();
        assert_eq!(calc.kind, SymbolKind::Class);

        // methods
        let add = result.definitions.iter().find(|d| d.name == "add").unwrap();
        assert_eq!(add.kind, SymbolKind::Function);

        let sub = result
            .definitions
            .iter()
            .find(|d| d.name == "subtract")
            .unwrap();
        assert_eq!(sub.kind, SymbolKind::Function);
    }

    #[test]
    fn test_parse_ts_interface_and_type() {
        let source = r#"
interface User {
    name: string;
    age: number;
}

type UserId = string;
"#;
        let result = parse_file("types.ts", source).unwrap();

        let user_iface = result
            .definitions
            .iter()
            .find(|d| d.name == "User")
            .unwrap();
        assert_eq!(user_iface.kind, SymbolKind::Interface);

        let user_id = result
            .definitions
            .iter()
            .find(|d| d.name == "UserId")
            .unwrap();
        assert_eq!(user_id.kind, SymbolKind::TypeAlias);
    }

    #[test]
    fn test_parse_ts_constants() {
        let source = r#"
const MAX_RETRIES = 3;
const API_URL = "https://example.com";
"#;
        let result = parse_file("config.ts", source).unwrap();
        assert_eq!(result.definitions.len(), 2);

        let max = result
            .definitions
            .iter()
            .find(|d| d.name == "MAX_RETRIES")
            .unwrap();
        assert_eq!(max.kind, SymbolKind::Constant);
    }

    // === TypeScript call sites ===

    #[test]
    fn test_parse_ts_call_sites() {
        let source = r#"
function processUser(user: User) {
    const validated = validateUser(user);
    const saved = db.save(validated);
    notifyAdmin(saved.id);
}
"#;
        let result = parse_file("handler.ts", source).unwrap();

        let call_names: Vec<&str> = result
            .call_sites
            .iter()
            .map(|c| c.callee.as_str())
            .collect();
        assert!(call_names.contains(&"validateUser"));
        assert!(call_names.contains(&"db.save"));
        assert!(call_names.contains(&"notifyAdmin"));

        // All calls should be inside processUser
        for call in &result.call_sites {
            assert_eq!(call.containing_function, Some("processUser".to_string()));
        }
    }

    #[test]
    fn test_parse_ts_call_sites_in_arrow() {
        let source = r#"
const handler = (req: Request) => {
    const data = parseBody(req);
    return respond(data);
};
"#;
        let result = parse_file("handler.ts", source).unwrap();
        let call_names: Vec<&str> = result
            .call_sites
            .iter()
            .map(|c| c.callee.as_str())
            .collect();
        assert!(call_names.contains(&"parseBody"));
        assert!(call_names.contains(&"respond"));

        for call in &result.call_sites {
            assert_eq!(call.containing_function, Some("handler".to_string()));
        }
    }

    // === Python imports ===

    #[test]
    fn test_parse_python_imports() {
        let source = r#"
import os
import json as j
from pathlib import Path
from typing import List, Optional
from . import utils
from ..models import User as U
"#;
        let result = parse_file("app.py", source).unwrap();
        assert_eq!(result.imports.len(), 6);

        // import os
        assert_eq!(result.imports[0].source, "os");
        assert!(result.imports[0].is_namespace);
        assert_eq!(result.imports[0].names[0].name, "os");

        // import json as j
        assert_eq!(result.imports[1].source, "json");
        assert_eq!(result.imports[1].names[0].name, "json");
        assert_eq!(result.imports[1].names[0].alias, Some("j".to_string()));

        // from pathlib import Path
        assert_eq!(result.imports[2].source, "pathlib");
        assert!(!result.imports[2].is_namespace);
        assert_eq!(result.imports[2].names[0].name, "Path");

        // from typing import List, Optional
        assert_eq!(result.imports[3].source, "typing");
        assert_eq!(result.imports[3].names.len(), 2);
        assert_eq!(result.imports[3].names[0].name, "List");
        assert_eq!(result.imports[3].names[1].name, "Optional");

        // from . import utils (relative import)
        assert_eq!(result.imports[4].source, ".");
        assert_eq!(result.imports[4].names[0].name, "utils");

        // from ..models import User as U
        assert!(result.imports[5].source.contains("models"));
        assert_eq!(result.imports[5].names[0].name, "User");
        assert_eq!(result.imports[5].names[0].alias, Some("U".to_string()));
    }

    // === Python definitions ===

    #[test]
    fn test_parse_python_functions() {
        let source = r#"
def greet(name: str) -> str:
    return f"Hello {name}"

class UserService:
    def create_user(self, data: dict) -> User:
        return User(**data)

    def delete_user(self, user_id: int) -> None:
        pass
"#;
        let result = parse_file("service.py", source).unwrap();

        // Top-level function
        let greet = result
            .definitions
            .iter()
            .find(|d| d.name == "greet")
            .unwrap();
        assert_eq!(greet.kind, SymbolKind::Function);

        // Class
        let svc = result
            .definitions
            .iter()
            .find(|d| d.name == "UserService")
            .unwrap();
        assert_eq!(svc.kind, SymbolKind::Class);

        // Methods
        let create = result
            .definitions
            .iter()
            .find(|d| d.name == "create_user")
            .unwrap();
        assert_eq!(create.kind, SymbolKind::Function);

        let delete = result
            .definitions
            .iter()
            .find(|d| d.name == "delete_user")
            .unwrap();
        assert_eq!(delete.kind, SymbolKind::Function);
    }

    #[test]
    fn test_parse_python_decorated_functions() {
        let source = r#"
from flask import Flask
app = Flask(__name__)

@app.route('/users', methods=['GET'])
def list_users():
    return get_all_users()

@staticmethod
def helper():
    pass
"#;
        let result = parse_file("routes.py", source).unwrap();

        let list_users = result
            .definitions
            .iter()
            .find(|d| d.name == "list_users")
            .unwrap();
        assert_eq!(list_users.kind, SymbolKind::Function);

        let helper = result
            .definitions
            .iter()
            .find(|d| d.name == "helper")
            .unwrap();
        assert_eq!(helper.kind, SymbolKind::Function);
    }

    // === Python class hierarchy ===

    #[test]
    fn test_parse_python_class_hierarchy() {
        let source = r#"
class Animal:
    pass

class Dog(Animal):
    def bark(self):
        pass

class GuideDog(Dog, ServiceAnimal):
    pass
"#;
        // Verify class definitions are detected
        let result = parse_file("models.py", source).unwrap();
        let classes: Vec<&str> = result
            .definitions
            .iter()
            .filter(|d| d.kind == SymbolKind::Class)
            .map(|d| d.name.as_str())
            .collect();
        assert!(classes.contains(&"Animal"));
        assert!(classes.contains(&"Dog"));
        assert!(classes.contains(&"GuideDog"));

        // Verify base class extraction
        let animal_bases = get_python_class_bases(source, "Animal").unwrap();
        assert!(animal_bases.is_empty());

        let dog_bases = get_python_class_bases(source, "Dog").unwrap();
        assert_eq!(dog_bases, vec!["Animal"]);

        let guide_bases = get_python_class_bases(source, "GuideDog").unwrap();
        assert_eq!(guide_bases, vec!["Dog", "ServiceAnimal"]);
    }

    // === Unknown language ===

    #[test]
    fn test_parse_unknown_language() {
        let source = "some random content that is not code";
        let result = parse_file("main.xyz", source).unwrap();
        assert_eq!(result.language, Language::Unknown);
        assert!(result.definitions.is_empty());
        assert!(result.imports.is_empty());
        assert!(result.exports.is_empty());
        assert!(result.call_sites.is_empty());
    }

    /// §13.3: Handles Rust `mod`, `use`, `pub` visibility.
    ///
    /// Rust is detected as Language::Rust but parsing currently falls through to the
    /// generic handler (no tree-sitter queries for Rust yet). This test verifies:
    /// 1. Rust files parse without error
    /// 2. Language is correctly detected as Rust
    /// 3. Graceful fallback produces empty definitions/imports/exports
    #[test]
    fn test_parse_rust_modules() {
        let source = r#"
mod handlers;
mod models;

use std::collections::HashMap;
use crate::models::User;

pub fn create_user(name: &str) -> User {
    User { name: name.to_string() }
}

pub(crate) fn internal_helper() -> bool {
    true
}
"#;
        let result = parse_file("src/lib.rs", source).unwrap();

        // Language should be correctly detected
        assert_eq!(result.language, Language::Rust);
        assert_eq!(result.path, "src/lib.rs");

        // Rust parsing is not yet implemented via tree-sitter queries,
        // so definitions/imports/exports are empty (graceful fallback).
        // This documents the current state and will catch when Rust parsing is added.
        assert!(
            result.definitions.is_empty(),
            "Rust definitions are not yet extracted (graceful fallback)"
        );
        assert!(
            result.imports.is_empty(),
            "Rust imports are not yet extracted (graceful fallback)"
        );
        assert!(
            result.exports.is_empty(),
            "Rust exports are not yet extracted (graceful fallback)"
        );
    }

    // === Changed symbols detection ===

    #[test]
    fn test_changed_symbols_detection() {
        let old_source = r#"
function foo() {}
function bar() {}
const VALUE = 42;
"#;
        let new_source = r#"
function foo() {
    return 1;
}
function baz() {}
const VALUE = 42;
"#;
        let old = parse_file("lib.ts", old_source).unwrap();
        let new = parse_file("lib.ts", new_source).unwrap();
        let changes = detect_changed_symbols(&old, &new);

        let added: Vec<&str> = changes
            .iter()
            .filter_map(|c| match c {
                SymbolChange::Added(d) => Some(d.name.as_str()),
                _ => None,
            })
            .collect();
        assert!(added.contains(&"baz"), "baz should be added");

        let removed: Vec<&str> = changes
            .iter()
            .filter_map(|c| match c {
                SymbolChange::Removed(d) => Some(d.name.as_str()),
                _ => None,
            })
            .collect();
        assert!(removed.contains(&"bar"), "bar should be removed");

        let modified: Vec<&str> = changes
            .iter()
            .filter_map(|c| match c {
                SymbolChange::Modified { old, .. } => Some(old.name.as_str()),
                _ => None,
            })
            .collect();
        assert!(modified.contains(&"foo"), "foo should be modified");

        // VALUE unchanged
        assert!(
            !changes.iter().any(|c| match c {
                SymbolChange::Added(d) | SymbolChange::Removed(d) => d.name == "VALUE",
                SymbolChange::Modified { old, .. } => old.name == "VALUE",
            }),
            "VALUE should be unchanged"
        );
    }

    #[test]
    fn test_changed_symbols_no_changes() {
        let source = "function foo() {}\n";
        let old = parse_file("lib.ts", source).unwrap();
        let new = parse_file("lib.ts", source).unwrap();
        let changes = detect_changed_symbols(&old, &new);
        assert!(changes.is_empty());
    }

    // === Language detection ===

    #[test]
    fn test_language_from_path() {
        assert_eq!(Language::from_path("app.ts"), Language::TypeScript);
        assert_eq!(Language::from_path("app.tsx"), Language::TypeScript);
        assert_eq!(Language::from_path("app.js"), Language::JavaScript);
        assert_eq!(Language::from_path("app.jsx"), Language::JavaScript);
        assert_eq!(Language::from_path("app.mjs"), Language::JavaScript);
        assert_eq!(Language::from_path("app.cjs"), Language::JavaScript);
        assert_eq!(Language::from_path("app.py"), Language::Python);
        assert_eq!(Language::from_path("app.pyi"), Language::Python);
        assert_eq!(Language::from_path("app.go"), Language::Go);
        assert_eq!(Language::from_path("app.rs"), Language::Rust);
        assert_eq!(Language::from_path("Makefile"), Language::Unknown);
    }

    // === Line numbers ===

    #[test]
    fn test_definition_line_numbers() {
        let source = "function foo() {\n  return 1;\n}\n\nfunction bar() {\n  return 2;\n}\n";
        let result = parse_file("lib.ts", source).unwrap();

        let foo = result.definitions.iter().find(|d| d.name == "foo").unwrap();
        assert_eq!(foo.start_line, 1);
        assert_eq!(foo.end_line, 3);

        let bar = result.definitions.iter().find(|d| d.name == "bar").unwrap();
        assert_eq!(bar.start_line, 5);
        assert_eq!(bar.end_line, 7);
    }

    // === Performance ===

    #[test]
    fn test_large_file_performance() {
        // Generate a 10K+ line TypeScript file
        let mut source = String::with_capacity(2_000_000);
        for i in 0..3000 {
            source.push_str(&format!(
                "function func_{i}(x: number): number {{\n  return x * {i};\n}}\n\n"
            ));
        }
        for i in 0..500 {
            source.push_str(&format!("const arrow_{i} = (x: number) => x + {i};\n"));
        }
        for i in 0..100 {
            source.push_str(&format!(
                "class Class_{i} {{\n  method_a() {{ return func_{i}(1); }}\n  method_b() {{ return arrow_{i}(2); }}\n}}\n\n"
            ));
        }

        let line_count = source.lines().count();
        assert!(
            line_count > 10_000,
            "generated file should have 10K+ lines, got {line_count}"
        );

        let start = std::time::Instant::now();
        let result = parse_file("large.ts", &source).unwrap();
        let elapsed = start.elapsed();

        assert!(
            elapsed.as_millis() < 500,
            "parsing 10K+ line file took {}ms, should be < 500ms",
            elapsed.as_millis()
        );

        // Sanity check: we extracted definitions
        assert!(result.definitions.len() > 3000);
        assert!(!result.call_sites.is_empty());
    }

    // === Python call sites ===

    #[test]
    fn test_parse_python_call_sites() {
        let source = r#"
def process(data):
    validated = validate(data)
    result = db.save(validated)
    return result
"#;
        let result = parse_file("handler.py", source).unwrap();
        let call_names: Vec<&str> = result
            .call_sites
            .iter()
            .map(|c| c.callee.as_str())
            .collect();
        assert!(call_names.contains(&"validate"));
        assert!(call_names.contains(&"db.save"));

        for call in &result.call_sites {
            assert_eq!(call.containing_function, Some("process".to_string()));
        }
    }

    // === Edge cases ===

    #[test]
    fn test_empty_source() {
        let result = parse_file("empty.ts", "").unwrap();
        assert!(result.definitions.is_empty());
        assert!(result.imports.is_empty());
        assert!(result.exports.is_empty());
        assert!(result.call_sites.is_empty());
    }

    #[test]
    fn test_parse_js_file_uses_typescript_parser() {
        let source = "function hello() { console.log('hi'); }\n";
        let result = parse_file("app.js", source).unwrap();
        assert_eq!(result.language, Language::JavaScript);
        assert_eq!(result.definitions.len(), 1);
        assert_eq!(result.definitions[0].name, "hello");
    }

    #[test]
    fn test_export_class_with_methods() {
        let source = r#"
export class Router {
    get(path: string) {}
    post(path: string) {}
}
"#;
        let result = parse_file("router.ts", source).unwrap();

        let class_export = result.exports.iter().find(|e| e.name == "Router").unwrap();
        assert!(!class_export.is_default);

        let methods: Vec<&str> = result
            .definitions
            .iter()
            .filter(|d| d.name == "get" || d.name == "post")
            .map(|d| d.name.as_str())
            .collect();
        assert!(methods.contains(&"get"));
        assert!(methods.contains(&"post"));
    }

    // ========================================================================
    // Data flow extraction — TypeScript
    // ========================================================================

    #[test]
    fn test_data_flow_ts_simple_assignment() {
        let source = r#"
function handler(req: any) {
    const data = parseBody(req);
    return respond(data);
}
"#;
        let info = extract_data_flow_info("handler.ts", source).unwrap();

        // Should detect `const data = parseBody(req)`
        assert_eq!(info.assignments.len(), 1);
        assert_eq!(info.assignments[0].variable, "data");
        assert_eq!(info.assignments[0].callee, "parseBody");
        assert_eq!(
            info.assignments[0].containing_function,
            Some("handler".to_string())
        );

        // Should detect both calls with their arguments
        let parse_call = info
            .calls_with_args
            .iter()
            .find(|c| c.callee == "parseBody")
            .unwrap();
        assert!(parse_call.arguments.contains(&"req".to_string()));

        let respond_call = info
            .calls_with_args
            .iter()
            .find(|c| c.callee == "respond")
            .unwrap();
        assert!(respond_call.arguments.contains(&"data".to_string()));
    }

    #[test]
    fn test_data_flow_ts_method_call_assignment() {
        let source = r#"
function process() {
    const user = db.findOne(id);
    return transform(user);
}
"#;
        let info = extract_data_flow_info("service.ts", source).unwrap();

        assert_eq!(info.assignments.len(), 1);
        assert_eq!(info.assignments[0].variable, "user");
        assert_eq!(info.assignments[0].callee, "db.findOne");
    }

    #[test]
    fn test_data_flow_ts_await_assignment() {
        let source = r#"
async function handler(req: any) {
    const data = await fetchData(req.id);
    return process(data);
}
"#;
        let info = extract_data_flow_info("handler.ts", source).unwrap();

        // Should unwrap the await and capture the call
        assert_eq!(info.assignments.len(), 1);
        assert_eq!(info.assignments[0].variable, "data");
        assert_eq!(info.assignments[0].callee, "fetchData");
    }

    #[test]
    fn test_data_flow_ts_chained_assignments() {
        let source = r#"
function pipeline(input: any) {
    const validated = validate(input);
    const processed = transform(validated);
    const result = save(processed);
    return result;
}
"#;
        let info = extract_data_flow_info("pipeline.ts", source).unwrap();

        assert_eq!(info.assignments.len(), 3);

        let vars: Vec<&str> = info
            .assignments
            .iter()
            .map(|a| a.variable.as_str())
            .collect();
        assert!(vars.contains(&"validated"));
        assert!(vars.contains(&"processed"));
        assert!(vars.contains(&"result"));

        let callees: Vec<&str> = info.assignments.iter().map(|a| a.callee.as_str()).collect();
        assert!(callees.contains(&"validate"));
        assert!(callees.contains(&"transform"));
        assert!(callees.contains(&"save"));
    }

    #[test]
    fn test_data_flow_ts_call_arguments_multiple() {
        let source = r#"
function merge(a: any, b: any) {
    const x = getFirst();
    const y = getSecond();
    return combine(x, y, 42);
}
"#;
        let info = extract_data_flow_info("merge.ts", source).unwrap();

        let combine_call = info
            .calls_with_args
            .iter()
            .find(|c| c.callee == "combine")
            .unwrap();
        assert!(combine_call.arguments.contains(&"x".to_string()));
        assert!(combine_call.arguments.contains(&"y".to_string()));
        // 42 is a literal, should also be captured as argument text
        assert!(combine_call.arguments.contains(&"42".to_string()));
    }

    #[test]
    fn test_data_flow_ts_arrow_function() {
        let source = r#"
const handler = (req: any) => {
    const data = parseBody(req);
    return respond(data);
};
"#;
        let info = extract_data_flow_info("handler.ts", source).unwrap();

        assert_eq!(info.assignments.len(), 1);
        assert_eq!(info.assignments[0].variable, "data");
        assert_eq!(info.assignments[0].callee, "parseBody");
        assert_eq!(
            info.assignments[0].containing_function,
            Some("handler".to_string())
        );
    }

    #[test]
    fn test_data_flow_ts_no_assignments() {
        let source = r#"
function simple() {
    console.log("hello");
    return 42;
}
"#;
        let info = extract_data_flow_info("simple.ts", source).unwrap();
        assert!(info.assignments.is_empty());
    }

    #[test]
    fn test_data_flow_ts_module_level() {
        let source = r#"
const config = loadConfig();
startServer(config);
"#;
        let info = extract_data_flow_info("main.ts", source).unwrap();

        assert_eq!(info.assignments.len(), 1);
        assert_eq!(info.assignments[0].variable, "config");
        assert_eq!(info.assignments[0].callee, "loadConfig");
        // Module-level has no containing function
        assert_eq!(info.assignments[0].containing_function, None);

        let start_call = info
            .calls_with_args
            .iter()
            .find(|c| c.callee == "startServer")
            .unwrap();
        assert!(start_call.arguments.contains(&"config".to_string()));
    }

    #[test]
    fn test_data_flow_ts_nested_call_as_argument() {
        let source = r#"
function process() {
    return save(transform(input));
}
"#;
        let info = extract_data_flow_info("process.ts", source).unwrap();

        // The inner call `transform(input)` should be captured
        let save_call = info
            .calls_with_args
            .iter()
            .find(|c| c.callee == "save")
            .unwrap();
        // The argument to save is the full nested call text
        assert_eq!(save_call.arguments.len(), 1);
        assert!(save_call.arguments[0].contains("transform"));
    }

    #[test]
    fn test_data_flow_ts_non_call_value_ignored() {
        let source = r#"
function process() {
    const x = 42;
    const y = "hello";
    const z = someVar;
    return x;
}
"#;
        let info = extract_data_flow_info("process.ts", source).unwrap();

        // None of these are function call assignments
        assert!(
            info.assignments.is_empty(),
            "literal and variable assignments should not be captured"
        );
    }

    // ========================================================================
    // Data flow extraction — Python
    // ========================================================================

    #[test]
    fn test_data_flow_python_simple_assignment() {
        let source = r#"
def handler(req):
    data = parse_body(req)
    return respond(data)
"#;
        let info = extract_data_flow_info("handler.py", source).unwrap();

        assert_eq!(info.assignments.len(), 1);
        assert_eq!(info.assignments[0].variable, "data");
        assert_eq!(info.assignments[0].callee, "parse_body");
        assert_eq!(
            info.assignments[0].containing_function,
            Some("handler".to_string())
        );

        let respond_call = info
            .calls_with_args
            .iter()
            .find(|c| c.callee == "respond")
            .unwrap();
        assert!(respond_call.arguments.contains(&"data".to_string()));
    }

    #[test]
    fn test_data_flow_python_chained() {
        let source = r#"
def pipeline(raw):
    validated = validate(raw)
    processed = transform(validated)
    save(processed)
"#;
        let info = extract_data_flow_info("pipeline.py", source).unwrap();

        assert_eq!(info.assignments.len(), 2);

        let vars: Vec<&str> = info
            .assignments
            .iter()
            .map(|a| a.variable.as_str())
            .collect();
        assert!(vars.contains(&"validated"));
        assert!(vars.contains(&"processed"));
    }

    #[test]
    fn test_data_flow_python_method_call() {
        let source = r#"
def get_user(user_id):
    user = db.find_one(user_id)
    return serialize(user)
"#;
        let info = extract_data_flow_info("service.py", source).unwrap();

        assert_eq!(info.assignments.len(), 1);
        assert_eq!(info.assignments[0].callee, "db.find_one");
    }

    #[test]
    fn test_data_flow_unknown_language() {
        let info = extract_data_flow_info("main.rs", "fn main() {}").unwrap();
        assert!(info.assignments.is_empty());
        assert!(info.calls_with_args.is_empty());
    }

    #[test]
    fn test_data_flow_empty_source() {
        let info = extract_data_flow_info("empty.ts", "").unwrap();
        assert!(info.assignments.is_empty());
        assert!(info.calls_with_args.is_empty());
    }

    #[test]
    fn test_data_flow_ts_multiple_consumers() {
        let source = r#"
function process() {
    const data = fetchData();
    validate(data);
    transform(data);
    save(data);
}
"#;
        let info = extract_data_flow_info("process.ts", source).unwrap();

        // One assignment, three consumers
        assert_eq!(info.assignments.len(), 1);
        assert_eq!(info.assignments[0].variable, "data");

        let consumers_using_data: Vec<&str> = info
            .calls_with_args
            .iter()
            .filter(|c| c.arguments.contains(&"data".to_string()))
            .map(|c| c.callee.as_str())
            .collect();
        assert!(consumers_using_data.contains(&"validate"));
        assert!(consumers_using_data.contains(&"transform"));
        assert!(consumers_using_data.contains(&"save"));
    }

    // ========================================================================
    // Phase 8 audit: edge case tests
    // ========================================================================

    #[test]
    fn test_ts_enum_declaration_not_captured() {
        // Known limitation: TS enums are not extracted as definitions.
        // This test documents the behavior so it's visible.
        let source = r#"
enum Color {
    Red,
    Green,
    Blue,
}
"#;
        let result = parse_file("types.ts", source).unwrap();
        // Enums are not captured — this documents the gap.
        assert!(
            result.definitions.iter().all(|d| d.name != "Color"),
            "TS enums are not captured by the current parser"
        );
    }

    #[test]
    fn test_changed_symbols_same_span_different_body() {
        // If a function changes body but keeps the same line count,
        // detect_changed_symbols won't flag it as modified (by design — compares span size).
        let old_source = "function foo() {\n  return 1;\n}\n";
        let new_source = "function foo() {\n  return 2;\n}\n";
        let old = parse_file("lib.ts", old_source).unwrap();
        let new = parse_file("lib.ts", new_source).unwrap();
        let changes = detect_changed_symbols(&old, &new);
        // Same span size → not detected as modified (design limitation)
        assert!(
            changes.is_empty(),
            "same-span changes are not detected by span comparison"
        );
    }

    #[test]
    fn test_ts_abstract_class() {
        let source = r#"
abstract class BaseService {
    abstract process(): void;
    helper() { return 1; }
}
"#;
        let result = parse_file("service.ts", source).unwrap();
        let base_svc = result.definitions.iter().find(|d| d.name == "BaseService");
        assert!(base_svc.is_some(), "abstract classes should be captured");
        assert_eq!(base_svc.unwrap().kind, SymbolKind::Class);

        // Method inside abstract class
        assert!(result.definitions.iter().any(|d| d.name == "helper"));
    }

    #[test]
    fn test_ts_generator_function() {
        let source = r#"
function* generate() {
    yield 1;
    yield 2;
}
"#;
        let result = parse_file("gen.ts", source).unwrap();
        let gen = result.definitions.iter().find(|d| d.name == "generate");
        assert!(gen.is_some(), "generator functions should be captured");
        assert_eq!(gen.unwrap().kind, SymbolKind::Function);
    }

    #[test]
    fn test_ts_multiple_classes_with_methods() {
        let source = r#"
class A {
    foo() {}
}
class B {
    foo() {}
    bar() {}
}
"#;
        let result = parse_file("classes.ts", source).unwrap();
        let classes: Vec<&str> = result
            .definitions
            .iter()
            .filter(|d| d.kind == SymbolKind::Class)
            .map(|d| d.name.as_str())
            .collect();
        assert!(classes.contains(&"A"));
        assert!(classes.contains(&"B"));

        // Both classes have foo() methods — both should be captured
        let foos: Vec<&Definition> = result
            .definitions
            .iter()
            .filter(|d| d.name == "foo" && d.kind == SymbolKind::Function)
            .collect();
        assert_eq!(foos.len(), 2, "both foo() methods should be captured");
    }

    #[test]
    fn test_ts_unicode_identifiers() {
        let source = r#"
function grüßen(名前: string): string {
    return `Hello ${名前}`;
}
const αβγ = 42;
"#;
        let result = parse_file("unicode.ts", source).unwrap();
        assert!(result.definitions.iter().any(|d| d.name == "grüßen"));
        assert!(result.definitions.iter().any(|d| d.name == "αβγ"));
    }

    #[test]
    fn test_ts_syntax_error_partial_parse() {
        // tree-sitter does partial parsing on syntax errors but recovery is
        // not guaranteed for all subsequent definitions.
        let source = r#"
function valid() { return 1; }
const x = {{{
function alsoValid() { return 2; }
"#;
        let result = parse_file("broken.ts", source).unwrap();
        // The definition before the error should be extracted
        assert!(result.definitions.iter().any(|d| d.name == "valid"));
        // Parsing doesn't fail — no panic, just potentially missing later defs
        assert!(result.language == Language::TypeScript);
    }

    #[test]
    fn test_ts_export_default_class() {
        let source = r#"export default class App {
    render() {}
}"#;
        let result = parse_file("app.ts", source).unwrap();
        let app_export = result.exports.iter().find(|e| e.name == "App");
        assert!(
            app_export.is_some(),
            "export default class should be captured"
        );
        assert!(app_export.unwrap().is_default);
    }

    #[test]
    fn test_ts_export_interface_and_type() {
        let source = r#"
export interface Config {
    port: number;
}
export type ID = string;
"#;
        let result = parse_file("types.ts", source).unwrap();
        assert!(result.exports.iter().any(|e| e.name == "Config"));
        assert!(result.exports.iter().any(|e| e.name == "ID"));
        assert!(result
            .definitions
            .iter()
            .any(|d| d.name == "Config" && d.kind == SymbolKind::Interface));
        assert!(result
            .definitions
            .iter()
            .any(|d| d.name == "ID" && d.kind == SymbolKind::TypeAlias));
    }

    #[test]
    fn test_python_decorated_class_with_methods() {
        let source = r#"
@dataclass
class User:
    name: str

    def greet(self):
        pass

    @staticmethod
    def create(name):
        pass
"#;
        let result = parse_file("models.py", source).unwrap();
        assert!(result
            .definitions
            .iter()
            .any(|d| d.name == "User" && d.kind == SymbolKind::Class));
        assert!(result.definitions.iter().any(|d| d.name == "greet"));
        assert!(result.definitions.iter().any(|d| d.name == "create"));
    }

    #[test]
    fn test_python_wildcard_import() {
        let source = "from os.path import *\n";
        let result = parse_file("app.py", source).unwrap();
        assert_eq!(result.imports.len(), 1);
        assert!(result.imports[0].names.iter().any(|n| n.name == "*"));
    }

    #[test]
    fn test_python_relative_import_parent() {
        let source = "from .. import utils\n";
        let result = parse_file("sub/mod.py", source).unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].source, "..");
    }

    #[test]
    fn test_ts_deeply_nested_calls() {
        // Verify recursive call collection handles nesting
        let source = r#"
function outer() {
    function middle() {
        function inner() {
            deepCall();
        }
        middleCall();
    }
    outerCall();
}
"#;
        let result = parse_file("nested.ts", source).unwrap();
        let callees: Vec<&str> = result
            .call_sites
            .iter()
            .map(|c| c.callee.as_str())
            .collect();
        assert!(callees.contains(&"deepCall"));
        assert!(callees.contains(&"middleCall"));
        assert!(callees.contains(&"outerCall"));

        // Containing function resolution
        let deep = result
            .call_sites
            .iter()
            .find(|c| c.callee == "deepCall")
            .unwrap();
        assert_eq!(deep.containing_function, Some("inner".to_string()));
    }

    #[test]
    fn test_ts_module_level_calls_no_containing() {
        let source = "init();\nconfigure();\n";
        let result = parse_file("init.ts", source).unwrap();
        for call in &result.call_sites {
            assert_eq!(
                call.containing_function, None,
                "top-level calls should have no containing function"
            );
        }
    }

    #[test]
    fn test_language_from_path_edge_cases() {
        assert_eq!(Language::from_path(""), Language::Unknown);
        assert_eq!(Language::from_path("noext"), Language::Unknown);
        assert_eq!(Language::from_path(".ts"), Language::TypeScript);
        assert_eq!(Language::from_path("a/b/c.py"), Language::Python);
        assert_eq!(Language::from_path("my.module.ts"), Language::TypeScript);
    }

    #[test]
    fn test_ts_comments_only_file() {
        let source = r#"
// This is a comment
/* block comment */
/** JSDoc */
"#;
        let result = parse_file("comments.ts", source).unwrap();
        assert!(result.definitions.is_empty());
        assert!(result.imports.is_empty());
        assert!(result.exports.is_empty());
        assert!(result.call_sites.is_empty());
    }

    #[test]
    fn test_data_flow_python_keyword_args_only() {
        let source = r#"
def main():
    result = connect(host='localhost', port=5432)
"#;
        let info = extract_data_flow_info("main.py", source).unwrap();

        let connect_call = info
            .calls_with_args
            .iter()
            .find(|c| c.callee == "connect")
            .unwrap();
        // Keyword arg values should be captured
        assert!(connect_call.arguments.contains(&"'localhost'".to_string()));
        assert!(connect_call.arguments.contains(&"5432".to_string()));
    }

    #[test]
    fn test_ts_let_var_declarations() {
        let source = r#"
let mutable = 42;
var legacy = "old";
"#;
        let result = parse_file("vars.ts", source).unwrap();
        assert!(result.definitions.iter().any(|d| d.name == "mutable"));
        assert!(result.definitions.iter().any(|d| d.name == "legacy"));
    }

    #[test]
    fn test_ts_export_multiple_vars() {
        let source = "export const A = 1, B = 2;\n";
        let result = parse_file("consts.ts", source).unwrap();
        assert!(result.exports.iter().any(|e| e.name == "A"));
        assert!(result.exports.iter().any(|e| e.name == "B"));
    }

    // ========================================================================
    // Go parsing tests
    // ========================================================================

    #[test]
    fn test_go_language_detection() {
        assert_eq!(Language::from_path("main.go"), Language::Go);
        assert_eq!(Language::from_path("handlers/user.go"), Language::Go);
    }

    #[test]
    fn test_go_simple_imports() {
        let source = r#"
package main

import "fmt"
import "net/http"
"#;
        let result = parse_file("main.go", source).unwrap();
        assert_eq!(result.language, Language::Go);
        assert_eq!(result.imports.len(), 2);

        assert_eq!(result.imports[0].source, "fmt");
        assert!(result.imports[0].is_namespace);
        assert_eq!(result.imports[0].names[0].name, "fmt");

        assert_eq!(result.imports[1].source, "net/http");
        assert!(result.imports[1].is_namespace);
        assert_eq!(result.imports[1].names[0].name, "http");
    }

    #[test]
    fn test_go_grouped_imports() {
        let source = r#"
package main

import (
    "fmt"
    "net/http"
    "github.com/gin-gonic/gin"
)
"#;
        let result = parse_file("main.go", source).unwrap();
        assert_eq!(result.imports.len(), 3);
        assert_eq!(result.imports[0].source, "fmt");
        assert_eq!(result.imports[1].source, "net/http");
        assert_eq!(result.imports[2].source, "github.com/gin-gonic/gin");
        assert_eq!(result.imports[2].names[0].name, "gin");
    }

    #[test]
    fn test_go_aliased_import() {
        let source = r#"
package main

import (
    myhttp "net/http"
    _ "database/sql"
)
"#;
        let result = parse_file("main.go", source).unwrap();
        assert_eq!(result.imports.len(), 2);

        // Aliased import
        assert_eq!(result.imports[0].source, "net/http");
        assert_eq!(result.imports[0].names[0].name, "http");
        assert_eq!(result.imports[0].names[0].alias, Some("myhttp".to_string()));

        // Blank import (side-effect only)
        assert_eq!(result.imports[1].source, "database/sql");
        assert!(result.imports[1].names.is_empty());
    }

    #[test]
    fn test_go_function_definitions() {
        let source = r#"
package main

func main() {
    fmt.Println("Hello")
}

func greet(name string) string {
    return "Hello " + name
}

func add(a, b int) int {
    return a + b
}
"#;
        let result = parse_file("main.go", source).unwrap();
        assert_eq!(result.language, Language::Go);

        let fns: Vec<&str> = result
            .definitions
            .iter()
            .filter(|d| d.kind == SymbolKind::Function)
            .map(|d| d.name.as_str())
            .collect();
        assert!(fns.contains(&"main"));
        assert!(fns.contains(&"greet"));
        assert!(fns.contains(&"add"));
    }

    #[test]
    fn test_go_struct_definitions() {
        let source = r#"
package models

type User struct {
    ID   int
    Name string
    Email string
}

type Config struct {
    Port int
    Host string
}
"#;
        let result = parse_file("models.go", source).unwrap();

        let user = result
            .definitions
            .iter()
            .find(|d| d.name == "User")
            .unwrap();
        assert_eq!(user.kind, SymbolKind::Class); // structs map to Class

        let config = result
            .definitions
            .iter()
            .find(|d| d.name == "Config")
            .unwrap();
        assert_eq!(config.kind, SymbolKind::Class);
    }

    #[test]
    fn test_go_interface_definitions() {
        let source = r#"
package service

type UserService interface {
    GetUser(id int) (*User, error)
    CreateUser(data UserInput) (*User, error)
    DeleteUser(id int) error
}

type Repository interface {
    Find(id int) (interface{}, error)
    Save(entity interface{}) error
}
"#;
        let result = parse_file("service.go", source).unwrap();

        let user_svc = result
            .definitions
            .iter()
            .find(|d| d.name == "UserService")
            .unwrap();
        assert_eq!(user_svc.kind, SymbolKind::Interface);

        let repo = result
            .definitions
            .iter()
            .find(|d| d.name == "Repository")
            .unwrap();
        assert_eq!(repo.kind, SymbolKind::Interface);
    }

    #[test]
    fn test_go_method_declarations() {
        let source = r#"
package models

type User struct {
    Name string
}

func (u *User) Greet() string {
    return "Hello " + u.Name
}

func (u User) String() string {
    return u.Name
}
"#;
        let result = parse_file("models.go", source).unwrap();

        let methods: Vec<&str> = result
            .definitions
            .iter()
            .filter(|d| d.kind == SymbolKind::Function)
            .map(|d| d.name.as_str())
            .collect();
        assert!(
            methods.contains(&"Greet"),
            "method Greet should be detected"
        );
        assert!(
            methods.contains(&"String"),
            "method String should be detected"
        );
    }

    #[test]
    fn test_go_constants() {
        let source = r#"
package config

const MaxRetries = 3
const (
    DefaultPort = 8080
    DefaultHost = "localhost"
)
"#;
        let result = parse_file("config.go", source).unwrap();

        let consts: Vec<&str> = result
            .definitions
            .iter()
            .filter(|d| d.kind == SymbolKind::Constant)
            .map(|d| d.name.as_str())
            .collect();
        assert!(consts.contains(&"MaxRetries"));
        assert!(consts.contains(&"DefaultPort"));
        assert!(consts.contains(&"DefaultHost"));
    }

    #[test]
    fn test_go_type_alias() {
        let source = r#"
package types

type UserID int64
type Handler func(w http.ResponseWriter, r *http.Request)
"#;
        let result = parse_file("types.go", source).unwrap();

        // UserID should be detected as TypeAlias (not struct/interface)
        let user_id = result
            .definitions
            .iter()
            .find(|d| d.name == "UserID")
            .unwrap();
        assert_eq!(user_id.kind, SymbolKind::TypeAlias);

        let handler = result
            .definitions
            .iter()
            .find(|d| d.name == "Handler")
            .unwrap();
        assert_eq!(handler.kind, SymbolKind::TypeAlias);
    }

    #[test]
    fn test_go_call_sites() {
        let source = r#"
package main

import "fmt"

func process(data string) {
    validated := validate(data)
    result := db.Save(validated)
    fmt.Println(result)
}
"#;
        let result = parse_file("handler.go", source).unwrap();

        let callees: Vec<&str> = result
            .call_sites
            .iter()
            .map(|c| c.callee.as_str())
            .collect();
        assert!(callees.contains(&"validate"));
        assert!(callees.contains(&"db.Save"));
        assert!(callees.contains(&"fmt.Println"));

        // All calls inside process function
        for call in &result.call_sites {
            assert_eq!(call.containing_function, Some("process".to_string()));
        }
    }

    #[test]
    fn test_go_call_sites_in_method() {
        let source = r#"
package service

func (s *UserService) Create(data UserInput) (*User, error) {
    validated := s.validate(data)
    return s.repo.Save(validated)
}
"#;
        let result = parse_file("service.go", source).unwrap();

        let callees: Vec<&str> = result
            .call_sites
            .iter()
            .map(|c| c.callee.as_str())
            .collect();
        assert!(callees.contains(&"s.validate"));
        assert!(callees.contains(&"s.repo.Save"));

        for call in &result.call_sites {
            assert_eq!(call.containing_function, Some("Create".to_string()));
        }
    }

    #[test]
    fn test_go_exported_symbols() {
        let source = r#"
package models

type User struct {
    Name string
}

type internalState struct {
    cache map[string]string
}

func GetUser(id int) *User {
    return nil
}

func helper() {
}

const MaxSize = 100
const defaultTimeout = 30
"#;
        let result = parse_file("models.go", source).unwrap();

        let export_names: Vec<&str> = result.exports.iter().map(|e| e.name.as_str()).collect();
        // Uppercase = exported
        assert!(export_names.contains(&"User"));
        assert!(export_names.contains(&"GetUser"));
        assert!(export_names.contains(&"MaxSize"));
        // Lowercase = not exported
        assert!(!export_names.contains(&"internalState"));
        assert!(!export_names.contains(&"helper"));
        assert!(!export_names.contains(&"defaultTimeout"));
    }

    #[test]
    fn test_go_data_flow_short_var_decl() {
        let source = r#"
package main

func handler(req string) {
    data := parseBody(req)
    result := transform(data)
    save(result)
}
"#;
        let info = extract_data_flow_info("handler.go", source).unwrap();

        assert_eq!(info.assignments.len(), 2);

        let vars: Vec<&str> = info
            .assignments
            .iter()
            .map(|a| a.variable.as_str())
            .collect();
        assert!(vars.contains(&"data"));
        assert!(vars.contains(&"result"));

        let callees: Vec<&str> = info.assignments.iter().map(|a| a.callee.as_str()).collect();
        assert!(callees.contains(&"parseBody"));
        assert!(callees.contains(&"transform"));
    }

    #[test]
    fn test_go_data_flow_method_call() {
        let source = r#"
package main

func getUser(id int) {
    user := db.FindOne(id)
    save(user)
}
"#;
        let info = extract_data_flow_info("service.go", source).unwrap();

        assert_eq!(info.assignments.len(), 1);
        assert_eq!(info.assignments[0].variable, "user");
        assert_eq!(info.assignments[0].callee, "db.FindOne");
        assert_eq!(
            info.assignments[0].containing_function,
            Some("getUser".to_string())
        );
    }

    #[test]
    fn test_go_data_flow_call_with_args() {
        let source = r#"
package main

func process() {
    x := getFirst()
    y := getSecond()
    combine(x, y, 42)
}
"#;
        let info = extract_data_flow_info("process.go", source).unwrap();

        let combine = info
            .calls_with_args
            .iter()
            .find(|c| c.callee == "combine")
            .unwrap();
        assert!(combine.arguments.contains(&"x".to_string()));
        assert!(combine.arguments.contains(&"y".to_string()));
        assert!(combine.arguments.contains(&"42".to_string()));
    }

    #[test]
    fn test_go_empty_source() {
        let source = "package main\n";
        let result = parse_file("empty.go", source).unwrap();
        assert_eq!(result.language, Language::Go);
        assert!(result.definitions.is_empty());
        assert!(result.imports.is_empty());
        assert!(result.call_sites.is_empty());
    }

    #[test]
    fn test_go_goroutine_call() {
        let source = r#"
package main

func startWorker() {
    go processQueue()
    go handleMessages()
}
"#;
        let result = parse_file("worker.go", source).unwrap();

        // Goroutine calls should still be detected as call sites
        let callees: Vec<&str> = result
            .call_sites
            .iter()
            .map(|c| c.callee.as_str())
            .collect();
        assert!(callees.contains(&"processQueue"));
        assert!(callees.contains(&"handleMessages"));
    }

    #[test]
    fn test_go_var_declaration() {
        let source = r#"
package config

var GlobalConfig Config
var (
    Logger  *log.Logger
    Verbose bool
)
"#;
        let result = parse_file("config.go", source).unwrap();
        let vars: Vec<&str> = result.definitions.iter().map(|d| d.name.as_str()).collect();
        assert!(vars.contains(&"GlobalConfig"));
        assert!(vars.contains(&"Logger"));
        assert!(vars.contains(&"Verbose"));
    }

    #[test]
    fn test_go_http_handler_pattern() {
        let source = r#"
package main

import "net/http"

func main() {
    http.HandleFunc("/users", handleUsers)
    http.ListenAndServe(":8080", nil)
}

func handleUsers(w http.ResponseWriter, r *http.Request) {
    fmt.Fprintln(w, "users")
}
"#;
        let result = parse_file("main.go", source).unwrap();

        let fns: Vec<&str> = result
            .definitions
            .iter()
            .filter(|d| d.kind == SymbolKind::Function)
            .map(|d| d.name.as_str())
            .collect();
        assert!(fns.contains(&"main"));
        assert!(fns.contains(&"handleUsers"));

        // Verify http import
        assert!(result.imports.iter().any(|i| i.source == "net/http"));

        // Verify call sites
        let callees: Vec<&str> = result
            .call_sites
            .iter()
            .map(|c| c.callee.as_str())
            .collect();
        assert!(callees.contains(&"http.HandleFunc"));
        assert!(callees.contains(&"http.ListenAndServe"));
    }
