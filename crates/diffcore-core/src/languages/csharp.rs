//! Csharp extraction code.
//!
//! Moved from `query_engine.rs` during the per-language module split.
//! Pure mechanical move — no logic changes.

use crate::ast::{Definition, ImportedName, ImportInfo};
use crate::languages::common::{
    collect_matches, hash_str, node_span, node_text,
    extract_definitions_standard, CollectedMatch,
};
use crate::query_engine::{QueryEngineError, QueryWithCaptures};
use crate::types::SymbolKind;
use tree_sitter::{Node, QueryCursor};


pub(crate) fn extract_imports(
root: &Node,
    source: &[u8],
    qwc: &QueryWithCaptures,
) -> Result<Vec<ImportInfo>, QueryEngineError> {
    let mut cursor = QueryCursor::new();
    let matches = collect_matches(&mut cursor, &qwc.query, *root, source);

    let stmt_idx = qwc.capture_index("stmt");
    let source_idx = qwc.capture_index("source");

    let mut imports = Vec::new();
    let mut seen: Vec<(usize, String)> = Vec::new();

    for m in &matches {
        let mut line = 0usize;

        for &(idx, node) in &m.captures {
            if Some(idx) == stmt_idx {
                line = node.start_position().row + 1;
            }
        }

        if m.has_capture(source_idx) {
            let source_text = m
                .get_capture(source_idx)
                .map(|n| node_text(&n, source).to_string())
                .unwrap_or_default();
            if source_text.is_empty() {
                continue;
            }

            // Skip alias using directives — if the using_directive has a `name:`
            // field, it's `using Alias = Type;`, the captured source is the alias
            // identifier, not the namespace. We detect this by checking if the
            // stmt node text contains '='.
            let stmt_text = m
                .get_capture(stmt_idx)
                .map(|n| node_text(&n, source).to_string())
                .unwrap_or_default();
            if stmt_text.contains('=') {
                continue;
            }

            let key = (line, source_text.clone());
            if seen.contains(&key) {
                continue;
            }
            seen.push(key);

            // C# `using` imports an entire namespace, not a single class.
            // Keep the full namespace as source for framework detection.
            // Use "*" as the name to indicate namespace import (like Java wildcard).
            imports.push(ImportInfo {
                source: source_text,
                names: vec![ImportedName {
                    name: "*".to_string(),
                    alias: None,
                }],
                is_default: false,
                is_namespace: true,
                line,
            });
        }
    }

    Ok(imports)
}

pub(crate) fn extract_definitions(
    matches: &[CollectedMatch<'_>],
    source: &[u8],
    qwc: &QueryWithCaptures,
    definitions: &mut Vec<Definition>,
    seen_nodes: &mut Vec<(usize, usize)>,
) {
    if extract_definitions_standard(
        matches,
        source,
        qwc,
        definitions,
        seen_nodes,
    ) {
        // Standard path took over.
    } else {
    let method_name_idx = qwc.capture_index("method_name");
    let method_node_idx = qwc.capture_index("method_node");
    let ctor_name_idx = qwc.capture_index("ctor_name");
    let ctor_node_idx = qwc.capture_index("ctor_node");
    let class_name_idx = qwc.capture_index("class_name");
    let class_node_idx = qwc.capture_index("class_node");
    let struct_name_idx = qwc.capture_index("struct_name");
    let struct_node_idx = qwc.capture_index("struct_node");
    let iface_name_idx = qwc.capture_index("iface_name");
    let iface_node_idx = qwc.capture_index("iface_node");
    let enum_name_idx = qwc.capture_index("enum_name");
    let enum_node_idx = qwc.capture_index("enum_node");
    let record_name_idx = qwc.capture_index("record_name");
    let record_node_idx = qwc.capture_index("record_node");
    let prop_name_idx = qwc.capture_index("prop_name");
    let prop_node_idx = qwc.capture_index("prop_node");
    let field_name_idx = qwc.capture_index("field_name");
    let field_node_idx = qwc.capture_index("field_node");
    let delegate_name_idx = qwc.capture_index("delegate_name");
    let delegate_node_idx = qwc.capture_index("delegate_node");

    let csharp_def_captures: &[(Option<u32>, Option<u32>, SymbolKind)] = &[
        (method_name_idx, method_node_idx, SymbolKind::Function),
        (ctor_name_idx, ctor_node_idx, SymbolKind::Function),
        (class_name_idx, class_node_idx, SymbolKind::Class),
        (struct_name_idx, struct_node_idx, SymbolKind::Class),
        (iface_name_idx, iface_node_idx, SymbolKind::Interface),
        (enum_name_idx, enum_node_idx, SymbolKind::Class),
        (record_name_idx, record_node_idx, SymbolKind::Class),
        (prop_name_idx, prop_node_idx, SymbolKind::Constant),
        (field_name_idx, field_node_idx, SymbolKind::Constant),
        (delegate_name_idx, delegate_node_idx, SymbolKind::Interface),
    ];

    for m in matches {
        for &(name_cap, node_cap, kind) in csharp_def_captures {
            if m.has_capture(name_cap) {
                let name_text = m
                    .get_capture(name_cap)
                    .map(|n| node_text(&n, source).to_string())
                    .unwrap_or_default();
                let (start_line, end_line, node_start) = node_span(m, node_cap);
                if !name_text.is_empty() {
                    let key = (node_start, hash_str(&name_text));
                    if !seen_nodes.contains(&key) {
                        seen_nodes.push(key);
                        definitions.push(Definition {
                            name: name_text,
                            kind,
                            start_line,
                            end_line,
                        });
                    }
                }
                break;
            }
        }
    }
    } // close `else` opened above for the standard-convention fallback
}


#[cfg(test)]
#[allow(
    unused_imports,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr
)]
mod tests {
    use crate::ast::Language;
    use crate::query_engine::{shared_test_engine, QueryEngine};
    use crate::types::SymbolKind;

    fn engine() -> &'static QueryEngine {
        shared_test_engine()
    }

    #[test]
    fn test_csharp_language_detection() {
        assert_eq!(Language::from_path("Program.cs"), Language::CSharp);
        assert_eq!(
            Language::from_path("src/Controllers/UserController.cs"),
            Language::CSharp
        );
        assert_eq!(Language::from_path("Models/User.cs"), Language::CSharp);
    }

    #[test]
    fn test_csharp_simple_using() {
        let e = engine();
        let result = e.parse_file("App.cs", "using System;").unwrap();
        assert_eq!(result.language, Language::CSharp);
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].source, "System");
    }

    #[test]
    fn test_csharp_qualified_using() {
        let e = engine();
        let result = e
            .parse_file("App.cs", "using System.Collections.Generic;")
            .unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].source, "System.Collections.Generic");
        assert_eq!(result.imports[0].names[0].name, "*");
        assert!(result.imports[0].is_namespace);
    }

    #[test]
    fn test_csharp_multiple_usings() {
        let e = engine();
        let source = r#"
using System;
using System.Linq;
using Microsoft.AspNetCore.Mvc;
using Microsoft.EntityFrameworkCore;
"#;
        let result = e.parse_file("Controller.cs", source).unwrap();
        assert_eq!(result.imports.len(), 4);
        let sources: Vec<&str> = result.imports.iter().map(|i| i.source.as_str()).collect();
        assert!(sources.contains(&"System"));
        assert!(sources.contains(&"System.Linq"));
        assert!(sources.contains(&"Microsoft.AspNetCore.Mvc"));
        assert!(sources.contains(&"Microsoft.EntityFrameworkCore"));
    }

    #[test]
    fn test_csharp_aspnet_using() {
        let e = engine();
        let result = e
            .parse_file("App.cs", "using Microsoft.AspNetCore.Mvc;")
            .unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].source, "Microsoft.AspNetCore.Mvc");
        assert_eq!(result.imports[0].names[0].name, "*");
    }

    #[test]
    fn test_csharp_class_definition() {
        let e = engine();
        let source = r#"
public class UserService
{
}
"#;
        let result = e.parse_file("UserService.cs", source).unwrap();
        let class_defs: Vec<&str> = result
            .definitions
            .iter()
            .filter(|d| d.kind == SymbolKind::Class)
            .map(|d| d.name.as_str())
            .collect();
        assert!(class_defs.contains(&"UserService"));
    }

    #[test]
    fn test_csharp_interface_definition() {
        let e = engine();
        let source = r#"
public interface IUserRepository
{
    User FindById(int id);
}
"#;
        let result = e.parse_file("IUserRepository.cs", source).unwrap();
        let iface_defs: Vec<&str> = result
            .definitions
            .iter()
            .filter(|d| d.kind == SymbolKind::Interface)
            .map(|d| d.name.as_str())
            .collect();
        assert!(iface_defs.contains(&"IUserRepository"));
    }

    #[test]
    fn test_csharp_struct_definition() {
        let e = engine();
        let source = r#"
public struct Point
{
    public int X;
    public int Y;
}
"#;
        let result = e.parse_file("Point.cs", source).unwrap();
        let struct_defs: Vec<&str> = result
            .definitions
            .iter()
            .filter(|d| d.kind == SymbolKind::Class)
            .map(|d| d.name.as_str())
            .collect();
        assert!(struct_defs.contains(&"Point"));
    }

    #[test]
    fn test_csharp_record_definition() {
        let e = engine();
        let source = r#"
public record UserDto(string Name, string Email);
"#;
        let result = e.parse_file("UserDto.cs", source).unwrap();
        let record_defs: Vec<&str> = result
            .definitions
            .iter()
            .filter(|d| d.kind == SymbolKind::Class)
            .map(|d| d.name.as_str())
            .collect();
        assert!(record_defs.contains(&"UserDto"));
    }

    #[test]
    fn test_csharp_method_definitions() {
        let e = engine();
        let source = r#"
public class UserService
{
    public User FindUser(int id)
    {
        return null;
    }

    public void CreateUser(string name)
    {
    }
}
"#;
        let result = e.parse_file("UserService.cs", source).unwrap();
        let fn_defs: Vec<&str> = result
            .definitions
            .iter()
            .filter(|d| d.kind == SymbolKind::Function)
            .map(|d| d.name.as_str())
            .collect();
        assert!(fn_defs.contains(&"FindUser"));
        assert!(fn_defs.contains(&"CreateUser"));
    }

    #[test]
    fn test_csharp_enum_definition() {
        let e = engine();
        let source = r#"
public enum Status
{
    Active,
    Inactive
}
"#;
        let result = e.parse_file("Status.cs", source).unwrap();
        let class_defs: Vec<&str> = result
            .definitions
            .iter()
            .filter(|d| d.kind == SymbolKind::Class)
            .map(|d| d.name.as_str())
            .collect();
        assert!(class_defs.contains(&"Status"));
    }

    #[test]
    fn test_csharp_constructor_definition() {
        let e = engine();
        let source = r#"
public class UserService
{
    public UserService(IUserRepository repo)
    {
    }
}
"#;
        let result = e.parse_file("UserService.cs", source).unwrap();
        let fn_defs: Vec<&str> = result
            .definitions
            .iter()
            .filter(|d| d.kind == SymbolKind::Function)
            .map(|d| d.name.as_str())
            .collect();
        assert!(fn_defs.contains(&"UserService"));
    }

    #[test]
    fn test_csharp_property_definition() {
        let e = engine();
        let source = r#"
public class Config
{
    public string ApiKey { get; set; }
    public int MaxRetries { get; set; } = 3;
}
"#;
        let result = e.parse_file("Config.cs", source).unwrap();
        let prop_defs: Vec<&str> = result
            .definitions
            .iter()
            .filter(|d| d.kind == SymbolKind::Constant)
            .map(|d| d.name.as_str())
            .collect();
        assert!(prop_defs.contains(&"ApiKey"));
        assert!(prop_defs.contains(&"MaxRetries"));
    }

    #[test]
    fn test_csharp_field_definition() {
        let e = engine();
        let source = r#"
public class Config
{
    private readonly string _connectionString;
    public static readonly int MaxRetries = 3;
}
"#;
        let result = e.parse_file("Config.cs", source).unwrap();
        let field_defs: Vec<&str> = result
            .definitions
            .iter()
            .filter(|d| d.kind == SymbolKind::Constant)
            .map(|d| d.name.as_str())
            .collect();
        assert!(field_defs.contains(&"_connectionString"));
        assert!(field_defs.contains(&"MaxRetries"));
    }

    #[test]
    fn test_csharp_delegate_definition() {
        let e = engine();
        let source = r#"
public delegate void EventHandler(object sender, EventArgs e);
"#;
        let result = e.parse_file("EventHandler.cs", source).unwrap();
        let delegate_defs: Vec<&str> = result
            .definitions
            .iter()
            .filter(|d| d.kind == SymbolKind::Interface)
            .map(|d| d.name.as_str())
            .collect();
        assert!(delegate_defs.contains(&"EventHandler"));
    }

    #[test]
    fn test_csharp_method_call() {
        let e = engine();
        let source = r#"
public class App
{
    public void Run()
    {
        _userService.FindUser(42);
    }
}
"#;
        let result = e.parse_file("App.cs", source).unwrap();
        let callees: Vec<&str> = result
            .call_sites
            .iter()
            .map(|c| c.callee.as_str())
            .collect();
        assert!(callees.contains(&"FindUser"));
    }

    #[test]
    fn test_csharp_direct_call() {
        let e = engine();
        let source = r#"
public class App
{
    public void Run()
    {
        DoSomething();
    }
}
"#;
        let result = e.parse_file("App.cs", source).unwrap();
        let callees: Vec<&str> = result
            .call_sites
            .iter()
            .map(|c| c.callee.as_str())
            .collect();
        assert!(callees.contains(&"DoSomething"));
    }

    #[test]
    fn test_csharp_constructor_call() {
        let e = engine();
        let source = r#"
public class App
{
    public void Run()
    {
        var user = new User("Alice");
    }
}
"#;
        let result = e.parse_file("App.cs", source).unwrap();
        let callees: Vec<&str> = result
            .call_sites
            .iter()
            .map(|c| c.callee.as_str())
            .collect();
        assert!(callees.contains(&"User"));
    }

    #[test]
    fn test_csharp_call_containing_function() {
        let e = engine();
        let source = r#"
public class App
{
    public void Run()
    {
        DoSomething();
    }
}
"#;
        let result = e.parse_file("App.cs", source).unwrap();
        let call = result
            .call_sites
            .iter()
            .find(|c| c.callee == "DoSomething")
            .unwrap();
        assert_eq!(call.containing_function.as_deref(), Some("Run"));
    }

    #[test]
    fn test_csharp_data_flow_assignment() {
        let e = engine();
        let source = r#"
public class App
{
    public void Run()
    {
        var user = FindUser(1);
    }
}
"#;
        let df = e.extract_data_flow("App.cs", source).unwrap();
        assert!(!df.assignments.is_empty());
        assert_eq!(df.assignments[0].variable, "user");
        assert_eq!(df.assignments[0].callee, "FindUser");
    }

    #[test]
    fn test_csharp_data_flow_member_assignment() {
        let e = engine();
        let source = r#"
public class App
{
    public void Run()
    {
        var users = _repository.GetAll();
    }
}
"#;
        let df = e.extract_data_flow("App.cs", source).unwrap();
        assert!(!df.assignments.is_empty());
        assert_eq!(df.assignments[0].variable, "users");
        assert_eq!(df.assignments[0].callee, "GetAll");
    }

    #[test]
    fn test_csharp_data_flow_constructor_assignment() {
        let e = engine();
        let source = r#"
public class App
{
    public void Run()
    {
        var svc = new UserService();
    }
}
"#;
        let df = e.extract_data_flow("App.cs", source).unwrap();
        assert!(!df.assignments.is_empty());
        assert_eq!(df.assignments[0].variable, "svc");
        assert_eq!(df.assignments[0].callee, "UserService");
    }

    #[test]
    fn test_csharp_empty_source() {
        let e = engine();
        let result = e.parse_file("App.cs", "").unwrap();
        assert_eq!(result.language, Language::CSharp);
        assert!(result.definitions.is_empty());
        assert!(result.imports.is_empty());
    }

    #[test]
    fn test_csharp_full_aspnet_controller() {
        let e = engine();
        let source = r#"
using System.Collections.Generic;
using Microsoft.AspNetCore.Mvc;

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
"#;
        let result = e.parse_file("UsersController.cs", source).unwrap();

        // Imports
        assert!(result.imports.len() >= 2);
        let import_sources: Vec<&str> = result.imports.iter().map(|i| i.source.as_str()).collect();
        assert!(import_sources.contains(&"System.Collections.Generic"));
        assert!(import_sources.contains(&"Microsoft.AspNetCore.Mvc"));

        // Definitions
        let def_names: Vec<&str> = result.definitions.iter().map(|d| d.name.as_str()).collect();
        assert!(def_names.contains(&"UsersController"));
        assert!(def_names.contains(&"GetUsers"));
        assert!(def_names.contains(&"CreateUser"));

        // Call sites
        let callees: Vec<&str> = result
            .call_sites
            .iter()
            .map(|c| c.callee.as_str())
            .collect();
        assert!(callees.contains(&"FindAll"));
        assert!(callees.contains(&"Save"));
        assert!(callees.contains(&"Ok"));
    }

}
