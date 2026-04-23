//! Entrypoint detection module.
//!
//! Automatically detects entry points into the application by analyzing
//! file paths, AST-extracted symbols, imports, call sites, and exports.

use crate::ast::{ImportInfo, Language, ParsedFile};
use crate::ir::IrFile;
use crate::types::{Entrypoint, EntrypointType};

/// Detect all entrypoints across a set of parsed files.
pub fn detect_entrypoints(files: &[ParsedFile]) -> Vec<Entrypoint> {
    let mut entrypoints = Vec::new();
    for file in files {
        detect_file_entrypoints(file, &mut entrypoints);
    }
    // Deduplicate by (file, symbol) pair
    entrypoints.sort_by(|a, b| (&a.file, &a.symbol).cmp(&(&b.file, &b.symbol)));
    entrypoints.dedup_by(|a, b| a.file == b.file && a.symbol == b.symbol);
    entrypoints
}

/// Detect all entrypoints from IR files (declarative query engine / IR path).
///
/// Converts IR files to ParsedFile and delegates to the existing detection logic.
/// The entrypoint detection heuristics operate on the same fields available in both
/// representations, so results are identical.
pub fn detect_entrypoints_ir(files: &[IrFile]) -> Vec<Entrypoint> {
    let parsed: Vec<ParsedFile> = files.iter().map(|f| f.to_parsed_file()).collect();
    detect_entrypoints(&parsed)
}

fn detect_file_entrypoints(file: &ParsedFile, out: &mut Vec<Entrypoint>) {
    detect_test_file(file, out);
    detect_http_routes(file, out);
    detect_path_based_http_routes(file, out);
    detect_cli_commands(file, out);
    detect_path_based_cli_commands(file, out);
    detect_queue_consumers(file, out);
    detect_cron_jobs(file, out);
    detect_react_pages(file, out);
    detect_event_handlers(file, out);
    detect_effect_ts(file, out);
    detect_rust_modules(file, out);
}

// ---------------------------------------------------------------------------
// Test file detection
// ---------------------------------------------------------------------------

fn detect_test_file(file: &ParsedFile, out: &mut Vec<Entrypoint>) {
    if is_test_path(&file.path) {
        // Find test functions/describes as symbols, or use the file itself
        let test_symbols: Vec<&str> = file
            .definitions
            .iter()
            .filter(|d| is_test_symbol_name(&d.name))
            .map(|d| d.name.as_str())
            .collect();

        if test_symbols.is_empty() {
            out.push(Entrypoint {
                file: file.path.clone(),
                symbol: file_stem(&file.path),
                entrypoint_type: EntrypointType::TestFile,
            });
        } else {
            for sym in test_symbols {
                out.push(Entrypoint {
                    file: file.path.clone(),
                    symbol: sym.to_string(),
                    entrypoint_type: EntrypointType::TestFile,
                });
            }
        }
    }
}

fn is_test_path(path: &str) -> bool {
    let lower = path.to_lowercase();
    // File name patterns
    lower.contains(".test.") || lower.contains(".spec.") || lower.contains("_test.")
        || lower.ends_with("_test.py")
        || lower.ends_with("_test.ts")
        || lower.ends_with("_test.js")
        // Directory patterns
        || lower.contains("__tests__/")
        || lower.contains("/tests/")
        || lower.contains("/test/")
        || lower.starts_with("tests/")
        || lower.starts_with("test/")
        // Python test convention
        || path.split('/').last().map_or(false, |f| f.starts_with("test_"))
        // Go test files
        || lower.ends_with("_test.go")
        // Rust test files
        || lower.ends_with("_test.rs")
        || lower.ends_with("_tests.rs")
        // Java test files
        || (lower.ends_with(".java") && (
            path.split('/').last().map_or(false, |f| f.ends_with("Test.java") || f.ends_with("Tests.java") || f.starts_with("Test"))
        ))
        // C# test files
        || (lower.ends_with(".cs") && (
            path.split('/').last().map_or(false, |f| f.ends_with("Tests.cs") || f.ends_with("Test.cs"))
        ))
        // PHP test files
        || (lower.ends_with(".php") && (
            path.split('/').last().map_or(false, |f| f.ends_with("Test.php") || f.ends_with("Tests.php") || f.starts_with("test_"))
        ))
        // Ruby test files: RSpec spec/*_spec.rb, Minitest test/*_test.rb
        || (lower.ends_with(".rb") && (
            path.split('/').last().map_or(false, |f| f.ends_with("_spec.rb") || f.ends_with("_test.rb") || f.starts_with("test_"))
            || lower.contains("/spec/")
        ))
        // Kotlin test files: JUnit *Test.kt, KotlinTest *Test.kt
        || (lower.ends_with(".kt") && (
            path.split('/').last().map_or(false, |f| f.ends_with("Test.kt") || f.ends_with("Tests.kt") || f.starts_with("Test"))
        ))
        // Swift test files: XCTest *Tests.swift, *Test.swift
        || (lower.ends_with(".swift") && (
            path.split('/').last().map_or(false, |f| f.ends_with("Tests.swift") || f.ends_with("Test.swift"))
            || lower.contains("/tests/")
        ))
        // C test files: *_test.c, test_*.c, or in test/ directory
        || ((lower.ends_with(".c") || lower.ends_with(".h")) && (
            path.split('/').last().map_or(false, |f| f.ends_with("_test.c") || f.ends_with("_tests.c") || f.starts_with("test_"))
            || lower.contains("/tests/") || lower.contains("/test/")
        ))
        // C++ test files: *_test.cpp, *Test.cpp, test_*.cpp, or in test/ directory
        || ((lower.ends_with(".cpp") || lower.ends_with(".cc") || lower.ends_with(".cxx")) && (
            path.split('/').last().map_or(false, |f| {
                f.ends_with("_test.cpp") || f.ends_with("_test.cc") || f.ends_with("_test.cxx")
                || f.ends_with("_tests.cpp") || f.ends_with("_tests.cc")
                || f.ends_with("Test.cpp") || f.ends_with("Test.cc")
                || f.starts_with("test_")
            })
            || lower.contains("/tests/") || lower.contains("/test/")
        ))
        // Scala test files: *Test.scala, *Spec.scala, *Suite.scala, or in test/ directory
        || (lower.ends_with(".scala") && (
            path.split('/').last().map_or(false, |f| {
                f.ends_with("Test.scala") || f.ends_with("Tests.scala")
                || f.ends_with("Spec.scala") || f.ends_with("Suite.scala")
                || f.starts_with("Test")
            })
            || lower.contains("/test/") || lower.contains("/tests/")
        ))
}

fn is_test_symbol_name(name: &str) -> bool {
    name.starts_with("test_")
        || name.starts_with("it_")
        || name == "describe"
        || name == "it"
        || name == "test"
        // Go test conventions: Test*, Benchmark*, Example*
        || name.starts_with("Test")
        || name.starts_with("Benchmark")
        || name.starts_with("Example")
        // C/C++ test frameworks: Google Test TEST/TEST_F, Catch2 TEST_CASE
        || name == "TEST"
        || name == "TEST_F"
        || name == "TEST_P"
        || name == "TEST_CASE"
        || name == "SCENARIO"
}

// ---------------------------------------------------------------------------
// HTTP route detection
// ---------------------------------------------------------------------------

fn detect_http_routes(file: &ParsedFile, out: &mut Vec<Entrypoint>) {
    match file.language {
        Language::TypeScript | Language::JavaScript => detect_http_routes_js(file, out),
        Language::Python => detect_http_routes_python(file, out),
        Language::Go => detect_http_routes_go(file, out),
        Language::Rust => detect_http_routes_rust(file, out),
        Language::Java => detect_http_routes_java(file, out),
        Language::CSharp => detect_http_routes_csharp(file, out),
        Language::Php => detect_http_routes_php(file, out),
        Language::Ruby => detect_http_routes_ruby(file, out),
        Language::Kotlin => detect_http_routes_kotlin(file, out),
        Language::Swift => detect_http_routes_swift(file, out),
        Language::C => detect_http_routes_c_cpp(file, out),
        Language::Cpp => detect_http_routes_c_cpp(file, out),
        Language::Scala => detect_http_routes_scala(file, out),
        // Extras: no language-specific HTTP route detection (yet).
        Language::Bash
        | Language::Haskell
        | Language::Nix
        | Language::Lua
        | Language::Perl
        | Language::Elixir
        | Language::Erlang
        | Language::Zig
        | Language::OCaml
        | Language::Julia
        | Language::Dart
        | Language::R
        | Language::Fish
        | Language::Html
        | Language::Css
        | Language::Scss
        | Language::Json
        | Language::Yaml
        | Language::Toml
        | Language::Markdown
        | Language::GraphQl
        | Language::Vue
        | Language::Svelte => {}
        Language::Unknown => {}
    }
}

/// Detect Express/Fastify-style route registrations: app.get(), router.post(), etc.
/// Also detect Next.js/file-based routing patterns.
fn detect_http_routes_js(file: &ParsedFile, out: &mut Vec<Entrypoint>) {
    let http_methods = [
        "get", "post", "put", "delete", "patch", "options", "head", "all",
    ];
    let router_objects = ["app", "router", "server"];

    // Check for Next.js App Router conventions (route.ts exporting HTTP methods)
    if is_nextjs_route_file(&file.path) {
        for export in &file.exports {
            let upper = export.name.to_uppercase();
            if http_methods.contains(&upper.to_lowercase().as_str())
                || upper == "GET"
                || upper == "POST"
                || upper == "PUT"
                || upper == "DELETE"
                || upper == "PATCH"
                || upper == "OPTIONS"
                || upper == "HEAD"
            {
                out.push(Entrypoint {
                    file: file.path.clone(),
                    symbol: export.name.clone(),
                    entrypoint_type: EntrypointType::HttpRoute,
                });
            }
        }
    }

    // Check for Next.js Pages Router conventions (default export from pages/)
    if is_nextjs_pages_file(&file.path) {
        for export in &file.exports {
            if export.is_default {
                out.push(Entrypoint {
                    file: file.path.clone(),
                    symbol: export.name.clone(),
                    entrypoint_type: EntrypointType::HttpRoute,
                });
                break;
            }
        }
    }

    // Check call sites for router.get/post/... patterns
    for call in &file.call_sites {
        if let Some((obj, method)) = call.callee.split_once('.') {
            if router_objects.contains(&obj) && http_methods.contains(&method) {
                let symbol = call
                    .containing_function
                    .clone()
                    .unwrap_or_else(|| call.callee.clone());
                out.push(Entrypoint {
                    file: file.path.clone(),
                    symbol,
                    entrypoint_type: EntrypointType::HttpRoute,
                });
            }
        }
    }
}

fn is_nextjs_route_file(path: &str) -> bool {
    let lower = path.to_lowercase();
    // Next.js App Router: app/**/route.ts
    (lower.contains("/app/") || lower.starts_with("app/"))
        && path
            .split('/')
            .last()
            .map_or(false, |f| f.starts_with("route."))
}

fn is_nextjs_pages_file(path: &str) -> bool {
    let lower = path.to_lowercase();
    // Next.js Pages Router: pages/**/*.tsx (excluding _app, _document, _error, api/)
    let in_pages = lower.contains("/pages/") || lower.starts_with("pages/");
    if !in_pages {
        return false;
    }
    let filename = path.split('/').last().unwrap_or("");
    !filename.starts_with('_') && !lower.contains("/api/")
}

/// Detect Flask/FastAPI/Django route decorators in Python.
fn detect_http_routes_python(file: &ParsedFile, out: &mut Vec<Entrypoint>) {
    let route_decorators = [
        "app.route",
        "app.get",
        "app.post",
        "app.put",
        "app.delete",
        "app.patch",
        "router.route",
        "router.get",
        "router.post",
        "router.put",
        "router.delete",
        "router.patch",
        "api_view",
    ];

    // Check call sites for decorator-style route registrations
    for call in &file.call_sites {
        if route_decorators.iter().any(|d| call.callee == *d) {
            if let Some(ref func) = call.containing_function {
                out.push(Entrypoint {
                    file: file.path.clone(),
                    symbol: func.clone(),
                    entrypoint_type: EntrypointType::HttpRoute,
                });
            }
        }
    }

    // Also check if file path suggests it's a views/routes module
    let is_route_module = file.path.contains("/views/")
        || file.path.contains("/routes/")
        || file.path.contains("/endpoints/")
        || file.path.ends_with("views.py")
        || file.path.ends_with("routes.py")
        || file.path.ends_with("endpoints.py");

    if is_route_module {
        // Functions in route modules that import from web frameworks are likely handlers
        let has_web_framework_import = file.imports.iter().any(|i| is_web_framework_import(i));
        if has_web_framework_import {
            for def in &file.definitions {
                if def.kind == crate::types::SymbolKind::Function
                    && !def.name.starts_with('_')
                    && def.name != "__init__"
                {
                    out.push(Entrypoint {
                        file: file.path.clone(),
                        symbol: def.name.clone(),
                        entrypoint_type: EntrypointType::HttpRoute,
                    });
                }
            }
        }
    }
}

/// Detect Go HTTP route handlers: net/http, gin, echo, chi, fiber patterns.
fn detect_http_routes_go(file: &ParsedFile, out: &mut Vec<Entrypoint>) {
    let http_methods = [
        "get", "post", "put", "delete", "patch", "options", "head", "any", "group",
    ];

    // Check if we have a Go web framework import
    let has_http_import = file.imports.iter().any(|i| {
        i.source == "net/http"
            || i.source == "github.com/gin-gonic/gin"
            || i.source.starts_with("github.com/labstack/echo")
            || i.source.starts_with("github.com/go-chi/chi")
            || i.source.starts_with("github.com/gofiber/fiber")
            || i.source.starts_with("github.com/gorilla/mux")
    });

    if !has_http_import {
        return;
    }

    // Check for handler function patterns
    for call in &file.call_sites {
        let callee = &call.callee;

        // Standard library: http.HandleFunc, http.Handle
        if callee == "http.HandleFunc" || callee == "http.Handle" || callee == "http.ListenAndServe"
        {
            let symbol = call
                .containing_function
                .clone()
                .unwrap_or_else(|| call.callee.clone());
            out.push(Entrypoint {
                file: file.path.clone(),
                symbol,
                entrypoint_type: EntrypointType::HttpRoute,
            });
            continue;
        }

        // Framework patterns: router.GET, r.Post, c.Get, app.Get, e.GET, etc.
        if let Some((_, method)) = callee.rsplit_once('.') {
            let method_lower = method.to_lowercase();
            if http_methods.contains(&method_lower.as_str())
                || method == "HandleFunc"
                || method == "Handle"
            {
                let symbol = call
                    .containing_function
                    .clone()
                    .unwrap_or_else(|| call.callee.clone());
                out.push(Entrypoint {
                    file: file.path.clone(),
                    symbol,
                    entrypoint_type: EntrypointType::HttpRoute,
                });
            }
        }
    }

    // Detect handler-like functions by convention
    let is_handler_module = file.path.contains("/handler")
        || file.path.contains("/handlers/")
        || file.path.contains("/routes/")
        || file.path.contains("/api/");

    if is_handler_module {
        for def in &file.definitions {
            if def.kind == crate::types::SymbolKind::Function
                && def.name.chars().next().map_or(false, |c| c.is_uppercase())
                && (def.name.contains("Handler")
                    || def.name.ends_with("Handle")
                    || def.name.starts_with("Handle"))
            {
                out.push(Entrypoint {
                    file: file.path.clone(),
                    symbol: def.name.clone(),
                    entrypoint_type: EntrypointType::HttpRoute,
                });
            }
        }
    }
}

fn detect_http_routes_rust(file: &ParsedFile, out: &mut Vec<Entrypoint>) {
    // Check if we have a Rust web framework import
    let has_http_import = file.imports.iter().any(|i| {
        i.source.starts_with("actix_web")
            || i.source.starts_with("actix-web")
            || i.source.starts_with("axum")
            || i.source.starts_with("rocket")
            || i.source.starts_with("warp")
            || i.source.starts_with("hyper")
            || i.source.starts_with("tower")
    });

    if !has_http_import {
        return;
    }

    // Check for actix-web attribute-style route handlers: #[get("/")], #[post("/")]
    // These are detected by looking for functions in handler-like modules
    // and for call patterns like Router::new().route()
    for call in &file.call_sites {
        let callee = &call.callee;

        // Axum: Router::new, .route(), get(), post(), etc.
        if callee.contains("Router::new")
            || callee.contains(".route")
            || callee.contains("axum::routing::get")
            || callee.contains("axum::routing::post")
            || callee.contains("axum::routing::put")
            || callee.contains("axum::routing::delete")
            || callee.contains("routing::get")
            || callee.contains("routing::post")
            || callee.contains("routing::put")
            || callee.contains("routing::delete")
            || callee == "get"
            || callee == "post"
            || callee == "put"
            || callee == "delete"
        {
            let symbol = call
                .containing_function
                .clone()
                .unwrap_or_else(|| call.callee.clone());
            out.push(Entrypoint {
                file: file.path.clone(),
                symbol,
                entrypoint_type: EntrypointType::HttpRoute,
            });
        }

        // Actix-web: web::resource, web::scope, App::new().service()
        if callee.contains("web::resource")
            || callee.contains("web::scope")
            || callee.contains("App::new")
            || callee.contains(".service")
            || callee.contains("HttpServer::new")
        {
            let symbol = call
                .containing_function
                .clone()
                .unwrap_or_else(|| call.callee.clone());
            out.push(Entrypoint {
                file: file.path.clone(),
                symbol,
                entrypoint_type: EntrypointType::HttpRoute,
            });
        }

        // Rocket: routes!, mount()
        if callee.contains("routes!")
            || callee.contains(".mount")
            || callee.contains("rocket::build")
        {
            let symbol = call
                .containing_function
                .clone()
                .unwrap_or_else(|| call.callee.clone());
            out.push(Entrypoint {
                file: file.path.clone(),
                symbol,
                entrypoint_type: EntrypointType::HttpRoute,
            });
        }
    }

    // Detect handler-like functions by path convention
    let is_handler_module = file.path.contains("/handler")
        || file.path.contains("/handlers/")
        || file.path.contains("/routes/")
        || file.path.contains("/api/");

    if is_handler_module {
        for def in &file.definitions {
            if def.kind == crate::types::SymbolKind::Function {
                out.push(Entrypoint {
                    file: file.path.clone(),
                    symbol: def.name.clone(),
                    entrypoint_type: EntrypointType::HttpRoute,
                });
            }
        }
    }
}

/// Detect Java HTTP route handlers: Spring Boot, JAX-RS, Servlet patterns.
fn detect_http_routes_java(file: &ParsedFile, out: &mut Vec<Entrypoint>) {
    // Check if we have a Java web framework import
    let has_spring_import = file.imports.iter().any(|i| {
        i.source.starts_with("org.springframework.web")
            || i.source.starts_with("org.springframework.boot")
            || i.source.starts_with("org.springframework.stereotype")
    });

    let has_jaxrs_import = file
        .imports
        .iter()
        .any(|i| i.source.starts_with("jakarta.ws.rs") || i.source.starts_with("javax.ws.rs"));

    let has_servlet_import = file
        .imports
        .iter()
        .any(|i| i.source.starts_with("jakarta.servlet") || i.source.starts_with("javax.servlet"));

    if !has_spring_import && !has_jaxrs_import && !has_servlet_import {
        return;
    }

    // Spring Boot: look for @GetMapping, @PostMapping, @RequestMapping, etc. in import names
    if has_spring_import {
        let has_mapping_annotation = file.imports.iter().any(|i| {
            i.names.iter().any(|n| {
                n.name.ends_with("Mapping")
                    || n.name == "RestController"
                    || n.name == "Controller"
                    || n.name == "RequestMapping"
            })
        });

        if has_mapping_annotation {
            // All public methods in a controller are potential endpoints
            for def in &file.definitions {
                if def.kind == crate::types::SymbolKind::Function && def.name != "<init>" {
                    out.push(Entrypoint {
                        file: file.path.clone(),
                        symbol: def.name.clone(),
                        entrypoint_type: EntrypointType::HttpRoute,
                    });
                }
            }
        }
    }

    // JAX-RS: @GET, @POST, @Path
    if has_jaxrs_import {
        let has_path_annotation = file.imports.iter().any(|i| {
            i.names.iter().any(|n| {
                n.name == "Path"
                    || n.name == "GET"
                    || n.name == "POST"
                    || n.name == "PUT"
                    || n.name == "DELETE"
            })
        });

        if has_path_annotation {
            for def in &file.definitions {
                if def.kind == crate::types::SymbolKind::Function {
                    out.push(Entrypoint {
                        file: file.path.clone(),
                        symbol: def.name.clone(),
                        entrypoint_type: EntrypointType::HttpRoute,
                    });
                }
            }
        }
    }

    // Servlet: doGet, doPost, etc.
    if has_servlet_import {
        for def in &file.definitions {
            if def.kind == crate::types::SymbolKind::Function
                && (def.name.starts_with("do")
                    && [
                        "doGet",
                        "doPost",
                        "doPut",
                        "doDelete",
                        "doHead",
                        "doOptions",
                    ]
                    .contains(&def.name.as_str()))
            {
                out.push(Entrypoint {
                    file: file.path.clone(),
                    symbol: def.name.clone(),
                    entrypoint_type: EntrypointType::HttpRoute,
                });
            }
        }
    }

    // Detect handler-like files by path convention
    let is_controller_module = file.path.contains("/controller")
        || file.path.contains("/controllers/")
        || file.path.contains("/resource/")
        || file.path.contains("/resources/")
        || file.path.contains("/endpoint/")
        || file.path.contains("/endpoints/")
        || file.path.contains("/api/");

    if is_controller_module && !has_spring_import && !has_jaxrs_import {
        for def in &file.definitions {
            if def.kind == crate::types::SymbolKind::Function {
                out.push(Entrypoint {
                    file: file.path.clone(),
                    symbol: def.name.clone(),
                    entrypoint_type: EntrypointType::HttpRoute,
                });
            }
        }
    }
}

/// Detect C# HTTP route handlers: ASP.NET Core controllers, minimal API patterns.
fn detect_http_routes_csharp(file: &ParsedFile, out: &mut Vec<Entrypoint>) {
    // Check for ASP.NET Core web framework imports
    let has_aspnet_import = file.imports.iter().any(|i| {
        i.source.starts_with("Microsoft.AspNetCore")
            || i.source.starts_with("Microsoft.AspNet")
            || i.source == "System.Web"
            || i.source.starts_with("System.Web")
    });

    if !has_aspnet_import {
        // Also check path-based heuristic for controller files
        let is_controller_file = file.path.contains("/Controllers/")
            || file.path.contains("/controllers/")
            || file.path.contains("/Endpoints/")
            || file.path.contains("/endpoints/")
            || file.path.contains("/Api/")
            || file.path.contains("/api/")
            || file
                .path
                .split('/')
                .last()
                .map_or(false, |f| f.ends_with("Controller.cs"));

        if is_controller_file {
            for def in &file.definitions {
                if def.kind == crate::types::SymbolKind::Function {
                    out.push(Entrypoint {
                        file: file.path.clone(),
                        symbol: def.name.clone(),
                        entrypoint_type: EntrypointType::HttpRoute,
                    });
                }
            }
        }
        return;
    }

    // ASP.NET Core: controller classes with action methods
    // C# using directives import entire namespaces, so check source directly
    let has_controller_import = file.imports.iter().any(|i| {
        i.source.starts_with("Microsoft.AspNetCore.Mvc")
            || i.source.starts_with("Microsoft.AspNetCore.Http")
            || i.source.starts_with("Microsoft.AspNetCore.Routing")
    });

    if has_controller_import {
        for def in &file.definitions {
            if def.kind == crate::types::SymbolKind::Function
                && def.name
                    != file
                        .path
                        .split('/')
                        .last()
                        .unwrap_or("")
                        .trim_end_matches(".cs")
            {
                out.push(Entrypoint {
                    file: file.path.clone(),
                    symbol: def.name.clone(),
                    entrypoint_type: EntrypointType::HttpRoute,
                });
            }
        }
    }

    // Minimal API: app.MapGet, app.MapPost, etc.
    let map_methods = ["MapGet", "MapPost", "MapPut", "MapDelete", "MapPatch"];
    for call in &file.call_sites {
        if map_methods
            .iter()
            .any(|m| call.callee == *m || call.callee.ends_with(&format!(".{m}")))
        {
            let symbol = call
                .containing_function
                .clone()
                .unwrap_or_else(|| call.callee.clone());
            out.push(Entrypoint {
                file: file.path.clone(),
                symbol,
                entrypoint_type: EntrypointType::HttpRoute,
            });
        }
    }
}

/// Detect PHP HTTP route handlers: Laravel controllers, Symfony controllers, route files.
fn detect_http_routes_php(file: &ParsedFile, out: &mut Vec<Entrypoint>) {
    // Check for PHP web framework imports
    let has_laravel_import = file
        .imports
        .iter()
        .any(|i| i.source.starts_with("Illuminate\\") || i.source.starts_with("Laravel\\"));

    let has_symfony_import = file
        .imports
        .iter()
        .any(|i| i.source.starts_with("Symfony\\"));

    if !has_laravel_import && !has_symfony_import {
        // Path-based heuristic for controller files
        let is_controller_file = file.path.contains("/Controllers/")
            || file.path.contains("/controllers/")
            || file.path.contains("/routes/")
            || file.path.contains("/Routes/")
            || file.path.contains("/api/")
            || file.path.contains("/Api/")
            || file
                .path
                .split('/')
                .last()
                .map_or(false, |f| f.ends_with("Controller.php"));

        if is_controller_file {
            for def in &file.definitions {
                if def.kind == crate::types::SymbolKind::Function
                    && !def.name.starts_with('_')
                    && def.name != "__construct"
                {
                    out.push(Entrypoint {
                        file: file.path.clone(),
                        symbol: def.name.clone(),
                        entrypoint_type: EntrypointType::HttpRoute,
                    });
                }
            }
        }
        return;
    }

    // Laravel: controller action methods (public methods excluding __construct)
    if has_laravel_import {
        for def in &file.definitions {
            if def.kind == crate::types::SymbolKind::Function
                && !def.name.starts_with('_')
                && def.name != "__construct"
            {
                out.push(Entrypoint {
                    file: file.path.clone(),
                    symbol: def.name.clone(),
                    entrypoint_type: EntrypointType::HttpRoute,
                });
            }
        }
    }

    // Symfony: controller action methods
    if has_symfony_import {
        for def in &file.definitions {
            if def.kind == crate::types::SymbolKind::Function
                && !def.name.starts_with('_')
                && def.name != "__construct"
            {
                out.push(Entrypoint {
                    file: file.path.clone(),
                    symbol: def.name.clone(),
                    entrypoint_type: EntrypointType::HttpRoute,
                });
            }
        }
    }
}

fn detect_http_routes_ruby(file: &ParsedFile, out: &mut Vec<Entrypoint>) {
    // Check for Ruby web framework imports
    let has_rails_import = file.imports.iter().any(|i| {
        i.source == "action_controller" || i.source == "rails" || i.source == "active_record"
    });

    let has_sinatra_import = file
        .imports
        .iter()
        .any(|i| i.source == "sinatra" || i.source == "sinatra/base");

    let has_grape_import = file.imports.iter().any(|i| i.source == "grape");

    // Sinatra-style: detect get/post/put/delete/patch route DSL calls
    if has_sinatra_import || has_grape_import {
        let http_methods = ["get", "post", "put", "delete", "patch", "options", "head"];
        for call in &file.call_sites {
            if http_methods.contains(&call.callee.as_str()) {
                out.push(Entrypoint {
                    file: file.path.clone(),
                    symbol: call.callee.clone(),
                    entrypoint_type: EntrypointType::HttpRoute,
                });
            }
        }
        return;
    }

    // Rails: controller action methods
    if has_rails_import {
        for def in &file.definitions {
            if def.kind == crate::types::SymbolKind::Function
                && !def.name.starts_with('_')
                && def.name != "initialize"
            {
                out.push(Entrypoint {
                    file: file.path.clone(),
                    symbol: def.name.clone(),
                    entrypoint_type: EntrypointType::HttpRoute,
                });
            }
        }
        return;
    }

    // Path-based heuristic for Rails controller files
    let is_controller_file = file.path.contains("/controllers/")
        || file.path.contains("/Controllers/")
        || file.path.contains("/routes/")
        || file
            .path
            .split('/')
            .last()
            .map_or(false, |f| f.ends_with("_controller.rb"));

    if is_controller_file {
        for def in &file.definitions {
            if def.kind == crate::types::SymbolKind::Function
                && !def.name.starts_with('_')
                && def.name != "initialize"
            {
                out.push(Entrypoint {
                    file: file.path.clone(),
                    symbol: def.name.clone(),
                    entrypoint_type: EntrypointType::HttpRoute,
                });
            }
        }
    }
}

fn detect_http_routes_kotlin(file: &ParsedFile, out: &mut Vec<Entrypoint>) {
    // Check for Kotlin web framework imports
    let has_ktor_import = file.imports.iter().any(|i| i.source.starts_with("io.ktor"));

    let has_spring_import = file
        .imports
        .iter()
        .any(|i| i.source.starts_with("org.springframework"));

    // Ktor route DSL: get, post, put, delete, patch, route
    if has_ktor_import {
        let http_methods = [
            "get", "post", "put", "delete", "patch", "head", "options", "route",
        ];
        for call in &file.call_sites {
            if http_methods.contains(&call.callee.as_str()) {
                out.push(Entrypoint {
                    file: file.path.clone(),
                    symbol: call.callee.clone(),
                    entrypoint_type: EntrypointType::HttpRoute,
                });
            }
        }
        return;
    }

    // Spring Boot (Kotlin): controller methods with annotation-style imports
    if has_spring_import {
        for def in &file.definitions {
            if def.kind == crate::types::SymbolKind::Function && !def.name.starts_with('_') {
                out.push(Entrypoint {
                    file: file.path.clone(),
                    symbol: def.name.clone(),
                    entrypoint_type: EntrypointType::HttpRoute,
                });
            }
        }
        return;
    }

    // Path-based heuristic for controller files
    let is_controller_file = file.path.contains("/controllers/")
        || file.path.contains("/controller/")
        || file.path.contains("/routes/")
        || file
            .path
            .split('/')
            .last()
            .map_or(false, |f| f.ends_with("Controller.kt"));

    if is_controller_file {
        for def in &file.definitions {
            if def.kind == crate::types::SymbolKind::Function && !def.name.starts_with('_') {
                out.push(Entrypoint {
                    file: file.path.clone(),
                    symbol: def.name.clone(),
                    entrypoint_type: EntrypointType::HttpRoute,
                });
            }
        }
    }
}

fn detect_http_routes_swift(file: &ParsedFile, out: &mut Vec<Entrypoint>) {
    // Check for Vapor framework imports
    let has_vapor_import = file.imports.iter().any(|i| i.source == "Vapor");

    if has_vapor_import {
        // Vapor route DSL: get, post, put, delete, patch, route
        let http_methods = [
            "get", "post", "put", "delete", "patch", "head", "options", "route",
        ];
        for call in &file.call_sites {
            if http_methods.contains(&call.callee.as_str()) {
                out.push(Entrypoint {
                    file: file.path.clone(),
                    symbol: call.callee.clone(),
                    entrypoint_type: EntrypointType::HttpRoute,
                });
            }
        }
        return;
    }

    // Path-based heuristic for controller files
    let is_controller_file = file.path.contains("/Controllers/")
        || file.path.contains("/controllers/")
        || file.path.contains("/Routes/")
        || file.path.contains("/routes/")
        || file
            .path
            .split('/')
            .last()
            .map_or(false, |f| f.ends_with("Controller.swift"));

    if is_controller_file {
        for def in &file.definitions {
            if def.kind == crate::types::SymbolKind::Function && !def.name.starts_with('_') {
                out.push(Entrypoint {
                    file: file.path.clone(),
                    symbol: def.name.clone(),
                    entrypoint_type: EntrypointType::HttpRoute,
                });
            }
        }
    }
}

fn detect_http_routes_c_cpp(file: &ParsedFile, out: &mut Vec<Entrypoint>) {
    // Crow framework: CROW_ROUTE, app.route_dynamic
    // cpp-httplib: svr.Get, svr.Post, etc.
    // Pistache: Rest::Routes::Get, Rest::Routes::Post
    // Drogon: app().registerHandler
    let http_methods = ["Get", "Post", "Put", "Delete", "Patch", "Head", "Options"];

    // Check for C++ HTTP framework imports
    let has_http_framework = file.imports.iter().any(|i| {
        i.source.contains("crow")
            || i.source.contains("httplib")
            || i.source.contains("pistache")
            || i.source.contains("drogon")
            || i.source.contains("cpprest")
            || i.source.contains("beast")
            || i.source.contains("mongoose")
    });

    if has_http_framework {
        for call in &file.call_sites {
            let callee_lower = call.callee.to_lowercase();
            if http_methods
                .iter()
                .any(|m| callee_lower == m.to_lowercase())
                || callee_lower.contains("route")
                || callee_lower.contains("handler")
                || callee_lower == "crow_route"
            {
                out.push(Entrypoint {
                    file: file.path.clone(),
                    symbol: call.callee.clone(),
                    entrypoint_type: EntrypointType::HttpRoute,
                });
            }
        }
        return;
    }

    // Path-based heuristic for handler/controller files
    let is_handler_file = file.path.contains("/handlers/")
        || file.path.contains("/controllers/")
        || file.path.contains("/routes/")
        || file.path.contains("/endpoints/");

    if is_handler_file {
        for def in &file.definitions {
            if def.kind == crate::types::SymbolKind::Function && !def.name.starts_with('_') {
                out.push(Entrypoint {
                    file: file.path.clone(),
                    symbol: def.name.clone(),
                    entrypoint_type: EntrypointType::HttpRoute,
                });
            }
        }
    }
}

fn detect_http_routes_scala(file: &ParsedFile, out: &mut Vec<Entrypoint>) {
    // Check for Scala web framework imports
    let has_akka_http = file
        .imports
        .iter()
        .any(|i| i.source.starts_with("akka.http"));

    let has_play_import = file
        .imports
        .iter()
        .any(|i| i.source.starts_with("play.api.mvc") || i.source.starts_with("play.mvc"));

    let has_http4s_import = file
        .imports
        .iter()
        .any(|i| i.source.starts_with("org.http4s"));

    // Akka HTTP route DSL: get, post, put, delete, patch, path, pathPrefix, complete
    if has_akka_http {
        let http_methods = [
            "get",
            "post",
            "put",
            "delete",
            "patch",
            "head",
            "options",
            "path",
            "pathPrefix",
            "complete",
        ];
        for call in &file.call_sites {
            if http_methods.contains(&call.callee.as_str()) {
                out.push(Entrypoint {
                    file: file.path.clone(),
                    symbol: call.callee.clone(),
                    entrypoint_type: EntrypointType::HttpRoute,
                });
            }
        }
        return;
    }

    // Play Framework controller actions
    if has_play_import {
        for def in &file.definitions {
            if def.kind == crate::types::SymbolKind::Function && !def.name.starts_with('_') {
                out.push(Entrypoint {
                    file: file.path.clone(),
                    symbol: def.name.clone(),
                    entrypoint_type: EntrypointType::HttpRoute,
                });
            }
        }
        return;
    }

    // http4s route DSL
    if has_http4s_import {
        let http_methods = ["get", "post", "put", "delete", "patch"];
        for call in &file.call_sites {
            let callee_lower = call.callee.to_lowercase();
            if http_methods.contains(&callee_lower.as_str()) {
                out.push(Entrypoint {
                    file: file.path.clone(),
                    symbol: call.callee.clone(),
                    entrypoint_type: EntrypointType::HttpRoute,
                });
            }
        }
        return;
    }

    // Path-based heuristic for controller files
    let is_controller_file = file.path.contains("/controllers/")
        || file.path.contains("/controller/")
        || file.path.contains("/routes/")
        || file
            .path
            .split('/')
            .last()
            .map_or(false, |f| f.ends_with("Controller.scala"));

    if is_controller_file {
        for def in &file.definitions {
            if def.kind == crate::types::SymbolKind::Function && !def.name.starts_with('_') {
                out.push(Entrypoint {
                    file: file.path.clone(),
                    symbol: def.name.clone(),
                    entrypoint_type: EntrypointType::HttpRoute,
                });
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Shared path-based entrypoint detection helpers (all languages)
// ---------------------------------------------------------------------------

/// Tier 1: Path suggests route/handler AND needs framework import confirmation.
fn is_route_handler_path(path: &str) -> bool {
    let lower = path.to_lowercase();
    // Directory patterns
    lower.contains("/routes/")
        || lower.contains("/route/")
        || lower.contains("/handlers/")
        || lower.contains("/handler/")
        || lower.contains("/controllers/")
        || lower.contains("/controller/")
        || lower.contains("/endpoints/")
        || lower.contains("/endpoint/")
        // File name patterns
        || has_filename_pattern(&lower, "routes")
        || has_filename_pattern(&lower, "route")
        || has_filename_pattern(&lower, "handler")
        || has_filename_pattern(&lower, "handlers")
        || has_filename_pattern(&lower, "controller")
        || has_filename_pattern(&lower, "controllers")
        || has_filename_pattern(&lower, "endpoint")
        || has_filename_pattern(&lower, "endpoints")
}

/// Tier 2: Very strong path signal — no import check needed.
fn is_strong_route_handler_path(path: &str) -> bool {
    let lower = path.to_lowercase();
    // Strong directory patterns (not just /api/ — too generic)
    lower.contains("/routes/")
        || lower.contains("/handlers/")
        || lower.contains("/controllers/")
        || lower.contains("/endpoints/")
        // File name patterns with strong signal
        || has_filename_pattern(&lower, "controller")
        || has_filename_pattern(&lower, "controllers")
        // Files with "entrypoint" in name
        || lower.contains("entrypoint")
}

/// Check if a file path contains a `.{pattern}.` segment in its filename.
/// e.g., `has_filename_pattern("src/billing.controller.ts", "controller")` → true
fn has_filename_pattern(lower_path: &str, pattern: &str) -> bool {
    if let Some(filename) = lower_path.rsplit('/').next() {
        let dot_pattern = format!(".{}.", pattern);
        filename.contains(&dot_pattern)
    } else {
        false
    }
}

/// Check if the path is in a CLI-related directory.
fn is_cli_command_path(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.contains("/commands/")
        || lower.contains("/command/")
        || lower.contains("/cmd/")
        || lower.contains("/cli/")
        || has_filename_pattern(&lower, "command")
        || has_filename_pattern(&lower, "commands")
        || has_filename_pattern(&lower, "cli")
}

/// Check if a path is in a /views/ directory (Python/Ruby/PHP only).
fn is_views_path(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.contains("/views/")
}

// ---------------------------------------------------------------------------
// Per-language web framework import helpers
// ---------------------------------------------------------------------------

/// Python web framework imports.
fn is_web_framework_import(imp: &ImportInfo) -> bool {
    is_python_web_framework_import(imp)
}

fn is_python_web_framework_import(imp: &ImportInfo) -> bool {
    let src = &imp.source;
    src == "flask"
        || src == "fastapi"
        || src.starts_with("django.")
        || src == "django"
        || src == "starlette"
        || src.starts_with("starlette.")
        || src == "sanic"
        || src == "aiohttp"
        || src.starts_with("aiohttp.")
        || src == "tornado"
        || src.starts_with("tornado.")
        || src == "bottle"
        || src == "falcon"
        || src == "pyramid"
        || src.starts_with("pyramid.")
}

fn is_js_web_framework_import(imp: &ImportInfo) -> bool {
    let src = &imp.source;
    src == "express"
        || src == "fastify"
        || src == "@hapi/hapi"
        || src == "koa"
        || src == "@trpc/server"
        || src == "hono"
        || src == "@nestjs/common"
        || src == "@nestjs/core"
        || src == "next"
        || src == "nuxt"
        || src.starts_with("@remix-run/")
        || src == "sveltekit"
        || src == "restify"
        || src == "polka"
        || src == "@effect/platform"
}

fn is_go_web_framework_import(imp: &ImportInfo) -> bool {
    let src = &imp.source;
    src == "net/http"
        || src == "github.com/gin-gonic/gin"
        || src.starts_with("github.com/labstack/echo")
        || src.starts_with("github.com/go-chi/chi")
        || src.starts_with("github.com/gofiber/fiber")
        || src.starts_with("github.com/gorilla/mux")
}

fn is_rust_web_framework_import(imp: &ImportInfo) -> bool {
    let src = &imp.source;
    src == "actix_web"
        || src == "actix-web"
        || src.starts_with("actix_web::")
        || src.starts_with("actix-web::")
        || src == "axum"
        || src.starts_with("axum::")
        || src == "rocket"
        || src.starts_with("rocket::")
        || src == "warp"
        || src.starts_with("warp::")
        || src == "hyper"
        || src.starts_with("hyper::")
        || src == "tower"
        || src.starts_with("tower::")
}

fn is_java_web_framework_import(imp: &ImportInfo) -> bool {
    let src = &imp.source;
    src.starts_with("org.springframework.web")
        || src.starts_with("javax.ws.rs")
        || src.starts_with("jakarta.ws.rs")
        || src.starts_with("io.javalin")
        || src.starts_with("io.micronaut.http")
        || src.starts_with("io.quarkus")
}

fn is_csharp_web_framework_import(imp: &ImportInfo) -> bool {
    let src = &imp.source;
    src.starts_with("Microsoft.AspNetCore")
        || src.starts_with("System.Web.Http")
        || src.starts_with("Carter")
        || src.starts_with("ServiceStack")
}

fn is_php_web_framework_import(imp: &ImportInfo) -> bool {
    let src = &imp.source;
    src.starts_with("Illuminate\\Routing")
        || src.starts_with("Symfony\\Component\\Routing")
        || src.starts_with("Slim\\App")
}

fn is_ruby_web_framework_import(imp: &ImportInfo) -> bool {
    let src = &imp.source;
    src == "sinatra" || src == "rails" || src == "grape" || src == "hanami" || src == "roda"
}

fn is_kotlin_web_framework_import(imp: &ImportInfo) -> bool {
    let src = &imp.source;
    src.starts_with("org.springframework.web")
        || src.starts_with("io.ktor")
        || src.starts_with("io.javalin")
        || src.starts_with("io.micronaut.http")
}

fn is_swift_web_framework_import(imp: &ImportInfo) -> bool {
    let src = &imp.source;
    src == "Vapor" || src == "Kitura" || src == "Hummingbird"
}

fn is_scala_web_framework_import(imp: &ImportInfo) -> bool {
    let src = &imp.source;
    src.starts_with("akka.http")
        || src.starts_with("http4s")
        || src.starts_with("play.api")
        || src.starts_with("zio.http")
        || src.starts_with("cask")
}

// ---------------------------------------------------------------------------
// Per-language CLI framework import helpers
// ---------------------------------------------------------------------------

fn is_js_cli_framework_import(imp: &ImportInfo) -> bool {
    let src = &imp.source;
    src == "commander"
        || src == "yargs"
        || src == "meow"
        || src == "cac"
        || src == "oclif"
        || src == "@effect/cli"
        || src == "inquirer"
        || src == "vorpal"
        || src == "caporal"
        || src == "clipanion"
}

fn is_python_cli_framework_import(imp: &ImportInfo) -> bool {
    let src = &imp.source;
    src == "argparse"
        || src == "click"
        || src == "typer"
        || src == "fire"
        || src == "docopt"
        || src == "plac"
}

fn is_go_cli_framework_import(imp: &ImportInfo) -> bool {
    let src = &imp.source;
    src.starts_with("github.com/spf13/cobra")
        || src.starts_with("github.com/urfave/cli")
        || src == "flag"
}

fn is_rust_cli_framework_import(imp: &ImportInfo) -> bool {
    let src = &imp.source;
    src.starts_with("clap")
        || src.starts_with("structopt")
        || src.starts_with("argh")
        || src.starts_with("gumdrop")
}

/// Get the appropriate web framework import checker for a given language.
fn has_web_framework_import_for_lang(file: &ParsedFile) -> bool {
    file.imports.iter().any(|imp| match file.language {
        Language::TypeScript | Language::JavaScript => is_js_web_framework_import(imp),
        Language::Python => is_python_web_framework_import(imp),
        Language::Go => is_go_web_framework_import(imp),
        Language::Rust => is_rust_web_framework_import(imp),
        Language::Java => is_java_web_framework_import(imp),
        Language::CSharp => is_csharp_web_framework_import(imp),
        Language::Php => is_php_web_framework_import(imp),
        Language::Ruby => is_ruby_web_framework_import(imp),
        Language::Kotlin => is_kotlin_web_framework_import(imp),
        Language::Swift => is_swift_web_framework_import(imp),
        Language::Scala => is_scala_web_framework_import(imp),
        Language::C | Language::Cpp | Language::Unknown => false,
        // Extras: no web framework heuristics (yet).
        Language::Bash
        | Language::Haskell
        | Language::Nix
        | Language::Lua
        | Language::Perl
        | Language::Elixir
        | Language::Erlang
        | Language::Zig
        | Language::OCaml
        | Language::Julia
        | Language::Dart
        | Language::R
        | Language::Fish
        | Language::Html
        | Language::Css
        | Language::Scss
        | Language::Json
        | Language::Yaml
        | Language::Toml
        | Language::Markdown
        | Language::GraphQl
        | Language::Vue
        | Language::Svelte => false,
    })
}

/// Get the appropriate CLI framework import checker for a given language.
fn has_cli_framework_import_for_lang(file: &ParsedFile) -> bool {
    file.imports.iter().any(|imp| match file.language {
        Language::TypeScript | Language::JavaScript => is_js_cli_framework_import(imp),
        Language::Python => is_python_cli_framework_import(imp),
        Language::Go => is_go_cli_framework_import(imp),
        Language::Rust => is_rust_cli_framework_import(imp),
        _ => false,
    })
}

/// Add all exported/public functions as entrypoints (used by path-based detection).
fn add_exported_functions_as_entrypoints(
    file: &ParsedFile,
    ep_type: EntrypointType,
    out: &mut Vec<Entrypoint>,
) {
    for def in &file.definitions {
        if def.kind == crate::types::SymbolKind::Function && !def.name.starts_with('_') {
            out.push(Entrypoint {
                file: file.path.clone(),
                symbol: def.name.clone(),
                entrypoint_type: ep_type.clone(),
            });
        }
    }
    // Also add exports as entrypoints for JS/TS
    if matches!(file.language, Language::TypeScript | Language::JavaScript) {
        for export in &file.exports {
            out.push(Entrypoint {
                file: file.path.clone(),
                symbol: export.name.clone(),
                entrypoint_type: ep_type.clone(),
            });
        }
    }
}

/// Universal path-based HTTP route detection — applies after call-site detection.
///
/// Tier 1: path + framework import → all exported functions become entrypoints.
/// Tier 2: strong path only → same, no import check.
fn detect_path_based_http_routes(file: &ParsedFile, out: &mut Vec<Entrypoint>) {
    let is_views = is_views_path(&file.path)
        && matches!(
            file.language,
            Language::Python | Language::Ruby | Language::Php
        );

    if is_strong_route_handler_path(&file.path) {
        add_exported_functions_as_entrypoints(file, EntrypointType::HttpRoute, out);
    } else if (is_route_handler_path(&file.path) || is_views)
        && has_web_framework_import_for_lang(file)
    {
        add_exported_functions_as_entrypoints(file, EntrypointType::HttpRoute, out);
    }
}

/// Universal path-based CLI command detection — applies after call-site detection.
fn detect_path_based_cli_commands(file: &ParsedFile, out: &mut Vec<Entrypoint>) {
    if is_cli_command_path(&file.path) {
        if has_cli_framework_import_for_lang(file) {
            add_exported_functions_as_entrypoints(file, EntrypointType::CliCommand, out);
        }
        // Strong signal: /commands/ directory is strong enough without import check
        let lower = file.path.to_lowercase();
        if lower.contains("/commands/") || lower.contains("/command/") {
            add_exported_functions_as_entrypoints(file, EntrypointType::CliCommand, out);
        }
    }
}

// ---------------------------------------------------------------------------
// CLI command detection
// ---------------------------------------------------------------------------

fn detect_cli_commands(file: &ParsedFile, out: &mut Vec<Entrypoint>) {
    // Detect main() functions
    let has_main = file.definitions.iter().any(|d| d.name == "main");

    if has_main {
        // Python: if __name__ == '__main__' pattern (detected via main def + path convention)
        // JS/TS: main() function in entry-like files
        // Go: func main() is always a CLI entrypoint
        let is_cli_path = is_cli_file_path(&file.path);
        if is_cli_path
            || file.language == Language::Python
            || file.language == Language::Go
            || file.language == Language::Rust
            || file.language == Language::Java
            || file.language == Language::CSharp
            || file.language == Language::Php
            || file.language == Language::Ruby
            || file.language == Language::Kotlin
            || file.language == Language::Swift
            || file.language == Language::C
            || file.language == Language::Cpp
        {
            out.push(Entrypoint {
                file: file.path.clone(),
                symbol: "main".to_string(),
                entrypoint_type: EntrypointType::CliCommand,
            });
        }
    }

    // Check for argument parser imports (Python)
    if file.language == Language::Python {
        let has_argparse = file
            .imports
            .iter()
            .any(|i| i.source == "argparse" || i.source == "click" || i.source == "typer");
        if has_argparse {
            // Functions decorated with @click.command or @app.command are CLI entrypoints
            for call in &file.call_sites {
                if call.callee == "click.command"
                    || call.callee == "click.group"
                    || call.callee == "app.command"
                    || call.callee == "typer.command"
                {
                    if let Some(ref func) = call.containing_function {
                        out.push(Entrypoint {
                            file: file.path.clone(),
                            symbol: func.clone(),
                            entrypoint_type: EntrypointType::CliCommand,
                        });
                    }
                }
            }
        }
    }

    // Go: check for cobra imports
    if file.language == Language::Go {
        let has_cobra = file
            .imports
            .iter()
            .any(|i| i.source.starts_with("github.com/spf13/cobra"));
        if has_cobra {
            for call in &file.call_sites {
                if call.callee.contains("cobra.Command") || call.callee.contains("AddCommand") {
                    if let Some(ref func) = call.containing_function {
                        out.push(Entrypoint {
                            file: file.path.clone(),
                            symbol: func.clone(),
                            entrypoint_type: EntrypointType::CliCommand,
                        });
                    }
                }
            }
        }
    }

    // Rust: check for clap imports
    if file.language == Language::Rust {
        let has_clap = file.imports.iter().any(|i| i.source.starts_with("clap"));
        if has_clap {
            for call in &file.call_sites {
                if call.callee.contains("Command::new")
                    || call.callee.contains("Parser::parse")
                    || call.callee.contains("clap::Command")
                {
                    if let Some(ref func) = call.containing_function {
                        out.push(Entrypoint {
                            file: file.path.clone(),
                            symbol: func.clone(),
                            entrypoint_type: EntrypointType::CliCommand,
                        });
                    }
                }
            }
        }
    }

    // JS/TS: check for commander/yargs imports
    if matches!(file.language, Language::TypeScript | Language::JavaScript) {
        let has_cli_framework = file.imports.iter().any(|i| {
            i.source == "commander"
                || i.source == "yargs"
                || i.source == "meow"
                || i.source == "cac"
        });
        if has_cli_framework {
            out.push(Entrypoint {
                file: file.path.clone(),
                symbol: file_stem(&file.path),
                entrypoint_type: EntrypointType::CliCommand,
            });
        }
    }

    // Check for bin-like file paths
    if is_bin_path(&file.path) && !has_main {
        out.push(Entrypoint {
            file: file.path.clone(),
            symbol: file_stem(&file.path),
            entrypoint_type: EntrypointType::CliCommand,
        });
    }
}

fn is_cli_file_path(path: &str) -> bool {
    path.contains("/cli/")
        || path.contains("/cmd/")
        || path.contains("/bin/")
        || path.ends_with("/main.ts")
        || path.ends_with("/main.js")
        || path.ends_with("/cli.ts")
        || path.ends_with("/cli.js")
        || path.ends_with("/cli.py")
        || path.ends_with("/main.go")
}

fn is_bin_path(path: &str) -> bool {
    path.contains("/bin/") || path.starts_with("bin/")
}

// ---------------------------------------------------------------------------
// Queue consumer detection
// ---------------------------------------------------------------------------

fn detect_queue_consumers(file: &ParsedFile, out: &mut Vec<Entrypoint>) {
    let consumer_patterns = [
        "subscribe",
        "consume",
        "onMessage",
        "on_message",
        "process",
        "handle_message",
        "handleMessage",
    ];

    let queue_imports = [
        "amqplib",
        "bull",
        "bullmq",
        "bee-queue",
        "sqs-consumer",
        "kafkajs",
        "celery",
        "kombu",
        "pika",
        "aio_pika",
        "rq",
    ];

    let has_queue_import = file.imports.iter().any(|i| {
        queue_imports
            .iter()
            .any(|q| i.source == *q || i.source.starts_with(&format!("{q}/")))
    });

    if !has_queue_import {
        return;
    }

    // Look for consumer registration call sites
    for call in &file.call_sites {
        let callee_lower = call.callee.to_lowercase();
        if consumer_patterns
            .iter()
            .any(|p| callee_lower.contains(&p.to_lowercase()))
        {
            let symbol = call
                .containing_function
                .clone()
                .unwrap_or_else(|| call.callee.clone());
            out.push(Entrypoint {
                file: file.path.clone(),
                symbol,
                entrypoint_type: EntrypointType::QueueConsumer,
            });
        }
    }

    // Check for worker/consumer file path patterns
    if is_worker_path(&file.path) {
        for def in &file.definitions {
            if def.kind == crate::types::SymbolKind::Function
                && (def.name.contains("process")
                    || def.name.contains("handle")
                    || def.name.contains("consume")
                    || def.name == "run"
                    || def.name == "execute")
            {
                out.push(Entrypoint {
                    file: file.path.clone(),
                    symbol: def.name.clone(),
                    entrypoint_type: EntrypointType::QueueConsumer,
                });
            }
        }
    }
}

fn is_worker_path(path: &str) -> bool {
    path.contains("/workers/")
        || path.contains("/jobs/")
        || path.contains("/consumers/")
        || path.contains("/tasks/")
        || path.ends_with("_worker.py")
        || path.ends_with("_worker.ts")
        || path.ends_with("_worker.js")
        || path.ends_with("Worker.ts")
        || path.ends_with("Worker.js")
}

// ---------------------------------------------------------------------------
// Cron job detection
// ---------------------------------------------------------------------------

fn detect_cron_jobs(file: &ParsedFile, out: &mut Vec<Entrypoint>) {
    let cron_imports = [
        "node-cron",
        "cron",
        "node-schedule",
        "agenda",
        "apscheduler",
        "schedule",
        "celery",
        "celery.schedules",
    ];

    let has_cron_import = file.imports.iter().any(|i| {
        cron_imports
            .iter()
            .any(|c| i.source == *c || i.source.starts_with(&format!("{c}.")))
    });

    if !has_cron_import && !is_cron_path(&file.path) {
        return;
    }

    let cron_call_patterns = ["schedule", "cron", "every", "interval", "addJob", "add_job"];

    for call in &file.call_sites {
        if cron_call_patterns.iter().any(|p| call.callee.contains(p)) {
            let symbol = call
                .containing_function
                .clone()
                .unwrap_or_else(|| call.callee.clone());
            out.push(Entrypoint {
                file: file.path.clone(),
                symbol,
                entrypoint_type: EntrypointType::CronJob,
            });
        }
    }
}

fn is_cron_path(path: &str) -> bool {
    path.contains("/cron/")
        || path.contains("/scheduler/")
        || path.contains("/scheduled/")
        || path.ends_with("_cron.py")
        || path.ends_with("_scheduler.py")
}

// ---------------------------------------------------------------------------
// React page detection
// ---------------------------------------------------------------------------

fn detect_react_pages(file: &ParsedFile, out: &mut Vec<Entrypoint>) {
    if !matches!(file.language, Language::TypeScript | Language::JavaScript) {
        return;
    }

    // Already handled by HTTP route detection for Next.js route files
    if is_nextjs_route_file(&file.path) {
        return;
    }

    // React page conventions: default export from pages/ or app/ directories
    let is_page = is_react_page_path(&file.path);
    if !is_page {
        return;
    }

    for export in &file.exports {
        if export.is_default {
            out.push(Entrypoint {
                file: file.path.clone(),
                symbol: export.name.clone(),
                entrypoint_type: EntrypointType::ReactPage,
            });
            return;
        }
    }

    // Also check for page.tsx (Next.js App Router page component)
    if file
        .path
        .split('/')
        .last()
        .map_or(false, |f| f.starts_with("page."))
    {
        for export in &file.exports {
            if export.is_default {
                out.push(Entrypoint {
                    file: file.path.clone(),
                    symbol: export.name.clone(),
                    entrypoint_type: EntrypointType::ReactPage,
                });
                return;
            }
        }
    }
}

fn is_react_page_path(path: &str) -> bool {
    let lower = path.to_lowercase();
    // Next.js App Router page.tsx
    let filename = path.split('/').last().unwrap_or("");
    if filename.starts_with("page.") {
        return true;
    }
    // Pages directory (excluding API routes and internals)
    let in_pages = lower.contains("/pages/") || lower.starts_with("pages/");
    if in_pages {
        return !lower.contains("/api/") && !filename.starts_with('_');
    }
    false
}

// ---------------------------------------------------------------------------
// Event handler detection
// ---------------------------------------------------------------------------

fn detect_event_handlers(file: &ParsedFile, out: &mut Vec<Entrypoint>) {
    let listener_patterns = ["addEventListener", "on", "addListener", "once", "subscribe"];

    let event_imports = ["events", "eventemitter3", "mitt", "rxjs", "socket.io", "ws"];

    let has_event_import = file.imports.iter().any(|i| {
        event_imports
            .iter()
            .any(|e| i.source == *e || i.source.starts_with(&format!("{e}/")))
    });

    if !has_event_import {
        return;
    }

    for call in &file.call_sites {
        // Match patterns like emitter.on(), socket.addEventListener(), etc.
        let callee_parts: Vec<&str> = call.callee.rsplitn(2, '.').collect();
        if callee_parts.len() == 2 {
            let method = callee_parts[0];
            if listener_patterns.contains(&method) {
                let symbol = call
                    .containing_function
                    .clone()
                    .unwrap_or_else(|| call.callee.clone());
                out.push(Entrypoint {
                    file: file.path.clone(),
                    symbol,
                    entrypoint_type: EntrypointType::EventHandler,
                });
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Effect.ts entrypoint detection
// ---------------------------------------------------------------------------

fn detect_effect_ts(file: &ParsedFile, out: &mut Vec<Entrypoint>) {
    if !matches!(file.language, Language::TypeScript | Language::JavaScript) {
        return;
    }

    detect_effect_http_routes(file, out);
    detect_effect_cli_commands(file, out);
    detect_effect_queue_consumers(file, out);
    detect_effect_cron_jobs(file, out);
    detect_effect_test_files(file, out);
    detect_effect_event_handlers(file, out);
    detect_effect_services(file, out);
}

/// Check if an import source is from the Effect platform HTTP packages.
fn has_effect_http_import(imports: &[ImportInfo]) -> bool {
    imports.iter().any(|i| {
        i.source == "@effect/platform"
            || i.source.starts_with("@effect/platform/Http")
            || i.source == "@effect/platform-node"
            || i.source == "@effect/platform-bun"
    })
}

/// Check if specific Effect HTTP names are imported.
fn has_effect_http_name(imports: &[ImportInfo]) -> bool {
    let http_names = [
        "HttpApi",
        "HttpApiEndpoint",
        "HttpApiGroup",
        "HttpRouter",
        "HttpServer",
        "HttpApiBuilder",
    ];
    imports.iter().any(|i| {
        // Direct subpath import like @effect/platform/HttpApi
        if let Some(last) = i.source.rsplit('/').next() {
            if http_names.contains(&last) {
                return true;
            }
        }
        // Named import from @effect/platform
        i.names
            .iter()
            .any(|n| http_names.contains(&n.name.as_str()))
    })
}

/// Detect Effect.ts HTTP routes: HttpApi, HttpApiEndpoint, HttpApiGroup, HttpRouter, HttpServer.
fn detect_effect_http_routes(file: &ParsedFile, out: &mut Vec<Entrypoint>) {
    if !has_effect_http_import(&file.imports) && !has_effect_http_name(&file.imports) {
        return;
    }

    let route_call_prefixes = [
        "HttpApiEndpoint.get",
        "HttpApiEndpoint.post",
        "HttpApiEndpoint.put",
        "HttpApiEndpoint.del",
        "HttpApiEndpoint.delete",
        "HttpApiEndpoint.patch",
        "HttpApiEndpoint.head",
        "HttpApiEndpoint.options",
        "HttpApiEndpoint.make",
        "HttpApi.make",
        "HttpApi.empty",
        "HttpApiGroup.make",
        "HttpRouter.get",
        "HttpRouter.post",
        "HttpRouter.put",
        "HttpRouter.del",
        "HttpRouter.delete",
        "HttpRouter.patch",
        "HttpRouter.all",
        "HttpRouter.mount",
        "HttpRouter.route",
    ];

    for call in &file.call_sites {
        if route_call_prefixes.iter().any(|p| call.callee == *p) {
            let symbol = call
                .containing_function
                .clone()
                .unwrap_or_else(|| call.callee.clone());
            out.push(Entrypoint {
                file: file.path.clone(),
                symbol,
                entrypoint_type: EntrypointType::HttpRoute,
            });
        }
    }

    // Also check definitions that reference HttpApi/HttpRouter types
    let http_def_names = ["HttpApi", "HttpApiGroup", "HttpRouter"];
    for def in &file.definitions {
        if http_def_names.iter().any(|n| def.name.contains(n)) {
            out.push(Entrypoint {
                file: file.path.clone(),
                symbol: def.name.clone(),
                entrypoint_type: EntrypointType::HttpRoute,
            });
        }
    }
}

/// Detect Effect.ts CLI commands: @effect/cli Command, Args, Options.
fn detect_effect_cli_commands(file: &ParsedFile, out: &mut Vec<Entrypoint>) {
    let has_cli_import = file.imports.iter().any(|i| {
        i.source == "@effect/cli"
            || i.source.starts_with("@effect/cli/")
            || i.names
                .iter()
                .any(|n| n.name == "Command" || n.name == "Args" || n.name == "Options")
                && i.source.starts_with("@effect/cli")
    });

    if !has_cli_import {
        return;
    }

    let cli_calls = [
        "Command.make",
        "Command.run",
        "Command.provide",
        "Command.withHandler",
    ];

    for call in &file.call_sites {
        if cli_calls.iter().any(|c| call.callee == *c) {
            let symbol = call
                .containing_function
                .clone()
                .unwrap_or_else(|| call.callee.clone());
            out.push(Entrypoint {
                file: file.path.clone(),
                symbol,
                entrypoint_type: EntrypointType::CliCommand,
            });
        }
    }
}

/// Detect Effect.ts queue consumers: Queue, PubSub consumer patterns.
fn detect_effect_queue_consumers(file: &ParsedFile, out: &mut Vec<Entrypoint>) {
    let has_queue_import = file.imports.iter().any(|i| {
        (i.source == "effect" || i.source == "effect/Queue" || i.source == "effect/PubSub")
            && i.names
                .iter()
                .any(|n| n.name == "Queue" || n.name == "PubSub")
            || i.source == "effect/Queue"
            || i.source == "effect/PubSub"
    });

    if !has_queue_import {
        return;
    }

    let consumer_calls = [
        "Queue.take",
        "Queue.poll",
        "Queue.dequeue",
        "Queue.takeBetween",
        "Queue.takeAll",
        "Queue.takeUpTo",
        "PubSub.subscribe",
        "PubSub.subscribeScopedWith",
    ];

    for call in &file.call_sites {
        if consumer_calls.iter().any(|c| call.callee == *c) {
            let symbol = call
                .containing_function
                .clone()
                .unwrap_or_else(|| call.callee.clone());
            out.push(Entrypoint {
                file: file.path.clone(),
                symbol,
                entrypoint_type: EntrypointType::QueueConsumer,
            });
        }
    }
}

/// Detect Effect.ts cron jobs: Schedule, @effect/cron patterns.
fn detect_effect_cron_jobs(file: &ParsedFile, out: &mut Vec<Entrypoint>) {
    let has_schedule_import = file.imports.iter().any(|i| {
        i.source == "@effect/cron"
            || i.source.starts_with("@effect/cron/")
            || ((i.source == "effect" || i.source == "effect/Schedule")
                && i.names.iter().any(|n| n.name == "Schedule"))
            || i.source == "effect/Schedule"
    });

    if !has_schedule_import {
        return;
    }

    let schedule_calls = [
        "Schedule.cron",
        "Schedule.fixed",
        "Schedule.spaced",
        "Schedule.recurring",
        "Schedule.forever",
        "Schedule.once",
        "Schedule.dayOfWeek",
        "Schedule.hourOfDay",
        "Cron.make",
        "Cron.parse",
        "Cron.match",
    ];

    for call in &file.call_sites {
        if schedule_calls.iter().any(|c| call.callee == *c) {
            let symbol = call
                .containing_function
                .clone()
                .unwrap_or_else(|| call.callee.clone());
            out.push(Entrypoint {
                file: file.path.clone(),
                symbol,
                entrypoint_type: EntrypointType::CronJob,
            });
        }
    }
}

/// Detect Effect.ts test files: @effect/vitest it.effect, it.scoped patterns.
fn detect_effect_test_files(file: &ParsedFile, out: &mut Vec<Entrypoint>) {
    let has_vitest_import = file
        .imports
        .iter()
        .any(|i| i.source == "@effect/vitest" || i.source.starts_with("@effect/vitest/"));

    if !has_vitest_import {
        return;
    }

    let test_calls = ["it.effect", "it.scoped", "it.live", "it.flakyTest"];

    for call in &file.call_sites {
        if test_calls.iter().any(|c| call.callee == *c) {
            let symbol = call
                .containing_function
                .clone()
                .unwrap_or_else(|| call.callee.clone());
            out.push(Entrypoint {
                file: file.path.clone(),
                symbol,
                entrypoint_type: EntrypointType::TestFile,
            });
        }
    }
}

/// Detect Effect.ts event handlers: Stream, PubSub, Hub listener patterns.
fn detect_effect_event_handlers(file: &ParsedFile, out: &mut Vec<Entrypoint>) {
    let has_stream_import = file.imports.iter().any(|i| {
        (i.source == "effect"
            && i.names
                .iter()
                .any(|n| n.name == "Stream" || n.name == "Hub"))
            || i.source == "effect/Stream"
            || i.source == "effect/Hub"
    });

    if !has_stream_import {
        return;
    }

    let handler_calls = [
        "Stream.run",
        "Stream.runForEach",
        "Stream.runCollect",
        "Stream.runDrain",
        "Stream.runFold",
        "Stream.runScoped",
        "Hub.subscribe",
        "Hub.publishAll",
    ];

    for call in &file.call_sites {
        if handler_calls.iter().any(|c| call.callee == *c) {
            let symbol = call
                .containing_function
                .clone()
                .unwrap_or_else(|| call.callee.clone());
            out.push(Entrypoint {
                file: file.path.clone(),
                symbol,
                entrypoint_type: EntrypointType::EventHandler,
            });
        }
    }
}

/// Detect Effect.ts services: Effect.Service, Context.Tag, Layer definitions.
fn detect_effect_services(file: &ParsedFile, out: &mut Vec<Entrypoint>) {
    let has_effect_import = file.imports.iter().any(|i| {
        (i.source == "effect"
            && i.names
                .iter()
                .any(|n| n.name == "Effect" || n.name == "Context" || n.name == "Layer"))
            || i.source == "effect/Effect"
            || i.source == "effect/Context"
            || i.source == "effect/Layer"
    });

    if !has_effect_import {
        return;
    }

    let service_calls = [
        "Effect.Service",
        "Context.Tag",
        "Context.GenericTag",
        "Layer.effect",
        "Layer.succeed",
        "Layer.scoped",
        "Layer.sync",
        "Layer.fail",
        "Layer.provide",
        "Layer.merge",
    ];

    for call in &file.call_sites {
        if service_calls.iter().any(|c| call.callee == *c) {
            let symbol = call
                .containing_function
                .clone()
                .unwrap_or_else(|| call.callee.clone());
            out.push(Entrypoint {
                file: file.path.clone(),
                symbol,
                entrypoint_type: EntrypointType::EffectService,
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Rust module/plugin entrypoint detection
// ---------------------------------------------------------------------------

/// Detect Rust module/plugin entrypoints by path convention.
///
/// Many Rust projects use a module/plugin pattern where each file in
/// `src/modules/`, `src/commands/`, `src/plugins/`, `src/handlers/` etc.
/// is a self-contained entrypoint. Also detects `main.rs` and `lib.rs`
/// as structural entrypoints when they define public functions.
fn detect_rust_modules(file: &ParsedFile, out: &mut Vec<Entrypoint>) {
    if file.language != Language::Rust {
        return;
    }

    let lower = file.path.to_lowercase();

    // Rust module/plugin directories — files here are treated as entrypoints
    let is_module_path = lower.contains("/modules/")
        || lower.contains("/commands/")
        || lower.contains("/plugins/")
        || lower.contains("/handlers/")
        || lower.contains("/subcommands/")
        || lower.contains("/cmds/");

    // Skip mod.rs — it's a re-export barrel, not a module entrypoint itself
    let is_mod_rs = lower.ends_with("/mod.rs");

    if is_module_path && !is_mod_rs {
        let stem = file_stem(&file.path);
        out.push(Entrypoint {
            file: file.path.clone(),
            symbol: stem,
            entrypoint_type: EntrypointType::CliCommand,
        });
    }

    // Paired config files (src/configs/*.rs) that mirror modules
    // These are entrypoints because they define the config struct for a module
    let is_config_path = lower.contains("/configs/") && !is_mod_rs;
    if is_config_path {
        let stem = file_stem(&file.path);
        out.push(Entrypoint {
            file: file.path.clone(),
            symbol: stem,
            entrypoint_type: EntrypointType::CliCommand,
        });
    }

    // main.rs / lib.rs as structural entrypoints (if they have definitions)
    let is_root_rs = lower.ends_with("/main.rs")
        || lower.ends_with("/lib.rs")
        || lower == "src/main.rs"
        || lower == "src/lib.rs";
    if is_root_rs && !file.definitions.is_empty() {
        out.push(Entrypoint {
            file: file.path.clone(),
            symbol: file_stem(&file.path),
            entrypoint_type: EntrypointType::CliCommand,
        });
    }
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

fn file_stem(path: &str) -> String {
    path.split('/')
        .last()
        .unwrap_or(path)
        .split('.')
        .next()
        .unwrap_or(path)
        .to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr
)]
mod tests;
