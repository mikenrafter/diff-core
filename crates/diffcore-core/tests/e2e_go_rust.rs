#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr
)]
//! E2E integration tests for Go and Rust language support.

mod helpers;

use helpers::graph_assertions::{
    assert_all_files_accounted, assert_language_detected, assert_valid_json_schema,
    assert_valid_mermaid, assert_valid_scores,
};
use helpers::repo_builder::{run_pipeline, RepoBuilder};

// ─── Go Integration Tests ────────────────────────────────────────────────

/// Test: Synthetic Go HTTP API with handler → service → repo pattern.
///
/// Creates a Go app with Gin framework:
///   main.go → handlers/user.go → services/user.go → repositories/user.go
///
/// Verifies:
///   - Go language detection
///   - Import extraction from Go files
///   - Function/struct/interface definitions
///   - Call site detection
///   - HTTP route entrypoint detection
///   - Framework detection (Gin)
///   - Pipeline produces valid groups and JSON output
#[test]
fn test_e2e_go_http_api() {
    let rb = RepoBuilder::new();

    // Initial commit
    rb.write_file("go.mod", "module github.com/example/api\n\ngo 1.21\n");
    rb.commit("Initial commit: go.mod");
    rb.create_branch("main");

    // Feature branch: add Go API
    rb.create_branch("feature/go-api");
    rb.checkout("feature/go-api");

    rb.write_file(
        "cmd/server/main.go",
        r#"
package main

import (
    "github.com/gin-gonic/gin"
    "github.com/example/api/handlers"
)

func main() {
    r := gin.Default()
    handlers.RegisterRoutes(r)
    r.Run(":8080")
}
"#,
    );

    rb.write_file(
        "handlers/user.go",
        r#"
package handlers

import (
    "github.com/gin-gonic/gin"
    "github.com/example/api/services"
)

func RegisterRoutes(r *gin.Engine) {
    r.GET("/users/:id", GetUser)
    r.POST("/users", CreateUser)
}

func GetUser(c *gin.Context) {
    id := c.Param("id")
    user := services.FindUser(id)
    c.JSON(200, user)
}

func CreateUser(c *gin.Context) {
    data := services.ParseInput(c)
    user := services.CreateUser(data)
    c.JSON(201, user)
}
"#,
    );

    rb.write_file(
        "services/user.go",
        r#"
package services

import (
    "github.com/gin-gonic/gin"
    "github.com/example/api/repositories"
)

type UserInput struct {
    Name  string
    Email string
}

func ParseInput(c *gin.Context) UserInput {
    var input UserInput
    c.BindJSON(&input)
    return input
}

func FindUser(id string) *repositories.User {
    return repositories.GetByID(id)
}

func CreateUser(data UserInput) *repositories.User {
    user := repositories.User{
        Name:  data.Name,
        Email: data.Email,
    }
    return repositories.Insert(&user)
}
"#,
    );

    rb.write_file(
        "repositories/user.go",
        r#"
package repositories

type User struct {
    ID    string
    Name  string
    Email string
}

var users = make(map[string]*User)

func GetByID(id string) *User {
    return users[id]
}

func Insert(user *User) *User {
    user.ID = "generated-id"
    users[user.ID] = user
    return user
}
"#,
    );

    rb.commit("Add Go HTTP API with handler-service-repo");

    let result = run_pipeline(rb.path(), "main", "feature/go-api");

    // Verify basic output shape
    assert_valid_json_schema(&result);
    assert_valid_scores(&result);

    // Verify Go files were detected
    assert_language_detected(&result, "go");

    // Verify all changed files are accounted for
    assert_all_files_accounted(&result);

    // Verify there are flow groups
    assert!(
        !result.groups.is_empty(),
        "should produce at least one flow group"
    );

    // Verify CLI entrypoint detection (func main)
    let has_cli_ep = result.groups.iter().any(|g| {
        g.entrypoint.as_ref().map_or(false, |ep| {
            ep.entrypoint_type == diffcore_core::types::EntrypointType::CliCommand
        })
    });
    assert!(has_cli_ep, "should detect func main() as CLI entrypoint");

    // Verify Mermaid graph is valid
    assert_valid_mermaid(&result);
}

/// Test: Go test file detection.
///
/// Verifies that `_test.go` files are detected as test file entrypoints,
/// and that Go Test* functions are recognized as test symbols.
#[test]
fn test_e2e_go_test_file_detection() {
    let rb = RepoBuilder::new();

    rb.write_file("go.mod", "module example.com/app\n\ngo 1.21\n");
    rb.commit("Initial commit");
    rb.create_branch("main");

    rb.create_branch("feature/tests");
    rb.checkout("feature/tests");

    rb.write_file(
        "handlers/user.go",
        r#"
package handlers

func GetUser(id string) string {
    return "user-" + id
}
"#,
    );

    rb.write_file(
        "handlers/user_test.go",
        r#"
package handlers

import "testing"

func TestGetUser(t *testing.T) {
    result := GetUser("123")
    if result != "user-123" {
        t.Errorf("unexpected: %s", result)
    }
}

func BenchmarkGetUser(b *testing.B) {
    for i := 0; i < b.N; i++ {
        GetUser("123")
    }
}
"#,
    );

    rb.commit("Add user handler with tests");

    let result = run_pipeline(rb.path(), "main", "feature/tests");

    // Verify test file entrypoint detection
    let test_eps: Vec<_> = result
        .groups
        .iter()
        .flat_map(|g| g.entrypoint.as_ref())
        .filter(|ep| ep.entrypoint_type == diffcore_core::types::EntrypointType::TestFile)
        .collect();
    assert!(
        !test_eps.is_empty(),
        "should detect _test.go as test file entrypoint"
    );
}

// =====================================================================
// Rust language integration tests
// =====================================================================

/// Test: Synthetic Rust axum API with handler→service→repo pattern.
///
/// Creates a 5-file Rust HTTP API using axum and verifies the full pipeline:
/// language detection, import extraction, definition extraction, call sites,
/// entrypoint detection, framework detection, grouping, and JSON output.
#[test]
fn test_e2e_rust_axum_api() {
    let rb = RepoBuilder::new();

    // Initial commit
    rb.write_file(
        "Cargo.toml",
        r#"[package]
name = "my-api"
version = "0.1.0"
edition = "2021"

[dependencies]
axum = "0.7"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
sqlx = "0.7"
"#,
    );
    rb.commit("Initial commit: Cargo.toml");
    rb.create_branch("main");

    // Feature branch: add Rust API
    rb.create_branch("feature/rust-api");
    rb.checkout("feature/rust-api");

    rb.write_file(
        "src/main.rs",
        r#"
use axum::{Router, routing::get, routing::post};
use crate::handlers;

mod handlers;
mod services;
mod repositories;
mod models;

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/users/:id", get(handlers::get_user))
        .route("/users", post(handlers::create_user));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
"#,
    );

    rb.write_file(
        "src/handlers.rs",
        r#"
use axum::{extract::Path, Json};
use crate::models::User;
use crate::services;

pub async fn get_user(Path(id): Path<u64>) -> Json<User> {
    let user = services::find_user(id).await;
    Json(user)
}

pub async fn create_user(Json(input): Json<CreateUserInput>) -> Json<User> {
    let user = services::create_user(input.name, input.email).await;
    Json(user)
}

#[derive(serde::Deserialize)]
pub struct CreateUserInput {
    pub name: String,
    pub email: String,
}
"#,
    );

    rb.write_file(
        "src/services.rs",
        r#"
use crate::models::User;
use crate::repositories;

pub async fn find_user(id: u64) -> User {
    repositories::get_by_id(id).await
}

pub async fn create_user(name: String, email: String) -> User {
    let user = User {
        id: 0,
        name,
        email,
    };
    repositories::insert(user).await
}
"#,
    );

    rb.write_file(
        "src/repositories.rs",
        r#"
use crate::models::User;
use sqlx::PgPool;

pub async fn get_by_id(id: u64) -> User {
    User {
        id,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    }
}

pub async fn insert(user: User) -> User {
    User {
        id: 1,
        ..user
    }
}
"#,
    );

    rb.write_file(
        "src/models.rs",
        r#"
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: u64,
    pub name: String,
    pub email: String,
}
"#,
    );

    rb.commit("Add Rust axum HTTP API with handler-service-repo");

    let result = run_pipeline(rb.path(), "main", "feature/rust-api");

    // Verify basic output shape
    assert_valid_json_schema(&result);
    assert_valid_scores(&result);

    // Verify Rust files were detected
    assert_language_detected(&result, "rust");

    // Verify all changed files are accounted for
    assert_all_files_accounted(&result);

    // Verify there are flow groups
    assert!(
        !result.groups.is_empty(),
        "should produce at least one flow group"
    );

    // Verify entrypoint detection (fn main or HTTP routes)
    let has_entrypoint = result.groups.iter().any(|g| g.entrypoint.is_some());
    assert!(has_entrypoint, "should detect at least one entrypoint");

    // Verify HTTP route detection (axum Router patterns)
    let has_http_ep = result.groups.iter().any(|g| {
        g.entrypoint.as_ref().map_or(false, |ep| {
            ep.entrypoint_type == diffcore_core::types::EntrypointType::HttpRoute
        })
    });
    assert!(has_http_ep, "should detect axum HTTP route entrypoints");

    // Verify framework detection (Axum)
    let has_axum = result
        .summary
        .frameworks_detected
        .iter()
        .any(|f| f.contains("Axum") || f.contains("axum"));
    assert!(
        has_axum,
        "should detect Axum framework; detected: {:?}",
        result.summary.frameworks_detected
    );

    // Verify Mermaid graph is valid
    assert_valid_mermaid(&result);
}

/// Test: Rust test file detection.
///
/// Verifies that `_test.rs` files and functions with test_ prefix are detected.
#[test]
fn test_e2e_rust_test_file_detection() {
    let rb = RepoBuilder::new();

    rb.write_file(
        "Cargo.toml",
        r#"[package]
name = "my-app"
version = "0.1.0"
edition = "2021"
"#,
    );
    rb.commit("Initial commit");
    rb.create_branch("main");

    rb.create_branch("feature/tests");
    rb.checkout("feature/tests");

    rb.write_file(
        "src/lib.rs",
        r#"
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}
"#,
    );

    rb.write_file(
        "src/lib_test.rs",
        r#"
use crate::add;

fn test_add() {
    assert_eq!(add(2, 3), 5);
}

fn test_add_negative() {
    assert_eq!(add(-1, 1), 0);
}
"#,
    );

    rb.commit("Add lib with tests");

    let result = run_pipeline(rb.path(), "main", "feature/tests");

    // Verify test file detection
    let test_eps: Vec<_> = result
        .groups
        .iter()
        .flat_map(|g| g.entrypoint.as_ref())
        .filter(|ep| ep.entrypoint_type == diffcore_core::types::EntrypointType::TestFile)
        .collect();
    assert!(
        !test_eps.is_empty(),
        "should detect _test.rs as test file entrypoint"
    );
}
