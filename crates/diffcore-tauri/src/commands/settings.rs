//! LLM settings, API key, and configuration commands.

use std::path::PathBuf;

use diffcore_core::config::DiffcoreConfig;
use diffcore_core::llm;

use super::CommandError;

/// Check whether LLM access is configured and available.
///
/// This includes API-key-based providers plus subscription-backed Codex/Claude CLIs.
#[tauri::command]
pub fn check_api_key(repo_path: Option<String>) -> Result<bool, CommandError> {
    Ok(get_llm_settings(repo_path)?.has_api_key)
}

/// Get LLM settings from the shared global config plus repo-local overrides.
///
/// Reads `~/.diffcore/config.toml`, merges in any repo-local `[llm]` overrides, resolves
/// CLI/API availability, and returns a unified `LlmSettings` struct for the settings panel.
#[tauri::command]
pub fn get_llm_settings(repo_path: Option<String>) -> Result<LlmSettings, CommandError> {
    let (config, workdir) = super::load_config_from_path(repo_path.as_deref());
    let codex_status = llm::codex_cli::detect_status();
    let claude_status = llm::claude_cli::detect_status();

    let configured_provider = config.llm.provider.as_deref();
    let provider =
        super::preferred_provider_for_runtime(configured_provider, &codex_status, &claude_status);
    let model = super::preferred_model_for_runtime(
        config.llm.model.clone(),
        configured_provider,
        &provider,
    );

    let has_api_key = match provider.as_str() {
        "codex" => codex_status.authenticated,
        "claude" => claude_status.authenticated,
        _ => llm::resolve_api_key(&config.llm, &provider).is_ok(),
    };

    let api_key_source = match provider.as_str() {
        "codex" => match (codex_status.installed, codex_status.authenticated) {
            (true, true) => "Codex CLI login".to_string(),
            (true, false) => "Codex CLI installed, not logged in".to_string(),
            (false, _) => "Codex CLI not installed".to_string(),
        },
        "claude" => match (claude_status.installed, claude_status.authenticated) {
            (true, true) => "Claude Code subscription".to_string(),
            (true, false) => "Claude Code installed, not logged in".to_string(),
            (false, _) => "Claude Code not installed".to_string(),
        },
        _ if config.llm.key_cmd.is_some() => "key_cmd".to_string(),
        _ if config.llm.key.as_ref().is_some_and(|k| !k.is_empty()) => {
            "~/.diffcore/config.toml".to_string()
        }
        _ if std::env::var("DIFFCORE_API_KEY").is_ok() => "DIFFCORE_API_KEY".to_string(),
        _ => {
            let env_var = match provider.as_str() {
                "anthropic" => "ANTHROPIC_API_KEY",
                "openai" => "OPENAI_API_KEY",
                "gemini" => "GEMINI_API_KEY",
                "openrouter" => "OPENROUTER_API_KEY",
                "github_copilot" => "GITHUB_COPILOT_TOKEN",
                _ => "none",
            };
            if std::env::var(env_var).is_ok() {
                env_var.to_string()
            } else if workdir.is_some() {
                "none (configure in ~/.diffcore/config.toml or env)".to_string()
            } else {
                "none".to_string()
            }
        }
    };

    let configured_refinement_provider = config
        .llm
        .refinement
        .provider
        .as_deref()
        .or(configured_provider);
    let refinement_provider = super::preferred_provider_for_runtime(
        configured_refinement_provider,
        &codex_status,
        &claude_status,
    );
    let refinement_model = super::preferred_model_for_runtime(
        config
            .llm
            .refinement
            .model
            .clone()
            .or(config.llm.model.clone()),
        configured_refinement_provider,
        &refinement_provider,
    );

    Ok(LlmSettings {
        annotations_enabled: config.llm.annotations_enabled,
        refinement_enabled: config.llm.refinement.enabled,
        provider,
        model,
        api_key_source,
        has_api_key,
        refinement_provider,
        refinement_model,
        refinement_max_iterations: config.llm.refinement.max_iterations,
        global_config_path: display_global_config_path(),
        codex_available: codex_status.installed,
        codex_authenticated: codex_status.authenticated,
        claude_available: claude_status.installed,
        claude_authenticated: claude_status.authenticated,
        include_uncommitted: config.diff.include_uncommitted,
    })
}

/// Save LLM settings to the shared global config.
///
/// Loads the existing global config, updates the `[llm]` section with the provided
/// settings, and writes back to `~/.diffcore/config.toml`.
#[tauri::command]
pub fn save_llm_settings(_repo_path: String, settings: LlmSettings) -> Result<(), CommandError> {
    let mut config =
        DiffcoreConfig::load_global().map_err(|e| CommandError::Config(format!("{}", e)))?;

    // Update LLM section
    config.llm.provider = Some(settings.provider);
    config.llm.model = Some(settings.model);
    // Don't overwrite key_cmd — that's managed manually
    config.llm.refinement.enabled = settings.refinement_enabled;
    config.llm.refinement.provider = Some(settings.refinement_provider);
    config.llm.refinement.model = Some(settings.refinement_model);
    config.llm.refinement.max_iterations = settings.refinement_max_iterations;
    config.llm.annotations_enabled = settings.annotations_enabled;

    // Update diff behavior
    config.diff.include_uncommitted = settings.include_uncommitted;

    config
        .save_global()
        .map_err(|e| CommandError::Config(format!("Failed to save config: {}", e)))?;

    Ok(())
}

/// Save an API key to `~/.diffcore/config.toml` under `[llm] key = "..."`.
///
/// The key is stored directly in the config file. Precedence is maintained:
/// `key_cmd` > `key` (config) > env vars.
#[tauri::command]
pub fn save_api_key(_repo_path: String, api_key: String) -> Result<(), CommandError> {
    let mut config =
        DiffcoreConfig::load_global().map_err(|e| CommandError::Config(format!("{}", e)))?;

    config.llm.key = Some(api_key);

    config
        .save_global()
        .map_err(|e| CommandError::Config(format!("Failed to save config: {}", e)))?;

    Ok(())
}

/// Remove the stored API key from `~/.diffcore/config.toml`.
#[tauri::command]
pub fn clear_api_key(_repo_path: String) -> Result<(), CommandError> {
    let mut config =
        DiffcoreConfig::load_global().map_err(|e| CommandError::Config(format!("{}", e)))?;

    config.llm.key = None;

    config
        .save_global()
        .map_err(|e| CommandError::Config(format!("Failed to save config: {}", e)))?;

    Ok(())
}

/// Re-export the shared `ModelInfo` type for the Tauri frontend.
pub use llm::models::ModelInfo;

/// Fetch available models from a provider's API.
///
/// Delegates to the shared `diffcore-core` model listing module, which handles
/// caching, API key resolution, and provider-specific fetching.
/// Pass `force_refresh: true` to bypass the 24-hour cache.
#[tauri::command]
pub async fn fetch_provider_models(
    provider: String,
    force_refresh: bool,
) -> Result<Vec<ModelInfo>, CommandError> {
    llm::models::fetch_provider_models(&provider, force_refresh)
        .await
        .map_err(|e| match e {
            llm::models::ModelListError::Network(msg) => CommandError::Network(msg),
            llm::models::ModelListError::Config(msg) => CommandError::Config(msg),
            llm::models::ModelListError::UnknownProvider(p) => {
                CommandError::Config(format!("Unknown provider: {}", p))
            }
        })
}

/// Get the current ignore paths from `.diffcore.toml`.
#[tauri::command]
pub fn get_ignore_paths(repo_path: Option<String>) -> Result<Vec<String>, CommandError> {
    let (config, _workdir) = super::load_config_from_path(repo_path.as_deref());
    Ok(config.ignore.paths)
}

/// Save ignore paths to `.diffcore.toml`.
///
/// Loads the existing config (preserving other sections), updates the ignore
/// paths, and writes back.
#[tauri::command]
pub fn save_ignore_paths(repo_path: String, paths: Vec<String>) -> Result<(), CommandError> {
    let repo_path_buf = PathBuf::from(&repo_path);
    let repo_path_buf = std::fs::canonicalize(&repo_path_buf)
        .map_err(|e| CommandError::Io(format!("Invalid repo path: {}", e)))?;
    let repo = git2::Repository::discover(&repo_path_buf)
        .map_err(|e| CommandError::Git(format!("Not a git repository: {}", e)))?;
    let workdir = repo
        .workdir()
        .ok_or_else(|| CommandError::Git("Bare repositories are not supported".to_string()))?;

    let mut config = DiffcoreConfig::load_from_dir(workdir)
        .map_err(|e| CommandError::Config(format!("{}", e)))?;

    config.ignore.paths = paths;

    config
        .save_to_dir(workdir)
        .map_err(|e| CommandError::Config(format!("Failed to save config: {}", e)))?;

    Ok(())
}

/// LLM settings for the UI — surface for the settings panel.
///
/// Contains the current provider/model configuration, API key status,
/// and annotation/refinement toggle states. Returned by `get_llm_settings`
/// and accepted by `save_llm_settings`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LlmSettings {
    /// Whether LLM annotations are enabled (controls visibility of Summarize PR / Analyze buttons).
    pub annotations_enabled: bool,
    /// Whether LLM refinement is enabled.
    pub refinement_enabled: bool,
    /// Selected LLM backend: subscription-backed CLI or direct API provider.
    pub provider: String,
    /// Selected model identifier.
    pub model: String,
    /// How the API key is configured.
    pub api_key_source: String,
    /// Whether an API key is actually available (resolvable).
    pub has_api_key: bool,
    /// Refinement provider (can differ from annotation provider).
    pub refinement_provider: String,
    /// Refinement model.
    pub refinement_model: String,
    /// Maximum refinement iterations.
    pub refinement_max_iterations: u32,
    /// Where shared LLM settings are stored.
    pub global_config_path: String,
    /// Whether Codex CLI is installed.
    pub codex_available: bool,
    /// Whether Codex CLI is logged in and ready.
    pub codex_authenticated: bool,
    /// Whether Claude Code is installed.
    pub claude_available: bool,
    /// Whether Claude Code is logged in and ready.
    pub claude_authenticated: bool,
    /// Whether to include uncommitted working tree changes in branch comparisons.
    pub include_uncommitted: bool,
}

fn display_global_config_path() -> String {
    DiffcoreConfig::global_config_path()
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_else(|| "~/.diffcore/config.toml".to_string())
}
