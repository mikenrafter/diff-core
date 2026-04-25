#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr
)]
//! E2E integration tests for systems languages: Swift and C/C++.

mod helpers;

use helpers::graph_assertions::{
    assert_all_files_accounted, assert_json_roundtrip, assert_language_detected,
    assert_valid_json_schema, assert_valid_mermaid, assert_valid_scores,
};
use helpers::repo_builder::{run_pipeline, RepoBuilder};

#[test]
fn test_e2e_swift_vapor_api() {
    let rb = RepoBuilder::new();

    // Initial commit
    rb.write_file(
        "Package.swift",
        "// swift-tools-version:5.9\nimport PackageDescription\n",
    );
    rb.commit("Initial commit: Package.swift");
    rb.create_branch("main");

    // Feature branch: add Vapor REST API
    rb.create_branch("feature/swift-api");
    rb.checkout("feature/swift-api");

    rb.write_file(
        "Sources/App/Controllers/UserController.swift",
        r#"import Vapor
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
"#,
    );

    rb.write_file(
        "Sources/App/Services/UserService.swift",
        r#"import Foundation
import Vapor

class UserService {
    let repository: UserRepository

    init(repository: UserRepository) {
        self.repository = repository
    }

    func findAll() -> [User] {
        let users = repository.findAll()
        return users
    }

    func findById(id: UUID) -> User? {
        let user = repository.findById(id: id)
        return user
    }

    func create(name: String) -> User {
        let user = repository.save(name: name)
        return user
    }
}
"#,
    );

    rb.write_file(
        "Sources/App/Repositories/UserRepository.swift",
        r#"import Foundation
import Fluent

class UserRepository {
    let db: Database

    init(db: Database) {
        self.db = db
    }

    func findAll() -> [User] {
        let results = db.query(User.self)
        return results
    }

    func findById(id: UUID) -> User? {
        let result = db.find(User.self, id: id)
        return result
    }

    func save(name: String) -> User {
        let user = User(name: name)
        db.save(user)
        return user
    }
}
"#,
    );

    rb.write_file(
        "Sources/App/Models/User.swift",
        r#"import Foundation
import Fluent

final class User: Model, Content {
    static let schema = "users"

    var id: UUID?
    var name: String

    init() {}

    init(id: UUID? = nil, name: String) {
        self.id = id
        self.name = name
    }
}
"#,
    );

    rb.write_file(
        "Sources/App/configure.swift",
        r#"import Vapor
import Fluent

func configure(app: Application) throws {
    let controller = UserController()
    try app.register(collection: controller)
}
"#,
    );

    rb.commit("Add Vapor REST API with controller-service-repo");

    let result = run_pipeline(rb.path(), "main", "feature/swift-api");

    // Verify basic output shape
    assert_valid_json_schema(&result);
    assert_valid_scores(&result);

    // Verify Swift files were detected
    assert_language_detected(&result, "swift");

    // Verify all changed files are accounted for
    assert_all_files_accounted(&result);

    // Verify there are flow groups
    assert!(
        !result.groups.is_empty(),
        "should produce at least one flow group"
    );

    // Verify entrypoint detection (Vapor route handlers)
    let has_entrypoint = result.groups.iter().any(|g| g.entrypoint.is_some());
    assert!(
        has_entrypoint,
        "should detect at least one entrypoint (Vapor routes)"
    );

    // Verify Vapor framework detection
    let has_vapor = result
        .summary
        .frameworks_detected
        .iter()
        .any(|f| f.contains("Vapor"));
    assert!(
        has_vapor,
        "should detect Vapor framework; detected: {:?}",
        result.summary.frameworks_detected
    );

    // Verify Mermaid graph is valid
    assert_valid_mermaid(&result);

    // Verify JSON roundtrip
    assert_json_roundtrip(&result);
}

/// Test: Swift test file detection.
///
/// Verifies that *Tests.swift and *Test.swift files are detected as test entrypoints.
#[test]
fn test_e2e_swift_test_file_detection() {
    let rb = RepoBuilder::new();

    rb.write_file("Package.swift", "// swift-tools-version:5.9\n");
    rb.commit("Initial commit");
    rb.create_branch("main");

    rb.create_branch("feature/tests");
    rb.checkout("feature/tests");

    rb.write_file(
        "Sources/App/Services/UserService.swift",
        r#"import Foundation

class UserService {
    func greet(name: String) -> String {
        return "Hello, \(name)"
    }
}
"#,
    );

    rb.write_file(
        "Tests/AppTests/UserServiceTests.swift",
        r#"import XCTest
import Foundation

final class UserServiceTests: XCTestCase {
    func testGreet() {
        let service = UserService()
        let result = service.greet(name: "Alice")
    }

    func testGreetEmpty() {
        let service = UserService()
        let result = service.greet(name: "")
    }
}
"#,
    );

    rb.commit("Add UserService with XCTest tests");

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
        "should detect *Tests.swift as test file entrypoint"
    );

    // Verify Swift language detected
    assert_language_detected(&result, "swift");

    // Verify XCTest framework detected
    let has_xctest = result
        .summary
        .frameworks_detected
        .iter()
        .any(|f| f.contains("XCTest"));
    assert!(
        has_xctest,
        "should detect XCTest framework; detected: {:?}",
        result.summary.frameworks_detected
    );
}

// ─── C++ Integration Tests ──────────────────────────────────────────────

/// Test: Synthetic C++ REST API with handler→service→repo pattern.
///
/// Verifies: C++ language detection, #include import resolution,
/// class/function extraction, call graph across files, framework detection,
/// entrypoint detection (main), semantic grouping.
#[test]
fn test_e2e_cpp_http_server() {
    let rb = RepoBuilder::new();

    // Initial commit
    rb.write_file(
        "CMakeLists.txt",
        "cmake_minimum_required(VERSION 3.14)\nproject(myapp)\n",
    );
    rb.commit("Initial commit: CMakeLists.txt");
    rb.create_branch("main");

    // Feature branch: add HTTP server
    rb.create_branch("feature/cpp-api");
    rb.checkout("feature/cpp-api");

    rb.write_file(
        "src/handlers/user_handler.cpp",
        r#"#include <iostream>
#include "user_handler.hpp"
#include "../services/user_service.hpp"

void UserHandler::handle_list(const Request& req) {
    auto users = service_.list_users();
    send_response(req, users);
}

void UserHandler::handle_create(const Request& req) {
    auto name = parse_body(req);
    auto user = service_.create_user(name);
    send_response(req, user);
}
"#,
    );

    rb.write_file(
        "src/services/user_service.hpp",
        r#"#pragma once
#include <string>
#include <vector>
#include "../models/user.hpp"
#include "../repositories/user_repository.hpp"

class UserService {
public:
    std::vector<User> list_users();
    User create_user(const std::string& name);
private:
    UserRepository repo_;
};
"#,
    );

    rb.write_file(
        "src/services/user_service.cpp",
        r#"#include "user_service.hpp"

std::vector<User> UserService::list_users() {
    auto result = repo_.find_all();
    return result;
}

User UserService::create_user(const std::string& name) {
    auto user = repo_.save(name);
    return user;
}
"#,
    );

    rb.write_file(
        "src/repositories/user_repository.hpp",
        r#"#pragma once
#include <string>
#include <vector>
#include "../models/user.hpp"

class UserRepository {
public:
    std::vector<User> find_all();
    User save(const std::string& name);
};
"#,
    );

    rb.write_file(
        "src/repositories/user_repository.cpp",
        r#"#include "user_repository.hpp"
#include <sqlite3.h>

std::vector<User> UserRepository::find_all() {
    auto db = open_db();
    auto results = query(db, "SELECT * FROM users");
    return results;
}

User UserRepository::save(const std::string& name) {
    auto db = open_db();
    auto result = execute(db, "INSERT INTO users (name) VALUES (?)", name);
    return result;
}
"#,
    );

    rb.write_file(
        "src/models/user.hpp",
        r#"#pragma once
#include <string>

struct User {
    int id;
    std::string name;
};
"#,
    );

    rb.write_file(
        "src/main.cpp",
        r#"#include <iostream>
#include "handlers/user_handler.hpp"

int main() {
    auto handler = UserHandler();
    auto server = init_server();
    register_routes(server, handler);
    start_server(server);
    return 0;
}
"#,
    );

    rb.commit("Add C++ HTTP server with handler-service-repo");

    let result = run_pipeline(rb.path(), "main", "feature/cpp-api");

    // Verify basic output shape
    assert_valid_json_schema(&result);
    assert_valid_scores(&result);

    // Verify C++ files were detected
    assert_language_detected(&result, "cpp");

    // Verify all changed files are accounted for
    assert_all_files_accounted(&result);

    // Verify there are flow groups
    assert!(
        !result.groups.is_empty(),
        "should produce at least one flow group"
    );

    // Verify entrypoint detection (main function)
    let has_entrypoint = result.groups.iter().any(|g| g.entrypoint.is_some());
    assert!(
        has_entrypoint,
        "should detect at least one entrypoint (main)"
    );

    // Verify C++ STL framework detection
    let has_stl = result
        .summary
        .frameworks_detected
        .iter()
        .any(|f| f.contains("STL") || f.contains("C++"));
    assert!(
        has_stl,
        "should detect C++ STL; detected: {:?}",
        result.summary.frameworks_detected
    );

    // Verify Mermaid graph is valid
    assert_valid_mermaid(&result);

    // Verify JSON roundtrip
    assert_json_roundtrip(&result);
}

/// Test: C test file detection.
///
/// Verifies that *_test.c and files in test/ directories are detected as test entrypoints.
#[test]
fn test_e2e_c_test_file_detection() {
    let rb = RepoBuilder::new();

    rb.write_file("Makefile", "all: build\n");
    rb.commit("Initial commit");
    rb.create_branch("main");

    rb.create_branch("feature/tests");
    rb.checkout("feature/tests");

    rb.write_file(
        "src/math.c",
        r#"#include "math.h"

int add(int a, int b) {
    return a + b;
}

int multiply(int a, int b) {
    return a * b;
}
"#,
    );

    rb.write_file(
        "tests/math_test.c",
        r#"#include <stdio.h>
#include "../src/math.h"

void test_add() {
    int result = add(2, 3);
    printf("test_add: %s\n", result == 5 ? "PASS" : "FAIL");
}

void test_multiply() {
    int result = multiply(3, 4);
    printf("test_multiply: %s\n", result == 12 ? "PASS" : "FAIL");
}

int main() {
    test_add();
    test_multiply();
    return 0;
}
"#,
    );

    rb.commit("Add math module with tests");

    let result = run_pipeline(rb.path(), "main", "feature/tests");

    assert_valid_json_schema(&result);
    assert_all_files_accounted(&result);

    // Verify C language detected
    assert_language_detected(&result, "c");

    // Verify test file detected as entrypoint
    let has_test_entrypoint = result.groups.iter().any(|g| {
        g.entrypoint.as_ref().map_or(false, |e| {
            e.entrypoint_type == diffcore_core::types::EntrypointType::TestFile
                || e.entrypoint_type == diffcore_core::types::EntrypointType::CliCommand
        })
    });
    assert!(
        has_test_entrypoint,
        "should detect test file entrypoint; groups: {:?}",
        result
            .groups
            .iter()
            .map(|g| (&g.name, &g.entrypoint))
            .collect::<Vec<_>>()
    );
}

