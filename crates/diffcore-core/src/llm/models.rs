//! Dynamic model listing for LLM providers.
//!
//! Fetches available models from provider APIs (Anthropic, OpenRouter, OpenAI, Gemini)
//! with a 24-hour disk cache at `~/.diffcore/cache/models/<provider>.json`.
//! CLI-backed providers (Codex, Claude) return static lists since they have no listing API.

use std::path::PathBuf;

use log::warn;
use serde::{Deserialize, Serialize};

use crate::config::DiffcoreConfig;
use crate::llm::resolve_api_key;

/// Model descriptor returned by provider model listing APIs.
///
/// Contains the model ID (used in API calls) and a human-friendly display name.
/// The `context_length` is populated when the provider returns it (e.g., OpenRouter).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Model identifier for API calls (e.g., "claude-sonnet-4-6", "anthropic/claude-sonnet-4-6").
    pub id: String,
    /// Human-readable display name (e.g., "Claude Sonnet 4.6").
    pub display_name: String,
    /// Context window size in tokens, if known.
    pub context_length: Option<u64>,
}

/// Errors that can occur during model listing.
#[derive(Debug, thiserror::Error)]
pub enum ModelListError {
    #[error("Network error: {0}")]
    Network(String),
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("Unknown provider: {0}")]
    UnknownProvider(String),
}

/// Fetch available models for a provider, using a 24-hour disk cache.
///
/// Pass `force_refresh: true` to bypass the cache and hit the provider API directly.
/// CLI-backed providers (`codex`, `claude`) always return static lists.
pub async fn fetch_provider_models(
    provider: &str,
    force_refresh: bool,
) -> Result<Vec<ModelInfo>, ModelListError> {
    let cache_dir = models_cache_dir();
    let cache_file = cache_dir.join(format!("{}.json", provider));

    if !force_refresh {
        if let Some(cached) = read_models_cache(&cache_file) {
            return Ok(cached);
        }
    }

    // CLI-backed providers: return static lists, no API to call
    if matches!(provider, "codex" | "claude") {
        return Ok(static_models_for_cli_provider(provider));
    }

    // Resolve API key from global config
    let config = DiffcoreConfig::load_global().unwrap_or_default();
    let api_key = match provider {
        "openrouter" => resolve_api_key(&config.llm, "openrouter").ok(),
        "anthropic" | "openai" | "gemini" => {
            let key = resolve_api_key(&config.llm, provider)
                .map_err(|e| ModelListError::Config(format!("No API key for {}: {}", provider, e)))?;
            Some(key)
        }
        other => return Err(ModelListError::UnknownProvider(other.to_string())),
    };

    let models = match provider {
        "anthropic" => fetch_anthropic_models(api_key.as_deref().unwrap_or("")).await?,
        "openrouter" => fetch_openrouter_models(api_key.as_deref()).await?,
        "openai" => fetch_openai_models(api_key.as_deref().unwrap_or("")).await?,
        "gemini" => fetch_gemini_models(api_key.as_deref().unwrap_or("")).await?,
        _ => unreachable!(),
    };

    if let Err(e) = write_models_cache(&cache_dir, &cache_file, &models) {
        warn!("Failed to cache model list for {}: {}", provider, e);
    }

    Ok(models)
}

/// Fetch available models using a pre-resolved API key (skips config resolution).
///
/// Useful when the caller already has the key (e.g., Tauri settings panel).
pub async fn fetch_provider_models_with_key(
    provider: &str,
    api_key: Option<&str>,
    force_refresh: bool,
) -> Result<Vec<ModelInfo>, ModelListError> {
    let cache_dir = models_cache_dir();
    let cache_file = cache_dir.join(format!("{}.json", provider));

    if !force_refresh {
        if let Some(cached) = read_models_cache(&cache_file) {
            return Ok(cached);
        }
    }

    if matches!(provider, "codex" | "claude") {
        return Ok(static_models_for_cli_provider(provider));
    }

    let models = match provider {
        "anthropic" => fetch_anthropic_models(api_key.unwrap_or("")).await?,
        "openrouter" => fetch_openrouter_models(api_key).await?,
        "openai" => fetch_openai_models(api_key.unwrap_or("")).await?,
        "gemini" => fetch_gemini_models(api_key.unwrap_or("")).await?,
        other => return Err(ModelListError::UnknownProvider(other.to_string())),
    };

    if let Err(e) = write_models_cache(&cache_dir, &cache_file, &models) {
        warn!("Failed to cache model list for {}: {}", provider, e);
    }

    Ok(models)
}

/// All supported provider names for model listing.
pub const SUPPORTED_PROVIDERS: &[&str] = &[
    "anthropic",
    "openai",
    "gemini",
    "openrouter",
    "codex",
    "claude",
];

// ── Static models for CLI-backed providers ──────────────────────────

fn static_models_for_cli_provider(provider: &str) -> Vec<ModelInfo> {
    let models = match provider {
        "codex" => vec![
            ("default", "Default"),
            ("gpt-5.4", "GPT-5.4"),
            ("gpt-5.4-mini", "GPT-5.4 Mini"),
            ("gpt-4.1", "GPT-4.1"),
            ("o4-mini", "o4-mini"),
            ("o3", "o3"),
        ],
        "claude" => vec![
            ("default", "Default"),
            ("claude-opus-4-6", "Claude Opus 4.6"),
            ("claude-sonnet-4-6", "Claude Sonnet 4.6"),
            ("claude-haiku-4-5", "Claude Haiku 4.5"),
        ],
        _ => vec![],
    };
    models
        .into_iter()
        .map(|(id, name)| ModelInfo {
            id: id.to_string(),
            display_name: name.to_string(),
            context_length: None,
        })
        .collect()
}

// ── Disk cache ──────────────────────────────────────────────────────

fn models_cache_dir() -> PathBuf {
    DiffcoreConfig::global_config_path()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".diffcore")
        })
        .join("cache")
        .join("models")
}

fn read_models_cache(cache_file: &std::path::Path) -> Option<Vec<ModelInfo>> {
    let metadata = std::fs::metadata(cache_file).ok()?;
    let modified = metadata.modified().ok()?;
    let age = std::time::SystemTime::now()
        .duration_since(modified)
        .ok()?;

    // 24-hour TTL
    if age.as_secs() > 86_400 {
        return None;
    }

    let bytes = std::fs::read(cache_file).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn write_models_cache(
    cache_dir: &std::path::Path,
    cache_file: &std::path::Path,
    models: &[ModelInfo],
) -> Result<(), std::io::Error> {
    std::fs::create_dir_all(cache_dir)?;
    let json = serde_json::to_vec_pretty(models)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    std::fs::write(cache_file, json)?;
    Ok(())
}

// ── Provider API fetchers ───────────────────────────────────────────

async fn fetch_anthropic_models(api_key: &str) -> Result<Vec<ModelInfo>, ModelListError> {
    let client = reqwest::Client::new();
    let response = client
        .get("https://api.anthropic.com/v1/models")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .send()
        .await
        .map_err(|e| ModelListError::Network(format!("Anthropic models API: {}", e)))?;

    if !response.status().is_success() {
        return Err(ModelListError::Network(format!(
            "Anthropic models API returned {}",
            response.status()
        )));
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| ModelListError::Network(format!("Parse Anthropic models: {}", e)))?;

    let models = body["data"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|m| {
            let id = m["id"].as_str()?.to_string();
            let display_name = m["display_name"]
                .as_str()
                .unwrap_or_else(|| m["id"].as_str().unwrap_or(""))
                .to_string();
            Some(ModelInfo {
                id,
                display_name,
                context_length: None,
            })
        })
        .collect();

    Ok(models)
}

async fn fetch_openrouter_models(
    api_key: Option<&str>,
) -> Result<Vec<ModelInfo>, ModelListError> {
    let client = reqwest::Client::new();
    let mut request = client.get("https://openrouter.ai/api/v1/models");
    if let Some(key) = api_key {
        request = request.header("Authorization", format!("Bearer {}", key));
    }

    let response = request
        .send()
        .await
        .map_err(|e| ModelListError::Network(format!("OpenRouter models API: {}", e)))?;

    if !response.status().is_success() {
        return Err(ModelListError::Network(format!(
            "OpenRouter models API returned {}",
            response.status()
        )));
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| ModelListError::Network(format!("Parse OpenRouter models: {}", e)))?;

    let mut models: Vec<ModelInfo> = body["data"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|m| {
            let id = m["id"].as_str()?.to_string();
            let name = m["name"]
                .as_str()
                .unwrap_or_else(|| m["id"].as_str().unwrap_or(""))
                .to_string();
            let context_length = m["context_length"].as_u64();
            Some(ModelInfo {
                id,
                display_name: name,
                context_length,
            })
        })
        .collect();

    models.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    Ok(models)
}

async fn fetch_openai_models(api_key: &str) -> Result<Vec<ModelInfo>, ModelListError> {
    let client = reqwest::Client::new();
    let response = client
        .get("https://api.openai.com/v1/models")
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .map_err(|e| ModelListError::Network(format!("OpenAI models API: {}", e)))?;

    if !response.status().is_success() {
        return Err(ModelListError::Network(format!(
            "OpenAI models API returned {}",
            response.status()
        )));
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| ModelListError::Network(format!("Parse OpenAI models: {}", e)))?;

    let mut models: Vec<ModelInfo> = body["data"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|m| {
            let id = m["id"].as_str()?.to_string();
            if !id.starts_with("gpt-")
                && !id.starts_with("o1")
                && !id.starts_with("o3")
                && !id.starts_with("o4")
            {
                return None;
            }
            Some(ModelInfo {
                id: id.clone(),
                display_name: id,
                context_length: None,
            })
        })
        .collect();

    models.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(models)
}

async fn fetch_gemini_models(api_key: &str) -> Result<Vec<ModelInfo>, ModelListError> {
    let client = reqwest::Client::new();
    let response = client
        .get(format!(
            "https://generativelanguage.googleapis.com/v1beta/models?key={}",
            api_key
        ))
        .send()
        .await
        .map_err(|e| ModelListError::Network(format!("Gemini models API: {}", e)))?;

    if !response.status().is_success() {
        return Err(ModelListError::Network(format!(
            "Gemini models API returned {}",
            response.status()
        )));
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| ModelListError::Network(format!("Parse Gemini models: {}", e)))?;

    let models: Vec<ModelInfo> = body["models"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|m| {
            let full_name = m["name"].as_str()?;
            let id = full_name.strip_prefix("models/").unwrap_or(full_name);
            let display_name = m["displayName"]
                .as_str()
                .unwrap_or(id)
                .to_string();
            let methods = m["supportedGenerationMethods"].as_array()?;
            let supports_content = methods
                .iter()
                .any(|m| m.as_str() == Some("generateContent"));
            if !supports_content {
                return None;
            }
            Some(ModelInfo {
                id: id.to_string(),
                display_name,
                context_length: m["inputTokenLimit"].as_u64(),
            })
        })
        .collect();

    Ok(models)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_codex_models_are_nonempty() {
        let models = static_models_for_cli_provider("codex");
        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.id == "default"));
    }

    #[test]
    fn static_claude_models_are_nonempty() {
        let models = static_models_for_cli_provider("claude");
        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.id == "claude-sonnet-4-6"));
    }

    #[test]
    fn unknown_cli_provider_returns_empty() {
        let models = static_models_for_cli_provider("nonexistent");
        assert!(models.is_empty());
    }

    #[test]
    fn supported_providers_list() {
        assert!(SUPPORTED_PROVIDERS.contains(&"anthropic"));
        assert!(SUPPORTED_PROVIDERS.contains(&"openrouter"));
        assert!(!SUPPORTED_PROVIDERS.contains(&"unknown"));
    }
}
