//! Ruby extraction code.
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
    let require_relative_idx = qwc.capture_index("require_relative_source");
    let include_name_idx = qwc.capture_index("include_name");

    let mut imports: Vec<ImportInfo> = Vec::new();
    let mut seen: Vec<(usize, String)> = Vec::new();

    for m in &matches {
        let mut line = 0usize;

        for &(idx, node) in &m.captures {
            if Some(idx) == stmt_idx {
                line = node.start_position().row + 1;
            }
        }

        // Handle `require 'gem_name'`
        if m.has_capture(source_idx) {
            let source_text = m
                .get_capture(source_idx)
                .map(|n| node_text(&n, source).to_string())
                .unwrap_or_default();
            if source_text.is_empty() {
                continue;
            }

            let key = (line, source_text.clone());
            if seen.contains(&key) {
                continue;
            }
            seen.push(key);

            imports.push(ImportInfo {
                source: source_text,
                names: vec![],
                is_default: false,
                is_namespace: false,
                line,
            });
        }

        // Handle `require_relative '../models/user'`
        if m.has_capture(require_relative_idx) {
            let path_text = m
                .get_capture(require_relative_idx)
                .map(|n| node_text(&n, source).to_string())
                .unwrap_or_default();
            if path_text.is_empty() {
                continue;
            }

            let key = (line, path_text.clone());
            if seen.contains(&key) {
                continue;
            }
            seen.push(key);

            imports.push(ImportInfo {
                source: path_text,
                names: vec![],
                is_default: false,
                is_namespace: false,
                line,
            });
        }

        // Handle `include ModuleName` / `extend ModuleName`
        if m.has_capture(include_name_idx) {
            let mod_name = m
                .get_capture(include_name_idx)
                .map(|n| node_text(&n, source).to_string())
                .unwrap_or_default();
            if mod_name.is_empty() {
                continue;
            }

            let key = (line, mod_name.clone());
            if seen.contains(&key) {
                continue;
            }
            seen.push(key);

            imports.push(ImportInfo {
                source: mod_name.clone(),
                names: vec![ImportedName {
                    name: mod_name,
                    alias: None,
                }],
                is_default: false,
                is_namespace: false,
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
    let singleton_method_name_idx = qwc.capture_index("singleton_method_name");
    let singleton_method_node_idx = qwc.capture_index("singleton_method_node");
    let class_name_idx = qwc.capture_index("class_name");
    let class_node_idx = qwc.capture_index("class_node");
    let module_name_idx = qwc.capture_index("module_name");
    let module_node_idx = qwc.capture_index("module_node");
    let const_name_idx = qwc.capture_index("const_name");
    let const_node_idx = qwc.capture_index("const_node");

    let ruby_def_captures: &[(Option<u32>, Option<u32>, SymbolKind)] = &[
        (method_name_idx, method_node_idx, SymbolKind::Function),
        (
            singleton_method_name_idx,
            singleton_method_node_idx,
            SymbolKind::Function,
        ),
        (class_name_idx, class_node_idx, SymbolKind::Class),
        (module_name_idx, module_node_idx, SymbolKind::Module),
        (const_name_idx, const_node_idx, SymbolKind::Constant),
    ];

    for m in matches {
        for &(name_cap, node_cap, kind) in ruby_def_captures {
            if m.has_capture(name_cap) {
                let name_node = m.get_capture(name_cap);
                let name_text = name_node
                    .map(|n| node_text(&n, source).to_string())
                    .unwrap_or_default();
                let (start_line, end_line, _node_start) = node_span(m, node_cap);
                if !name_text.is_empty() {
                    let name_start = name_node.map(|n| n.start_byte()).unwrap_or(0);
                    let key = (name_start, hash_str(&name_text));
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
    fn test_ruby_language_detection() {
        assert_eq!(
            Language::from_path("app/controllers/users_controller.rb"),
            Language::Ruby
        );
        assert_eq!(Language::from_path("app/models/user.rb"), Language::Ruby);
        assert_eq!(
            Language::from_path("spec/models/user_spec.rb"),
            Language::Ruby
        );
    }

    #[test]
    fn test_ruby_require_import() {
        let e = engine();
        let result = e
            .parse_file(
                "app/models/user.rb",
                "require 'json'\nrequire 'active_record'\n",
            )
            .unwrap();
        assert_eq!(result.language, Language::Ruby);
        assert_eq!(result.imports.len(), 2);
        let sources: Vec<&str> = result.imports.iter().map(|i| i.source.as_str()).collect();
        assert!(sources.contains(&"json"));
        assert!(sources.contains(&"active_record"));
    }

    #[test]
    fn test_ruby_require_relative_import() {
        let e = engine();
        let result = e
            .parse_file(
                "app/services/user_service.rb",
                "require_relative '../models/user'\nrequire_relative '../repositories/user_repository'\n",
            )
            .unwrap();
        assert_eq!(result.imports.len(), 2);
        let sources: Vec<&str> = result.imports.iter().map(|i| i.source.as_str()).collect();
        assert!(sources.contains(&"../models/user"));
        assert!(sources.contains(&"../repositories/user_repository"));
    }

    #[test]
    fn test_ruby_include_extend_imports() {
        let e = engine();
        let result = e
            .parse_file(
                "app/models/user.rb",
                "class User\n  include Logging\n  extend ClassMethods\nend\n",
            )
            .unwrap();
        let import_sources: Vec<&str> = result.imports.iter().map(|i| i.source.as_str()).collect();
        assert!(import_sources.contains(&"Logging"));
        assert!(import_sources.contains(&"ClassMethods"));
        // include/extend imports should have names
        let logging_import = result
            .imports
            .iter()
            .find(|i| i.source == "Logging")
            .unwrap();
        assert_eq!(logging_import.names[0].name, "Logging");
    }

    #[test]
    fn test_ruby_method_definition() {
        let e = engine();
        let result = e
            .parse_file(
                "app/services/user_service.rb",
                "class UserService\n  def create(attrs)\n    User.new(attrs)\n  end\n\n  def find(id)\n    User.find(id)\n  end\nend\n",
            )
            .unwrap();
        let def_names: Vec<&str> = result.definitions.iter().map(|d| d.name.as_str()).collect();
        assert!(def_names.contains(&"create"));
        assert!(def_names.contains(&"find"));
        assert!(def_names.contains(&"UserService"));
        assert!(
            result
                .definitions
                .iter()
                .filter(|d| d.kind == SymbolKind::Function)
                .count()
                >= 2
        );
    }

    #[test]
    fn test_ruby_singleton_method() {
        let e = engine();
        let result = e
            .parse_file(
                "app/services/user_service.rb",
                "class UserService\n  def self.instance\n    @inst ||= new\n  end\nend\n",
            )
            .unwrap();
        let def_names: Vec<&str> = result.definitions.iter().map(|d| d.name.as_str()).collect();
        assert!(def_names.contains(&"instance"));
        assert!(def_names.contains(&"UserService"));
    }

    #[test]
    fn test_ruby_class_definition() {
        let e = engine();
        let result = e
            .parse_file(
                "app/models/user.rb",
                "class User < ActiveRecord::Base\n  def name\n    @name\n  end\nend\n",
            )
            .unwrap();
        let class_def = result
            .definitions
            .iter()
            .find(|d| d.name == "User")
            .unwrap();
        assert_eq!(class_def.kind, SymbolKind::Class);
    }

    #[test]
    fn test_ruby_module_definition() {
        let e = engine();
        let result = e
            .parse_file(
                "app/concerns/logging.rb",
                "module Logging\n  def log(msg)\n    puts msg\n  end\nend\n",
            )
            .unwrap();
        let mod_def = result
            .definitions
            .iter()
            .find(|d| d.name == "Logging")
            .unwrap();
        assert_eq!(mod_def.kind, SymbolKind::Module);
        let def_names: Vec<&str> = result.definitions.iter().map(|d| d.name.as_str()).collect();
        assert!(def_names.contains(&"log"));
    }

    #[test]
    fn test_ruby_constant_assignment() {
        let e = engine();
        let result = e
            .parse_file(
                "config/constants.rb",
                "MAX_RETRIES = 3\nDEFAULT_TIMEOUT = 30\n",
            )
            .unwrap();
        let def_names: Vec<&str> = result.definitions.iter().map(|d| d.name.as_str()).collect();
        assert!(def_names.contains(&"MAX_RETRIES"));
        assert!(def_names.contains(&"DEFAULT_TIMEOUT"));
        assert!(result
            .definitions
            .iter()
            .all(|d| d.kind == SymbolKind::Constant));
    }

    #[test]
    fn test_ruby_method_call() {
        let e = engine();
        let result = e
            .parse_file(
                "app/services/user_service.rb",
                "class UserService\n  def create(attrs)\n    user = User.new(attrs)\n    user.save()\n    EventBus.publish('user.created', user)\n  end\nend\n",
            )
            .unwrap();
        let callees: Vec<&str> = result
            .call_sites
            .iter()
            .map(|c| c.callee.as_str())
            .collect();
        assert!(callees.contains(&"new"));
        assert!(callees.contains(&"save"));
        assert!(callees.contains(&"publish"));
    }

    #[test]
    fn test_ruby_call_containing_function() {
        let e = engine();
        let result = e
            .parse_file(
                "app/services/user_service.rb",
                "class UserService\n  def create(attrs)\n    User.new(attrs)\n  end\n\n  def find(id)\n    User.find(id)\n  end\nend\n",
            )
            .unwrap();
        let create_call = result
            .call_sites
            .iter()
            .find(|c| c.callee == "new")
            .unwrap();
        assert_eq!(create_call.containing_function, Some("create".to_string()));
        let find_call = result
            .call_sites
            .iter()
            .find(|c| c.callee == "find")
            .unwrap();
        assert_eq!(find_call.containing_function, Some("find".to_string()));
    }

    #[test]
    fn test_ruby_data_flow_assignment() {
        let e = engine();
        let result = e
            .extract_data_flow("app/services/user_service.rb", "class UserService\n  def create(attrs)\n    user = User.new(attrs)\n    result = user.save()\n  end\nend\n")
            .unwrap();
        let var_names: Vec<&str> = result
            .assignments
            .iter()
            .map(|a| a.variable.as_str())
            .collect();
        assert!(var_names.contains(&"user"));
        assert!(var_names.contains(&"result"));
    }

    #[test]
    fn test_ruby_data_flow_instance_var_assignment() {
        let e = engine();
        let result = e
            .extract_data_flow(
                "app/controllers/users_controller.rb",
                "class UsersController\n  def index\n    @users = User.all()\n  end\nend\n",
            )
            .unwrap();
        let var_names: Vec<&str> = result
            .assignments
            .iter()
            .map(|a| a.variable.as_str())
            .collect();
        assert!(var_names.contains(&"@users"));
    }

    #[test]
    fn test_ruby_empty_source() {
        let e = engine();
        let result = e.parse_file("empty.rb", "").unwrap();
        assert_eq!(result.language, Language::Ruby);
        assert!(result.definitions.is_empty());
        assert!(result.imports.is_empty());
        assert!(result.call_sites.is_empty());
    }

    #[test]
    fn test_ruby_multiple_imports() {
        let e = engine();
        let result = e
            .parse_file(
                "app/services/user_service.rb",
                "require 'json'\nrequire 'logger'\nrequire_relative '../models/user'\nrequire_relative '../repositories/user_repo'\n",
            )
            .unwrap();
        assert_eq!(result.imports.len(), 4);
    }

    #[test]
    fn test_ruby_full_rails_controller() {
        let e = engine();
        let source = "require 'action_controller'\nrequire_relative '../models/user'\nrequire_relative '../services/user_service'\n\nclass UsersController < ApplicationController\n  include Authentication\n\n  def index\n    @users = User.all()\n    respond_to()\n  end\n\n  def show\n    @user = User.find(params())\n  end\n\n  def create\n    @user = User.new(user_params())\n    @user.save()\n    redirect_to(@user)\n  end\n\n  private\n\n  def user_params\n    params().require().permit()\n  end\nend\n";
        let result = e
            .parse_file("app/controllers/users_controller.rb", source)
            .unwrap();
        assert_eq!(result.language, Language::Ruby);

        // Imports
        let import_sources: Vec<&str> = result.imports.iter().map(|i| i.source.as_str()).collect();
        assert!(import_sources.contains(&"action_controller"));
        assert!(import_sources.contains(&"../models/user"));
        assert!(import_sources.contains(&"../services/user_service"));
        assert!(import_sources.contains(&"Authentication"));

        // Definitions
        let def_names: Vec<&str> = result.definitions.iter().map(|d| d.name.as_str()).collect();
        assert!(def_names.contains(&"UsersController"));
        assert!(def_names.contains(&"index"));
        assert!(def_names.contains(&"show"));
        assert!(def_names.contains(&"create"));
        assert!(def_names.contains(&"user_params"));

        // Call sites
        let callees: Vec<&str> = result
            .call_sites
            .iter()
            .map(|c| c.callee.as_str())
            .collect();
        assert!(callees.contains(&"all"));
        assert!(callees.contains(&"respond_to"));
        assert!(callees.contains(&"find"));
        assert!(callees.contains(&"new"));
        assert!(callees.contains(&"save"));
        assert!(callees.contains(&"redirect_to"));
    }

}
