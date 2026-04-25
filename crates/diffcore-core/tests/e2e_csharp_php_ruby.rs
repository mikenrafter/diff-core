#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr
)]
//! E2E integration tests for C#, PHP, and Ruby language support.

mod helpers;

use helpers::graph_assertions::{
    assert_all_files_accounted, assert_json_roundtrip, assert_language_detected,
    assert_valid_json_schema, assert_valid_mermaid, assert_valid_scores,
};
use helpers::repo_builder::{run_pipeline, RepoBuilder};

// ---------------------------------------------------------------------------
// C# integration tests (Phase 11.2)
// ---------------------------------------------------------------------------

/// Test: C# ASP.NET Core Web API with controller → service → repository pattern.
///
/// Verifies full pipeline: language detection, file accounting, flow groups,
/// entrypoint detection, framework detection, and Mermaid graph.
#[test]
fn test_e2e_csharp_aspnet_core_api() {
    let rb = RepoBuilder::new();

    // Initial commit
    rb.write_file(
        "MyApp.csproj",
        r#"<Project Sdk="Microsoft.NET.Sdk.Web">
    <PropertyGroup>
        <TargetFramework>net8.0</TargetFramework>
    </PropertyGroup>
    <ItemGroup>
        <PackageReference Include="Microsoft.EntityFrameworkCore" Version="8.0.0" />
    </ItemGroup>
</Project>
"#,
    );
    rb.commit("Initial commit: csproj");
    rb.create_branch("main");

    // Feature branch: add ASP.NET Core API
    rb.create_branch("feature/csharp-api");
    rb.checkout("feature/csharp-api");

    rb.write_file(
        "Program.cs",
        r#"
using Microsoft.AspNetCore.Builder;
using Microsoft.Extensions.DependencyInjection;

var builder = WebApplication.CreateBuilder(args);
builder.Services.AddControllers();
var app = builder.Build();
app.MapControllers();
app.Run();
"#,
    );

    rb.write_file(
        "Controllers/UsersController.cs",
        r#"
using System.Collections.Generic;
using Microsoft.AspNetCore.Mvc;
using MyApp.Models;
using MyApp.Services;

namespace MyApp.Controllers
{
    [ApiController]
    [Route("api/[controller]")]
    public class UsersController : ControllerBase
    {
        private readonly IUserService _userService;

        public UsersController(IUserService userService)
        {
            _userService = userService;
        }

        [HttpGet]
        public ActionResult<IEnumerable<User>> GetUsers()
        {
            return Ok(_userService.FindAll());
        }

        [HttpPost]
        public ActionResult<User> CreateUser(User user)
        {
            return Ok(_userService.Save(user));
        }
    }
}
"#,
    );

    rb.write_file(
        "Services/UserService.cs",
        r#"
using System.Collections.Generic;
using MyApp.Models;
using MyApp.Repositories;

namespace MyApp.Services
{
    public interface IUserService
    {
        List<User> FindAll();
        User Save(User user);
    }

    public class UserService : IUserService
    {
        private readonly IUserRepository _repository;

        public UserService(IUserRepository repository)
        {
            _repository = repository;
        }

        public List<User> FindAll()
        {
            return _repository.FindAll();
        }

        public User Save(User user)
        {
            return _repository.Save(user);
        }
    }
}
"#,
    );

    rb.write_file(
        "Repositories/UserRepository.cs",
        r#"
using System.Collections.Generic;
using MyApp.Models;

namespace MyApp.Repositories
{
    public interface IUserRepository
    {
        List<User> FindAll();
        User Save(User user);
    }

    public class UserRepository : IUserRepository
    {
        private readonly List<User> _users = new List<User>();

        public List<User> FindAll()
        {
            return _users;
        }

        public User Save(User user)
        {
            _users.Add(user);
            return user;
        }
    }
}
"#,
    );

    rb.write_file(
        "Models/User.cs",
        r#"
namespace MyApp.Models
{
    public record User(int Id, string Name, string Email);
}
"#,
    );

    rb.commit("Add ASP.NET Core Web API with controller-service-repo");

    let result = run_pipeline(rb.path(), "main", "feature/csharp-api");

    // Verify basic output shape
    assert_valid_json_schema(&result);
    assert_valid_scores(&result);

    // Verify C# files were detected
    assert_language_detected(&result, "csharp");

    // Verify all changed files are accounted for
    assert_all_files_accounted(&result);

    // Verify there are flow groups
    assert!(
        !result.groups.is_empty(),
        "should produce at least one flow group"
    );

    // Verify entrypoint detection (Main or HTTP routes)
    let has_entrypoint = result.groups.iter().any(|g| g.entrypoint.is_some());
    assert!(has_entrypoint, "should detect at least one entrypoint");

    // Verify ASP.NET Core framework detection
    let has_aspnet = result
        .summary
        .frameworks_detected
        .iter()
        .any(|f| f.contains("ASP.NET"));
    assert!(
        has_aspnet,
        "should detect ASP.NET Core framework; detected: {:?}",
        result.summary.frameworks_detected
    );

    // Verify Mermaid graph is valid
    assert_valid_mermaid(&result);
}

/// Test: C# test file detection.
///
/// Verifies that *Test.cs and *Tests.cs files are detected as test entrypoints.
#[test]
fn test_e2e_csharp_test_file_detection() {
    let rb = RepoBuilder::new();

    rb.write_file(
        "MyApp.csproj",
        r#"<Project Sdk="Microsoft.NET.Sdk.Web">
    <PropertyGroup>
        <TargetFramework>net8.0</TargetFramework>
    </PropertyGroup>
</Project>
"#,
    );
    rb.commit("Initial commit");
    rb.create_branch("main");

    rb.create_branch("feature/tests");
    rb.checkout("feature/tests");

    rb.write_file(
        "Services/UserService.cs",
        r#"
namespace MyApp.Services
{
    public class UserService
    {
        public string GetGreeting(string name)
        {
            return $"Hello, {name}!";
        }
    }
}
"#,
    );

    rb.write_file(
        "Tests/UserServiceTests.cs",
        r#"
using Xunit;
using MyApp.Services;

namespace MyApp.Tests
{
    public class UserServiceTests
    {
        [Fact]
        public void GetGreeting_ReturnsExpected()
        {
            var svc = new UserService();
            var result = svc.GetGreeting("World");
            Assert.Equal("Hello, World!", result);
        }
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
        "should detect *Tests.cs as test file entrypoint"
    );
}

// ─── PHP E2E Tests ────────────────────────────────────────────────────────

/// Test: Synthetic Laravel REST API with controller → service → model pattern.
///
/// Verifies PHP parsing, import resolution, entrypoint detection (Laravel controllers),
/// framework detection (Laravel), and flow grouping.
#[test]
fn test_e2e_php_laravel_api() {
    let rb = RepoBuilder::new();

    // Initial commit
    rb.write_file(
        "composer.json",
        r#"{"name": "example/demo", "require": {"laravel/framework": "^11.0"}}"#,
    );
    rb.commit("Initial commit: composer.json");
    rb.create_branch("main");

    // Feature branch: add Laravel REST API
    rb.create_branch("feature/php-api");
    rb.checkout("feature/php-api");

    rb.write_file(
        "app/Http/Controllers/UserController.php",
        r#"<?php

namespace App\Http\Controllers;

use Illuminate\Http\Request;
use App\Models\User;
use App\Services\UserService;

class UserController extends Controller
{
    private $userService;

    public function __construct(UserService $userService)
    {
        $this->userService = $userService;
    }

    public function index()
    {
        $users = User::all();
        return response()->json($users);
    }

    public function store(Request $request)
    {
        $data = $request->validated();
        $user = User::create($data);
        return response()->json($user, 201);
    }

    public function show(User $user)
    {
        return response()->json($user);
    }

    public function destroy(User $user)
    {
        $user->delete();
        return response()->json(null, 204);
    }
}
"#,
    );

    rb.write_file(
        "app/Models/User.php",
        r#"<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Model;

class User extends Model
{
    protected $fillable = ['name', 'email'];

    public function posts()
    {
        return $this->hasMany(Post::class);
    }
}
"#,
    );

    rb.write_file(
        "app/Services/UserService.php",
        r#"<?php

namespace App\Services;

use App\Models\User;

class UserService
{
    public function findAll()
    {
        return User::all();
    }

    public function findById($id)
    {
        return User::find($id);
    }

    public function create(array $data)
    {
        return User::create($data);
    }

    public function update(User $user, array $data)
    {
        $user->update($data);
        return $user;
    }

    public function delete(User $user)
    {
        $user->delete();
    }
}
"#,
    );

    rb.write_file(
        "app/Providers/AppServiceProvider.php",
        r#"<?php

namespace App\Providers;

use Illuminate\Support\ServiceProvider;

class AppServiceProvider extends ServiceProvider
{
    public function register()
    {
        //
    }

    public function boot()
    {
        //
    }
}
"#,
    );

    rb.commit("Add Laravel REST API with controller-service-model");

    let result = run_pipeline(rb.path(), "main", "feature/php-api");

    // Verify basic output shape
    assert_valid_json_schema(&result);
    assert_valid_scores(&result);

    // Verify PHP files were detected
    assert_language_detected(&result, "php");

    // Verify all changed files are accounted for
    assert_all_files_accounted(&result);

    // Verify there are flow groups
    assert!(
        !result.groups.is_empty(),
        "should produce at least one flow group"
    );

    // Verify entrypoint detection (controller action methods)
    let has_entrypoint = result.groups.iter().any(|g| g.entrypoint.is_some());
    assert!(
        has_entrypoint,
        "should detect at least one entrypoint (Laravel controller actions)"
    );

    // Verify Laravel framework detection
    let has_laravel = result
        .summary
        .frameworks_detected
        .iter()
        .any(|f| f.contains("Laravel"));
    assert!(
        has_laravel,
        "should detect Laravel framework; detected: {:?}",
        result.summary.frameworks_detected
    );

    // Verify Mermaid graph is valid
    assert_valid_mermaid(&result);

    // Verify JSON roundtrip
    assert_json_roundtrip(&result);
}

/// Test: PHP test file detection.
///
/// Verifies that *Test.php files are detected as test entrypoints.
#[test]
fn test_e2e_php_test_file_detection() {
    let rb = RepoBuilder::new();

    rb.write_file(
        "composer.json",
        r#"{"name": "example/demo", "require-dev": {"phpunit/phpunit": "^11.0"}}"#,
    );
    rb.commit("Initial commit");
    rb.create_branch("main");

    rb.create_branch("feature/tests");
    rb.checkout("feature/tests");

    rb.write_file(
        "app/Services/UserService.php",
        r#"<?php

namespace App\Services;

class UserService
{
    public function greet($name)
    {
        return "Hello, " . $name;
    }
}
"#,
    );

    rb.write_file(
        "tests/Unit/UserServiceTest.php",
        r#"<?php

namespace Tests\Unit;

use PHPUnit\Framework\TestCase;
use App\Services\UserService;

class UserServiceTest extends TestCase
{
    public function test_greet()
    {
        $service = new UserService();
        $result = $service->greet("Alice");
        $this->assertEquals("Hello, Alice", $result);
    }

    public function test_greet_empty()
    {
        $service = new UserService();
        $result = $service->greet("");
        $this->assertEquals("Hello, ", $result);
    }
}
"#,
    );

    rb.commit("Add UserService with PHPUnit tests");

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
        "should detect *Test.php as test file entrypoint"
    );

    // Verify PHP language detected
    assert_language_detected(&result, "php");

    // Verify PHPUnit framework detected
    let has_phpunit = result
        .summary
        .frameworks_detected
        .iter()
        .any(|f| f.contains("PHPUnit"));
    assert!(
        has_phpunit,
        "should detect PHPUnit framework; detected: {:?}",
        result.summary.frameworks_detected
    );
}

/// Test: Ruby Rails REST API with controller→service→model pattern.
///
/// Verifies that the pipeline can:
/// - Parse Ruby source files via tree-sitter
/// - Extract require/require_relative imports, include/extend mixins
/// - Detect class, module, and method definitions
/// - Detect Rails controller action entrypoints
/// - Detect Rails framework from imports
/// - Cluster files into meaningful flow groups
#[test]
fn test_e2e_ruby_rails_api() {
    let rb = RepoBuilder::new();

    // Initial commit
    rb.write_file(
        "Gemfile",
        "source 'https://rubygems.org'\ngem 'rails', '~> 7.1'\n",
    );
    rb.commit("Initial commit: Gemfile");
    rb.create_branch("main");

    // Feature branch: add Rails REST API
    rb.create_branch("feature/ruby-api");
    rb.checkout("feature/ruby-api");

    rb.write_file(
        "app/controllers/users_controller.rb",
        r#"require 'action_controller'
require_relative '../models/user'
require_relative '../services/user_service'

class UsersController < ApplicationController
  include Authentication

  def index
    @users = User.all()
    respond_to()
  end

  def show
    @user = User.find(params())
  end

  def create
    @user = UserService.new().create(user_params())
    redirect_to(@user)
  end

  def destroy
    @user = User.find(params())
    @user.destroy()
  end

  private

  def user_params
    params().require().permit()
  end
end
"#,
    );

    rb.write_file(
        "app/models/user.rb",
        r#"require 'active_record'

class User < ActiveRecord::Base
  include Validatable

  def full_name
    first_name.to_s()
  end

  def active?
    status == 'active'
  end
end
"#,
    );

    rb.write_file(
        "app/services/user_service.rb",
        r#"require_relative '../models/user'

class UserService
  def create(attrs)
    user = User.new(attrs)
    user.save()
    notify(user)
    user
  end

  def find(id)
    User.find(id)
  end

  private

  def notify(user)
    EventBus.publish('user.created', user)
  end
end
"#,
    );

    rb.write_file(
        "config/routes.rb",
        r#"require 'action_controller'

Rails.application.routes.draw()
"#,
    );

    rb.commit("Add Rails REST API with controller-service-model");

    let result = run_pipeline(rb.path(), "main", "feature/ruby-api");

    // Verify basic output shape
    assert_valid_json_schema(&result);
    assert_valid_scores(&result);

    // Verify Ruby files were detected
    assert_language_detected(&result, "ruby");

    // Verify all changed files are accounted for
    assert_all_files_accounted(&result);

    // Verify there are flow groups
    assert!(
        !result.groups.is_empty(),
        "should produce at least one flow group"
    );

    // Verify entrypoint detection (controller action methods)
    let has_entrypoint = result.groups.iter().any(|g| g.entrypoint.is_some());
    assert!(
        has_entrypoint,
        "should detect at least one entrypoint (Rails controller actions)"
    );

    // Verify Rails framework detection
    let has_rails = result
        .summary
        .frameworks_detected
        .iter()
        .any(|f| f.contains("Rails"));
    assert!(
        has_rails,
        "should detect Rails framework; detected: {:?}",
        result.summary.frameworks_detected
    );

    // Verify Mermaid graph is valid
    assert_valid_mermaid(&result);

    // Verify JSON roundtrip
    assert_json_roundtrip(&result);
}

/// Test: Ruby test file detection.
///
/// Verifies that *_spec.rb and *_test.rb files are detected as test entrypoints.
#[test]
fn test_e2e_ruby_test_file_detection() {
    let rb = RepoBuilder::new();

    rb.write_file("Gemfile", "source 'https://rubygems.org'\ngem 'rspec'\n");
    rb.commit("Initial commit");
    rb.create_branch("main");

    rb.create_branch("feature/tests");
    rb.checkout("feature/tests");

    rb.write_file(
        "app/services/user_service.rb",
        "class UserService\n  def greet(name)\n    name.to_s()\n  end\nend\n",
    );

    rb.write_file(
        "spec/services/user_service_spec.rb",
        r#"require 'rspec'
require_relative '../../app/services/user_service'

RSpec.describe(UserService)

class UserServiceSpec
  def test_greet
    service = UserService.new()
    result = service.greet("Alice")
  end

  def test_greet_empty
    service = UserService.new()
    result = service.greet("")
  end
end
"#,
    );

    rb.commit("Add UserService with RSpec tests");

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
        "should detect *_spec.rb as test file entrypoint"
    );

    // Verify Ruby language detected
    assert_language_detected(&result, "ruby");

    // Verify RSpec framework detected
    let has_rspec = result
        .summary
        .frameworks_detected
        .iter()
        .any(|f| f.contains("RSpec"));
    assert!(
        has_rspec,
        "should detect RSpec framework; detected: {:?}",
        result.summary.frameworks_detected
    );
}
