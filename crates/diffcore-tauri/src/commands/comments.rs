//! Review comment management commands.

use std::path::PathBuf;

use super::CommandError;

// ── Review Comments ──────────────────────────────────────────────────

/// A single review comment — can be scoped to a group, file, or code range.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReviewComment {
    /// Unique identifier for the comment.
    pub id: String,
    /// Comment scope: "code", "file", or "group".
    #[serde(rename = "type")]
    pub comment_type: String,
    /// The flow group this comment belongs to.
    pub group_id: String,
    /// File path (null for group-level comments).
    pub file_path: Option<String>,
    /// Start line (null for file/group-level comments).
    pub start_line: Option<u32>,
    /// End line (null for file/group-level comments).
    pub end_line: Option<u32>,
    /// The selected code snippet (for code-level comments).
    pub selected_code: Option<String>,
    /// The comment text.
    pub text: String,
    /// ISO 8601 timestamp when the comment was created.
    pub created_at: String,
}

/// Container for persisted comments, keyed by analysis hash.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CommentsFile {
    /// Hash of the analysis run these comments belong to.
    pub analysis_hash: String,
    /// All comments for this analysis.
    pub comments: Vec<ReviewComment>,
}

/// Get the `.diffcore/comments.json` path for a repo.
fn comments_file_path(repo_path: &str) -> Result<PathBuf, CommandError> {
    let repo_path = PathBuf::from(repo_path);
    let repo_path = std::fs::canonicalize(&repo_path)
        .map_err(|e| CommandError::Io(format!("Invalid repo path: {}", e)))?;
    let repo = git2::Repository::discover(&repo_path)
        .map_err(|e| CommandError::Git(format!("Not a git repository: {}", e)))?;
    let workdir = repo
        .workdir()
        .ok_or_else(|| CommandError::Git("Bare repositories are not supported".to_string()))?;
    Ok(workdir.join(".diffcore").join("comments.json"))
}

/// Save a comment to `.diffcore/comments.json`.
///
/// Creates the `.diffcore/` directory if it doesn't exist. Appends to existing
/// comments if the analysis hash matches, otherwise starts fresh.
#[tauri::command]
pub fn save_comment(
    repo_path: String,
    analysis_hash: String,
    comment: ReviewComment,
) -> Result<(), CommandError> {
    let path = comments_file_path(&repo_path)?;

    // Ensure .diffcore directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            CommandError::Io(format!("Failed to create .diffcore directory: {}", e))
        })?;
    }

    // Load existing comments or start fresh
    let mut comments_file = load_comments_from_file(&path, &analysis_hash);
    comments_file.comments.push(comment);

    // Write back
    let json = serde_json::to_string_pretty(&comments_file)
        .map_err(|e| CommandError::Io(format!("Failed to serialize comments: {}", e)))?;
    std::fs::write(&path, json)
        .map_err(|e| CommandError::Io(format!("Failed to write comments file: {}", e)))?;

    Ok(())
}

/// Delete a comment by ID from `.diffcore/comments.json`.
#[tauri::command]
pub fn delete_comment(
    repo_path: String,
    analysis_hash: String,
    comment_id: String,
) -> Result<(), CommandError> {
    let path = comments_file_path(&repo_path)?;
    let mut comments_file = load_comments_from_file(&path, &analysis_hash);
    comments_file.comments.retain(|c| c.id != comment_id);

    let json = serde_json::to_string_pretty(&comments_file)
        .map_err(|e| CommandError::Io(format!("Failed to serialize comments: {}", e)))?;
    std::fs::write(&path, json)
        .map_err(|e| CommandError::Io(format!("Failed to write comments file: {}", e)))?;

    Ok(())
}

/// Load all comments for a given analysis hash from `.diffcore/comments.json`.
#[tauri::command]
pub fn load_comments(
    repo_path: String,
    analysis_hash: String,
) -> Result<Vec<ReviewComment>, CommandError> {
    let path = comments_file_path(&repo_path)?;
    let comments_file = load_comments_from_file(&path, &analysis_hash);
    Ok(comments_file.comments)
}

/// Export all comments as a formatted string ready for pasting to an AI agent.
///
/// Includes absolute file paths, code snippets for code-level comments,
/// and group context.
#[tauri::command]
pub fn export_comments(repo_path: String, analysis_hash: String) -> Result<String, CommandError> {
    let path = comments_file_path(&repo_path)?;
    let comments_file = load_comments_from_file(&path, &analysis_hash);

    let repo_path_buf = PathBuf::from(&repo_path);
    let repo_path_buf = std::fs::canonicalize(&repo_path_buf)
        .map_err(|e| CommandError::Io(format!("Invalid repo path: {}", e)))?;
    let repo = git2::Repository::discover(&repo_path_buf)
        .map_err(|e| CommandError::Git(format!("Not a git repository: {}", e)))?;
    let workdir = repo
        .workdir()
        .ok_or_else(|| CommandError::Git("Bare repositories are not supported".to_string()))?
        .to_string_lossy()
        .to_string();
    let workdir = if workdir.ends_with('/') {
        workdir[..workdir.len() - 1].to_string()
    } else {
        workdir
    };

    let mut output = String::new();

    for comment in &comments_file.comments {
        match comment.comment_type.as_str() {
            "code" => {
                if let Some(ref fp) = comment.file_path {
                    let abs_path = format!("{}/{}", workdir, fp);
                    if let (Some(start), Some(end)) = (comment.start_line, comment.end_line) {
                        output.push_str(&format!("{}:{}-{}\n", abs_path, start, end));
                    } else {
                        output.push_str(&format!("{}\n", abs_path));
                    }
                    if let Some(ref code) = comment.selected_code {
                        output.push_str("```\n");
                        output.push_str(code);
                        if !code.ends_with('\n') {
                            output.push('\n');
                        }
                        output.push_str("```\n");
                    }
                    output.push_str(&format!("> {}\n\n", comment.text));
                }
            }
            "file" => {
                if let Some(ref fp) = comment.file_path {
                    let abs_path = format!("{}/{}", workdir, fp);
                    output.push_str(&format!("{}\n", abs_path));
                    output.push_str(&format!("> {}\n\n", comment.text));
                }
            }
            "group" => {
                output.push_str(&format!("Flow: \"{}\"\n", comment.group_id));
                output.push_str(&format!("> {}\n\n", comment.text));
            }
            _ => {}
        }
    }

    Ok(output)
}

/// Load comments from a file, returning empty if file doesn't exist or hash doesn't match.
fn load_comments_from_file(path: &PathBuf, analysis_hash: &str) -> CommentsFile {
    if let Ok(data) = std::fs::read_to_string(path) {
        if let Ok(existing) = serde_json::from_str::<CommentsFile>(&data) {
            if existing.analysis_hash == analysis_hash {
                return existing;
            }
        }
    }
    CommentsFile {
        analysis_hash: analysis_hash.to_string(),
        comments: vec![],
    }
}

// ══════════════════════════════════════════════════════════════════════
// Branch-based comment cache (~/.diffcore/cache/comments/)
// ══════════════════════════════════════════════════════════════════════

/// Resolve the global comment cache directory.
/// Respects `DIFFCORE_COMMENT_CACHE_DIR` for testing.
fn comment_cache_dir() -> Option<PathBuf> {
    std::env::var_os("DIFFCORE_COMMENT_CACHE_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| {
                PathBuf::from(home)
                    .join(".diffcore")
                    .join("cache")
                    .join("comments")
            })
        })
}

/// Compute a cache key for a repo+branch combo.
///
/// Uses the git common dir (shared across worktrees) + current branch name,
/// so worktrees on the same branch share comments, while different branches
/// on the same repo are isolated.
pub fn comment_cache_key(repo_path: &str) -> Result<String, CommandError> {
    use sha2::{Digest, Sha256};

    let repo_path_buf = PathBuf::from(repo_path);
    let repo_path_buf = std::fs::canonicalize(&repo_path_buf)
        .map_err(|e| CommandError::Io(format!("Invalid repo path: {}", e)))?;
    let repo = git2::Repository::discover(&repo_path_buf)
        .map_err(|e| CommandError::Git(format!("Not a git repository: {}", e)))?;

    // Use the git dir path for identity. For worktrees, resolve the main
    // repo's git dir via `commondir()` if available, otherwise use `path()`.
    // git2 stores the common dir at `.git/commondir` for linked worktrees.
    let git_dir = repo.path();
    let common_dir_file = git_dir.join("commondir");
    let identity_dir = if common_dir_file.exists() {
        // Linked worktree — read the commondir reference to get the main repo's git dir
        std::fs::read_to_string(&common_dir_file)
            .ok()
            .and_then(|rel| {
                let trimmed = rel.trim();
                let resolved = if std::path::Path::new(trimmed).is_absolute() {
                    PathBuf::from(trimmed)
                } else {
                    git_dir.join(trimmed)
                };
                std::fs::canonicalize(resolved).ok()
            })
            .unwrap_or_else(|| git_dir.to_path_buf())
    } else {
        git_dir.to_path_buf()
    };
    let common_dir = identity_dir.to_string_lossy().to_string();

    // Get current branch name
    let branch = match repo.head() {
        Ok(head) => head.shorthand().unwrap_or("HEAD").to_string(),
        Err(_) => "HEAD".to_string(),
    };

    let mut hasher = Sha256::new();
    hasher.update(common_dir.as_bytes());
    hasher.update(b"\n");
    hasher.update(branch.as_bytes());
    Ok(hex::encode(hasher.finalize()))
}

/// Branch-cached comment file — just a Vec of comments, no analysis hash.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CachedCommentsFile {
    pub comments: Vec<ReviewComment>,
}

/// Load comments for the current repo+branch from the global cache.
fn load_cached_comments_file(cache_key: &str) -> CachedCommentsFile {
    let Some(dir) = comment_cache_dir() else {
        return CachedCommentsFile { comments: vec![] };
    };
    let path = dir.join(format!("{}.json", cache_key));
    match std::fs::read_to_string(&path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or(CachedCommentsFile { comments: vec![] }),
        Err(_) => CachedCommentsFile { comments: vec![] },
    }
}

/// Write comments for the current repo+branch to the global cache.
fn write_cached_comments_file(
    cache_key: &str,
    file: &CachedCommentsFile,
) -> Result<(), CommandError> {
    let dir = comment_cache_dir()
        .ok_or_else(|| CommandError::Io("Cannot determine comment cache directory".to_string()))?;
    std::fs::create_dir_all(&dir)
        .map_err(|e| CommandError::Io(format!("Failed to create comment cache dir: {}", e)))?;
    let path = dir.join(format!("{}.json", cache_key));
    let json = serde_json::to_string_pretty(file)
        .map_err(|e| CommandError::Io(format!("Failed to serialize comments: {}", e)))?;
    std::fs::write(&path, json)
        .map_err(|e| CommandError::Io(format!("Failed to write comment cache: {}", e)))?;
    Ok(())
}

/// Save a comment to the branch-based cache.
#[tauri::command]
pub fn save_comment_cached(repo_path: String, comment: ReviewComment) -> Result<(), CommandError> {
    let key = comment_cache_key(&repo_path)?;
    let mut file = load_cached_comments_file(&key);
    file.comments.push(comment);
    write_cached_comments_file(&key, &file)
}

/// Load all comments for the current repo+branch from the cache.
#[tauri::command]
pub fn load_comments_cached(repo_path: String) -> Result<Vec<ReviewComment>, CommandError> {
    let key = comment_cache_key(&repo_path)?;
    let file = load_cached_comments_file(&key);
    Ok(file.comments)
}

/// Delete a comment by ID from the branch-based cache.
#[tauri::command]
pub fn delete_comment_cached(repo_path: String, comment_id: String) -> Result<(), CommandError> {
    let key = comment_cache_key(&repo_path)?;
    let mut file = load_cached_comments_file(&key);
    file.comments.retain(|c| c.id != comment_id);
    write_cached_comments_file(&key, &file)
}

/// Update a comment's text by ID in the branch-based cache.
#[tauri::command]
pub fn update_comment_cached(
    repo_path: String,
    comment_id: String,
    new_text: String,
) -> Result<(), CommandError> {
    let key = comment_cache_key(&repo_path)?;
    let mut file = load_cached_comments_file(&key);
    if let Some(comment) = file.comments.iter_mut().find(|c| c.id == comment_id) {
        comment.text = new_text;
    }
    write_cached_comments_file(&key, &file)
}

// ══════════════════════════════════════════════════════════════════════
// Groups manifest import / file watching
// ══════════════════════════════════════════════════════════════════════

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
    fn test_load_comments_from_file_missing() {
        let path = std::env::temp_dir().join("diffcore_test_no_such_file.json");
        let result = load_comments_from_file(&path, "test_hash");
        assert_eq!(result.analysis_hash, "test_hash");
        assert!(result.comments.is_empty());
    }

    #[test]
    fn test_load_comments_from_file_wrong_hash() {
        let path = std::env::temp_dir().join("diffcore_test_wrong_hash.json");
        let data = CommentsFile {
            analysis_hash: "old_hash".to_string(),
            comments: vec![ReviewComment {
                id: "c1".to_string(),
                comment_type: "group".to_string(),
                group_id: "g1".to_string(),
                file_path: None,
                start_line: None,
                end_line: None,
                selected_code: None,
                text: "old comment".to_string(),
                created_at: "2026-03-20T14:30:00Z".to_string(),
            }],
        };
        std::fs::write(&path, serde_json::to_string(&data).unwrap()).unwrap();
        let result = load_comments_from_file(&path, "new_hash");
        assert_eq!(result.analysis_hash, "new_hash");
        assert!(result.comments.is_empty());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_load_comments_from_file_matching_hash() {
        let path = std::env::temp_dir().join("diffcore_test_matching_hash.json");
        let data = CommentsFile {
            analysis_hash: "matching_hash".to_string(),
            comments: vec![ReviewComment {
                id: "c1".to_string(),
                comment_type: "file".to_string(),
                group_id: "g1".to_string(),
                file_path: Some("test.ts".to_string()),
                start_line: None,
                end_line: None,
                selected_code: None,
                text: "test comment".to_string(),
                created_at: "2026-03-20T14:30:00Z".to_string(),
            }],
        };
        std::fs::write(&path, serde_json::to_string(&data).unwrap()).unwrap();
        let result = load_comments_from_file(&path, "matching_hash");
        assert_eq!(result.analysis_hash, "matching_hash");
        assert_eq!(result.comments.len(), 1);
        assert_eq!(result.comments[0].text, "test comment");
        std::fs::remove_file(&path).ok();
    }
}
