//! Data flow tracing and heuristic inference module.
//!
//! Analyzes parsed files to infer additional data flow edges beyond what
//! static import/call analysis can determine. Uses pattern matching on
//! call sites and identifiers to detect:
//!
//! - Database persistence patterns (`.save()`, `.insert()`, `INSERT INTO`)
//! - Database read patterns (`.find()`, `.query()`, `SELECT`)
//! - Event emission (`.emit()`, `.publish()`, `.dispatch()`)
//! - Event handling (`.on()`, `.subscribe()`, `.listen()`)
//! - Configuration reads (`process.env`, `os.environ`)
//! - HTTP outbound calls (`fetch()`, `axios.get()`)
//! - Logging calls (`console.log`, `logger.info`)
//!
//! Also detects frameworks from import patterns.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use aho_corasick::AhoCorasick;

use crate::ast::{CallSite, ParsedFile};
use crate::graph::{GraphEdge, SymbolGraph};
use crate::ir::IrFile;
use crate::types::EdgeType;

/// A data flow pattern detected via heuristic matching.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FlowPattern {
    /// Database write: `.save()`, `.insert()`, `.create()`, `.update()`, `.delete()`, `INSERT INTO`
    Persistence,
    /// Database read: `.find()`, `.query()`, `.select()`, `.findOne()`, `SELECT`
    DatabaseRead,
    /// Event emission: `.emit()`, `.publish()`, `.send()`, `.dispatch()`
    EventEmission,
    /// Event handling: `.on()`, `.subscribe()`, `.listen()`, `.addEventListener()`
    EventHandling,
    /// Configuration read: `process.env`, `os.environ`, `config.get()`
    ConfigRead,
    /// HTTP outbound call: `fetch()`, `axios.get()`, `requests.get()`
    HttpCall,
    /// Logging: `console.log`, `logger.info`, `logging.debug`
    Logging,
}

/// A heuristic edge inferred from code patterns.
#[derive(Debug, Clone, PartialEq)]
pub struct HeuristicEdge {
    /// Symbol id of the function containing the pattern (e.g. `file.ts::handler`)
    pub from_symbol: String,
    /// The file containing the pattern
    pub file: String,
    /// The detected flow pattern
    pub pattern: FlowPattern,
    /// Confidence score [0.0, 1.0]
    pub confidence: f64,
    /// The callee string that matched (evidence)
    pub evidence: String,
    /// Line number where the pattern was detected
    pub line: usize,
}

/// Result of data flow analysis across all files.
#[derive(Debug, Clone)]
pub struct FlowAnalysis {
    /// Heuristic edges inferred from code patterns.
    pub heuristic_edges: Vec<HeuristicEdge>,
    /// Frameworks detected from import patterns.
    pub frameworks_detected: Vec<String>,
}

/// A data flow edge connecting a producer function to a consumer function
/// through a shared variable within the same function scope.
///
/// Example: `const x = funcA(); funcB(x)` creates an edge from funcA → funcB via "x".
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DataFlowEdge {
    /// Callee of the assignment (the function producing data).
    pub producer: String,
    /// Callee of the consuming call (the function receiving the data).
    pub consumer: String,
    /// Variable name connecting the producer to the consumer.
    pub via: String,
    /// Symbol ID of the function containing both calls.
    pub containing_function: String,
    /// File path.
    pub file: String,
    /// Line of the consumer call.
    pub line: usize,
}

/// Configuration for flow analysis.
#[derive(Debug, Clone)]
pub struct FlowConfig {
    /// Maximum call chain depth to trace (prevents runaway on cycles).
    pub max_depth: usize,
}

impl Default for FlowConfig {
    fn default() -> Self {
        Self { max_depth: 10 }
    }
}

// ---------------------------------------------------------------------------
// Heuristic pattern matching rules
// ---------------------------------------------------------------------------

/// Persistence (database write) patterns.
const DB_WRITE_METHODS: &[&str] = &[
    ".save",
    ".insert",
    ".create",
    ".update",
    ".delete",
    ".remove",
    ".upsert",
    ".bulkCreate",
    ".bulkInsert",
    ".insertMany",
    ".updateMany",
    ".deleteMany",
    ".findAndUpdate",
    ".findOneAndUpdate",
    ".findOneAndDelete",
    ".findOneAndRemove",
    ".persist",
    ".flush",
    ".execute",
    ".run",
];

/// SQL write keywords (case-insensitive matching on string literals).
const SQL_WRITE_KEYWORDS: &[&str] = &[
    "INSERT INTO",
    "UPDATE ",
    "DELETE FROM",
    "DROP TABLE",
    "ALTER TABLE",
    "CREATE TABLE",
    "TRUNCATE",
];

/// Database read patterns.
const DB_READ_METHODS: &[&str] = &[
    ".find",
    ".findOne",
    ".findById",
    ".findAll",
    ".findMany",
    ".findFirst",
    ".findUnique",
    ".query",
    ".select",
    ".get",
    ".fetch",
    ".count",
    ".aggregate",
    ".groupBy",
    ".where",
];

/// SQL read keywords.
const SQL_READ_KEYWORDS: &[&str] = &["SELECT ", "SELECT\n"];

/// Event emission patterns.
const EVENT_EMIT_METHODS: &[&str] = &[
    ".emit",
    ".publish",
    ".send",
    ".dispatch",
    ".fire",
    ".trigger",
    ".broadcast",
    ".notify",
    ".produce",
    ".enqueue",
];

/// Event handling patterns.
const EVENT_HANDLE_METHODS: &[&str] = &[
    ".on",
    ".subscribe",
    ".listen",
    ".addEventListener",
    ".addListener",
    ".handle",
    ".consume",
    ".onMessage",
    ".onEvent",
];

/// Config read patterns.
const CONFIG_PATTERNS: &[&str] = &[
    "process.env",
    "os.environ",
    "os.getenv",
    "config.get",
    "config.set",
    "dotenv",
    "Deno.env",
];

/// HTTP outbound call patterns.
const HTTP_CALL_PATTERNS: &[&str] = &[
    "fetch",
    "axios.get",
    "axios.post",
    "axios.put",
    "axios.delete",
    "axios.patch",
    "axios.request",
    "requests.get",
    "requests.post",
    "requests.put",
    "requests.delete",
    "requests.patch",
    "http.get",
    "http.post",
    "http.request",
    "urllib.request",
    "httpx.get",
    "httpx.post",
];

/// Logging patterns.
const LOG_PATTERNS: &[&str] = &[
    "console.log",
    "console.error",
    "console.warn",
    "console.info",
    "console.debug",
    "console.trace",
    "logger.info",
    "logger.error",
    "logger.warn",
    "logger.debug",
    "logger.trace",
    "logger.fatal",
    "logging.info",
    "logging.error",
    "logging.warning",
    "logging.debug",
    "logging.critical",
    "log.info",
    "log.error",
    "log.warn",
    "log.debug",
];

// ---------------------------------------------------------------------------
// Framework detection
// ---------------------------------------------------------------------------

/// Known framework import sources and their display names.
const FRAMEWORK_IMPORTS: &[(&str, &str)] = &[
    // JavaScript/TypeScript
    ("express", "Express"),
    ("fastify", "Fastify"),
    ("next", "Next.js"),
    ("next/", "Next.js"),
    ("react", "React"),
    ("react-dom", "React"),
    ("vue", "Vue"),
    ("@angular/core", "Angular"),
    ("svelte", "Svelte"),
    ("@nestjs/common", "NestJS"),
    ("@nestjs/core", "NestJS"),
    ("hono", "Hono"),
    ("koa", "Koa"),
    ("@effect/", "Effect.ts"),
    ("effect", "Effect.ts"),
    ("prisma", "Prisma"),
    ("@prisma/client", "Prisma"),
    ("typeorm", "TypeORM"),
    ("sequelize", "Sequelize"),
    ("mongoose", "Mongoose"),
    ("drizzle-orm", "Drizzle"),
    ("@trpc/server", "tRPC"),
    ("@trpc/client", "tRPC"),
    ("graphql", "GraphQL"),
    ("@apollo/server", "Apollo"),
    ("@apollo/client", "Apollo"),
    ("tailwindcss", "Tailwind CSS"),
    ("redux", "Redux"),
    ("@reduxjs/toolkit", "Redux"),
    ("zustand", "Zustand"),
    ("zod", "Zod"),
    ("vitest", "Vitest"),
    ("jest", "Jest"),
    ("@effect/vitest", "Effect.ts"),
    // Python
    ("fastapi", "FastAPI"),
    ("flask", "Flask"),
    ("django", "Django"),
    ("sqlalchemy", "SQLAlchemy"),
    ("pydantic", "Pydantic"),
    ("celery", "Celery"),
    ("pytest", "pytest"),
    ("alembic", "Alembic"),
    ("tortoise", "Tortoise ORM"),
    ("starlette", "Starlette"),
    ("aiohttp", "aiohttp"),
    ("httpx", "httpx"),
    ("uvicorn", "Uvicorn"),
    // Go
    ("net/http", "Go net/http"),
    ("github.com/gin-gonic/gin", "Gin"),
    ("github.com/labstack/echo", "Echo"),
    ("github.com/go-chi/chi", "Chi"),
    ("github.com/gofiber/fiber", "Fiber"),
    ("github.com/gorilla/mux", "Gorilla Mux"),
    ("google.golang.org/grpc", "gRPC"),
    ("github.com/spf13/cobra", "Cobra"),
    ("github.com/spf13/viper", "Viper"),
    ("gorm.io/gorm", "GORM"),
    ("github.com/jmoiron/sqlx", "sqlx"),
    ("database/sql", "Go database/sql"),
    ("github.com/go-playground/validator", "Go Validator"),
    ("github.com/stretchr/testify", "Testify"),
    // Rust
    ("actix_web", "Actix-web"),
    ("actix-web", "Actix-web"),
    ("axum", "Axum"),
    ("rocket", "Rocket"),
    ("warp", "Warp"),
    ("hyper", "Hyper"),
    ("tokio", "Tokio"),
    ("diesel", "Diesel"),
    ("sqlx", "SQLx"),
    ("sea_orm", "SeaORM"),
    ("sea-orm", "SeaORM"),
    ("clap", "Clap"),
    ("tauri", "Tauri"),
    ("serde", "Serde"),
    ("tower", "Tower"),
    ("tonic", "Tonic"),
    ("tracing", "Tracing"),
    // Java
    ("org.springframework.boot", "Spring Boot"),
    ("org.springframework.web", "Spring MVC"),
    ("org.springframework.data", "Spring Data"),
    ("org.springframework.stereotype", "Spring Boot"),
    ("org.springframework.beans", "Spring Boot"),
    ("org.springframework.context", "Spring Boot"),
    ("org.springframework.security", "Spring Security"),
    ("jakarta.persistence", "JPA"),
    ("javax.persistence", "JPA"),
    ("jakarta.ws.rs", "JAX-RS"),
    ("javax.ws.rs", "JAX-RS"),
    ("jakarta.servlet", "Servlet"),
    ("javax.servlet", "Servlet"),
    ("org.hibernate", "Hibernate"),
    ("org.junit", "JUnit"),
    ("org.junit.jupiter", "JUnit 5"),
    ("org.mockito", "Mockito"),
    ("com.google.inject", "Guice"),
    ("io.micronaut", "Micronaut"),
    ("io.quarkus", "Quarkus"),
    ("org.apache.maven", "Maven"),
    // C#
    ("Microsoft.AspNetCore", "ASP.NET Core"),
    ("Microsoft.AspNetCore.Mvc", "ASP.NET Core MVC"),
    ("Microsoft.AspNetCore.Builder", "ASP.NET Core"),
    ("Microsoft.AspNetCore.Http", "ASP.NET Core"),
    ("Microsoft.AspNetCore.Routing", "ASP.NET Core"),
    ("Microsoft.AspNetCore.Authorization", "ASP.NET Core"),
    ("Microsoft.AspNetCore.Identity", "ASP.NET Identity"),
    ("Microsoft.AspNetCore.SignalR", "SignalR"),
    ("Microsoft.EntityFrameworkCore", "Entity Framework Core"),
    ("Microsoft.Extensions.DependencyInjection", "ASP.NET Core"),
    ("Microsoft.Extensions.Logging", "ASP.NET Core"),
    ("Microsoft.Extensions.Configuration", "ASP.NET Core"),
    ("System.Linq", "LINQ"),
    ("Xunit", "xUnit"),
    ("NUnit", "NUnit"),
    ("Microsoft.VisualStudio.TestTools", "MSTest"),
    ("Moq", "Moq"),
    ("FluentAssertions", "FluentAssertions"),
    ("MediatR", "MediatR"),
    ("AutoMapper", "AutoMapper"),
    ("Newtonsoft.Json", "Newtonsoft.Json"),
    ("System.Text.Json", "System.Text.Json"),
    ("Dapper", "Dapper"),
    ("Microsoft.AspNetCore.Components", "Blazor"),
    // PHP (use namespace segments without trailing backslash;
    // the match logic adds \ as a separator)
    ("Illuminate", "Laravel"),
    ("Illuminate\\Http", "Laravel"),
    ("Illuminate\\Routing", "Laravel"),
    ("Illuminate\\Database", "Laravel Eloquent"),
    ("Illuminate\\Queue", "Laravel Queue"),
    ("Illuminate\\Console", "Laravel Artisan"),
    ("Illuminate\\Support", "Laravel"),
    ("Laravel", "Laravel"),
    ("Symfony", "Symfony"),
    ("Symfony\\Component\\HttpFoundation", "Symfony"),
    ("Symfony\\Component\\Console", "Symfony Console"),
    ("Symfony\\Component\\Routing", "Symfony"),
    ("Doctrine\\ORM", "Doctrine ORM"),
    ("Doctrine\\DBAL", "Doctrine DBAL"),
    ("Slim", "Slim"),
    ("GuzzleHttp", "Guzzle"),
    ("Monolog", "Monolog"),
    ("PHPUnit", "PHPUnit"),
    ("Livewire", "Livewire"),
    ("Inertia", "Inertia"),
    // Ruby
    ("rails", "Rails"),
    ("action_controller", "Rails"),
    ("active_record", "Rails ActiveRecord"),
    ("active_support", "Rails"),
    ("action_view", "Rails"),
    ("action_mailer", "Rails"),
    ("active_job", "Rails ActiveJob"),
    ("active_storage", "Rails"),
    ("action_cable", "Rails ActionCable"),
    ("sinatra", "Sinatra"),
    ("rack", "Rack"),
    ("grape", "Grape"),
    ("hanami", "Hanami"),
    ("rspec", "RSpec"),
    ("minitest", "Minitest"),
    ("sidekiq", "Sidekiq"),
    ("devise", "Devise"),
    ("pundit", "Pundit"),
    ("cancancan", "CanCanCan"),
    ("sequel", "Sequel"),
    ("mongoid", "Mongoid"),
    ("dry-rb", "dry-rb"),
    ("roda", "Roda"),
    ("puma", "Puma"),
    ("faraday", "Faraday"),
    ("httparty", "HTTParty"),
    ("factory_bot", "FactoryBot"),
    ("rubocop", "RuboCop"),
    // Kotlin
    ("io.ktor", "Ktor"),
    ("io.ktor.server", "Ktor"),
    ("io.ktor.client", "Ktor Client"),
    ("io.ktor.routing", "Ktor"),
    ("org.springframework", "Spring Boot"),
    ("org.springframework.boot", "Spring Boot"),
    ("org.springframework.web", "Spring MVC"),
    ("org.springframework.data", "Spring Data"),
    ("org.jetbrains.exposed", "Exposed"),
    ("org.jetbrains.compose", "Jetpack Compose"),
    ("androidx.compose", "Jetpack Compose"),
    ("kotlinx.coroutines", "Kotlin Coroutines"),
    ("kotlinx.serialization", "Kotlin Serialization"),
    ("org.junit", "JUnit"),
    ("kotlin.test", "Kotlin Test"),
    ("io.kotest", "Kotest"),
    ("io.mockk", "MockK"),
    ("org.koin", "Koin"),
    ("com.squareup.retrofit2", "Retrofit"),
    ("com.squareup.okhttp3", "OkHttp"),
    ("io.arrow-kt", "Arrow"),
    ("com.google.dagger", "Dagger/Hilt"),
    // Swift
    ("SwiftUI", "SwiftUI"),
    ("UIKit", "UIKit"),
    ("Foundation", "Foundation"),
    ("Vapor", "Vapor"),
    ("Fluent", "Fluent"),
    ("FluentPostgresDriver", "Fluent"),
    ("FluentSQLiteDriver", "Fluent"),
    ("FluentMySQLDriver", "Fluent"),
    ("XCTest", "XCTest"),
    ("Combine", "Combine"),
    ("CoreData", "Core Data"),
    ("SwiftData", "SwiftData"),
    ("Alamofire", "Alamofire"),
    ("Kitura", "Kitura"),
    ("Perfect", "Perfect"),
    ("Hummingbird", "Hummingbird"),
    ("Observation", "Observation"),
    ("SwiftNIO", "SwiftNIO"),
    ("GRDB", "GRDB"),
    ("SnapKit", "SnapKit"),
    ("Quick", "Quick"),
    ("Nimble", "Nimble"),
    // C
    ("stdio.h", "C stdio"),
    ("stdlib.h", "C stdlib"),
    ("string.h", "C string"),
    ("pthread.h", "POSIX threads"),
    ("unistd.h", "POSIX"),
    ("curl/curl.h", "libcurl"),
    ("sqlite3.h", "SQLite3"),
    ("mysql.h", "MySQL C API"),
    ("libpq-fe.h", "PostgreSQL libpq"),
    ("openssl/ssl.h", "OpenSSL"),
    ("jansson.h", "Jansson"),
    ("cjson/cJSON.h", "cJSON"),
    ("check.h", "Check"),
    ("cmocka.h", "CMocka"),
    // C++
    ("iostream", "C++ STL"),
    ("vector", "C++ STL"),
    ("memory", "C++ STL"),
    ("string", "C++ STL"),
    ("algorithm", "C++ STL"),
    ("thread", "C++ STL"),
    ("mutex", "C++ STL"),
    ("boost/asio.hpp", "Boost.Asio"),
    ("boost/beast.hpp", "Boost.Beast"),
    ("boost/", "Boost"),
    ("crow.h", "Crow"),
    ("crow/crow.h", "Crow"),
    ("httplib.h", "cpp-httplib"),
    ("pistache/endpoint.h", "Pistache"),
    ("pistache/", "Pistache"),
    ("drogon/drogon.h", "Drogon"),
    ("drogon/", "Drogon"),
    ("cpprest/", "C++ REST SDK"),
    ("nlohmann/json.hpp", "nlohmann/json"),
    ("sqlite3.h", "SQLite3"),
    ("pqxx/pqxx", "libpqxx"),
    ("mysql++.h", "MySQL++"),
    ("gtest/gtest.h", "Google Test"),
    ("gmock/gmock.h", "Google Mock"),
    ("catch2/catch.hpp", "Catch2"),
    ("catch2/", "Catch2"),
    ("doctest/doctest.h", "doctest"),
    ("fmt/format.h", "fmt"),
    ("spdlog/spdlog.h", "spdlog"),
    ("grpcpp/grpcpp.h", "gRPC C++"),
    ("grpc++/", "gRPC C++"),
    ("absl/", "Abseil"),
    ("folly/", "Folly"),
    ("Qt", "Qt"),
    ("QApplication", "Qt"),
    ("QWidget", "Qt"),
    // Scala
    ("play.api.mvc", "Play Framework"),
    ("play.mvc", "Play Framework"),
    ("akka.actor", "Akka"),
    ("akka.stream", "Akka Streams"),
    ("akka.http", "Akka HTTP"),
    ("scala.concurrent", "Scala Concurrency"),
    ("org.scalatest", "ScalaTest"),
    ("org.specs2", "Specs2"),
    ("org.scalatestplus", "ScalaTestPlus"),
    ("org.mockito", "Mockito Scala"),
    ("slick", "Slick"),
    ("doobie", "Doobie"),
    ("quill", "Quill"),
    ("scalikejdbc", "ScalikeJDBC"),
    ("circe", "Circe"),
    ("spray", "Spray"),
    ("org.http4s", "http4s"),
    ("cats", "Cats"),
    ("cats.effect", "Cats Effect"),
    ("zio", "ZIO"),
    ("monix", "Monix"),
    ("fs2", "FS2"),
    ("shapeless", "Shapeless"),
    ("com.typesafe.config", "Typesafe Config"),
    ("io.getquill", "Quill"),
    ("sttp", "sttp"),
    ("tapir", "Tapir"),
];

// ---------------------------------------------------------------------------
// Pre-compiled pattern matchers (built once, reused across all files)
// ---------------------------------------------------------------------------

/// Suffix set for DB write methods (method name after last dot, e.g. "save").
fn db_write_suffix_set() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&str>> = OnceLock::new();
    SET.get_or_init(|| {
        DB_WRITE_METHODS
            .iter()
            .map(|s| s.trim_start_matches('.'))
            .collect()
    })
}

/// Suffix set for DB read methods.
fn db_read_suffix_set() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&str>> = OnceLock::new();
    SET.get_or_init(|| {
        DB_READ_METHODS
            .iter()
            .map(|s| s.trim_start_matches('.'))
            .collect()
    })
}

/// Suffix set for event emission methods.
fn event_emit_suffix_set() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&str>> = OnceLock::new();
    SET.get_or_init(|| {
        EVENT_EMIT_METHODS
            .iter()
            .map(|s| s.trim_start_matches('.'))
            .collect()
    })
}

/// Suffix set for event handling methods.
fn event_handle_suffix_set() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&str>> = OnceLock::new();
    SET.get_or_init(|| {
        EVENT_HANDLE_METHODS
            .iter()
            .map(|s| s.trim_start_matches('.'))
            .collect()
    })
}

/// Exact-match set for logging patterns.
fn log_pattern_set() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&str>> = OnceLock::new();
    SET.get_or_init(|| LOG_PATTERNS.iter().copied().collect())
}

/// Exact-match set for known non-DB callees.
fn non_db_callee_set() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&str>> = OnceLock::new();
    SET.get_or_init(|| {
        [
            "JSON.parse",
            "JSON.stringify",
            "Object.create",
            "Object.assign",
            "Array.from",
            "Promise.resolve",
            "Promise.reject",
            "Date.now",
            "Math.round",
            "Math.floor",
            "Math.ceil",
            "Math.abs",
            "Math.min",
            "Math.max",
        ]
        .into_iter()
        .collect()
    })
}

/// Exact-match set for known non-DB receivers (lowercased).
fn non_db_receiver_set() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&str>> = OnceLock::new();
    SET.get_or_init(|| {
        [
            "array",
            "map",
            "set",
            "object",
            "string",
            "number",
            "promise",
            "json",
            "math",
            "date",
            "regexp",
            "cache",
            "localstorage",
            "sessionstorage",
            "window",
            "document",
            "navigator",
            "console",
            "process",
            "os",
            "path",
            "fs",
            "http",
            "https",
            "url",
            "buffer",
            "stream",
            "crypto",
            "util",
            "events",
            "child_process",
            "cluster",
            "net",
            "tls",
            "dns",
            "axios",
            "requests",
            "fetch",
            "httpx",
            "urllib",
            "list",
            "dict",
            "tuple",
            "frozenset",
            "deque",
            "defaultdict",
            "items",
            "result",
            "results",
            "data",
            "response",
            "request",
            "config",
            "env",
            "settings",
            "options",
            "args",
            "params",
            "logger",
            "log",
            "logging",
            "console",
        ]
        .into_iter()
        .collect()
    })
}

/// Aho-Corasick automaton for DB-keyword substring matching in receivers.
fn db_keyword_automaton() -> &'static AhoCorasick {
    static AC: OnceLock<AhoCorasick> = OnceLock::new();
    AC.get_or_init(|| {
        AhoCorasick::new([
            "db",
            "database",
            "repo",
            "repository",
            "model",
            "store",
            "dao",
            "collection",
            "prisma",
            "sequelize",
            "typeorm",
            "mongoose",
            "drizzle",
            "session",
            "connection",
            "pool",
            "client",
            "table",
            "entity",
            "schema",
            "migration",
            "knex",
            "query",
            "sql",
        ])
        .expect("valid patterns")
    })
}

/// Aho-Corasick automaton for ORM-specific names in confidence scoring.
fn orm_automaton() -> &'static AhoCorasick {
    static AC: OnceLock<AhoCorasick> = OnceLock::new();
    AC.get_or_init(|| {
        AhoCorasick::new([
            "prisma",
            "sequelize",
            "typeorm",
            "mongoose",
            "sqlalchemy",
            "drizzle",
        ])
        .expect("valid patterns")
    })
}

/// Aho-Corasick automaton for high-confidence receiver keywords in confidence scoring.
fn confidence_receiver_automaton() -> &'static AhoCorasick {
    static AC: OnceLock<AhoCorasick> = OnceLock::new();
    AC.get_or_init(|| {
        AhoCorasick::new(["db", "repo", "model", "store", "dao", "collection"])
            .expect("valid patterns")
    })
}

/// Aho-Corasick automaton for SQL write keywords (lowercased).
fn sql_write_automaton() -> &'static AhoCorasick {
    static AC: OnceLock<AhoCorasick> = OnceLock::new();
    AC.get_or_init(|| {
        let patterns: Vec<String> = SQL_WRITE_KEYWORDS
            .iter()
            .map(|k| k.to_lowercase())
            .collect();
        AhoCorasick::new(&patterns).expect("valid patterns")
    })
}

/// Aho-Corasick automaton for SQL read keywords (lowercased).
fn sql_read_automaton() -> &'static AhoCorasick {
    static AC: OnceLock<AhoCorasick> = OnceLock::new();
    AC.get_or_init(|| {
        let patterns: Vec<String> = SQL_READ_KEYWORDS.iter().map(|k| k.to_lowercase()).collect();
        AhoCorasick::new(&patterns).expect("valid patterns")
    })
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Analyze data flow patterns across all parsed files.
///
/// Scans call sites for heuristic patterns (DB writes, event emission, config reads, etc.)
/// and detects frameworks from import patterns.
pub fn analyze_data_flow(files: &[ParsedFile], _config: &FlowConfig) -> FlowAnalysis {
    let mut heuristic_edges = Vec::new();

    for file in files {
        let file_edges = detect_heuristic_patterns(file);
        heuristic_edges.extend(file_edges);
    }

    let frameworks_detected = detect_frameworks(files);

    FlowAnalysis {
        heuristic_edges,
        frameworks_detected,
    }
}

/// Enrich an existing symbol graph with heuristic edges.
///
/// For each heuristic edge, adds the appropriate edge type (Writes, Reads, Emits, Handles)
/// from the containing symbol to the file's module node (since the target is typically
/// an external resource like a database or event bus).
pub fn enrich_graph(graph: &mut SymbolGraph, analysis: &FlowAnalysis) {
    for edge in &analysis.heuristic_edges {
        let edge_type = match edge.pattern {
            FlowPattern::Persistence => EdgeType::Writes,
            FlowPattern::DatabaseRead => EdgeType::Reads,
            FlowPattern::EventEmission => EdgeType::Emits,
            FlowPattern::EventHandling => EdgeType::Handles,
            FlowPattern::ConfigRead => EdgeType::Reads,
            FlowPattern::HttpCall => EdgeType::Reads,
            FlowPattern::Logging => continue, // Don't add graph edges for logging
        };

        let from_idx = match graph.get_node(&edge.from_symbol) {
            Some(idx) => idx,
            None => {
                // Try the file-level module node as fallback
                match graph.get_node(&edge.file) {
                    Some(idx) => idx,
                    None => continue,
                }
            }
        };

        // For heuristic edges, we connect to the file's module node since the actual
        // target (database, event bus, etc.) is external and not in our graph.
        let to_idx = match graph.get_node(&edge.file) {
            Some(idx) => idx,
            None => continue,
        };

        // Don't add self-edges
        if from_idx == to_idx {
            continue;
        }

        graph.add_edge(from_idx, to_idx, GraphEdge { edge_type });
    }
}

/// Detect frameworks from import patterns across all files.
pub fn detect_frameworks(files: &[ParsedFile]) -> Vec<String> {
    let mut frameworks: HashSet<String> = HashSet::new();

    for file in files {
        for import in &file.imports {
            let source = &import.source;
            for &(pattern, name) in FRAMEWORK_IMPORTS {
                // Match exact, or prefixed by separator: slash (JS/TS),
                // dot (Python), :: (Rust), or backslash (PHP namespaces)
                if source == pattern
                    || source.starts_with(pattern)
                        && source.as_bytes().get(pattern.len()).map_or(false, |&b| {
                            b == b'/' || b == b'.' || b == b':' || b == b'\\'
                        })
                {
                    frameworks.insert(name.to_string());
                }
            }
        }
    }

    // Also detect Next.js from file structure conventions
    for file in files {
        let path = &file.path;
        if path.contains("pages/") || path.contains("app/") {
            if path.ends_with("page.tsx")
                || path.ends_with("page.ts")
                || path.ends_with("page.jsx")
                || path.ends_with("page.js")
                || path.ends_with("route.ts")
                || path.ends_with("route.js")
                || path.ends_with("layout.tsx")
                || path.ends_with("layout.ts")
            {
                frameworks.insert("Next.js".to_string());
            }
        }
    }

    let mut result: Vec<String> = frameworks.into_iter().collect();
    result.sort();
    result
}

// ---------------------------------------------------------------------------
// Internal: heuristic pattern detection
// ---------------------------------------------------------------------------

/// Detect heuristic data flow patterns in a single file's call sites.
fn detect_heuristic_patterns(file: &ParsedFile) -> Vec<HeuristicEdge> {
    let mut edges = Vec::new();

    for call in &file.call_sites {
        if let Some(edge) = classify_call_site(call, &file.path) {
            edges.push(edge);
        }
    }

    edges
}

/// Classify a single call site into a flow pattern, if any.
///
/// Pattern matching order is important: more specific patterns are checked first
/// to avoid false positives (e.g., `axios.get` is HTTP, not a DB read).
fn classify_call_site(call: &CallSite, file_path: &str) -> Option<HeuristicEdge> {
    let callee = &call.callee;
    let containing = call
        .containing_function
        .as_ref()
        .map(|f| format!("{}::{}", file_path, f))
        .unwrap_or_else(|| file_path.to_string());

    let make_edge = |pattern: FlowPattern, confidence: f64| HeuristicEdge {
        from_symbol: containing.clone(),
        file: file_path.to_string(),
        pattern,
        confidence,
        evidence: callee.clone(),
        line: call.line,
    };

    // 1. Logging — most specific, check first to prevent console.log matching elsewhere
    if let Some(pattern) = match_logging(callee) {
        return Some(make_edge(pattern, 0.95));
    }

    // 2. Config reads — specific patterns like process.env, os.environ
    if let Some(pattern) = match_config_read(callee) {
        return Some(make_edge(pattern, 0.9));
    }

    // 3. HTTP calls — check before DB reads so axios.get/requests.get match HTTP
    if let Some(pattern) = match_http_call(callee) {
        return Some(make_edge(pattern, 0.85));
    }

    // 4. Check for collection/stdlib false positives before DB patterns
    if is_collection_method(callee) {
        return None;
    }

    // 5. Event emission
    if let Some(pattern) = match_event_emission(callee) {
        return Some(make_edge(pattern, 0.8));
    }

    // 6. Event handling
    if let Some(pattern) = match_event_handling(callee) {
        return Some(make_edge(pattern, 0.8));
    }

    // 7. Persistence (DB writes) — with DB-like receiver guard
    if let Some(pattern) = match_persistence(callee) {
        return Some(make_edge(pattern, confidence_for_db_pattern(callee)));
    }

    // 8. Database reads — with DB-like receiver guard
    if let Some(pattern) = match_db_read(callee) {
        return Some(make_edge(pattern, confidence_for_db_pattern(callee)));
    }

    None
}

/// Check if a callee is a standard library collection/utility method (not a DB operation).
fn is_collection_method(callee: &str) -> bool {
    if let Some(method) = callee.split('.').last() {
        match method {
            // Array/list methods
            "push" | "pop" | "shift" | "unshift" | "splice" | "slice" | "concat" | "join"
            | "reverse" | "sort" | "fill" | "copyWithin" | "flat" | "flatMap" | "map"
            | "filter" | "reduce" | "forEach" | "some" | "every" | "includes" | "indexOf"
            | "find" | "findIndex"
            // Python list/set
            | "append" | "extend" | "clear" | "copy" | "items" | "len"
            // Object/Map/Set methods
            | "keys" | "values" | "entries" | "toString" | "toLocaleString" | "has" | "add"
            // JSON/utility
            | "parse" | "stringify" | "assign" | "from" | "resolve" | "reject"
            | "now" | "round" | "floor" | "ceil" | "abs" | "min" | "max"
            | "charAt" | "charCodeAt" | "trim" | "split" | "replace" | "match"
            | "startsWith" | "endsWith" | "padStart" | "padEnd" | "repeat"
            | "toLowerCase" | "toUpperCase" => {
                return true;
            }
            _ => {}
        }
    }

    // Known non-DB full callee patterns (HashSet lookup)
    if non_db_callee_set().contains(callee) {
        return true;
    }

    false
}

fn match_persistence(callee: &str) -> Option<FlowPattern> {
    // Must be a method call (has a dot) with a DB-like receiver
    if let Some(method) = callee.rsplit('.').next() {
        if callee.contains('.')
            && db_write_suffix_set().contains(method)
            && has_db_like_receiver(callee)
        {
            return Some(FlowPattern::Persistence);
        }
    }

    // SQL keywords in the callee string (single-pass Aho-Corasick)
    let lower = callee.to_lowercase();
    if sql_write_automaton().is_match(&lower) {
        return Some(FlowPattern::Persistence);
    }

    None
}

fn match_db_read(callee: &str) -> Option<FlowPattern> {
    // Must be a method call with a DB-like receiver
    if let Some(method) = callee.rsplit('.').next() {
        if callee.contains('.')
            && db_read_suffix_set().contains(method)
            && has_db_like_receiver(callee)
        {
            return Some(FlowPattern::DatabaseRead);
        }
    }

    // SQL keywords (single-pass Aho-Corasick)
    let lower = callee.to_lowercase();
    if sql_read_automaton().is_match(&lower) {
        return Some(FlowPattern::DatabaseRead);
    }

    None
}

/// Check if the receiver (part before the last dot) looks like a database/ORM object.
///
/// Returns true for receivers containing DB-related keywords like "db", "repo",
/// "model", "store", "collection", "prisma", "session", etc.
/// Returns false for single-letter variables, known non-DB names, and stdlib objects.
fn has_db_like_receiver(callee: &str) -> bool {
    // Get the receiver (everything before the last method)
    let parts: Vec<&str> = callee.rsplitn(2, '.').collect();
    let receiver = if parts.len() == 2 {
        parts[1]
    } else {
        return false;
    };
    let lower = receiver.to_lowercase();

    // Skip single-letter variable names (too ambiguous)
    if receiver.len() <= 1 {
        return false;
    }

    // Skip known non-DB receivers (HashSet lookup)
    if non_db_receiver_set().contains(lower.as_str()) {
        return false;
    }

    // Positive signal: receiver contains DB-related keywords (Aho-Corasick single-pass)
    if db_keyword_automaton().is_match(&lower) {
        return true;
    }

    // Also match if it looks like a specific ORM method chain (e.g., prisma.user)
    let first_part = callee.split('.').next().unwrap_or("");
    let first_lower = first_part.to_lowercase();
    if db_keyword_automaton().is_match(&first_lower) {
        return true;
    }

    // For multi-part receivers like "prisma.user", check the first part
    if receiver.contains('.') {
        let root = receiver.split('.').next().unwrap_or("");
        let root_lower = root.to_lowercase();
        if db_keyword_automaton().is_match(&root_lower) {
            return true;
        }
    }

    false
}

fn match_event_emission(callee: &str) -> Option<FlowPattern> {
    if let Some(method) = callee.rsplit('.').next() {
        if callee.contains('.') && event_emit_suffix_set().contains(method) {
            return Some(FlowPattern::EventEmission);
        }
    }
    None
}

fn match_event_handling(callee: &str) -> Option<FlowPattern> {
    if let Some(method) = callee.rsplit('.').next() {
        if callee.contains('.') && event_handle_suffix_set().contains(method) {
            return Some(FlowPattern::EventHandling);
        }
    }
    None
}

fn match_config_read(callee: &str) -> Option<FlowPattern> {
    // Fast check: process.env and os.environ are the most common config patterns
    if callee.starts_with("process.env") || callee.starts_with("os.environ") {
        return Some(FlowPattern::ConfigRead);
    }

    for &pattern in CONFIG_PATTERNS {
        // Match exact, dot-prefix (member access), or bracket-prefix
        if callee == pattern
            || callee.starts_with(pattern)
                && callee
                    .as_bytes()
                    .get(pattern.len())
                    .map_or(true, |&b| b == b'.' || b == b'[')
        {
            return Some(FlowPattern::ConfigRead);
        }
    }

    None
}

fn match_http_call(callee: &str) -> Option<FlowPattern> {
    for &pattern in HTTP_CALL_PATTERNS {
        if callee == pattern
            || callee.starts_with(pattern) && callee.as_bytes().get(pattern.len()) == Some(&b'.')
        {
            return Some(FlowPattern::HttpCall);
        }
    }
    None
}

fn match_logging(callee: &str) -> Option<FlowPattern> {
    if log_pattern_set().contains(callee) {
        Some(FlowPattern::Logging)
    } else {
        None
    }
}

/// Assign confidence based on how specific the DB pattern is.
fn confidence_for_db_pattern(callee: &str) -> f64 {
    let lower = callee.to_lowercase();

    // ORM-specific method chains are high confidence (Aho-Corasick single-pass)
    if orm_automaton().is_match(&lower) {
        return 0.95;
    }

    // Methods with "db" or "repo" or "repository" in the receiver are high confidence
    let receiver = callee.split('.').next().unwrap_or("");
    let lower_receiver = receiver.to_lowercase();
    if confidence_receiver_automaton().is_match(&lower_receiver) {
        return 0.9;
    }

    // SQL keywords are high confidence (Aho-Corasick single-pass)
    if sql_write_automaton().is_match(&lower) || sql_read_automaton().is_match(&lower) {
        return 0.95;
    }

    // Generic methods like `.save()` on unknown receivers are medium confidence
    0.7
}

/// Trace call chains to a configurable depth, collecting all reachable symbols.
///
/// Given a starting symbol, follows call edges in the graph up to `max_depth` hops.
/// Returns the list of symbol IDs reachable from the start, in BFS order.
pub fn trace_call_chain(graph: &SymbolGraph, start: &str, max_depth: usize) -> Vec<String> {
    use std::collections::VecDeque;

    let start_idx = match graph.get_node(start) {
        Some(idx) => idx,
        None => return vec![],
    };

    let mut visited: HashSet<petgraph::graph::NodeIndex> = HashSet::new();
    let mut queue: VecDeque<(petgraph::graph::NodeIndex, usize)> = VecDeque::new();
    let mut result = Vec::new();

    visited.insert(start_idx);
    queue.push_back((start_idx, 0));

    while let Some((current, depth)) = queue.pop_front() {
        if depth > 0 {
            result.push(graph.graph[current].id.clone());
        }

        if depth >= max_depth {
            continue;
        }

        // Follow outgoing Calls edges
        for neighbor in graph
            .graph
            .neighbors_directed(current, petgraph::Direction::Outgoing)
        {
            if visited.insert(neighbor) {
                // Check if the edge is a Calls edge
                if let Some(edge) = graph.graph.find_edge(current, neighbor) {
                    if graph.graph[edge].edge_type == EdgeType::Calls {
                        queue.push_back((neighbor, depth + 1));
                    }
                }
            }
        }
    }

    result
}

// ---------------------------------------------------------------------------
// Full data flow tracing
// ---------------------------------------------------------------------------

/// Build data flow edges from extracted data flow info for a single file.
///
/// Connects variable assignments from calls to subsequent calls that use those
/// variables as arguments within the same function scope.
///
/// Example: in `function f() { const x = funcA(); funcB(x); }`,
/// produces `DataFlowEdge { producer: "funcA", consumer: "funcB", via: "x" }`.
pub fn build_data_flow_edges(
    info: &crate::ast::DataFlowInfo,
    file_path: &str,
) -> Vec<DataFlowEdge> {
    use std::collections::HashMap;

    let mut edges = Vec::new();

    // Group assignments by containing function for scope-aware matching.
    let mut assignments_by_scope: HashMap<Option<&str>, Vec<&crate::ast::VarCallAssignment>> =
        HashMap::new();
    for assignment in &info.assignments {
        let key = assignment.containing_function.as_deref();
        assignments_by_scope
            .entry(key)
            .or_default()
            .push(assignment);
    }

    // For each call, check if any argument matches a variable assigned from another call.
    for call in &info.calls_with_args {
        let scope_key = call.containing_function.as_deref();
        if let Some(scope_assignments) = assignments_by_scope.get(&scope_key) {
            for arg in &call.arguments {
                for assignment in scope_assignments {
                    if assignment.variable == *arg && assignment.callee != call.callee {
                        let containing = match &call.containing_function {
                            Some(f) => format!("{}::{}", file_path, f),
                            None => file_path.to_string(),
                        };
                        edges.push(DataFlowEdge {
                            producer: assignment.callee.clone(),
                            consumer: call.callee.clone(),
                            via: arg.clone(),
                            containing_function: containing,
                            file: file_path.to_string(),
                            line: call.line,
                        });
                    }
                }
            }
        }
    }

    edges
}

/// Trace data flow across all files, producing edges that show how data moves
/// through variable assignments and function calls.
///
/// Requires source code for each file (re-parses with tree-sitter for finer-grained
/// extraction of variable assignments and call arguments).
pub fn trace_data_flow(files_with_source: &[(&str, &str)]) -> Vec<DataFlowEdge> {
    let mut all_edges = Vec::new();

    for &(path, source) in files_with_source {
        match crate::ast::extract_data_flow_info(path, source) {
            Ok(info) => {
                let edges = build_data_flow_edges(&info, path);
                all_edges.extend(edges);
            }
            Err(_) => continue,
        }
    }

    all_edges
}

// ---------------------------------------------------------------------------
// IR-based public API
// ---------------------------------------------------------------------------

/// Analyze data flow patterns from IR files (declarative query engine / IR path).
///
/// Delegates to the existing heuristic analysis via ParsedFile conversion.
/// The heuristic pattern matching operates on the same call site data available
/// in both representations.
pub fn analyze_data_flow_ir(files: &[IrFile], config: &FlowConfig) -> FlowAnalysis {
    let parsed: Vec<ParsedFile> = files.iter().map(|f| f.to_parsed_file()).collect();
    analyze_data_flow(&parsed, config)
}

/// Detect frameworks from IR files' import patterns.
pub fn detect_frameworks_ir(files: &[IrFile]) -> Vec<String> {
    let parsed: Vec<ParsedFile> = files.iter().map(|f| f.to_parsed_file()).collect();
    detect_frameworks(&parsed)
}

/// Build data flow edges directly from an IR file, without re-parsing source code.
///
/// This is the key improvement over the ParsedFile path: `IrFile` already contains
/// `assignments` (variable = call()) and `call_expressions` with arguments, so we
/// can trace producer → consumer edges without needing the original source text.
///
/// Example: given `const x = funcA(); funcB(x);` in the IR:
/// - `assignments` contains: pattern=x, value=Call(funcA), scope=f
/// - `call_expressions` contains: callee=funcB, args=["x"], scope=f
/// - Produces: `DataFlowEdge { producer: "funcA", consumer: "funcB", via: "x" }`
pub fn build_data_flow_edges_from_ir(file: &IrFile) -> Vec<DataFlowEdge> {
    let mut edges = Vec::new();

    // Group assignments by containing function for scope-aware matching.
    // Each entry: (variable_name, callee_name, line)
    let mut assignments_by_scope: HashMap<Option<&str>, Vec<(&str, &str, usize)>> = HashMap::new();

    for assignment in &file.assignments {
        if let (Some(var), Some(callee)) = (
            assignment.pattern.as_identifier(),
            assignment.value.callee_name(),
        ) {
            let scope = assignment.containing_function.as_deref();
            assignments_by_scope.entry(scope).or_default().push((
                var,
                callee,
                assignment.span.start_line,
            ));
        }
    }

    // For each call with arguments, check if any argument matches a variable
    // assigned from another call within the same scope.
    for call in &file.call_expressions {
        if call.arguments.is_empty() {
            continue;
        }
        let scope = call.containing_function.as_deref();
        if let Some(scope_assignments) = assignments_by_scope.get(&scope) {
            for arg in &call.arguments {
                for &(var, producer_callee, _line) in scope_assignments {
                    if var == arg.as_str() && producer_callee != call.callee {
                        let containing = match &call.containing_function {
                            Some(f) => format!("{}::{}", file.path, f),
                            None => file.path.to_string(),
                        };
                        edges.push(DataFlowEdge {
                            producer: producer_callee.to_string(),
                            consumer: call.callee.clone(),
                            via: arg.clone(),
                            containing_function: containing,
                            file: file.path.clone(),
                            line: call.span.start_line,
                        });
                    }
                }
            }
        }
    }

    edges
}

/// Trace data flow across all IR files, producing edges that show how data moves
/// through variable assignments and function calls.
///
/// Unlike `trace_data_flow` which requires source code and re-parses with tree-sitter,
/// this version works directly from the IR which already has assignments and call arguments.
pub fn trace_data_flow_ir(files: &[IrFile]) -> Vec<DataFlowEdge> {
    let mut all_edges = Vec::new();
    for file in files {
        let edges = build_data_flow_edges_from_ir(file);
        all_edges.extend(edges);
    }
    all_edges
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
