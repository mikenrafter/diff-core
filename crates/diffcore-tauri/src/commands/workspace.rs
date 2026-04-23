//! Workspace, git, and file content commands.

use std::collections::HashSet;
use std::path::PathBuf;

use grep_regex::RegexMatcherBuilder;
use grep_searcher::{sinks, SearcherBuilder};
use ignore::WalkBuilder;

use diffcore_core::git;

use super::{AppState, CommandError, FileDiffContent};



/// Summary of repository state for the UI.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RepoInfo {
    pub current_branch: Option<String>,
    pub default_branch: String,
    pub branches: Vec<git::BranchInfo>,
    pub worktrees: Vec<git::WorktreeInfo>,
    pub status: Option<git::BranchStatus>,
    /// Whether the opened path is a linked worktree (not the main worktree).
    pub is_worktree: bool,
}

/// List all local branches in the repository.
///
/// Returns branches sorted with current branch first, then alphabetically.
#[tauri::command]
pub fn list_branches(repo_path: String) -> Result<Vec<git::BranchInfo>, CommandError> {
    let repo = super::open_repo(&repo_path)?;
    git::list_branches(&repo).map_err(|e| CommandError::Git(format!("{}", e)))
}

/// List recent commits for commit-level ref selection in the UI.
#[tauri::command]
pub fn list_commits(repo_path: String, limit: Option<usize>) -> Result<Vec<git::CommitInfo>, CommandError> {
    let repo = super::open_repo(&repo_path)?;
    let bounded_limit = limit.unwrap_or(50).clamp(1, 200);
    git::list_recent_commits(&repo, bounded_limit)
        .map_err(|e| CommandError::Git(format!("{}", e)))
}

/// List all git worktrees for the repository.
#[tauri::command]
pub fn list_worktrees(repo_path: String) -> Result<Vec<git::WorktreeInfo>, CommandError> {
    let repo = super::open_repo(&repo_path)?;
    git::list_worktrees(&repo).map_err(|e| CommandError::Git(format!("{}", e)))
}

/// Get the current branch's tracking status (ahead/behind upstream).
#[tauri::command]
pub fn get_branch_status(repo_path: String) -> Result<git::BranchStatus, CommandError> {
    let repo = super::open_repo(&repo_path)?;
    git::get_branch_status(&repo).map_err(|e| CommandError::Git(format!("{}", e)))
}

/// Auto-detect the default branch and current branch for a repository.
///
/// Returns a summary useful for the UI to set up initial state.
#[tauri::command]
pub fn get_repo_info(repo_path: String) -> Result<RepoInfo, CommandError> {
    let repo = super::open_repo(&repo_path)?;

    let current = git::current_branch(&repo);
    let default_branch = git::detect_default_branch(&repo).unwrap_or_else(|_| "main".to_string());
    let branches = git::list_branches(&repo).map_err(|e| CommandError::Git(format!("{}", e)))?;
    let worktrees = git::list_worktrees(&repo).map_err(|e| CommandError::Git(format!("{}", e)))?;
    let status = git::get_branch_status(&repo).ok();
    let is_worktree = git::is_linked_worktree(&repo);

    Ok(RepoInfo {
        current_branch: current,
        default_branch,
        branches,
        worktrees,
        status,
        is_worktree,
    })
}

/// Return the first directory argument passed at app launch, if any.
#[tauri::command]
pub fn get_launch_directory() -> Option<String> {
    std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .find(|path| path.is_dir())
        .and_then(|path| std::fs::canonicalize(path).ok())
        .map(|path| path.to_string_lossy().to_string())
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileShortStatus {
    pub path: String,
    pub status: String,
}

#[tauri::command]
pub fn get_last_diff_file_statuses(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<FileShortStatus>, CommandError> {
    let guard = state
        .last_diff
        .lock()
        .map_err(|e| CommandError::Analysis(format!("Lock poisoned: {}", e)))?;

    let Some(cached) = guard.as_ref() else {
        return Ok(vec![]);
    };

    let mut out = Vec::with_capacity(cached.diff_result.files.len());
    for file in &cached.diff_result.files {
        let status = match file.status {
            diffcore_core::git::FileStatus::Added => "A",
            diffcore_core::git::FileStatus::Modified => "M",
            diffcore_core::git::FileStatus::Deleted => "D",
            diffcore_core::git::FileStatus::Renamed => "R",
            diffcore_core::git::FileStatus::Copied => "C",
        };
        out.push(FileShortStatus {
            path: file.path().to_string(),
            status: status.to_string(),
        });
    }

    Ok(out)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CrossFileSearchMatch {
    pub line_number: u32,
    pub line_text: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CrossFileSearchResult {
    pub file_path: String,
    pub matches: Vec<CrossFileSearchMatch>,
}

fn changed_files_from_state(state: &AppState) -> HashSet<String> {
    let mut files = HashSet::new();

    if let Ok(guard) = state.last_analysis.lock() {
        if let Some(analysis) = guard.as_ref() {
            for group in &analysis.groups {
                for file in &group.files {
                    files.insert(file.path.clone());
                }
            }
            if let Some(infra) = &analysis.infrastructure_group {
                for file in &infra.files {
                    files.insert(file.clone());
                }
            }
        }
    }

    if files.is_empty() {
        if let Ok(guard) = state.last_diff.lock() {
            if let Some(cached) = guard.as_ref() {
                for file in &cached.diff_result.files {
                    files.insert(file.path().to_string());
                }
            }
        }
    }

    files
}

fn workspace_files(workdir: &std::path::Path) -> Vec<String> {
    let mut builder = WalkBuilder::new(workdir);
    builder
        .hidden(false)
        .ignore(true)
        .git_ignore(true)
        .git_exclude(true)
        .parents(true);

    builder
        .build()
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().map(|ft| ft.is_file()).unwrap_or(false))
        .filter_map(|entry| {
            let rel = entry.path().strip_prefix(workdir).ok()?;
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            if rel_str.starts_with(".git/") {
                return None;
            }
            Some(rel_str)
        })
        .collect()
}

#[tauri::command]
pub fn cross_file_search(
    repo_path: String,
    query: String,
    show_unchanged_files: bool,
    max_results: Option<usize>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<CrossFileSearchResult>, CommandError> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(vec![]);
    }

    let repo = super::open_repo(&repo_path)?;
    let workdir = repo
        .workdir()
        .ok_or_else(|| CommandError::Git("Bare repositories are not supported".to_string()))?
        .to_path_buf();

    let mut candidates: Vec<String> = if show_unchanged_files {
        workspace_files(&workdir)
    } else {
        changed_files_from_state(&state).into_iter().collect()
    };
    candidates.sort();

    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(true)
        .fixed_strings(true)
        .build(query)
        .map_err(|e| CommandError::Analysis(format!("Invalid search query: {}", e)))?;

    let mut searcher = SearcherBuilder::new()
        .line_number(true)
        .multi_line(false)
        .binary_detection(grep_searcher::BinaryDetection::quit(b'\x00'))
        .build();

    let max_file_results = max_results.unwrap_or(200).max(1);
    let mut results = Vec::new();
    let mut total_matches = 0usize;

    for relative_path in candidates {
        if results.len() >= max_file_results || total_matches >= 1000 {
            break;
        }

        let absolute_path = workdir.join(&relative_path);
        let metadata = match std::fs::metadata(&absolute_path) {
            Ok(meta) => meta,
            Err(_) => continue,
        };
        if metadata.len() > 2 * 1024 * 1024 {
            continue;
        }

        let mut file_matches = Vec::new();

        let sink = sinks::UTF8(|line_number: u64, line: &str| {
            if total_matches >= 1000 || file_matches.len() >= 50 {
                return Ok(false);
            }
            let clean = line.trim_end_matches(&['\r', '\n'][..]).to_string();
            file_matches.push(CrossFileSearchMatch {
                line_number: line_number as u32,
                line_text: clean,
            });
            total_matches += 1;
            Ok(true)
        });

        if searcher.search_path(&matcher, &absolute_path, sink).is_err() {
            continue;
        }

        if !file_matches.is_empty() {
            results.push(CrossFileSearchResult {
                file_path: relative_path,
                matches: file_matches,
            });
        }
    }

    Ok(results)
}

#[tauri::command]
pub fn get_workspace_file_content(
    repo_path: String,
    file_path: String,
) -> Result<FileDiffContent, CommandError> {
    let repo = super::open_repo(&repo_path)?;
    let workdir = repo
        .workdir()
        .ok_or_else(|| CommandError::Git("Bare repositories are not supported".to_string()))?
        .to_path_buf();

    let absolute = workdir.join(&file_path);
    if !absolute.exists() || !absolute.is_file() {
        return Err(CommandError::Io(format!("File not found: {}", file_path)));
    }

    let content = std::fs::read_to_string(&absolute)
        .map_err(|e| CommandError::Io(format!("Failed to read file '{}': {}", file_path, e)))?;

    Ok(FileDiffContent {
        path: file_path.clone(),
        old_content: content.clone(),
        new_content: content,
        language: super::detect_language(&file_path),
    })
}

/// Parse a single file's source via the diffcore-core query engine and
/// return the language-agnostic IR (definitions, imports, exports, call
/// sites). Used by the source-explorer outline panel so it can show
/// symbols for any language the engine supports — replacing the
/// hand-written per-language regex parsers that used to live in
/// `SourceExplorer.tsx` and only covered TS/JS/Python/Go/Rust.
///
/// `path` is used only for language detection (via file extension); no
/// disk access happens. `source` is the raw text to parse. The shared
/// `QueryEngine` instance held on `AppState` caches per-language
/// tree-sitter query compilation across calls, so repeated outline
/// updates for the same language are cheap.
///
/// Returns an empty `ParsedFile` (with `Language::Unknown`) when the
/// path's extension is not recognised — the caller is expected to
/// degrade gracefully rather than treat that as an error.
#[tauri::command]
pub fn parse_file_content(
    path: String,
    source: String,
    state: tauri::State<'_, AppState>,
) -> Result<diffcore_core::ast::ParsedFile, CommandError> {
    state
        .query_engine
        .parse_file(&path, &source)
        .map_err(|e| CommandError::Analysis(format!("parse_file failed: {e}")))
}
