#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr
)]
//! E2E integration tests for JVM language support: Java, Kotlin, and Scala.

mod helpers;

use helpers::graph_assertions::{
    assert_all_files_accounted, assert_json_roundtrip, assert_language_detected,
    assert_valid_json_schema, assert_valid_mermaid, assert_valid_scores,
};
use helpers::repo_builder::{run_pipeline, RepoBuilder};

// ---------------------------------------------------------------------------
// Java integration tests (Phase 11.2)
// ---------------------------------------------------------------------------

/// Test: Java Spring Boot REST API with controller → service → repository pattern.
///
/// Verifies full pipeline: language detection, file accounting, flow groups,
/// entrypoint detection, framework detection, and Mermaid graph.
#[test]
fn test_e2e_java_spring_boot_api() {
    let rb = RepoBuilder::new();

    // Initial commit
    rb.write_file(
        "pom.xml",
        r#"<project>
    <modelVersion>4.0.0</modelVersion>
    <groupId>com.example</groupId>
    <artifactId>demo</artifactId>
    <version>0.0.1-SNAPSHOT</version>
    <dependencies>
        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-web</artifactId>
        </dependency>
    </dependencies>
</project>
"#,
    );
    rb.commit("Initial commit: pom.xml");
    rb.create_branch("main");

    // Feature branch: add Spring Boot API
    rb.create_branch("feature/java-api");
    rb.checkout("feature/java-api");

    rb.write_file(
        "src/main/java/com/example/demo/DemoApplication.java",
        r#"
package com.example.demo;

import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;

@SpringBootApplication
public class DemoApplication {
    public static void main(String[] args) {
        SpringApplication.run(DemoApplication.class, args);
    }
}
"#,
    );

    rb.write_file(
        "src/main/java/com/example/demo/controller/UserController.java",
        r#"
package com.example.demo.controller;

import java.util.List;
import org.springframework.web.bind.annotation.RestController;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import com.example.demo.model.User;
import com.example.demo.service.UserService;

@RestController
public class UserController {

    private final UserService userService;

    public UserController(UserService userService) {
        this.userService = userService;
    }

    @GetMapping("/users")
    public List<User> getUsers() {
        return userService.findAll();
    }

    @PostMapping("/users")
    public User createUser(@RequestBody User user) {
        return userService.save(user);
    }
}
"#,
    );

    rb.write_file(
        "src/main/java/com/example/demo/service/UserService.java",
        r#"
package com.example.demo.service;

import java.util.List;
import com.example.demo.model.User;
import com.example.demo.repository.UserRepository;

public class UserService {

    private final UserRepository userRepository;

    public UserService(UserRepository userRepository) {
        this.userRepository = userRepository;
    }

    public List<User> findAll() {
        return userRepository.findAll();
    }

    public User save(User user) {
        return userRepository.save(user);
    }
}
"#,
    );

    rb.write_file(
        "src/main/java/com/example/demo/repository/UserRepository.java",
        r#"
package com.example.demo.repository;

import java.util.List;
import java.util.ArrayList;
import com.example.demo.model.User;

public class UserRepository {

    private final List<User> users = new ArrayList<>();

    public List<User> findAll() {
        return users;
    }

    public User save(User user) {
        users.add(user);
        return user;
    }
}
"#,
    );

    rb.write_file(
        "src/main/java/com/example/demo/model/User.java",
        r#"
package com.example.demo.model;

public class User {
    private Long id;
    private String name;
    private String email;

    public User() {}

    public User(String name, String email) {
        this.name = name;
        this.email = email;
    }

    public Long getId() { return id; }
    public void setId(Long id) { this.id = id; }
    public String getName() { return name; }
    public void setName(String name) { this.name = name; }
    public String getEmail() { return email; }
    public void setEmail(String email) { this.email = email; }
}
"#,
    );

    rb.commit("Add Spring Boot REST API with controller-service-repo");

    let result = run_pipeline(rb.path(), "main", "feature/java-api");

    // Verify basic output shape
    assert_valid_json_schema(&result);
    assert_valid_scores(&result);

    // Verify Java files were detected
    assert_language_detected(&result, "java");

    // Verify all changed files are accounted for
    assert_all_files_accounted(&result);

    // Verify there are flow groups
    assert!(
        !result.groups.is_empty(),
        "should produce at least one flow group"
    );

    // Verify entrypoint detection (main or HTTP routes)
    let has_entrypoint = result.groups.iter().any(|g| g.entrypoint.is_some());
    assert!(has_entrypoint, "should detect at least one entrypoint");

    // Verify Spring Boot framework detection
    let has_spring = result
        .summary
        .frameworks_detected
        .iter()
        .any(|f| f.contains("Spring"));
    assert!(
        has_spring,
        "should detect Spring Boot framework; detected: {:?}",
        result.summary.frameworks_detected
    );

    // Verify Mermaid graph is valid
    assert_valid_mermaid(&result);
}

/// Test: Java test file detection.
///
/// Verifies that *Test.java files and @Test annotated methods are detected as test entrypoints.
#[test]
fn test_e2e_java_test_file_detection() {
    let rb = RepoBuilder::new();

    rb.write_file(
        "pom.xml",
        r#"<project>
    <modelVersion>4.0.0</modelVersion>
    <groupId>com.example</groupId>
    <artifactId>demo</artifactId>
    <version>0.0.1-SNAPSHOT</version>
</project>
"#,
    );
    rb.commit("Initial commit");
    rb.create_branch("main");

    rb.create_branch("feature/tests");
    rb.checkout("feature/tests");

    rb.write_file(
        "src/main/java/com/example/demo/UserService.java",
        r#"
package com.example.demo;

public class UserService {
    public String greet(String name) {
        return "Hello, " + name;
    }
}
"#,
    );

    rb.write_file(
        "src/test/java/com/example/demo/UserServiceTest.java",
        r#"
package com.example.demo;

import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.assertEquals;

public class UserServiceTest {

    @Test
    public void testGreet() {
        UserService svc = new UserService();
        assertEquals("Hello, Alice", svc.greet("Alice"));
    }

    @Test
    public void testGreetEmpty() {
        UserService svc = new UserService();
        assertEquals("Hello, ", svc.greet(""));
    }
}
"#,
    );

    rb.commit("Add UserService and tests");

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
        "should detect *Test.java as test file entrypoint"
    );
}

/// Test: Kotlin Ktor REST API with handler→service→repo pattern.
///
/// Verifies that the pipeline can:
/// - Parse Kotlin source files via tree-sitter
/// - Extract import statements (regular, aliased, wildcard)
/// - Detect fun, class, object, val/var definitions
/// - Detect Ktor route handler entrypoints
/// - Detect Ktor framework from imports
/// - Cluster files into meaningful flow groups
#[test]
fn test_e2e_kotlin_ktor_api() {
    let rb = RepoBuilder::new();

    // Initial commit
    rb.write_file("build.gradle.kts", "plugins {\n    kotlin(\"jvm\")\n}\n");
    rb.commit("Initial commit: build.gradle.kts");
    rb.create_branch("main");

    // Feature branch: add Ktor REST API
    rb.create_branch("feature/kotlin-api");
    rb.checkout("feature/kotlin-api");

    rb.write_file(
        "src/main/kotlin/routes/UserRoutes.kt",
        r#"import io.ktor.server.routing.Route
import io.ktor.server.routing.get
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

    get("/users/{id}") {
        val user = userService.findById(call)
        call.respond(user)
    }
}
"#,
    );

    rb.write_file(
        "src/main/kotlin/services/UserService.kt",
        r#"import com.example.repositories.UserRepository
import com.example.models.User

class UserService(private val repository: UserRepository) {
    fun findAll(): List<User> {
        val users = repository.findAll()
        return users
    }

    fun findById(id: String): User {
        val user = repository.findById(id)
        return user
    }

    fun create(data: Map<String, String>): User {
        val user = repository.save(data)
        return user
    }
}
"#,
    );

    rb.write_file(
        "src/main/kotlin/repositories/UserRepository.kt",
        r#"import org.jetbrains.exposed.sql.Database
import com.example.models.User

class UserRepository(private val db: Database) {
    fun findAll(): List<User> {
        val results = db.query("SELECT * FROM users")
        return results
    }

    fun findById(id: String): User {
        val result = db.query("SELECT * FROM users WHERE id = ?")
        return result
    }

    fun save(data: Map<String, String>): User {
        val result = db.execute("INSERT INTO users ...")
        return result
    }
}
"#,
    );

    rb.write_file(
        "src/main/kotlin/models/User.kt",
        r#"import kotlinx.serialization.Serializable

@Serializable
data class User(
    val id: String,
    val name: String,
    val email: String
)
"#,
    );

    rb.write_file(
        "src/main/kotlin/Application.kt",
        r#"import io.ktor.server.engine.embeddedServer
import io.ktor.server.netty.Netty
import com.example.routes.userRoutes
import com.example.services.UserService
import com.example.repositories.UserRepository

fun main() {
    val repo = UserRepository()
    val service = UserService(repo)
    embeddedServer(Netty, port = 8080) {
        userRoutes(service)
    }
}
"#,
    );

    rb.commit("Add Ktor REST API with routes-service-repo");

    let result = run_pipeline(rb.path(), "main", "feature/kotlin-api");

    // Verify basic output shape
    assert_valid_json_schema(&result);
    assert_valid_scores(&result);

    // Verify Kotlin files were detected
    assert_language_detected(&result, "kotlin");

    // Verify all changed files are accounted for
    assert_all_files_accounted(&result);

    // Verify there are flow groups
    assert!(
        !result.groups.is_empty(),
        "should produce at least one flow group"
    );

    // Verify entrypoint detection (Ktor route handlers or main)
    let has_entrypoint = result.groups.iter().any(|g| g.entrypoint.is_some());
    assert!(
        has_entrypoint,
        "should detect at least one entrypoint (Ktor routes or main)"
    );

    // Verify Ktor framework detection
    let has_ktor = result
        .summary
        .frameworks_detected
        .iter()
        .any(|f| f.contains("Ktor"));
    assert!(
        has_ktor,
        "should detect Ktor framework; detected: {:?}",
        result.summary.frameworks_detected
    );

    // Verify Mermaid graph is valid
    assert_valid_mermaid(&result);

    // Verify JSON roundtrip
    assert_json_roundtrip(&result);
}

/// Test: Kotlin test file detection.
///
/// Verifies that *Test.kt files are detected as test entrypoints.
#[test]
fn test_e2e_kotlin_test_file_detection() {
    let rb = RepoBuilder::new();

    rb.write_file("build.gradle.kts", "plugins {\n    kotlin(\"jvm\")\n}\n");
    rb.commit("Initial commit");
    rb.create_branch("main");

    rb.create_branch("feature/tests");
    rb.checkout("feature/tests");

    rb.write_file(
        "src/main/kotlin/services/UserService.kt",
        r#"import com.example.models.User

class UserService {
    fun greet(name: String): String {
        return "Hello, $name"
    }
}
"#,
    );

    rb.write_file(
        "src/test/kotlin/services/UserServiceTest.kt",
        r#"import org.junit.Test
import com.example.services.UserService

class UserServiceTest {
    fun testGreet() {
        val service = UserService()
        val result = service.greet("Alice")
    }

    fun testGreetEmpty() {
        val service = UserService()
        val result = service.greet("")
    }
}
"#,
    );

    rb.commit("Add UserService with JUnit tests");

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
        "should detect *Test.kt as test file entrypoint"
    );

    // Verify Kotlin language detected
    assert_language_detected(&result, "kotlin");

    // Verify JUnit framework detected
    let has_junit = result
        .summary
        .frameworks_detected
        .iter()
        .any(|f| f.contains("JUnit"));
    assert!(
        has_junit,
        "should detect JUnit framework; detected: {:?}",
        result.summary.frameworks_detected
    );
}

/// Test: Swift Vapor REST API with controller→service→repo pattern.
///
/// Creates a synthetic Swift Vapor app to verify:
/// - Swift file detection (.swift extension)
/// - Import extraction (module-level imports)
/// - Definition extraction (struct, class, protocol, func)
/// - Call site extraction (method calls, function calls)
/// - Entrypoint detection (Vapor route handlers)
/// - Framework detection (Vapor, Fluent)
/// - Semantic grouping and ranking
// ─── Scala Integration Tests ─────────────────────────────────────────────

/// Test: Synthetic Scala Akka HTTP API with handler → service → repository pattern.
#[test]
fn test_e2e_scala_akka_http_api() {
    let rb = RepoBuilder::new();

    rb.write_file(
        "build.sbt",
        "name := \"akka-api\"\nscalaVersion := \"2.13.12\"\n",
    );
    rb.commit("Initial commit");
    rb.create_branch("main");

    rb.create_branch("feature/users-api");
    rb.checkout("feature/users-api");

    rb.write_file(
        "src/main/scala/routes/UserRoutes.scala",
        r#"import akka.http.scaladsl.server.Directives._
import akka.http.scaladsl.server.Route
import com.example.services.UserService

class UserRoutes(service: UserService) {
  def routes(): Route = {
    pathPrefix("users") {
      get {
        val users = service.listUsers()
        complete(users.toString())
      } ~
      post {
        val user = service.createUser("test")
        complete(user.toString())
      }
    }
  }
}
"#,
    );

    rb.write_file(
        "src/main/scala/services/UserService.scala",
        r#"import com.example.repositories.UserRepository

class UserService(repo: UserRepository) {
  def listUsers(): List[User] = {
    val users = repo.findAll()
    users
  }

  def createUser(name: String): User = {
    val user = repo.save(name)
    println("Created user")
    user
  }
}
"#,
    );

    rb.write_file(
        "src/main/scala/repositories/UserRepository.scala",
        r#"import slick.jdbc.PostgresProfile.api._

class UserRepository(db: Database) {
  def findAll(): List[User] = {
    val result = db.run(users.result)
    result
  }

  def save(name: String): User = {
    val user = User(name)
    db.run(users.insertOrUpdate(user))
    user
  }
}
"#,
    );

    rb.write_file(
        "src/main/scala/models/User.scala",
        r#"case class User(id: String, name: String, email: String)

type UserId = String
"#,
    );

    rb.commit("Add users API with Akka HTTP");

    let result = run_pipeline(rb.path(), "main", "feature/users-api");

    assert_valid_json_schema(&result);
    assert_all_files_accounted(&result);

    // Verify Scala language detected
    assert_language_detected(&result, "scala");

    // Verify HTTP route entrypoint detected
    let has_http_entrypoint = result.groups.iter().any(|g| {
        g.entrypoint.as_ref().map_or(false, |e| {
            e.entrypoint_type == diffcore_core::types::EntrypointType::HttpRoute
        })
    });
    assert!(
        has_http_entrypoint,
        "should detect Akka HTTP route entrypoint; groups: {:?}",
        result
            .groups
            .iter()
            .map(|g| (&g.name, &g.entrypoint))
            .collect::<Vec<_>>()
    );

    // Verify framework detected
    let frameworks = &result.summary.frameworks_detected;
    assert!(
        frameworks
            .iter()
            .any(|f| f.contains("Akka") || f.contains("Slick")),
        "should detect Akka HTTP or Slick framework; got: {:?}",
        frameworks
    );
}

/// Test: Scala test file detection with ScalaTest.
#[test]
fn test_e2e_scala_test_file_detection() {
    let rb = RepoBuilder::new();

    rb.write_file("build.sbt", "name := \"scala-test\"\n");
    rb.commit("Initial commit");
    rb.create_branch("main");

    rb.create_branch("feature/tests");
    rb.checkout("feature/tests");

    rb.write_file(
        "src/main/scala/services/Calculator.scala",
        r#"object Calculator {
  def add(a: Int, b: Int): Int = a + b
  def multiply(a: Int, b: Int): Int = a * b
}
"#,
    );

    rb.write_file(
        "src/test/scala/services/CalculatorSpec.scala",
        r#"import org.scalatest.flatspec.AnyFlatSpec
import org.scalatest.matchers.should.Matchers

class CalculatorSpec extends AnyFlatSpec with Matchers {
  def testAdd(): Unit = {
    val result = Calculator.add(2, 3)
    result shouldEqual 5
  }

  def testMultiply(): Unit = {
    val result = Calculator.multiply(3, 4)
    result shouldEqual 12
  }
}
"#,
    );

    rb.commit("Add calculator with tests");

    let result = run_pipeline(rb.path(), "main", "feature/tests");

    assert_valid_json_schema(&result);
    assert_all_files_accounted(&result);

    // Verify Scala language detected
    assert_language_detected(&result, "scala");

    // Verify test file detected as entrypoint
    let has_test_entrypoint = result.groups.iter().any(|g| {
        g.entrypoint.as_ref().map_or(false, |e| {
            e.entrypoint_type == diffcore_core::types::EntrypointType::TestFile
        })
    });
    assert!(
        has_test_entrypoint,
        "should detect ScalaTest spec as test entrypoint; groups: {:?}",
        result
            .groups
            .iter()
            .map(|g| (&g.name, &g.entrypoint))
            .collect::<Vec<_>>()
    );

    // Verify ScalaTest framework detected
    let frameworks = &result.summary.frameworks_detected;
    assert!(
        frameworks.iter().any(|f| f.contains("ScalaTest")),
        "should detect ScalaTest framework; got: {:?}",
        frameworks
    );
}
