//! Tauri IPC commands — bridge between the React frontend and diffcore-core.
//!
//! Each `#[tauri::command]` function is callable from the frontend via `invoke()`.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use log::warn;

use crate::activity_stream::{self, JobHandle};

use diffcore_core::cache;
use diffcore_core::cluster;
use diffcore_core::config::DiffcoreConfig;
use diffcore_core::entrypoint;
use diffcore_core::flow::{self, FlowConfig};
use diffcore_core::git;
use diffcore_core::graph::SymbolGraph;
use diffcore_core::llm::BackendStatus;
use diffcore_core::output::{self, build_analysis_output};
use diffcore_core::pipeline;
use diffcore_core::query_engine::QueryEngine;
use diffcore_core::rank;
use diffcore_core::types::{AnalysisOutput, GroupRankInput};

async fn run_blocking<T>(
    f: impl FnOnce() -> Result<T, CommandError> + Send + 'static,
) -> Result<T, CommandError>
where
    T: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| CommandError::Analysis(format!("task join error: {}", e)))?
}

/// Application state shared across commands.
pub struct AppState {
    /// The most recent analysis result, available for subsequent queries.
    pub last_analysis: Mutex<Option<AnalysisOutput>>,
    /// Cached diff result from the most recent analysis, for instant file diff lookups.
    pub last_diff: Mutex<Option<CachedDiff>>,
    /// Background LLM job manager for live SSE activity streams.
    pub activity_manager: Arc<activity_stream::ActivityManager>,
    /// Base URL for the embedded localhost SSE server.
    pub activity_stream_base_url: Mutex<Option<String>>,
    /// Cache key from the most recent analysis, for refinement cache lookups.
    pub last_cache_key: Mutex<Option<String>>,
    /// Path to the currently watched manifest file.
    pub watched_manifest_path: Mutex<Option<PathBuf>>,
    /// Long-lived QueryEngine instance for on-demand single-file parsing
    /// (e.g. the source-explorer outline). Uses internal `OnceCell`s to
    /// cache compiled tree-sitter queries per language across calls, so
    /// the first parse of any given language pays the compilation cost
    /// once for the whole app lifetime.
    pub query_engine: Arc<QueryEngine>,
}

/// Cached diff result with the parameters that produced it, for cache invalidation.
pub struct CachedDiff {
    pub repo_path: PathBuf,
    pub base: Option<String>,
    pub diff_result: git::DiffResult,
}

impl AppState {
    pub fn new() -> Self {
        // Construct the QueryEngine eagerly so the field is non-Optional.
        // QueryEngine::new() itself is cheap — per-language tree-sitter
        // query compilation is deferred to the first parse of each
        // language via internal OnceCells. We fall back to a fresh
        // construction on error rather than panicking at startup; in
        // practice QueryEngine::new() is infallible today, but the
        // Result return type leaves room for future configuration loading.
        let query_engine = Arc::new(QueryEngine::new().unwrap_or_else(|e| {
            log::error!("QueryEngine construction failed at startup: {e}");
            // Re-attempt; if this also fails the app cannot parse files
            // but other commands continue to work, so we panic only as
            // a last resort. (Today new() can't actually fail.)
            QueryEngine::new().expect("QueryEngine::new() failed twice")
        }));
        Self {
            last_analysis: Mutex::new(None),
            last_diff: Mutex::new(None),
            activity_manager: Arc::new(activity_stream::ActivityManager::new()),
            activity_stream_base_url: Mutex::new(None),
            last_cache_key: Mutex::new(None),
            watched_manifest_path: Mutex::new(None),
            query_engine,
        }
    }

    pub fn init_activity_stream(&self) -> Result<(), CommandError> {
        let mut base_url = self
            .activity_stream_base_url
            .lock()
            .map_err(|e| CommandError::Analysis(format!("Lock poisoned: {}", e)))?;
        if base_url.is_none() {
            *base_url = Some(
                activity_stream::spawn_sse_server(Arc::clone(&self.activity_manager))
                    .map_err(|e| CommandError::Io(format!("Failed to start SSE server: {}", e)))?,
            );
        }
        Ok(())
    }

    pub fn create_llm_job(
        &self,
        operation: &str,
        provider: &str,
        model: &str,
        title: &str,
    ) -> Result<(JobHandle, AsyncLlmJobStart), CommandError> {
        self.init_activity_stream()?;
        let base_url = self
            .activity_stream_base_url
            .lock()
            .map_err(|e| CommandError::Analysis(format!("Lock poisoned: {}", e)))?
            .clone()
            .ok_or_else(|| {
                CommandError::Io("Activity SSE server was not initialized".to_string())
            })?;

        let manager = Arc::clone(&self.activity_manager);
        let operation = operation.to_string();
        let provider = provider.to_string();
        let model = model.to_string();
        let title = title.to_string();
        let job_operation = operation.clone();
        let job_provider = provider.clone();
        let job_model = model.clone();
        let job_title = title.clone();

        let handle = tauri::async_runtime::block_on(async move {
            manager
                .create_job(job_operation, job_provider, job_model, job_title)
                .await
        });

        let start = AsyncLlmJobStart {
            job_id: handle.job_id().to_string(),
            stream_url: format!("{}/llm/jobs/{}/events", base_url, handle.job_id()),
            operation,
            provider,
            model,
            title,
        };

        Ok((handle, start))
    }
}

/// Error type for Tauri commands — must implement `Into<tauri::InvokeError>`.
#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("Git error: {0}")]
    Git(String),
    #[error("Analysis error: {0}")]
    Analysis(String),
    #[error("Config error: {0}")]
    Config(String),
    #[error("IO error: {0}")]
    Io(String),
    #[error("LLM error: {0}")]
    Llm(String),
    #[error("Network error: {0}")]
    Network(String),
}

impl serde::Serialize for CommandError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

/// Analyze a git diff and return semantic flow groups.
///
/// This is the primary IPC command — equivalent to `diffcore analyze` in the CLI.
/// When `pr_preview` is true, uses merge-base diff (shows what the branch introduces
/// relative to where it diverged from the base).
#[tauri::command]
pub async fn analyze(
    repo_path: String,
    base: Option<String>,
    head: Option<String>,
    range: Option<String>,
    staged: bool,
    unstaged: bool,
    pr_preview: Option<bool>,
    include_uncommitted: Option<bool>,
    state: tauri::State<'_, AppState>,
) -> Result<AnalysisOutput, CommandError> {
    #[derive(Clone)]
    struct AnalyzeResult {
        repo_path: PathBuf,
        base: Option<String>,
        diff_result: git::DiffResult,
        cache_key: Option<String>,
        analysis_output: AnalysisOutput,
    }

    let state_last_diff = &state.last_diff;
    let state_last_analysis = &state.last_analysis;
    let state_last_cache_key = &state.last_cache_key;

    let res = run_blocking(move || {
        let repo_path_buf = PathBuf::from(&repo_path);
        let repo_path_buf = std::fs::canonicalize(&repo_path_buf)
            .map_err(|e| CommandError::Io(format!("Invalid repo path: {}", e)))?;

        let repo = git2::Repository::discover(&repo_path_buf)
            .map_err(|e| CommandError::Git(format!("Not a git repository: {}", e)))?;

        let workdir = repo
            .workdir()
            .ok_or_else(|| CommandError::Git("Bare repositories are not supported".to_string()))?
            .to_path_buf();

        let config = DiffcoreConfig::load_with_global_llm_from_dir(&workdir)
            .map_err(|e| CommandError::Config(format!("{}", e)))?;

        let effective_include_uncommitted =
            include_uncommitted.unwrap_or(config.diff.include_uncommitted);

        let (diff_result, diff_source) = extract_diff(
            &repo,
            base.clone(),
            head,
            range,
            staged,
            unstaged,
            pr_preview.unwrap_or(false),
            effective_include_uncommitted,
        )?;

        if diff_result.files.is_empty() {
            let empty_output = AnalysisOutput {
                version: "1.0.0".to_string(),
                diff_source: diff_source.clone(),
                summary: diffcore_core::types::AnalysisSummary {
                    total_files_changed: 0,
                    total_groups: 0,
                    languages_detected: vec![],
                    frameworks_detected: vec![],
                },
                groups: vec![],
                infrastructure_group: None,
                annotations: None,
            };
            return Ok(AnalyzeResult {
                repo_path: repo_path_buf,
                base,
                diff_result,
                cache_key: None,
                analysis_output: empty_output,
            });
        }

        let cache_key = if staged || unstaged {
            cache::compute_cache_key_working_dir(&diff_result, &workdir)
        } else {
            cache::compute_cache_key(&diff_result)
        };

        if let Some(cached) = cache::load_cached(&workdir, &cache_key) {
            return Ok(AnalyzeResult {
                repo_path: repo_path_buf,
                base,
                diff_result,
                cache_key: Some(cache_key),
                analysis_output: cached,
            });
        }

        let file_inputs: Vec<(&str, &str)> = diff_result
            .files
            .iter()
            .filter_map(|file_diff| {
                let content = file_diff
                    .new_content
                    .as_deref()
                    .or(file_diff.old_content.as_deref())?;
                let path = file_diff.path();
                if config.is_ignored(path) {
                    return None;
                }
                Some((path, content))
            })
            .collect();
        let parsed_files = pipeline::parse_files_parallel(&file_inputs);

        let workspace_map = diffcore_core::graph::build_workspace_map(&workdir);
        let mut graph = SymbolGraph::build_with_workspace(&parsed_files, &workspace_map);
        let entrypoints = entrypoint::detect_entrypoints(&parsed_files);

        let flow_analysis = flow::analyze_data_flow(&parsed_files, &FlowConfig::default());
        flow::enrich_graph(&mut graph, &flow_analysis);

        let changed_files: Vec<String> = diff_result
            .files
            .iter()
            .filter(|f| !config.is_ignored(f.path()))
            .map(|f| f.path().to_string())
            .collect();
        let cluster_result = cluster::cluster_files(&graph, &entrypoints, &changed_files);

        let weights = config.ranking.clone();
        let rank_inputs: Vec<GroupRankInput> = cluster_result
            .groups
            .iter()
            .map(|group| {
                let risk_flags = output::compute_group_risk_flags(
                    &group
                        .files
                        .iter()
                        .map(|f| f.path.as_str())
                        .collect::<Vec<_>>(),
                );
                let total_add: u32 = group.files.iter().map(|f| f.changes.additions).sum();
                let total_del: u32 = group.files.iter().map(|f| f.changes.deletions).sum();

                GroupRankInput {
                    group_id: group.id.clone(),
                    risk: rank::compute_risk_score(
                        risk_flags.has_schema_change,
                        risk_flags.has_api_change,
                        risk_flags.has_auth_change,
                        false,
                    ),
                    centrality: 0.5,
                    surface_area: rank::compute_surface_area(total_add, total_del, 1000),
                    uncertainty: if risk_flags.has_test_only { 0.1 } else { 0.5 },
                }
            })
            .collect();

        let ranked = rank::rank_groups(&rank_inputs, &weights);

        let analysis_output = build_analysis_output(
            &diff_result,
            diff_source.clone(),
            &parsed_files,
            &cluster_result,
            &ranked,
        );

        cache::store_cached(&workdir, &cache_key, &analysis_output);

        Ok(AnalyzeResult {
            repo_path: repo_path_buf,
            base,
            diff_result,
            cache_key: Some(cache_key),
            analysis_output,
        })
    })
    .await?;

    // Cache the diff result for subsequent get_file_diff() calls
    match state_last_diff.lock() {
        Ok(mut cached) => {
            *cached = Some(CachedDiff {
                repo_path: res.repo_path.clone(),
                base: res.base.clone(),
                diff_result: res.diff_result.clone(),
            });
        }
        Err(e) => warn!("Failed to update last_diff state (lock poisoned): {}", e),
    }

    // Store analysis for subsequent queries
    match state_last_analysis.lock() {
        Ok(mut last) => *last = Some(res.analysis_output.clone()),
        Err(e) => warn!(
            "Failed to update last_analysis state (lock poisoned): {}",
            e
        ),
    }

    // Store cache key for refinement cache lookups when available
    if let Some(cache_key) = res.cache_key {
        if let Ok(mut key) = state_last_cache_key.lock() {
            *key = Some(cache_key);
        }
    }

    Ok(res.analysis_output)
}

/// Get the most recent analysis result without re-running.
#[tauri::command]
pub fn get_last_analysis(
    state: tauri::State<'_, AppState>,
) -> Result<Option<AnalysisOutput>, CommandError> {
    let last = state
        .last_analysis
        .lock()
        .map_err(|e| CommandError::Analysis(format!("Lock poisoned: {}", e)))?;
    Ok(last.clone())
}

/// Generate a Mermaid diagram for a specific group by ID.
#[tauri::command]
pub fn get_mermaid(
    group_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, CommandError> {
    let last = state
        .last_analysis
        .lock()
        .map_err(|e| CommandError::Analysis(format!("Lock poisoned: {}", e)))?;

    let analysis = last.as_ref().ok_or_else(|| {
        CommandError::Analysis("No analysis available. Run analyze first.".into())
    })?;

    let group = analysis
        .groups
        .iter()
        .find(|g| g.id == group_id)
        .ok_or_else(|| CommandError::Analysis(format!("Group '{}' not found", group_id)))?;

    Ok(output::generate_mermaid(group))
}

/// Get the diff content (old + new) for a specific file.
/// Returns the raw old and new content for the Monaco diff viewer.
/// Uses the cached DiffResult from the last `analyze()` call when parameters match,
/// avoiding redundant git diff extraction for every file navigation.
#[tauri::command]
pub async fn get_file_diff(
    repo_path: String,
    file_path: String,
    base: Option<String>,
    head: Option<String>,
    range: Option<String>,
    staged: bool,
    unstaged: bool,
    include_uncommitted: Option<bool>,
    state: tauri::State<'_, AppState>,
) -> Result<FileDiffContent, CommandError> {
    // Try to use cached diff from the last analyze() call
    let cached_file = {
        let repo_path_buf = PathBuf::from(&repo_path);
        let repo_path_buf = std::fs::canonicalize(&repo_path_buf).ok();
        let cached = state.last_diff.lock().ok();
        cached.and_then(|guard| {
            let c = guard.as_ref()?;
            let rp = repo_path_buf.as_ref()?;
            if &c.repo_path == rp && c.base == base {
                c.diff_result
                    .files
                    .iter()
                    .find(|f| f.path() == file_path)
                    .map(|f| FileDiffContent {
                        path: file_path.clone(),
                        old_content: f.old_content.clone().unwrap_or_default(),
                        new_content: f.new_content.clone().unwrap_or_default(),
                        language: detect_language(&f.path()),
                    })
            } else {
                None
            }
        })
    };

    if let Some(content) = cached_file {
        return Ok(content);
    }

    // Cache miss — fall back to extracting from git
    run_blocking(move || {
        get_file_diff_uncached(
            repo_path,
            file_path,
            base,
            head,
            range,
            staged,
            unstaged,
            include_uncommitted.unwrap_or(true),
        )
    })
    .await
}

/// Core file diff logic without caching — also callable from integration tests.
pub fn get_file_diff_uncached(
    repo_path: String,
    file_path: String,
    base: Option<String>,
    head: Option<String>,
    range: Option<String>,
    staged: bool,
    unstaged: bool,
    include_uncommitted: bool,
) -> Result<FileDiffContent, CommandError> {
    // Security: reject paths with traversal components or absolute paths
    // to prevent path traversal via IPC from a compromised frontend.
    let fp = std::path::Path::new(&file_path);
    if fp.is_absolute()
        || fp
            .components()
            .any(|c| c == std::path::Component::ParentDir)
    {
        return Err(CommandError::Io(format!(
            "Invalid file path (path traversal rejected): {}",
            file_path
        )));
    }

    let repo_path_buf = PathBuf::from(&repo_path);
    let repo_path_buf = std::fs::canonicalize(&repo_path_buf)
        .map_err(|e| CommandError::Io(format!("Invalid repo path: {}", e)))?;

    let repo = git2::Repository::discover(&repo_path_buf)
        .map_err(|e| CommandError::Git(format!("Not a git repository: {}", e)))?;

    let (diff_result, _) = extract_diff(
        &repo,
        base,
        head,
        range,
        staged,
        unstaged,
        false,
        include_uncommitted,
    )?;

    let file_diff = diff_result
        .files
        .iter()
        .find(|f| f.path() == file_path)
        .ok_or_else(|| CommandError::Analysis(format!("File '{}' not found in diff", file_path)))?;

    Ok(FileDiffContent {
        path: file_path,
        old_content: file_diff.old_content.clone().unwrap_or_default(),
        new_content: file_diff.new_content.clone().unwrap_or_default(),
        language: detect_language(&file_diff.path()),
    })
}

pub(super) fn load_cached_analysis(
    state: &tauri::State<'_, AppState>,
) -> Result<AnalysisOutput, CommandError> {
    let last = state
        .last_analysis
        .lock()
        .map_err(|e| CommandError::Analysis(format!("Lock poisoned: {}", e)))?;
    last.clone()
        .ok_or_else(|| CommandError::Analysis("No analysis available. Run analyze first.".into()))
}

pub(super) fn provider_supports_tool_activity(provider: &str) -> bool {
    matches!(provider, "codex" | "claude")
}

pub(super) fn load_config_from_path(repo_path: Option<&str>) -> (DiffcoreConfig, Option<PathBuf>) {
    if let Some(path) = repo_path {
        let repo_path = PathBuf::from(path);
        if let Ok(canonical) = std::fs::canonicalize(&repo_path) {
            if let Ok(repo) = git2::Repository::discover(&canonical) {
                if let Some(workdir) = repo.workdir() {
                    let config =
                        DiffcoreConfig::load_with_global_llm_from_dir(workdir).unwrap_or_default();
                    return (config, Some(workdir.to_path_buf()));
                }
            }
        }
    }
    (DiffcoreConfig::load_global().unwrap_or_default(), None)
}

/// Get the default model for a provider.
pub(super) fn default_model_for_provider(provider: &str) -> &str {
    match provider {
        "codex" => "default",
        "claude" => "default",
        "anthropic" => "claude-sonnet-4-6",
        "openai" => "gpt-4.1",
        "gemini" => "gemini-2.5-flash",
        "openrouter" => "anthropic/claude-sonnet-4-6",
        "github_copilot" => "gpt-4.1",
        _ => "default",
    }
}

pub(super) fn default_provider_for_machine(
    codex_status: &BackendStatus,
    claude_status: &BackendStatus,
) -> &'static str {
    if codex_status.authenticated {
        "codex"
    } else if claude_status.authenticated {
        "claude"
    } else {
        "anthropic"
    }
}

pub(super) fn preferred_provider_for_runtime(
    configured_provider: Option<&str>,
    codex_status: &BackendStatus,
    claude_status: &BackendStatus,
) -> String {
    match configured_provider {
        Some("codex") if codex_status.authenticated => "codex".to_string(),
        Some("claude") if claude_status.authenticated => "claude".to_string(),
        Some(provider) if provider_supports_tool_activity(provider) => {
            default_provider_for_machine(codex_status, claude_status).to_string()
        }
        Some(provider) => {
            if codex_status.authenticated || claude_status.authenticated {
                default_provider_for_machine(codex_status, claude_status).to_string()
            } else {
                provider.to_string()
            }
        }
        None => default_provider_for_machine(codex_status, claude_status).to_string(),
    }
}

pub(super) fn preferred_model_for_runtime(
    configured_model: Option<String>,
    configured_provider: Option<&str>,
    resolved_provider: &str,
) -> String {
    if configured_provider == Some(resolved_provider) {
        configured_model
            .unwrap_or_else(|| default_model_for_provider(resolved_provider).to_string())
    } else {
        default_model_for_provider(resolved_provider).to_string()
    }
}

/// Open a repository from a path, with canonicalization and error handling.
pub(super) fn open_repo(repo_path: &str) -> Result<git2::Repository, CommandError> {
    let path = PathBuf::from(repo_path);
    let path = std::fs::canonicalize(&path)
        .map_err(|e| CommandError::Io(format!("Invalid repo path: {}", e)))?;
    git2::Repository::discover(&path)
        .map_err(|e| CommandError::Git(format!("Not a git repository: {}", e)))
}

/// Build a simple unified diff from old and new content.
pub(super) fn simple_unified_diff(old: &str, new: &str) -> String {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();
    let mut result = String::new();
    // Simple approach: show all old lines as removed, all new lines as added
    // For a real implementation, use a proper diff algorithm
    for line in &old_lines {
        result.push_str(&format!("-{}\n", line));
    }
    for line in &new_lines {
        result.push_str(&format!("+{}\n", line));
    }
    result
}

/// File diff content for the Monaco diff viewer.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileDiffContent {
    pub path: String,
    pub old_content: String,
    pub new_content: String,
    pub language: String,
}

// ── Internal helpers ──

pub(super) fn extract_diff(
    repo: &git2::Repository,
    base: Option<String>,
    head: Option<String>,
    range: Option<String>,
    staged: bool,
    unstaged: bool,
    pr_preview: bool,
    include_uncommitted: bool,
) -> Result<(git::DiffResult, diffcore_core::types::DiffSource), CommandError> {
    if let Some(ref range) = range {
        let diff = git::diff_range(repo, range).map_err(|e| CommandError::Git(format!("{}", e)))?;
        let source =
            output::diff_source_range(range, diff.base_sha.as_deref(), diff.head_sha.as_deref());
        Ok((diff, source))
    } else if staged {
        let diff = git::diff_staged(repo).map_err(|e| CommandError::Git(format!("{}", e)))?;
        let source = output::diff_source_staged();
        Ok((diff, source))
    } else if unstaged {
        let diff = git::diff_unstaged(repo).map_err(|e| CommandError::Git(format!("{}", e)))?;
        let source = output::diff_source_unstaged();
        Ok((diff, source))
    } else if pr_preview {
        // PR preview mode: use merge-base diff
        // Auto-detect default branch if no base ref provided
        let detected_default = if base.is_none() {
            git::detect_default_branch(repo).ok()
        } else {
            None
        };
        let base_ref = base
            .as_deref()
            .or(detected_default.as_deref())
            .unwrap_or("main");
        let head_ref = head.as_deref().unwrap_or("HEAD");
        if include_uncommitted {
            let diff = git::diff_merge_base_to_workdir(repo, base_ref, head_ref).map_err(|e| {
                CommandError::Git(format!(
                    "Failed to compute merge-base-to-workdir diff between '{}' and '{}': {}",
                    base_ref, head_ref, e
                ))
            })?;
            let source =
                output::diff_source_branch_with_worktree(base_ref, diff.base_sha.as_deref());
            Ok((diff, source))
        } else {
            let selected = git::diff_merge_base_with_worktree_fallback(repo, base_ref, head_ref)
                .map_err(|e| {
                    CommandError::Git(format!(
                        "Failed to compute merge-base diff between '{}' and '{}': {}",
                        base_ref, head_ref, e
                    ))
                })?;
            let source = if selected.used_worktree_fallback {
                output::diff_source_worktree(
                    Some(base_ref),
                    Some(head_ref),
                    selected.comparison_base_sha.as_deref(),
                    selected.comparison_head_sha.as_deref(),
                )
            } else {
                output::diff_source_branch(
                    base_ref,
                    head_ref,
                    selected.diff.base_sha.as_deref(),
                    selected.diff.head_sha.as_deref(),
                )
            };
            Ok((selected.diff, source))
        }
    } else {
        let base_ref = base.as_deref().unwrap_or("main");
        let head_ref = head.as_deref().unwrap_or("HEAD");
        if include_uncommitted {
            let diff = git::diff_branch_to_workdir(repo, base_ref)
                .map_err(|e| CommandError::Git(format!("{}", e)))?;
            let source =
                output::diff_source_branch_with_worktree(base_ref, diff.base_sha.as_deref());
            Ok((diff, source))
        } else {
            let selected = git::diff_refs_with_worktree_fallback(repo, base_ref, head_ref)
                .map_err(|e| CommandError::Git(format!("{}", e)))?;
            let source = if selected.used_worktree_fallback {
                output::diff_source_worktree(
                    Some(base_ref),
                    Some(head_ref),
                    selected.comparison_base_sha.as_deref(),
                    selected.comparison_head_sha.as_deref(),
                )
            } else {
                output::diff_source_branch(
                    base_ref,
                    head_ref,
                    selected.diff.base_sha.as_deref(),
                    selected.diff.head_sha.as_deref(),
                )
            };
            Ok((selected.diff, source))
        }
    }
}

pub(super) fn detect_language(path: &str) -> String {
    match path.rsplit('.').next() {
        // ── Core 13 ─────────────────────────────────────────────
        Some("ts" | "tsx") => "typescript".to_string(),
        Some("js" | "jsx" | "mjs" | "cjs") => "javascript".to_string(),
        Some("py" | "pyi") => "python".to_string(),
        Some("go") => "go".to_string(),
        Some("rs") => "rust".to_string(),
        Some("java") => "java".to_string(),
        Some("cs") => "csharp".to_string(),
        Some("php") => "php".to_string(),
        Some("rb") => "ruby".to_string(),
        Some("kt" | "kts") => "kotlin".to_string(),
        Some("swift") => "swift".to_string(),
        Some("c" | "h") => "c".to_string(),
        Some("cpp" | "cc" | "cxx" | "c++" | "hpp" | "hxx" | "h++" | "hh") => "cpp".to_string(),
        Some("scala" | "sc") => "scala".to_string(),
        // ── Extras (matching the lang-* Cargo features) ─────────
        Some("sh" | "bash" | "zsh") => "shell".to_string(),
        Some("hs" | "lhs") => "haskell".to_string(),
        Some("nix") => "nix".to_string(),
        Some("lua") => "lua".to_string(),
        Some("pl" | "pm" | "perl") => "perl".to_string(),
        Some("ex" | "exs") => "elixir".to_string(),
        Some("erl" | "hrl") => "erlang".to_string(),
        Some("zig" | "zon") => "zig".to_string(),
        Some("ml" | "mli") => "ocaml".to_string(),
        Some("jl") => "julia".to_string(),
        Some("dart") => "dart".to_string(),
        Some("r" | "R") => "r".to_string(),
        Some("fish") => "fish".to_string(),
        Some("html" | "htm") => "html".to_string(),
        Some("css") => "css".to_string(),
        Some("scss" | "sass") => "scss".to_string(),
        Some("vue") => "vue".to_string(),
        Some("svelte") => "svelte".to_string(),
        Some("graphql" | "gql") => "graphql".to_string(),
        // ── Data formats ────────────────────────────────────────
        Some("json") => "json".to_string(),
        Some("toml") => "toml".to_string(),
        Some("yaml" | "yml") => "yaml".to_string(),
        Some("md" | "markdown") => "markdown".to_string(),
        Some("sql") => "sql".to_string(),
        Some("prisma") => "prisma".to_string(),
        _ => "plaintext".to_string(),
    }
}

// ── Submodules ──────────────────────────────────────────────────────────────

pub mod app_state;
pub mod comments;
pub mod editor;
pub mod llm;
pub mod manifest;
pub mod settings;
pub mod workspace;

// Re-export public command items for internal tests and compatibility with
// older call sites that import from `commands::*`.
#[allow(unused_imports)]
pub use app_state::{load_last_app_state, save_app_state};
#[allow(unused_imports)]
pub use comments::{
    comment_cache_key, delete_comment, delete_comment_cached, export_comments, load_comments,
    load_comments_cached, save_comment, save_comment_cached, update_comment_cached, CommentsFile,
    ReviewComment,
};
#[allow(unused_imports)]
pub use editor::{check_editors_available, open_in_editor, save_file_content};
#[allow(unused_imports)]
pub use llm::{
    annotate_group, annotate_overview, get_cached_refinement, refine_groups, start_annotate_group,
    start_annotate_overview, start_refine_groups, store_refinement_cache, AsyncLlmJobStart,
    RefinementResult,
};
#[allow(unused_imports)]
pub use manifest::{
    export_groups_manifest, import_groups_manifest, unwatch_manifest, watch_manifest,
};
#[allow(unused_imports)]
pub use settings::{
    check_api_key, clear_api_key, fetch_provider_models, get_ignore_paths, get_llm_settings,
    save_api_key, save_ignore_paths, save_llm_settings, LlmSettings,
};
#[allow(unused_imports)]
pub use workspace::{
    cross_file_search, get_branch_status, get_last_diff_file_statuses, get_launch_directory,
    get_repo_info, get_workspace_file_content, list_branches, list_commits, list_repo_path_suggestions,
    list_worktrees,
    parse_file_content, CrossFileSearchMatch, CrossFileSearchResult, FileShortStatus, RepoInfo,
};

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr
)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_language_typescript() {
        assert_eq!(detect_language("src/app.ts"), "typescript");
        assert_eq!(detect_language("src/App.tsx"), "typescript");
    }

    #[test]
    fn test_detect_language_javascript() {
        assert_eq!(detect_language("index.js"), "javascript");
        assert_eq!(detect_language("App.jsx"), "javascript");
    }

    #[test]
    fn test_detect_language_python() {
        assert_eq!(detect_language("main.py"), "python");
    }

    #[test]
    fn test_detect_language_rust() {
        assert_eq!(detect_language("lib.rs"), "rust");
    }

    #[test]
    fn test_detect_language_json() {
        assert_eq!(detect_language("package.json"), "json");
    }

    #[test]
    fn test_detect_language_unknown() {
        assert_eq!(detect_language("Makefile"), "plaintext");
        assert_eq!(detect_language("noext"), "plaintext");
    }

    #[test]
    fn test_detect_language_yaml() {
        assert_eq!(detect_language("config.yaml"), "yaml");
        assert_eq!(detect_language("ci.yml"), "yaml");
    }

    #[test]
    fn test_detect_language_shell() {
        assert_eq!(detect_language("run.sh"), "shell");
        assert_eq!(detect_language("init.bash"), "shell");
    }

    #[test]
    fn test_detect_language_various() {
        assert_eq!(detect_language("main.go"), "go");
        assert_eq!(detect_language("App.java"), "java");
        assert_eq!(detect_language("app.rb"), "ruby");
        assert_eq!(detect_language("schema.prisma"), "prisma");
        assert_eq!(detect_language("query.sql"), "sql");
        assert_eq!(detect_language("style.css"), "css");
        assert_eq!(detect_language("page.html"), "html");
        assert_eq!(detect_language("README.md"), "markdown");
        assert_eq!(detect_language("config.toml"), "toml");
    }

    #[test]
    fn test_app_state_new() {
        let state = AppState::new();
        let last = state.last_analysis.lock().unwrap();
        assert!(last.is_none());
    }

    #[test]
    fn test_command_error_display() {
        let err = CommandError::Git("not found".to_string());
        assert_eq!(err.to_string(), "Git error: not found");

        let err = CommandError::Analysis("no data".to_string());
        assert_eq!(err.to_string(), "Analysis error: no data");

        let err = CommandError::Config("invalid".to_string());
        assert_eq!(err.to_string(), "Config error: invalid");

        let err = CommandError::Io("permission denied".to_string());
        assert_eq!(err.to_string(), "IO error: permission denied");

        let err = CommandError::Llm("no api key".to_string());
        assert_eq!(err.to_string(), "LLM error: no api key");
    }

    #[test]
    fn test_command_error_serialize() {
        let err = CommandError::Git("test error".to_string());
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(json, "\"Git error: test error\"");

        let err = CommandError::Llm("rate limited".to_string());
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(json, "\"LLM error: rate limited\"");
    }

    #[test]
    fn test_simple_unified_diff_basic() {
        let diff = simple_unified_diff("old line", "new line");
        assert!(diff.contains("-old line"));
        assert!(diff.contains("+new line"));
    }

    #[test]
    fn test_simple_unified_diff_empty() {
        let diff = simple_unified_diff("", "");
        assert!(diff.is_empty());
    }

    #[test]
    fn test_simple_unified_diff_multiline() {
        let diff = simple_unified_diff("a\nb", "c\nd\ne");
        assert!(diff.contains("-a\n"));
        assert!(diff.contains("-b\n"));
        assert!(diff.contains("+c\n"));
        assert!(diff.contains("+d\n"));
        assert!(diff.contains("+e\n"));
    }

    #[test]
    fn test_repo_info_serde_roundtrip() {
        let info = RepoInfo {
            current_branch: Some("feature-branch".to_string()),
            default_branch: "main".to_string(),
            branches: vec![
                git::BranchInfo {
                    name: "main".to_string(),
                    is_current: false,
                    has_upstream: true,
                },
                git::BranchInfo {
                    name: "feature-branch".to_string(),
                    is_current: true,
                    has_upstream: false,
                },
            ],
            worktrees: vec![git::WorktreeInfo {
                path: "/tmp/repo".to_string(),
                branch: Some("main".to_string()),
                is_main: true,
            }],
            status: Some(git::BranchStatus {
                branch: "feature-branch".to_string(),
                upstream: None,
                ahead: 0,
                behind: 0,
            }),
            is_worktree: false,
        };
        let json = serde_json::to_string(&info).unwrap();
        let back: RepoInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back.current_branch, Some("feature-branch".to_string()));
        assert_eq!(back.default_branch, "main");
        assert_eq!(back.branches.len(), 2);
        assert_eq!(back.worktrees.len(), 1);
        assert!(back.status.is_some());
    }

    #[test]
    fn test_repo_info_no_status() {
        let info = RepoInfo {
            current_branch: None,
            default_branch: "main".to_string(),
            branches: vec![],
            worktrees: vec![],
            status: None,
            is_worktree: false,
        };
        let json = serde_json::to_string(&info).unwrap();
        let back: RepoInfo = serde_json::from_str(&json).unwrap();
        assert!(back.current_branch.is_none());
        assert!(back.status.is_none());
    }

    #[test]
    fn test_check_api_key_no_repo() {
        // Without any env vars or config, should return false (no key configured)
        // Note: this test may pass or fail depending on whether env vars are set,
        // but it should never panic.
        let result = check_api_key(None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_api_key_invalid_path() {
        // Invalid path should not panic, should return Ok(bool)
        let result = check_api_key(Some("/nonexistent/path/to/repo".to_string()));
        assert!(result.is_ok());
    }

    #[test]
    fn test_llm_settings_serde_roundtrip() {
        let settings = LlmSettings {
            annotations_enabled: true,
            refinement_enabled: false,
            provider: "codex".to_string(),
            model: "default".to_string(),
            api_key_source: "Codex CLI login".to_string(),
            has_api_key: true,
            refinement_provider: "claude".to_string(),
            refinement_model: "default".to_string(),
            refinement_max_iterations: 2,
            global_config_path: "~/.diffcore/config.toml".to_string(),
            codex_available: true,
            codex_authenticated: true,
            claude_available: true,
            claude_authenticated: true,
            include_uncommitted: true,
        };
        let json = serde_json::to_string(&settings).unwrap();
        let back: LlmSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.provider, "codex");
        assert_eq!(back.model, "default");
        assert!(back.annotations_enabled);
        assert!(!back.refinement_enabled);
        assert!(back.has_api_key);
        assert_eq!(back.refinement_provider, "claude");
        assert_eq!(back.refinement_model, "default");
        assert_eq!(back.refinement_max_iterations, 2);
        assert!(back.codex_available);
        assert!(back.claude_authenticated);
    }

    #[test]
    fn test_llm_settings_all_providers() {
        for provider in &["codex", "claude", "anthropic", "openai", "gemini"] {
            let expected = default_model_for_provider(provider);
            assert!(
                !expected.is_empty(),
                "Provider '{}' should have a default model",
                provider
            );
        }
    }

    #[test]
    fn test_default_model_for_provider() {
        assert_eq!(default_model_for_provider("codex"), "default");
        assert_eq!(default_model_for_provider("claude"), "default");
        assert_eq!(default_model_for_provider("anthropic"), "claude-sonnet-4-6");
        assert_eq!(default_model_for_provider("openai"), "gpt-4.1");
        assert_eq!(default_model_for_provider("gemini"), "gemini-2.5-flash");
        assert_eq!(default_model_for_provider("unknown"), "default");
    }

    #[test]
    fn test_preferred_provider_for_runtime_prefers_authenticated_codex_over_direct_api() {
        let codex = BackendStatus {
            installed: true,
            authenticated: true,
        };
        let claude = BackendStatus {
            installed: true,
            authenticated: false,
        };

        assert_eq!(
            preferred_provider_for_runtime(Some("openai"), &codex, &claude),
            "codex"
        );
        assert_eq!(
            preferred_provider_for_runtime(Some("anthropic"), &codex, &claude),
            "codex"
        );
    }

    #[test]
    fn test_preferred_model_for_runtime_resets_to_provider_default_when_backend_changes() {
        assert_eq!(
            preferred_model_for_runtime(Some("gpt-5.4".to_string()), Some("openai"), "codex"),
            "default"
        );
        assert_eq!(
            preferred_model_for_runtime(Some("default".to_string()), Some("codex"), "codex"),
            "default"
        );
    }

    #[test]
    fn test_get_llm_settings_no_repo() {
        let result = get_llm_settings(None);
        assert!(result.is_ok());
        let settings = result.unwrap();
        assert!(!settings.provider.is_empty());
        assert!(!settings.model.is_empty());
        assert!(!settings.global_config_path.is_empty());
    }

    #[test]
    fn test_get_llm_settings_invalid_path() {
        let result = get_llm_settings(Some("/nonexistent/path".to_string()));
        assert!(result.is_ok());
        let settings = result.unwrap();
        assert!(!settings.provider.is_empty());
    }

    #[test]
    fn test_load_config_from_path_none() {
        let (_config, workdir) = load_config_from_path(None);
        assert!(workdir.is_none());
    }

    #[test]
    fn test_load_config_from_path_invalid() {
        let (_config, workdir) = load_config_from_path(Some("/nonexistent/path"));
        assert!(workdir.is_none());
    }

    #[test]
    fn test_refinement_result_serde_roundtrip() {
        use diffcore_core::llm::schema::RefinementResponse;

        let result = RefinementResult {
            refined_groups: vec![],
            infrastructure_group: None,
            refinement_response: RefinementResponse {
                splits: vec![],
                merges: vec![],
                re_ranks: vec![],
                reclassifications: vec![],
                reasoning: "No changes needed".to_string(),
            },
            provider: "anthropic".to_string(),
            model: "claude-sonnet-4-6".to_string(),
            had_changes: false,
            warnings: Vec::new(),
            stop_reason: diffcore_core::llm::refinement::RefinementIterationStopReason::NoOp,
            attempts_used: 1,
            parse_failures: 0,
        };
        let json = serde_json::to_string(&result).unwrap();
        let back: RefinementResult = serde_json::from_str(&json).unwrap();
        assert_eq!(back.provider, "anthropic");
        assert_eq!(back.model, "claude-sonnet-4-6");
        assert!(!back.had_changes);
        assert!(back.refined_groups.is_empty());
        assert!(back.infrastructure_group.is_none());
        assert!(back.warnings.is_empty());
        assert_eq!(back.attempts_used, 1);
        assert_eq!(back.parse_failures, 0);
    }

    #[test]
    fn test_refinement_result_with_changes() {
        use diffcore_core::llm::schema::{RefinementNewGroup, RefinementResponse, RefinementSplit};
        use diffcore_core::types::{ChangeStats, FileChange, FileRole, FlowGroup};

        let result = RefinementResult {
            refined_groups: vec![FlowGroup {
                id: "g1".to_string(),
                name: "Refined group".to_string(),
                entrypoint: None,
                files: vec![FileChange {
                    path: "test.ts".to_string(),
                    flow_position: 0,
                    role: FileRole::Entrypoint,
                    changes: ChangeStats {
                        additions: 10,
                        deletions: 5,
                    },
                    symbols_changed: vec![],
                }],
                edges: vec![],
                risk_score: 0.5,
                review_order: 1,
            }],
            infrastructure_group: None,
            refinement_response: RefinementResponse {
                splits: vec![RefinementSplit {
                    source_group_id: "g1".to_string(),
                    new_groups: vec![RefinementNewGroup {
                        name: "Sub A".to_string(),
                        files: vec!["test.ts".to_string()],
                    }],
                    reason: "test split".to_string(),
                }],
                merges: vec![],
                re_ranks: vec![],
                reclassifications: vec![],
                reasoning: "Split for clarity".to_string(),
            },
            provider: "openai".to_string(),
            model: "gpt-4.1".to_string(),
            had_changes: true,
            warnings: Vec::new(),
            stop_reason:
                diffcore_core::llm::refinement::RefinementIterationStopReason::MaxIterationsReached,
            attempts_used: 2,
            parse_failures: 1,
        };
        let json = serde_json::to_string(&result).unwrap();
        let back: RefinementResult = serde_json::from_str(&json).unwrap();
        assert!(back.had_changes);
        assert_eq!(back.refined_groups.len(), 1);
        assert_eq!(back.refinement_response.splits.len(), 1);
        assert_eq!(back.attempts_used, 2);
        assert_eq!(back.parse_failures, 1);
    }

    #[test]
    fn test_file_diff_content_serde_roundtrip() {
        let content = FileDiffContent {
            path: "src/main.ts".to_string(),
            old_content: "const x = 1;".to_string(),
            new_content: "const x = 2;".to_string(),
            language: "typescript".to_string(),
        };
        let json = serde_json::to_string(&content).unwrap();
        let back: FileDiffContent = serde_json::from_str(&json).unwrap();
        assert_eq!(back.path, "src/main.ts");
        assert_eq!(back.old_content, "const x = 1;");
        assert_eq!(back.new_content, "const x = 2;");
        assert_eq!(back.language, "typescript");
    }

    // ── Error handling edge case tests ────────────────────────────────

    #[test]
    fn test_command_error_all_variants_display() {
        let variants = vec![
            CommandError::Git("git error".into()),
            CommandError::Analysis("analysis error".into()),
            CommandError::Config("config error".into()),
            CommandError::Io("io error".into()),
            CommandError::Llm("llm error".into()),
        ];
        for err in &variants {
            let msg = err.to_string();
            assert!(!msg.is_empty());
            // Verify serialization works for all variants (sent to frontend)
            let json = serde_json::to_string(err).unwrap();
            assert!(!json.is_empty());
        }
    }

    #[test]
    fn test_detect_language_edge_cases() {
        // Path with multiple dots
        assert_eq!(detect_language("my.file.test.ts"), "typescript");
        // Hidden file
        assert_eq!(detect_language(".hidden.js"), "javascript");
        // No extension
        assert_eq!(detect_language("Makefile"), "plaintext");
        // Empty string
        assert_eq!(detect_language(""), "plaintext");
        // Path with spaces
        assert_eq!(detect_language("path with spaces/file.ts"), "typescript");
    }

    #[test]
    fn test_simple_unified_diff_only_additions() {
        let diff = simple_unified_diff("", "new line 1\nnew line 2");
        assert!(diff.contains("+new line 1"));
        assert!(diff.contains("+new line 2"));
        assert!(!diff.contains("-"));
    }

    #[test]
    fn test_simple_unified_diff_only_deletions() {
        let diff = simple_unified_diff("old line 1\nold line 2", "");
        assert!(diff.contains("-old line 1"));
        assert!(diff.contains("-old line 2"));
        assert!(!diff.contains("+"));
    }

    #[test]
    fn test_app_state_mutex_not_poisoned() {
        let state = AppState::new();
        // Lock, set, release
        {
            let mut last = state.last_analysis.lock().unwrap();
            *last = None;
        }
        // Lock again should succeed
        let last = state.last_analysis.lock().unwrap();
        assert!(last.is_none());
    }

    #[test]
    fn test_default_model_for_unknown_provider() {
        // Unknown providers should get a reasonable default
        let model = default_model_for_provider("nonexistent");
        assert!(!model.is_empty());
    }

    #[test]
    fn test_open_in_editor_nonexistent_file() {
        let result = open_in_editor(
            "vscode".to_string(),
            "/tmp/__nonexistent_file_12345__".to_string(),
        );
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("File not found"),
            "Expected file-not-found error, got: {}",
            err
        );
    }

    #[test]
    fn test_open_in_editor_unknown_editor() {
        // Create a temporary file to pass the file-exists check
        let tmp = std::env::temp_dir().join("diffcore_test_open_editor");
        std::fs::write(&tmp, "test").unwrap();
        let result = open_in_editor(
            "unknown_editor".to_string(),
            tmp.to_str().unwrap().to_string(),
        );
        std::fs::remove_file(&tmp).ok();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Unknown editor"),
            "Expected unknown-editor error, got: {}",
            err
        );
    }

    // ── Review comment tests ────────────────────────────────────────

    #[test]
    fn test_review_comment_serde_roundtrip() {
        let comment = ReviewComment {
            id: "c1".to_string(),
            comment_type: "code".to_string(),
            group_id: "group_1".to_string(),
            file_path: Some("src/auth.ts".to_string()),
            start_line: Some(42),
            end_line: Some(58),
            selected_code: Some("function validate() {}".to_string()),
            text: "Missing validation".to_string(),
            created_at: "2026-03-20T14:30:00Z".to_string(),
        };
        let json = serde_json::to_string(&comment).unwrap();
        let back: ReviewComment = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, "c1");
        assert_eq!(back.comment_type, "code");
        assert_eq!(back.group_id, "group_1");
        assert_eq!(back.file_path, Some("src/auth.ts".to_string()));
        assert_eq!(back.start_line, Some(42));
        assert_eq!(back.end_line, Some(58));
        assert_eq!(
            back.selected_code,
            Some("function validate() {}".to_string())
        );
        assert_eq!(back.text, "Missing validation");
    }

    #[test]
    fn test_review_comment_file_level() {
        let comment = ReviewComment {
            id: "c2".to_string(),
            comment_type: "file".to_string(),
            group_id: "group_1".to_string(),
            file_path: Some("src/auth.ts".to_string()),
            start_line: None,
            end_line: None,
            selected_code: None,
            text: "Should we add rate limiting?".to_string(),
            created_at: "2026-03-20T14:30:00Z".to_string(),
        };
        let json = serde_json::to_string(&comment).unwrap();
        let back: ReviewComment = serde_json::from_str(&json).unwrap();
        assert_eq!(back.comment_type, "file");
        assert!(back.start_line.is_none());
        assert!(back.selected_code.is_none());
    }

    #[test]
    fn test_review_comment_group_level() {
        let comment = ReviewComment {
            id: "c3".to_string(),
            comment_type: "group".to_string(),
            group_id: "group_1".to_string(),
            file_path: None,
            start_line: None,
            end_line: None,
            selected_code: None,
            text: "Overall looks good".to_string(),
            created_at: "2026-03-20T14:31:00Z".to_string(),
        };
        let json = serde_json::to_string(&comment).unwrap();
        let back: ReviewComment = serde_json::from_str(&json).unwrap();
        assert_eq!(back.comment_type, "group");
        assert!(back.file_path.is_none());
    }

    #[test]
    fn test_comments_file_serde_roundtrip() {
        let comments_file = CommentsFile {
            analysis_hash: "abc123".to_string(),
            comments: vec![
                ReviewComment {
                    id: "c1".to_string(),
                    comment_type: "code".to_string(),
                    group_id: "group_1".to_string(),
                    file_path: Some("src/auth.ts".to_string()),
                    start_line: Some(42),
                    end_line: Some(58),
                    selected_code: Some("fn validate()".to_string()),
                    text: "Missing validation".to_string(),
                    created_at: "2026-03-20T14:30:00Z".to_string(),
                },
                ReviewComment {
                    id: "c2".to_string(),
                    comment_type: "group".to_string(),
                    group_id: "group_1".to_string(),
                    file_path: None,
                    start_line: None,
                    end_line: None,
                    selected_code: None,
                    text: "Needs review".to_string(),
                    created_at: "2026-03-20T14:31:00Z".to_string(),
                },
            ],
        };
        let json = serde_json::to_string_pretty(&comments_file).unwrap();
        let back: CommentsFile = serde_json::from_str(&json).unwrap();
        assert_eq!(back.analysis_hash, "abc123");
        assert_eq!(back.comments.len(), 2);
        assert_eq!(back.comments[0].comment_type, "code");
        assert_eq!(back.comments[1].comment_type, "group");
    }

    #[test]
    fn test_review_comment_json_type_field() {
        // Verify the "type" field is correctly renamed from comment_type
        let comment = ReviewComment {
            id: "c1".to_string(),
            comment_type: "code".to_string(),
            group_id: "g1".to_string(),
            file_path: None,
            start_line: None,
            end_line: None,
            selected_code: None,
            text: "test".to_string(),
            created_at: "2026-03-20T14:30:00Z".to_string(),
        };
        let json = serde_json::to_string(&comment).unwrap();
        assert!(
            json.contains("\"type\":\"code\""),
            "JSON should use 'type' not 'comment_type': {}",
            json
        );
        // Verify deserialization from "type" field
        let back: ReviewComment = serde_json::from_str(&json).unwrap();
        assert_eq!(back.comment_type, "code");
    }

    // ── Open-in-editor / editor detection tests ─────────────────────

    #[test]
    fn test_check_editors_available_returns_all_editor_ids() {
        let result = check_editors_available();
        // Should always contain all 5 editor IDs
        for id in &["vscode", "cursor", "zed", "vim", "terminal"] {
            assert!(result.contains_key(*id), "Missing editor id: {}", id);
        }
        // Terminal should always be available
        assert_eq!(result["terminal"], true);
    }

    /// Gated behind `DIFFCORE_RUN_EDITOR_TESTS=1` because it actually spawns editor processes.
    #[test]
    fn test_open_in_editor_all_known_editors_accept_temp_file() {
        if std::env::var("DIFFCORE_RUN_EDITOR_TESTS").is_err() {
            eprintln!("Skipped: set DIFFCORE_RUN_EDITOR_TESTS=1 to run (launches real editors)");
            return;
        }

        // All known editor IDs should not return "Unknown editor" for a valid file
        let tmp = std::env::temp_dir().join("diffcore_test_known_editors");
        std::fs::write(&tmp, "test").unwrap();
        let path = tmp.to_str().unwrap().to_string();

        for editor in &["vscode", "cursor", "zed", "vim", "terminal"] {
            let result = open_in_editor(editor.to_string(), path.clone());
            // Result may be Ok (if editor is installed) or Err (not installed),
            // but should never be "Unknown editor"
            if let Err(e) = &result {
                let msg = e.to_string();
                assert!(
                    !msg.contains("Unknown editor"),
                    "Editor '{}' treated as unknown: {}",
                    editor,
                    msg
                );
            }
        }

        std::fs::remove_file(&tmp).ok();
    }

    // ── Update Comment Tests ──

    #[test]
    fn test_update_comment_cached_changes_text() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = init_test_repo(dir.path());

        let comment = ReviewComment {
            id: "test_update_1".to_string(),
            comment_type: "code".to_string(),
            group_id: "g1".to_string(),
            file_path: Some("src/main.ts".to_string()),
            start_line: Some(10),
            end_line: Some(15),
            selected_code: Some("const x = 1;".to_string()),
            text: "Original text".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };

        // Save then update
        save_comment_cached(repo_path.clone(), comment).unwrap();
        update_comment_cached(
            repo_path.clone(),
            "test_update_1".to_string(),
            "Updated text".to_string(),
        )
        .unwrap();

        let loaded = load_comments_cached(repo_path).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].text, "Updated text");
        assert_eq!(loaded[0].id, "test_update_1");
    }

    #[test]
    fn test_update_comment_preserves_other_fields() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = init_test_repo(dir.path());

        let comment = ReviewComment {
            id: "test_preserve_1".to_string(),
            comment_type: "code".to_string(),
            group_id: "group-abc".to_string(),
            file_path: Some("src/handler.ts".to_string()),
            start_line: Some(42),
            end_line: Some(50),
            selected_code: Some("function handler() {}".to_string()),
            text: "Before update".to_string(),
            created_at: "2026-03-15T12:00:00Z".to_string(),
        };

        save_comment_cached(repo_path.clone(), comment).unwrap();
        update_comment_cached(
            repo_path.clone(),
            "test_preserve_1".to_string(),
            "After update".to_string(),
        )
        .unwrap();

        let loaded = load_comments_cached(repo_path).unwrap();
        let c = &loaded[0];
        assert_eq!(c.text, "After update");
        assert_eq!(c.comment_type, "code");
        assert_eq!(c.group_id, "group-abc");
        assert_eq!(c.file_path, Some("src/handler.ts".to_string()));
        assert_eq!(c.start_line, Some(42));
        assert_eq!(c.end_line, Some(50));
        assert_eq!(c.selected_code, Some("function handler() {}".to_string()));
        assert_eq!(c.created_at, "2026-03-15T12:00:00Z");
    }

    #[test]
    fn test_update_nonexistent_comment_is_noop() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = init_test_repo(dir.path());

        let comment = ReviewComment {
            id: "existing_1".to_string(),
            comment_type: "file".to_string(),
            group_id: "g1".to_string(),
            file_path: Some("test.ts".to_string()),
            start_line: None,
            end_line: None,
            selected_code: None,
            text: "Should not change".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };

        save_comment_cached(repo_path.clone(), comment).unwrap();
        // Update a non-existent ID
        update_comment_cached(
            repo_path.clone(),
            "nonexistent_id".to_string(),
            "New text".to_string(),
        )
        .unwrap();

        let loaded = load_comments_cached(repo_path).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].text, "Should not change");
    }

    #[test]
    fn test_update_comment_among_multiple() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = init_test_repo(dir.path());

        for i in 1..=5 {
            let comment = ReviewComment {
                id: format!("multi_{}", i),
                comment_type: "code".to_string(),
                group_id: "g1".to_string(),
                file_path: Some("src/main.ts".to_string()),
                start_line: Some(i * 10),
                end_line: Some(i * 10 + 5),
                selected_code: None,
                text: format!("Comment {}", i),
                created_at: "2026-01-01T00:00:00Z".to_string(),
            };
            save_comment_cached(repo_path.clone(), comment).unwrap();
        }

        // Update only the 3rd comment
        update_comment_cached(
            repo_path.clone(),
            "multi_3".to_string(),
            "Updated comment 3".to_string(),
        )
        .unwrap();

        let loaded = load_comments_cached(repo_path).unwrap();
        assert_eq!(loaded.len(), 5);
        assert_eq!(loaded[0].text, "Comment 1");
        assert_eq!(loaded[1].text, "Comment 2");
        assert_eq!(loaded[2].text, "Updated comment 3");
        assert_eq!(loaded[3].text, "Comment 4");
        assert_eq!(loaded[4].text, "Comment 5");
    }

    #[test]
    fn test_update_comment_with_empty_text() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = init_test_repo(dir.path());

        let comment = ReviewComment {
            id: "empty_text_1".to_string(),
            comment_type: "file".to_string(),
            group_id: "g1".to_string(),
            file_path: Some("file.ts".to_string()),
            start_line: None,
            end_line: None,
            selected_code: None,
            text: "Has text".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };

        save_comment_cached(repo_path.clone(), comment).unwrap();
        update_comment_cached(
            repo_path.clone(),
            "empty_text_1".to_string(),
            "".to_string(),
        )
        .unwrap();

        let loaded = load_comments_cached(repo_path).unwrap();
        assert_eq!(loaded[0].text, "");
    }

    #[test]
    fn test_update_comment_with_special_characters() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = init_test_repo(dir.path());

        let comment = ReviewComment {
            id: "special_chars_1".to_string(),
            comment_type: "code".to_string(),
            group_id: "g1".to_string(),
            file_path: Some("src/main.ts".to_string()),
            start_line: Some(1),
            end_line: Some(5),
            selected_code: None,
            text: "Plain text".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };

        save_comment_cached(repo_path.clone(), comment).unwrap();
        let special_text =
            "Contains \"quotes\", newlines\n\ttabs, unicode: 🦀, and <html> & entities";
        update_comment_cached(
            repo_path.clone(),
            "special_chars_1".to_string(),
            special_text.to_string(),
        )
        .unwrap();

        let loaded = load_comments_cached(repo_path).unwrap();
        assert_eq!(loaded[0].text, special_text);
    }

    #[test]
    fn test_update_then_delete_comment() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = init_test_repo(dir.path());

        let comment = ReviewComment {
            id: "update_delete_1".to_string(),
            comment_type: "file".to_string(),
            group_id: "g1".to_string(),
            file_path: Some("test.ts".to_string()),
            start_line: None,
            end_line: None,
            selected_code: None,
            text: "Will be updated then deleted".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };

        save_comment_cached(repo_path.clone(), comment).unwrap();
        update_comment_cached(
            repo_path.clone(),
            "update_delete_1".to_string(),
            "Updated".to_string(),
        )
        .unwrap();
        delete_comment_cached(repo_path.clone(), "update_delete_1".to_string()).unwrap();

        let loaded = load_comments_cached(repo_path).unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn test_multiple_updates_to_same_comment() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = init_test_repo(dir.path());

        let comment = ReviewComment {
            id: "multi_update_1".to_string(),
            comment_type: "code".to_string(),
            group_id: "g1".to_string(),
            file_path: Some("src/main.ts".to_string()),
            start_line: Some(1),
            end_line: Some(3),
            selected_code: None,
            text: "Version 1".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };

        save_comment_cached(repo_path.clone(), comment).unwrap();

        for i in 2..=10 {
            update_comment_cached(
                repo_path.clone(),
                "multi_update_1".to_string(),
                format!("Version {}", i),
            )
            .unwrap();
        }

        let loaded = load_comments_cached(repo_path).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].text, "Version 10");
    }

    #[test]
    fn test_update_comment_on_empty_cache() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = init_test_repo(dir.path());

        // Update on empty cache should succeed (no comment found, noop)
        let result = update_comment_cached(
            repo_path.clone(),
            "no_such_id".to_string(),
            "text".to_string(),
        );
        assert!(result.is_ok());

        let loaded = load_comments_cached(repo_path).unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn test_update_comment_cached_invalid_repo() {
        let result = update_comment_cached(
            "/nonexistent/repo/path".to_string(),
            "id".to_string(),
            "text".to_string(),
        );
        assert!(result.is_err());
    }

    // ── ReviewComment Serde Tests ──

    #[test]
    fn test_review_comment_serde_all_fields() {
        let comment = ReviewComment {
            id: "c1".to_string(),
            comment_type: "code".to_string(),
            group_id: "g1".to_string(),
            file_path: Some("src/main.ts".to_string()),
            start_line: Some(10),
            end_line: Some(20),
            selected_code: Some("const x = 1;".to_string()),
            text: "This needs refactoring".to_string(),
            created_at: "2026-04-06T12:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&comment).unwrap();
        let back: ReviewComment = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, "c1");
        assert_eq!(back.comment_type, "code");
        assert_eq!(back.group_id, "g1");
        assert_eq!(back.file_path, Some("src/main.ts".to_string()));
        assert_eq!(back.start_line, Some(10));
        assert_eq!(back.end_line, Some(20));
        assert_eq!(back.selected_code, Some("const x = 1;".to_string()));
        assert_eq!(back.text, "This needs refactoring");
        assert_eq!(back.created_at, "2026-04-06T12:00:00Z");
    }

    #[test]
    fn test_review_comment_serde_minimal_fields() {
        let comment = ReviewComment {
            id: "c2".to_string(),
            comment_type: "group".to_string(),
            group_id: "g2".to_string(),
            file_path: None,
            start_line: None,
            end_line: None,
            selected_code: None,
            text: "Group-level comment".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&comment).unwrap();
        let back: ReviewComment = serde_json::from_str(&json).unwrap();
        assert_eq!(back.comment_type, "group");
        assert!(back.file_path.is_none());
        assert!(back.start_line.is_none());
        assert!(back.end_line.is_none());
        assert!(back.selected_code.is_none());
    }

    #[test]
    fn test_review_comment_type_rename_in_json() {
        // The `comment_type` field is serialized as `type` in JSON (via serde rename)
        let comment = ReviewComment {
            id: "c3".to_string(),
            comment_type: "file".to_string(),
            group_id: "g1".to_string(),
            file_path: Some("test.ts".to_string()),
            start_line: None,
            end_line: None,
            selected_code: None,
            text: "File comment".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&comment).unwrap();
        assert!(json.contains(r#""type":"file""#));
        assert!(!json.contains("comment_type"));
    }

    // ── LLM Settings Annotations Tests ──

    #[test]
    fn test_llm_settings_annotations_enabled_roundtrip_true() {
        let settings = LlmSettings {
            annotations_enabled: true,
            refinement_enabled: true,
            provider: "codex".to_string(),
            model: "default".to_string(),
            api_key_source: "test".to_string(),
            has_api_key: true,
            refinement_provider: "codex".to_string(),
            refinement_model: "default".to_string(),
            refinement_max_iterations: 1,
            global_config_path: "~/.diffcore/config.toml".to_string(),
            codex_available: false,
            codex_authenticated: false,
            claude_available: false,
            claude_authenticated: false,
            include_uncommitted: true,
        };
        let json = serde_json::to_string(&settings).unwrap();
        let back: LlmSettings = serde_json::from_str(&json).unwrap();
        assert!(back.annotations_enabled);
        assert!(back.refinement_enabled);
    }

    #[test]
    fn test_llm_settings_annotations_enabled_roundtrip_false() {
        let settings = LlmSettings {
            annotations_enabled: false,
            refinement_enabled: false,
            provider: "anthropic".to_string(),
            model: "claude-sonnet-4-6".to_string(),
            api_key_source: "env".to_string(),
            has_api_key: false,
            refinement_provider: "anthropic".to_string(),
            refinement_model: "claude-sonnet-4-6".to_string(),
            refinement_max_iterations: 3,
            global_config_path: "/tmp/config.toml".to_string(),
            codex_available: true,
            codex_authenticated: true,
            claude_available: true,
            claude_authenticated: true,
            include_uncommitted: false,
        };
        let json = serde_json::to_string(&settings).unwrap();
        let back: LlmSettings = serde_json::from_str(&json).unwrap();
        assert!(!back.annotations_enabled);
        assert!(!back.refinement_enabled);
    }

    #[test]
    fn test_llm_settings_all_fields_present_in_json() {
        let settings = LlmSettings {
            annotations_enabled: true,
            refinement_enabled: true,
            provider: "openai".to_string(),
            model: "gpt-4.1".to_string(),
            api_key_source: "config".to_string(),
            has_api_key: true,
            refinement_provider: "gemini".to_string(),
            refinement_model: "gemini-2.5-flash".to_string(),
            refinement_max_iterations: 2,
            global_config_path: "~/.diffcore/config.toml".to_string(),
            codex_available: true,
            codex_authenticated: false,
            claude_available: true,
            claude_authenticated: true,
            include_uncommitted: true,
        };
        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains("annotations_enabled"));
        assert!(json.contains("refinement_enabled"));
        assert!(json.contains("provider"));
        assert!(json.contains("model"));
        assert!(json.contains("has_api_key"));
        assert!(json.contains("refinement_provider"));
        assert!(json.contains("refinement_model"));
        assert!(json.contains("refinement_max_iterations"));
        assert!(json.contains("global_config_path"));
        assert!(json.contains("include_uncommitted"));
    }

    // ── Comment CRUD Integration Tests ──

    #[test]
    fn test_full_comment_lifecycle_save_load_update_delete() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = init_test_repo(dir.path());

        // 1. Start empty
        let loaded = load_comments_cached(repo_path.clone()).unwrap();
        assert!(loaded.is_empty());

        // 2. Save two comments
        let c1 = ReviewComment {
            id: "lifecycle_1".to_string(),
            comment_type: "code".to_string(),
            group_id: "g1".to_string(),
            file_path: Some("src/a.ts".to_string()),
            start_line: Some(5),
            end_line: Some(10),
            selected_code: Some("let a = 1;".to_string()),
            text: "First comment".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };
        let c2 = ReviewComment {
            id: "lifecycle_2".to_string(),
            comment_type: "file".to_string(),
            group_id: "g1".to_string(),
            file_path: Some("src/b.ts".to_string()),
            start_line: None,
            end_line: None,
            selected_code: None,
            text: "Second comment".to_string(),
            created_at: "2026-01-01T01:00:00Z".to_string(),
        };
        save_comment_cached(repo_path.clone(), c1).unwrap();
        save_comment_cached(repo_path.clone(), c2).unwrap();

        let loaded = load_comments_cached(repo_path.clone()).unwrap();
        assert_eq!(loaded.len(), 2);

        // 3. Update first comment
        update_comment_cached(
            repo_path.clone(),
            "lifecycle_1".to_string(),
            "Edited first".to_string(),
        )
        .unwrap();
        let loaded = load_comments_cached(repo_path.clone()).unwrap();
        assert_eq!(loaded[0].text, "Edited first");
        assert_eq!(loaded[1].text, "Second comment");

        // 4. Delete second comment
        delete_comment_cached(repo_path.clone(), "lifecycle_2".to_string()).unwrap();
        let loaded = load_comments_cached(repo_path.clone()).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "lifecycle_1");

        // 5. Update the remaining comment again
        update_comment_cached(
            repo_path.clone(),
            "lifecycle_1".to_string(),
            "Final edit".to_string(),
        )
        .unwrap();
        let loaded = load_comments_cached(repo_path.clone()).unwrap();
        assert_eq!(loaded[0].text, "Final edit");

        // 6. Delete last comment
        delete_comment_cached(repo_path.clone(), "lifecycle_1".to_string()).unwrap();
        let loaded = load_comments_cached(repo_path).unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn test_comment_types_code_file_group() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = init_test_repo(dir.path());

        let types = vec!["code", "file", "group"];
        for (i, t) in types.iter().enumerate() {
            let comment = ReviewComment {
                id: format!("type_test_{}", i),
                comment_type: t.to_string(),
                group_id: "g1".to_string(),
                file_path: if *t != "group" {
                    Some("test.ts".to_string())
                } else {
                    None
                },
                start_line: if *t == "code" { Some(1) } else { None },
                end_line: if *t == "code" { Some(5) } else { None },
                selected_code: if *t == "code" {
                    Some("code".to_string())
                } else {
                    None
                },
                text: format!("{} comment", t),
                created_at: "2026-01-01T00:00:00Z".to_string(),
            };
            save_comment_cached(repo_path.clone(), comment).unwrap();
        }

        let loaded = load_comments_cached(repo_path).unwrap();
        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded[0].comment_type, "code");
        assert_eq!(loaded[1].comment_type, "file");
        assert_eq!(loaded[2].comment_type, "group");
    }

    /// Helper to create a minimal git repo for comment cache tests.
    fn init_test_repo(dir: &std::path::Path) -> String {
        use std::process::Command;
        Command::new("git")
            .args(["init"])
            .current_dir(dir)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "--allow-empty", "-m", "init"])
            .current_dir(dir)
            .output()
            .unwrap();
        dir.to_str().unwrap().to_string()
    }
}
