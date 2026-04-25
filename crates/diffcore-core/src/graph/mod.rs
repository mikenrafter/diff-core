//! Symbol graph construction using petgraph.
//!
//! Builds a directed graph `G = (V, E)` from parsed AST data where:
//! - Vertices are symbols (functions, classes, types, modules)
//! - Edges represent relationships (imports, calls, extends)

use std::collections::HashMap;

use petgraph::graph::{DiGraph, NodeIndex};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::ast::{Definition, ExportInfo, Language, ParsedFile};
use crate::ir::{IrExport, IrFile, IrImportSpecifier, TypeDefKind};
use crate::types::{EdgeType, SymbolKind};

/// A node in the symbol graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SymbolNode {
    /// Unique identifier: `file_path::symbol_name`
    pub id: String,
    /// The symbol name.
    pub name: String,
    /// The file this symbol belongs to.
    pub file: String,
    /// The kind of symbol.
    pub kind: SymbolKind,
}

/// An edge in the symbol graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphEdge {
    pub edge_type: EdgeType,
}

/// The complete symbol graph built from parsed files.
#[derive(Debug)]
pub struct SymbolGraph {
    pub graph: DiGraph<SymbolNode, GraphEdge>,
    /// Map from symbol id (`file::name`) to node index for fast lookup.
    id_to_index: HashMap<String, NodeIndex>,
}

/// Errors from graph construction.
#[derive(Debug, thiserror::Error)]
pub enum GraphError {
    #[error("graph serialization error: {0}")]
    SerializationError(String),
}

/// Serializable representation for roundtrip testing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SerializableGraph {
    pub nodes: Vec<SymbolNode>,
    pub edges: Vec<SerializableEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SerializableEdge {
    pub from: String,
    pub to: String,
    pub edge_type: EdgeType,
}

impl SymbolGraph {
    /// Build a symbol graph from a collection of parsed files.
    pub fn build(files: &[ParsedFile]) -> Self {
        Self::build_with_workspace(files, &WorkspaceMap::new())
    }

    /// Build a symbol graph with workspace package resolution for monorepos.
    ///
    /// The `workspace_map` maps package names (e.g. `@scope/pkg`) to their
    /// entry file paths (e.g. `packages/pkg/src/index.ts`), enabling cross-package
    /// import edges in monorepo workspaces.
    pub fn build_with_workspace(files: &[ParsedFile], workspace_map: &WorkspaceMap) -> Self {
        let mut graph = DiGraph::new();
        let mut id_to_index: HashMap<String, NodeIndex> = HashMap::new();

        // Phase 1: Collect node data per file in parallel, then merge single-threaded.
        let node_batches: Vec<Vec<(String, SymbolNode)>> = files
            .par_iter()
            .map(|file| {
                let mut nodes = Vec::new();
                // Module node.
                let module_id = file.path.clone();
                nodes.push((
                    module_id,
                    SymbolNode {
                        id: file.path.clone(),
                        name: file_stem(&file.path),
                        file: file.path.clone(),
                        kind: SymbolKind::Module,
                    },
                ));
                // Definition nodes.
                for def in &file.definitions {
                    let sym_id = format!("{}::{}", file.path, def.name);
                    nodes.push((
                        sym_id.clone(),
                        SymbolNode {
                            id: sym_id,
                            name: def.name.clone(),
                            file: file.path.clone(),
                            kind: def.kind.clone(),
                        },
                    ));
                }
                nodes
            })
            .collect();

        for batch in node_batches {
            for (sym_id, node) in batch {
                if id_to_index.contains_key(&sym_id) {
                    continue; // skip duplicates
                }
                let idx = graph.add_node(node);
                id_to_index.insert(sym_id, idx);
            }
        }

        // Build lookup structures for import resolution.
        let file_exports = build_export_map(files);
        let file_defs = build_definition_map(files);

        // Phase 2: Compute edges per file in parallel, then add single-threaded.
        let edge_batches: Vec<Vec<(String, String, EdgeType)>> = files
            .par_iter()
            .map(|file| {
                let mut edges = Vec::new();
                collect_import_edges(
                    file,
                    files,
                    &file_exports,
                    &file_defs,
                    &id_to_index,
                    workspace_map,
                    &mut edges,
                );
                collect_call_edges(
                    file,
                    files,
                    &file_exports,
                    &file_defs,
                    &id_to_index,
                    workspace_map,
                    &mut edges,
                );
                collect_extends_edges(file, files, &file_defs, &id_to_index, &mut edges);
                edges
            })
            .collect();

        for batch in edge_batches {
            for (from_id, to_id, edge_type) in batch {
                if let (Some(&from_idx), Some(&to_idx)) =
                    (id_to_index.get(&from_id), id_to_index.get(&to_id))
                {
                    graph.add_edge(from_idx, to_idx, GraphEdge { edge_type });
                }
            }
        }

        SymbolGraph { graph, id_to_index }
    }

    /// Build a symbol graph from IR files (declarative query engine / IR path).
    ///
    /// This is the primary entry point for graph construction from the IR pipeline.
    /// It consumes `IrFile` types directly, enabling richer edge construction
    /// (e.g., class extends edges from `IrTypeDef.bases`).
    pub fn build_from_ir(files: &[IrFile]) -> Self {
        Self::build_from_ir_with_workspace(files, &WorkspaceMap::new())
    }

    /// Build a symbol graph from IR files with workspace package resolution.
    pub fn build_from_ir_with_workspace(files: &[IrFile], workspace_map: &WorkspaceMap) -> Self {
        let mut graph = DiGraph::new();
        let mut id_to_index: HashMap<String, NodeIndex> = HashMap::new();

        // Phase 1: Collect node data per file in parallel, then merge single-threaded.
        let node_batches: Vec<Vec<(String, SymbolNode)>> = files
            .par_iter()
            .map(|file| {
                let mut nodes = Vec::new();
                // Module node.
                nodes.push((
                    file.path.clone(),
                    SymbolNode {
                        id: file.path.clone(),
                        name: file_stem(&file.path),
                        file: file.path.clone(),
                        kind: SymbolKind::Module,
                    },
                ));
                // Function nodes.
                for f in &file.functions {
                    let sym_id = format!("{}::{}", file.path, f.name);
                    nodes.push((
                        sym_id.clone(),
                        SymbolNode {
                            id: sym_id,
                            name: f.name.clone(),
                            file: file.path.clone(),
                            kind: SymbolKind::Function,
                        },
                    ));
                }
                // Type definition nodes.
                for t in &file.type_defs {
                    let sym_id = format!("{}::{}", file.path, t.name);
                    let kind = match t.kind {
                        TypeDefKind::Class => SymbolKind::Class,
                        TypeDefKind::Struct => SymbolKind::Struct,
                        TypeDefKind::Interface => SymbolKind::Interface,
                        TypeDefKind::TypeAlias => SymbolKind::TypeAlias,
                        TypeDefKind::Enum => SymbolKind::Class,
                    };
                    nodes.push((
                        sym_id.clone(),
                        SymbolNode {
                            id: sym_id,
                            name: t.name.clone(),
                            file: file.path.clone(),
                            kind,
                        },
                    ));
                }
                // Constant nodes.
                for c in &file.constants {
                    let sym_id = format!("{}::{}", file.path, c.name);
                    nodes.push((
                        sym_id.clone(),
                        SymbolNode {
                            id: sym_id,
                            name: c.name.clone(),
                            file: file.path.clone(),
                            kind: SymbolKind::Constant,
                        },
                    ));
                }
                nodes
            })
            .collect();

        for batch in node_batches {
            for (sym_id, node) in batch {
                if id_to_index.contains_key(&sym_id) {
                    continue; // skip duplicates
                }
                let idx = graph.add_node(node);
                id_to_index.insert(sym_id, idx);
            }
        }

        // Build lookup structures.
        let file_exports = build_ir_export_map(files);
        let file_def_names = build_ir_def_names_map(files);
        let known_paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();

        // Phase 2: Compute edges per file in parallel, then add single-threaded.
        let edge_batches: Vec<Vec<(String, String, EdgeType)>> = files
            .par_iter()
            .map(|file| {
                let mut edges = Vec::new();
                collect_ir_import_edges(
                    file,
                    &file_exports,
                    &file_def_names,
                    &id_to_index,
                    &known_paths,
                    workspace_map,
                    &mut edges,
                );
                collect_ir_call_edges(
                    file,
                    files,
                    &file_def_names,
                    &id_to_index,
                    &known_paths,
                    workspace_map,
                    &mut edges,
                );
                collect_ir_extends_edges(file, files, &id_to_index, &known_paths, &mut edges);
                edges
            })
            .collect();

        for batch in edge_batches {
            for (from_id, to_id, edge_type) in batch {
                if let (Some(&from_idx), Some(&to_idx)) =
                    (id_to_index.get(&from_id), id_to_index.get(&to_id))
                {
                    graph.add_edge(from_idx, to_idx, GraphEdge { edge_type });
                }
            }
        }

        SymbolGraph { graph, id_to_index }
    }

    /// Number of nodes in the graph.
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Number of edges in the graph.
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    /// Look up a node index by symbol id.
    pub fn get_node(&self, id: &str) -> Option<NodeIndex> {
        self.id_to_index.get(id).copied()
    }

    /// Get the symbol node data for a given id.
    pub fn get_symbol(&self, id: &str) -> Option<&SymbolNode> {
        self.id_to_index.get(id).map(|idx| &self.graph[*idx])
    }

    /// Get all node ids in the graph.
    pub fn node_ids(&self) -> Vec<&str> {
        self.id_to_index.keys().map(|s| s.as_str()).collect()
    }

    /// Add an edge between two nodes by their indices.
    pub fn add_edge(&mut self, from: NodeIndex, to: NodeIndex, edge: GraphEdge) {
        self.graph.add_edge(from, to, edge);
    }

    /// Get all edges as (from_id, to_id, edge_type) tuples.
    pub fn edges(&self) -> Vec<(&str, &str, &EdgeType)> {
        self.graph
            .edge_indices()
            .filter_map(|e| {
                let (src, tgt) = self.graph.edge_endpoints(e)?;
                let edge = &self.graph[e];
                Some((
                    self.graph[src].id.as_str(),
                    self.graph[tgt].id.as_str(),
                    &edge.edge_type,
                ))
            })
            .collect()
    }

    /// Serialize the graph to a JSON-friendly structure.
    pub fn to_serializable(&self) -> SerializableGraph {
        let nodes: Vec<SymbolNode> = self
            .graph
            .node_indices()
            .map(|i| self.graph[i].clone())
            .collect();

        let edges: Vec<SerializableEdge> = self
            .graph
            .edge_indices()
            .filter_map(|e| {
                let (src, tgt) = self.graph.edge_endpoints(e)?;
                Some(SerializableEdge {
                    from: self.graph[src].id.clone(),
                    to: self.graph[tgt].id.clone(),
                    edge_type: self.graph[e].edge_type.clone(),
                })
            })
            .collect();

        SerializableGraph { nodes, edges }
    }

    /// Deserialize from a serializable graph back into a SymbolGraph.
    pub fn from_serializable(sg: &SerializableGraph) -> Self {
        let mut graph = DiGraph::new();
        let mut id_to_index: HashMap<String, NodeIndex> = HashMap::new();

        for node in &sg.nodes {
            let idx = graph.add_node(node.clone());
            id_to_index.insert(node.id.clone(), idx);
        }

        for edge in &sg.edges {
            if let (Some(&src), Some(&tgt)) =
                (id_to_index.get(&edge.from), id_to_index.get(&edge.to))
            {
                graph.add_edge(
                    src,
                    tgt,
                    GraphEdge {
                        edge_type: edge.edge_type.clone(),
                    },
                );
            }
        }

        SymbolGraph { graph, id_to_index }
    }
}

// ---------------------------------------------------------------------------
// Import resolution helpers
// ---------------------------------------------------------------------------

/// Map from file path to its exported symbol names.
fn build_export_map(files: &[ParsedFile]) -> HashMap<String, Vec<ExportInfo>> {
    files
        .iter()
        .map(|f| (f.path.clone(), f.exports.clone()))
        .collect()
}

/// Map from file path to its definitions.
fn build_definition_map(files: &[ParsedFile]) -> HashMap<String, Vec<Definition>> {
    files
        .iter()
        .map(|f| (f.path.clone(), f.definitions.clone()))
        .collect()
}

/// Resolve an import source path (e.g. `./utils`, `../models/user`) relative to the
/// importing file, returning the resolved file path if it exists in our file set.
///
/// Handles both JS/TS-style (`./utils`, `../models/user`) and Python-style
/// (`.models`, `..models`, `.models.user`) relative imports.
fn resolve_import_path(
    import_source: &str,
    importer_path: &str,
    known_files: &[&str],
) -> Option<String> {
    // Only resolve relative imports
    if !import_source.starts_with('.') {
        return None;
    }

    // Convert Python-style dot imports to path-style.
    // `.models` → `./models`, `..models` → `../models`, `.models.user` → `./models/user`
    let normalized_source = normalize_python_import(import_source);

    let importer_dir = parent_dir(importer_path);
    let resolved = normalize_path(&format!("{}/{}", importer_dir, normalized_source));

    // Try exact match first, then with common extensions.
    let candidates = [
        resolved.clone(),
        format!("{}.ts", resolved),
        format!("{}.tsx", resolved),
        format!("{}.js", resolved),
        format!("{}.jsx", resolved),
        format!("{}.py", resolved),
        format!("{}/index.ts", resolved),
        format!("{}/index.js", resolved),
        format!("{}/index.tsx", resolved),
    ];

    for candidate in &candidates {
        if known_files.contains(&candidate.as_str()) {
            return Some(candidate.clone());
        }
    }

    None
}

/// A map from workspace package name (e.g. `@monorepo/shared-types`) to its
/// entry file path relative to the repo root (e.g. `packages/shared-types/src/index.ts`).
pub type WorkspaceMap = HashMap<String, String>;

/// Resolve a non-relative import through a workspace package map.
///
/// When `import_source` is a bare specifier (e.g. `@monorepo/shared-types` or
/// `@monorepo/shared-types/utils`), look it up in the workspace map. If the
/// exact name matches, return its entry file. If only a prefix matches (e.g.
/// `@scope/pkg/sub`), try to resolve the sub-path relative to the package root.
fn resolve_workspace_import(
    import_source: &str,
    known_files: &[&str],
    workspace_map: &WorkspaceMap,
) -> Option<String> {
    // Skip relative imports (already handled by resolve_import_path).
    if import_source.starts_with('.') {
        return None;
    }

    // Try exact match first.
    if let Some(entry) = workspace_map.get(import_source) {
        if known_files.contains(&entry.as_str()) {
            return Some(entry.clone());
        }
    }

    // Try prefix match for deep imports like `@scope/pkg/sub/path`.
    // Find the longest matching package name.
    let mut best_match: Option<(&str, &str)> = None;
    for (pkg_name, entry_file) in workspace_map {
        if import_source.starts_with(pkg_name.as_str())
            && import_source[pkg_name.len()..].starts_with('/')
        {
            if best_match.map_or(true, |(prev, _)| pkg_name.len() > prev.len()) {
                best_match = Some((pkg_name.as_str(), entry_file.as_str()));
            }
        }
    }

    if let Some((pkg_name, entry_file)) = best_match {
        // Get package root directory from entry file path.
        let pkg_dir = parent_dir(parent_dir(entry_file).as_str());
        let sub_path = &import_source[pkg_name.len() + 1..]; // skip the '/'
        let resolved = format!("{}/{}", pkg_dir, sub_path);

        // Try with common extensions.
        let candidates = [
            resolved.clone(),
            format!("{}.ts", resolved),
            format!("{}.tsx", resolved),
            format!("{}.js", resolved),
            format!("{}.jsx", resolved),
            format!("{}/index.ts", resolved),
            format!("{}/index.js", resolved),
        ];

        for candidate in &candidates {
            if known_files.contains(&candidate.as_str()) {
                return Some(candidate.clone());
            }
        }
    }

    None
}

/// Try to resolve an import path, falling back to workspace resolution.
fn resolve_import_or_workspace(
    import_source: &str,
    importer_path: &str,
    known_files: &[&str],
    workspace_map: &WorkspaceMap,
) -> Option<String> {
    resolve_import_path(import_source, importer_path, known_files)
        .or_else(|| resolve_workspace_import(import_source, known_files, workspace_map))
}

/// Build a workspace package map by scanning `package.json` files in a directory.
///
/// Reads the root `package.json` for `workspaces` globs, then reads each
/// matched package's `package.json` for its `name` and `main` fields.
/// Returns a map from package name → entry file path (relative to repo root).
pub fn build_workspace_map(repo_root: &std::path::Path) -> WorkspaceMap {
    let mut map = WorkspaceMap::new();

    // Read root package.json for workspaces.
    let root_pkg = repo_root.join("package.json");
    let root_content = match std::fs::read_to_string(&root_pkg) {
        Ok(c) => c,
        Err(_) => return map,
    };
    let root_json: serde_json::Value = match serde_json::from_str(&root_content) {
        Ok(v) => v,
        Err(_) => return map,
    };

    // Extract workspace patterns.
    let workspace_patterns: Vec<String> = match root_json.get("workspaces") {
        Some(serde_json::Value::Array(arr)) => arr
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect(),
        // pnpm-style: { packages: [...] }
        Some(serde_json::Value::Object(obj)) => obj
            .get("packages")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
        _ => return map,
    };

    // Expand glob patterns to find package directories.
    for pattern in &workspace_patterns {
        let full_pattern = repo_root.join(pattern).join("package.json");
        if let Some(pattern_str) = full_pattern.to_str() {
            if let Ok(entries) = glob::glob(pattern_str) {
                for entry in entries.flatten() {
                    if let Ok(content) = std::fs::read_to_string(&entry) {
                        if let Ok(pkg_json) = serde_json::from_str::<serde_json::Value>(&content) {
                            let name = pkg_json.get("name").and_then(|v| v.as_str());
                            let main_field = pkg_json.get("main").and_then(|v| v.as_str());

                            if let Some(name) = name {
                                // Determine entry file path relative to repo root.
                                let pkg_dir = entry.parent().unwrap_or(repo_root.as_ref());
                                let entry_file = if let Some(main_path) = main_field {
                                    pkg_dir.join(main_path)
                                } else {
                                    // Default: try src/index.ts, then index.ts
                                    let src_index = pkg_dir.join("src/index.ts");
                                    if src_index.exists() {
                                        src_index
                                    } else {
                                        pkg_dir.join("index.ts")
                                    }
                                };

                                if let Ok(relative) = entry_file.strip_prefix(repo_root) {
                                    if let Some(rel_str) = relative.to_str() {
                                        map.insert(name.to_string(), rel_str.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    map
}

/// Get the parent directory of a file path.
fn parent_dir(path: &str) -> String {
    match path.rfind('/') {
        Some(pos) => path[..pos].to_string(),
        None => ".".to_string(),
    }
}

/// Get the file stem (filename without extension).
fn file_stem(path: &str) -> String {
    let filename = path.rsplit('/').next().unwrap_or(path);
    match filename.find('.') {
        Some(pos) => filename[..pos].to_string(),
        None => filename.to_string(),
    }
}

/// Normalize a path by resolving `.` and `..` segments.
fn normalize_path(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "." | "" => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

/// Convert Python-style dot imports to path-style relative imports.
///
/// - `.models` → `./models`
/// - `..models` → `../models`
/// - `.models.user` → `./models/user`
/// - `.` → `.`
/// - `...utils.helpers` → `../../utils/helpers`
fn normalize_python_import(source: &str) -> String {
    // Count leading dots.
    let dot_count = source.chars().take_while(|c| *c == '.').count();
    let remainder = &source[dot_count..];

    if dot_count == 0 {
        return source.to_string();
    }

    // Build the relative prefix: `.` → `./`, `..` → `../`, `...` → `../../`
    let prefix = if dot_count == 1 {
        ".".to_string()
    } else {
        let mut p = String::new();
        for i in 0..dot_count - 1 {
            if i > 0 {
                p.push('/');
            }
            p.push_str("..");
        }
        p
    };

    if remainder.is_empty() {
        return prefix;
    }

    // Convert remaining dots (module separators) to slashes.
    let path_part = remainder.replace('.', "/");
    format!("{}/{}", prefix, path_part)
}

// ---------------------------------------------------------------------------
// Edge construction
// ---------------------------------------------------------------------------

/// Collect import edge descriptors: file A imports symbol from file B.
/// Pushes `(from_id, to_id, EdgeType)` tuples for later insertion.
fn collect_import_edges(
    file: &ParsedFile,
    all_files: &[ParsedFile],
    file_exports: &HashMap<String, Vec<ExportInfo>>,
    file_defs: &HashMap<String, Vec<Definition>>,
    id_to_index: &HashMap<String, NodeIndex>,
    workspace_map: &WorkspaceMap,
    edges: &mut Vec<(String, String, EdgeType)>,
) {
    let known_paths: Vec<&str> = all_files.iter().map(|f| f.path.as_str()).collect();

    for import in &file.imports {
        let resolved = match resolve_import_or_workspace(
            &import.source,
            &file.path,
            &known_paths,
            workspace_map,
        ) {
            Some(p) => p,
            None => continue,
        };

        let from_module_id = file.path.clone();
        if !id_to_index.contains_key(&from_module_id) {
            continue;
        }

        // For each imported name, find matching export or definition in target file.
        if import.names.is_empty() {
            // Side-effect import: create module-to-module edge.
            if id_to_index.contains_key(&resolved) {
                edges.push((from_module_id.clone(), resolved.clone(), EdgeType::Imports));
            }
            continue;
        }

        for imported_name in &import.names {
            let target_name = &imported_name.name;

            // Try to find the symbol in the target file's definitions.
            let target_sym_id = format!("{}::{}", resolved, target_name);
            if id_to_index.contains_key(&target_sym_id) {
                edges.push((from_module_id.clone(), target_sym_id, EdgeType::Imports));
                continue;
            }

            // If importing a default, check if target has a matching export/def.
            if import.is_default || import.is_namespace {
                // Link to the module node itself.
                if id_to_index.contains_key(&resolved) {
                    edges.push((from_module_id.clone(), resolved.clone(), EdgeType::Imports));
                }
                continue;
            }

            // Check re-exports: target file may re-export from another file.
            if let Some(exports) = file_exports.get(&resolved) {
                for export in exports {
                    if export.name == *target_name && export.is_reexport {
                        if let Some(ref reexport_source) = export.source {
                            if let Some(reexport_resolved) = resolve_import_or_workspace(
                                reexport_source,
                                &resolved,
                                &known_paths,
                                workspace_map,
                            ) {
                                let reexport_sym_id =
                                    format!("{}::{}", reexport_resolved, target_name);
                                if id_to_index.contains_key(&reexport_sym_id) {
                                    edges.push((
                                        from_module_id.clone(),
                                        reexport_sym_id,
                                        EdgeType::Imports,
                                    ));
                                }
                            }
                        }
                    }
                }
            }

            // Fallback: Python-style — definition name matches directly.
            if let Some(defs) = file_defs.get(&resolved) {
                if defs.iter().any(|d| d.name == *target_name) {
                    let sym_id = format!("{}::{}", resolved, target_name);
                    if id_to_index.contains_key(&sym_id) {
                        edges.push((from_module_id.clone(), sym_id, EdgeType::Imports));
                    }
                }
            }
        }
    }
}

/// Collect call edge descriptors: function A calls function B.
fn collect_call_edges(
    file: &ParsedFile,
    all_files: &[ParsedFile],
    file_exports: &HashMap<String, Vec<ExportInfo>>,
    file_defs: &HashMap<String, Vec<Definition>>,
    id_to_index: &HashMap<String, NodeIndex>,
    workspace_map: &WorkspaceMap,
    edges: &mut Vec<(String, String, EdgeType)>,
) {
    let known_paths: Vec<&str> = all_files.iter().map(|f| f.path.as_str()).collect();

    // Build a map of imported names → resolved symbol ids for this file.
    let import_map = build_import_resolution_map(
        file,
        all_files,
        file_exports,
        file_defs,
        &known_paths,
        workspace_map,
    );

    for call in &file.call_sites {
        // Determine the calling symbol.
        let caller_id = match &call.containing_function {
            Some(func_name) => format!("{}::{}", file.path, func_name),
            None => file.path.clone(), // module-level call
        };

        // Resolve caller: try exact id, then module node.
        let resolved_caller_id = if id_to_index.contains_key(&caller_id) {
            caller_id
        } else if id_to_index.contains_key(&file.path) {
            file.path.clone()
        } else {
            continue;
        };

        // Resolve the callee.
        let callee_name = &call.callee;

        // Simple name (e.g., `validateUser`) — look up in import map or local defs.
        if let Some(target_id) = import_map.get(callee_name.as_str()) {
            if id_to_index.contains_key(target_id.as_str()) && resolved_caller_id != *target_id {
                edges.push((
                    resolved_caller_id.clone(),
                    target_id.clone(),
                    EdgeType::Calls,
                ));
            }
            continue;
        }

        // Method call (e.g., `db.save`) — check if `db` is an imported name.
        if let Some(dot_pos) = callee_name.find('.') {
            let receiver = &callee_name[..dot_pos];
            if let Some(target_module) = import_map.get(receiver) {
                let method = &callee_name[dot_pos + 1..];
                let method_id = format!("{}::{}", target_module.trim_end_matches("::*"), method);
                if id_to_index.contains_key(&method_id) && resolved_caller_id != method_id {
                    edges.push((resolved_caller_id.clone(), method_id, EdgeType::Calls));
                    continue;
                }
                if id_to_index.contains_key(target_module.as_str())
                    && resolved_caller_id != *target_module
                {
                    edges.push((
                        resolved_caller_id.clone(),
                        target_module.clone(),
                        EdgeType::Calls,
                    ));
                    continue;
                }
            }
        }

        // Local function call — same file.
        let local_id = format!("{}::{}", file.path, callee_name);
        if id_to_index.contains_key(&local_id) && resolved_caller_id != local_id {
            edges.push((resolved_caller_id.clone(), local_id, EdgeType::Calls));
        }
    }
}

/// Build a map from imported name → resolved symbol id for a given file.
fn build_import_resolution_map(
    file: &ParsedFile,
    all_files: &[ParsedFile],
    _file_exports: &HashMap<String, Vec<ExportInfo>>,
    _file_defs: &HashMap<String, Vec<Definition>>,
    known_paths: &[&str],
    workspace_map: &WorkspaceMap,
) -> HashMap<String, String> {
    let mut map = HashMap::new();

    for import in &file.imports {
        let resolved = match resolve_import_or_workspace(
            &import.source,
            &file.path,
            known_paths,
            workspace_map,
        ) {
            Some(p) => p,
            None => continue,
        };

        if import.is_namespace {
            // `import * as X from './mod'` or Python `import X`
            // Map X → resolved module path.
            for name in &import.names {
                let local_name = name.alias.as_ref().unwrap_or(&name.name);
                map.insert(local_name.clone(), resolved.clone());
            }
            continue;
        }

        for name in &import.names {
            let local_name = name.alias.as_ref().unwrap_or(&name.name);
            // Try to resolve to a specific symbol in the target file.
            let target_sym_id = format!("{}::{}", resolved, name.name);

            // Check if this symbol exists in the target file's definitions.
            let target_file = all_files.iter().find(|f| f.path == resolved);
            if let Some(tf) = target_file {
                if tf.definitions.iter().any(|d| d.name == name.name) {
                    map.insert(local_name.clone(), target_sym_id);
                    continue;
                }
            }

            // Default import — map to module.
            if import.is_default {
                map.insert(local_name.clone(), resolved.clone());
            } else {
                // Map to the symbol id even if we can't verify it exists.
                map.insert(local_name.clone(), target_sym_id);
            }
        }
    }

    map
}

/// Collect extends edge descriptors for class inheritance (Python).
/// Currently a stub — ParsedFile lacks class base info, so no edges are emitted.
fn collect_extends_edges(
    file: &ParsedFile,
    _all_files: &[ParsedFile],
    _file_defs: &HashMap<String, Vec<Definition>>,
    _id_to_index: &HashMap<String, NodeIndex>,
    _edges: &mut Vec<(String, String, EdgeType)>,
) {
    if file.language != Language::Python {
        return;
    }
    // ParsedFile doesn't store class base info, so no extends edges can be produced.
    // The IR path (build_from_ir → collect_ir_extends_edges) handles this via IrTypeDef.bases.
}

// ---------------------------------------------------------------------------
// IR-based lookup helpers
// ---------------------------------------------------------------------------

/// Map from file path to its IR exports.
fn build_ir_export_map(files: &[IrFile]) -> HashMap<String, Vec<IrExport>> {
    files
        .iter()
        .map(|f| (f.path.clone(), f.exports.clone()))
        .collect()
}

/// Map from file path to (name, kind) pairs for all definitions.
fn build_ir_def_names_map(files: &[IrFile]) -> HashMap<String, Vec<(String, SymbolKind)>> {
    files
        .iter()
        .map(|f| {
            let mut defs = Vec::new();
            for func in &f.functions {
                defs.push((func.name.clone(), SymbolKind::Function));
            }
            for td in &f.type_defs {
                let kind = match td.kind {
                    TypeDefKind::Class => SymbolKind::Class,
                    TypeDefKind::Struct => SymbolKind::Struct,
                    TypeDefKind::Interface => SymbolKind::Interface,
                    TypeDefKind::TypeAlias => SymbolKind::TypeAlias,
                    TypeDefKind::Enum => SymbolKind::Class,
                };
                defs.push((td.name.clone(), kind));
            }
            for c in &f.constants {
                defs.push((c.name.clone(), SymbolKind::Constant));
            }
            (f.path.clone(), defs)
        })
        .collect()
}

/// Collect import edge descriptors from IR imports.
fn collect_ir_import_edges(
    file: &IrFile,
    file_exports: &HashMap<String, Vec<IrExport>>,
    file_defs: &HashMap<String, Vec<(String, SymbolKind)>>,
    id_to_index: &HashMap<String, NodeIndex>,
    known_paths: &[&str],
    workspace_map: &WorkspaceMap,
    edges: &mut Vec<(String, String, EdgeType)>,
) {
    if !id_to_index.contains_key(&file.path) {
        return;
    }
    let from_id = file.path.clone();

    for import in &file.imports {
        let resolved = match resolve_import_or_workspace(
            &import.source,
            &file.path,
            known_paths,
            workspace_map,
        ) {
            Some(p) => p,
            None => continue,
        };

        // Check if this import is side-effect only.
        let is_side_effect = import.specifiers.is_empty()
            || import
                .specifiers
                .iter()
                .all(|s| matches!(s, IrImportSpecifier::SideEffect));

        if is_side_effect {
            if id_to_index.contains_key(&resolved) {
                edges.push((from_id.clone(), resolved.clone(), EdgeType::Imports));
            }
            continue;
        }

        for spec in &import.specifiers {
            match spec {
                IrImportSpecifier::Named { name, .. } => {
                    let target_sym_id = format!("{}::{}", resolved, name);
                    if id_to_index.contains_key(&target_sym_id) {
                        edges.push((from_id.clone(), target_sym_id, EdgeType::Imports));
                        continue;
                    }

                    // Check re-exports.
                    if let Some(exports) = file_exports.get(&resolved) {
                        for export in exports {
                            if export.name == *name && export.is_reexport {
                                if let Some(ref reexport_source) = export.source {
                                    if let Some(reexport_resolved) = resolve_import_or_workspace(
                                        reexport_source,
                                        &resolved,
                                        known_paths,
                                        workspace_map,
                                    ) {
                                        let reexport_sym_id =
                                            format!("{}::{}", reexport_resolved, name);
                                        if id_to_index.contains_key(&reexport_sym_id) {
                                            edges.push((
                                                from_id.clone(),
                                                reexport_sym_id,
                                                EdgeType::Imports,
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Fallback: definition name matches directly.
                    if let Some(defs) = file_defs.get(&resolved) {
                        if defs.iter().any(|(n, _): &(String, SymbolKind)| n == name) {
                            let sym_id = format!("{}::{}", resolved, name);
                            if id_to_index.contains_key(&sym_id) {
                                edges.push((from_id.clone(), sym_id, EdgeType::Imports));
                            }
                        }
                    }
                }
                IrImportSpecifier::Default(_) | IrImportSpecifier::Namespace(_) => {
                    if id_to_index.contains_key(&resolved) {
                        edges.push((from_id.clone(), resolved.clone(), EdgeType::Imports));
                    }
                }
                IrImportSpecifier::SideEffect => {
                    // Already handled above.
                }
            }
        }
    }
}

/// Build import resolution map from IR imports for call edge resolution.
fn build_ir_import_resolution_map(
    file: &IrFile,
    all_files: &[IrFile],
    _file_defs: &HashMap<String, Vec<(String, SymbolKind)>>,
    known_paths: &[&str],
    workspace_map: &WorkspaceMap,
) -> HashMap<String, String> {
    let mut map = HashMap::new();

    for import in &file.imports {
        let resolved = match resolve_import_or_workspace(
            &import.source,
            &file.path,
            known_paths,
            workspace_map,
        ) {
            Some(p) => p,
            None => continue,
        };

        for spec in &import.specifiers {
            match spec {
                IrImportSpecifier::Namespace(local) => {
                    map.insert(local.clone(), resolved.clone());
                }
                IrImportSpecifier::Named { name, alias } => {
                    let local_name = alias.as_deref().unwrap_or(name.as_str());
                    let target_sym_id = format!("{}::{}", resolved, name);

                    // Check if this symbol exists in the target file.
                    let target_file = all_files.iter().find(|f| f.path == resolved);
                    if let Some(tf) = target_file {
                        let has_def = tf.functions.iter().any(|d| d.name == *name)
                            || tf.type_defs.iter().any(|d| d.name == *name)
                            || tf.constants.iter().any(|d| d.name == *name);
                        if has_def {
                            map.insert(local_name.to_string(), target_sym_id);
                            continue;
                        }
                    }

                    // Map to the symbol id even if we can't verify.
                    map.insert(local_name.to_string(), target_sym_id);
                }
                IrImportSpecifier::Default(local) => {
                    map.insert(local.clone(), resolved.clone());
                }
                IrImportSpecifier::SideEffect => {}
            }
        }
    }

    map
}

/// Collect call edge descriptors from IR call expressions.
fn collect_ir_call_edges(
    file: &IrFile,
    all_files: &[IrFile],
    file_defs: &HashMap<String, Vec<(String, SymbolKind)>>,
    id_to_index: &HashMap<String, NodeIndex>,
    known_paths: &[&str],
    workspace_map: &WorkspaceMap,
    edges: &mut Vec<(String, String, EdgeType)>,
) {
    let import_map =
        build_ir_import_resolution_map(file, all_files, file_defs, known_paths, workspace_map);

    for call in &file.call_expressions {
        let caller_id = match &call.containing_function {
            Some(func_name) => format!("{}::{}", file.path, func_name),
            None => file.path.clone(),
        };

        // Resolve caller: try exact id, then module node.
        let resolved_caller_id = if id_to_index.contains_key(&caller_id) {
            caller_id
        } else if id_to_index.contains_key(&file.path) {
            file.path.clone()
        } else {
            continue;
        };

        let callee_name = &call.callee;

        // Simple name — look up in import map or local defs.
        if let Some(target_id) = import_map.get(callee_name.as_str()) {
            if id_to_index.contains_key(target_id.as_str()) && resolved_caller_id != *target_id {
                edges.push((
                    resolved_caller_id.clone(),
                    target_id.clone(),
                    EdgeType::Calls,
                ));
            }
            continue;
        }

        // Method call (e.g., `db.save`).
        if let Some(dot_pos) = callee_name.find('.') {
            let receiver = &callee_name[..dot_pos];
            if let Some(target_module) = import_map.get(receiver) {
                let method = &callee_name[dot_pos + 1..];
                let method_id = format!("{}::{}", target_module.trim_end_matches("::*"), method);
                if id_to_index.contains_key(&method_id) && resolved_caller_id != method_id {
                    edges.push((resolved_caller_id.clone(), method_id, EdgeType::Calls));
                    continue;
                }
                if id_to_index.contains_key(target_module.as_str())
                    && resolved_caller_id != *target_module
                {
                    edges.push((
                        resolved_caller_id.clone(),
                        target_module.clone(),
                        EdgeType::Calls,
                    ));
                    continue;
                }
            }
        }

        // Local function call.
        let local_id = format!("{}::{}", file.path, callee_name);
        if id_to_index.contains_key(&local_id) && resolved_caller_id != local_id {
            edges.push((resolved_caller_id.clone(), local_id, EdgeType::Calls));
        }
    }
}

/// Collect extends edge descriptors from IR type definitions with bases.
///
/// Unlike the ParsedFile-based version which cannot determine class bases,
/// the IR path has `IrTypeDef.bases` populated from the query engine, enabling
/// real extends edge construction.
fn collect_ir_extends_edges(
    file: &IrFile,
    all_files: &[IrFile],
    id_to_index: &HashMap<String, NodeIndex>,
    known_paths: &[&str],
    edges: &mut Vec<(String, String, EdgeType)>,
) {
    let import_map = build_ir_import_resolution_map(
        file,
        all_files,
        &HashMap::new(),
        known_paths,
        &WorkspaceMap::new(),
    );

    for td in &file.type_defs {
        if td.bases.is_empty() {
            continue;
        }

        let child_id = format!("{}::{}", file.path, td.name);
        if !id_to_index.contains_key(&child_id) {
            continue;
        }

        for base in &td.bases {
            // Try imported name first.
            if let Some(target_id) = import_map.get(base.as_str()) {
                if id_to_index.contains_key(target_id.as_str()) && child_id != *target_id {
                    edges.push((child_id.clone(), target_id.clone(), EdgeType::Extends));
                    continue;
                }
            }

            // Try local definition.
            let local_id = format!("{}::{}", file.path, base);
            if id_to_index.contains_key(&local_id) && child_id != local_id {
                edges.push((child_id.clone(), local_id, EdgeType::Extends));
            }
        }
    }
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

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr
)]
mod tests_ir;
