//! GitHub Copilot API client for diffcore LLM annotations.
//!
//! Routes requests through GitHub Copilot's hosted chat endpoint
//! (`https://api.githubcopilot.com/chat/completions`). The endpoint speaks
//! the OpenAI Chat Completions wire format, so the request/response shapes
//! mirror the OpenRouter provider — only the auth headers differ.
//!
//! ## Auth
//!
//! Copilot accepts a short-lived session token in the `Authorization` header.
//! We accept whatever the user pastes (or what's in `GITHUB_COPILOT_TOKEN` /
//! `DIFFCORE_API_KEY`) and send it as a bearer credential. Users who only
//! have a GitHub OAuth token (`ghu_...` / `gho_...`) need to exchange it
//! for a Copilot session token themselves before pasting; we deliberately
//! avoid doing that exchange here to keep this provider as thin as the
//! OpenRouter one.
//!
//! Copilot's gateway also requires editor-identification headers
//! (`Editor-Version`, `Editor-Plugin-Version`, `Copilot-Integration-Id`).
//! The values below are stable, public identifiers used by other
//! third-party Copilot integrations and are not tied to a specific IDE.
//!
//! ## Models
//!
//! Copilot does not expose a public `/models` listing endpoint, so the model
//! list is static (see `llm::models::static_models_for_cli_provider`). Common
//! ids include `gpt-4.1`, `gpt-5`, `claude-sonnet-4-6`, and `gemini-2.5-pro`.

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::schema::{
    flatten_json_schema, judge_json_schema, pass1_json_schema, pass2_json_schema,
    refinement_json_schema, JudgeResponse, Pass1Response, Pass2Response, RefinementResponse,
};
use super::{
    judge_system_prompt, judge_user_prompt, pass1_system_prompt, pass1_user_prompt,
    pass2_system_prompt, pass2_user_prompt, refinement_system_prompt, refinement_user_prompt,
    truncate_to_token_budget, LlmError, LlmProvider,
};
use crate::llm::schema::{JudgeRequest, Pass1Request, Pass2Request, RefinementRequest};

const GITHUB_COPILOT_API_URL: &str = "https://api.githubcopilot.com/chat/completions";

/// Identifies diffcore to Copilot's gateway. Required by the API.
const COPILOT_INTEGRATION_ID: &str = "vscode-chat";
/// Editor-Version header value. Copilot rejects requests without it.
const COPILOT_EDITOR_VERSION: &str = "diffcore/0.1.0";
/// Editor-Plugin-Version header value. Mirrors the diffcore release line.
const COPILOT_EDITOR_PLUGIN_VERSION: &str = "diffcore/0.1.0";

/// GitHub Copilot chat provider.
///
/// Talks the OpenAI-compatible chat completions wire format to Copilot's
/// hosted gateway. Always asks for `json_schema` structured output; Copilot
/// honors it for models that support it natively and falls back to plain
/// text for the rest, which the parser tolerates by stripping any markdown
/// fencing the model adds around the JSON payload.
#[derive(Debug, Clone)]
pub struct GitHubCopilotProvider {
    api_key: String,
    model: String,
    client: Client,
    /// Base URL (overridable for testing with mock servers).
    base_url: String,
}

impl GitHubCopilotProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            api_key,
            model,
            client: Client::new(),
            base_url: GITHUB_COPILOT_API_URL.to_string(),
        }
    }

    /// Create with a custom base URL (for testing with mock servers).
    #[cfg(any(test, feature = "test-support"))]
    pub fn with_base_url(api_key: String, model: String, base_url: String) -> Self {
        Self {
            api_key,
            model,
            client: Client::new(),
            base_url,
        }
    }

    /// Build and send a Chat Completions request with structured output support.
    ///
    /// We always request `json_schema` structured outputs — Copilot will
    /// downgrade gracefully for models that don't support it natively, in
    /// which case the model still produces JSON content that the JSON-fence
    /// stripper in `parse_json_response` can consume.
    async fn send_structured_message(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        schema: serde_json::Value,
        schema_name: &str,
    ) -> Result<String, LlmError> {
        let schema = flatten_json_schema(schema);

        let max_input = self.max_context_tokens().saturating_sub(4096);
        let truncated_user = truncate_to_token_budget(user_prompt, max_input);

        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: Some(system_prompt.to_string()),
            },
            ChatMessage {
                role: "user".to_string(),
                content: Some(truncated_user),
            },
        ];

        let response_format = CopilotResponseFormat {
            r#type: "json_schema".to_string(),
            json_schema: Some(CopilotJsonSchema {
                name: schema_name.to_string(),
                strict: true,
                schema,
            }),
        };

        let request = CopilotRequest {
            model: self.model.clone(),
            messages,
            temperature: Some(0.0),
            max_tokens: Some(4096),
            response_format: Some(response_format),
        };

        let response = self
            .client
            .post(&self.base_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            // Copilot's gateway rejects requests that don't identify the
            // editor / integration — these headers are required, not optional.
            .header("Editor-Version", COPILOT_EDITOR_VERSION)
            .header("Editor-Plugin-Version", COPILOT_EDITOR_PLUGIN_VERSION)
            .header("Copilot-Integration-Id", COPILOT_INTEGRATION_ID)
            .header("User-Agent", "diffcore/0.1.0")
            .json(&request)
            .send()
            .await?;

        let status = response.status().as_u16();

        if status == 429 {
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok());
            return Err(LlmError::RateLimited {
                retry_after_secs: retry_after,
            });
        }

        if status == 401 || status == 403 {
            // Most common reason: the user pasted a GitHub OAuth token
            // (`ghu_…` / `gho_…`) instead of a Copilot session token.
            return Err(LlmError::AuthError(
                "Invalid GitHub Copilot token. Paste a Copilot session token, \
                 not a raw GitHub OAuth token."
                    .to_string(),
            ));
        }

        if status == 408 || status == 504 {
            return Err(LlmError::Timeout(120));
        }

        super::check_response_size(&response)?;
        let body = response.text().await?;

        if status != 200 {
            return Err(LlmError::ApiError {
                status,
                message: super::redact_api_keys(&body),
            });
        }

        let api_response: CopilotResponse = serde_json::from_str(&body).map_err(|e| {
            LlmError::ParseResponse(format!(
                "Failed to parse GitHub Copilot response: {} — body: {}",
                e,
                &body[..body.len().min(500)]
            ))
        })?;

        let text = api_response
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .unwrap_or_default();

        if text.is_empty() {
            return Err(LlmError::ParseResponse(
                "GitHub Copilot response contained no text content".to_string(),
            ));
        }

        Ok(text)
    }
}

/// Conservative context-window estimate for Copilot-routed models.
///
/// Copilot proxies a small handful of upstream models (OpenAI, Anthropic,
/// Google, …). We mirror the same heuristics OpenRouter uses so prompt
/// truncation stays predictable across providers.
fn copilot_context_window(model: &str) -> usize {
    if model.contains("gemini-2") || model.contains("gpt-4.1") || model.contains("gpt-5") {
        1_000_000
    } else if model.contains("claude-sonnet-4")
        || model.contains("claude-opus-4")
        || model.contains("claude-3.5")
        || model.contains("claude-3-opus")
    {
        200_000
    } else if model.contains("o3") || model.contains("o4") {
        128_000
    } else {
        128_000
    }
}

#[async_trait]
impl LlmProvider for GitHubCopilotProvider {
    fn name(&self) -> &str {
        "github_copilot"
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn max_context_tokens(&self) -> usize {
        copilot_context_window(&self.model)
    }

    async fn annotate_overview(&self, request: &Pass1Request) -> Result<Pass1Response, LlmError> {
        let system = pass1_system_prompt();
        let user = pass1_user_prompt(request);
        let response_text = self
            .send_structured_message(&system, &user, pass1_json_schema(), "pass1_response")
            .await?;
        parse_json_response::<Pass1Response>(&response_text)
    }

    async fn annotate_group(&self, request: &Pass2Request) -> Result<Pass2Response, LlmError> {
        let system = pass2_system_prompt();
        let user = pass2_user_prompt(request);
        let response_text = self
            .send_structured_message(&system, &user, pass2_json_schema(), "pass2_response")
            .await?;
        parse_json_response::<Pass2Response>(&response_text)
    }

    async fn evaluate_quality(&self, request: &JudgeRequest) -> Result<JudgeResponse, LlmError> {
        let system = judge_system_prompt();
        let user = judge_user_prompt(request);
        let response_text = self
            .send_structured_message(&system, &user, judge_json_schema(), "judge_response")
            .await?;
        parse_json_response::<JudgeResponse>(&response_text)
    }

    async fn refine_groups(
        &self,
        request: &RefinementRequest,
    ) -> Result<RefinementResponse, LlmError> {
        let system = refinement_system_prompt();
        let user = refinement_user_prompt(request);
        let response_text = self
            .send_structured_message(
                &system,
                &user,
                refinement_json_schema(),
                "refinement_response",
            )
            .await?;
        parse_json_response::<RefinementResponse>(&response_text)
    }
}

/// Parse a JSON response, stripping any markdown fencing the LLM may add.
fn parse_json_response<T: serde::de::DeserializeOwned>(text: &str) -> Result<T, LlmError> {
    super::parse_structured_json_response(text)
}

/// Strip markdown code fences from JSON responses.
#[cfg(test)]
fn strip_markdown_json(text: &str) -> String {
    super::strip_markdown_json(text)
}

// ── Copilot API Types ──
// OpenAI-compatible. Distinct local types (vs reusing OpenRouter's) keep the
// providers loosely coupled — Copilot's wire format is free to drift.

#[derive(Debug, Serialize)]
struct CopilotRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<CopilotResponseFormat>,
}

#[derive(Debug, Serialize)]
struct CopilotResponseFormat {
    r#type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    json_schema: Option<CopilotJsonSchema>,
}

#[derive(Debug, Serialize)]
struct CopilotJsonSchema {
    name: String,
    strict: bool,
    schema: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CopilotResponse {
    choices: Vec<CopilotChoice>,
    #[allow(dead_code)]
    model: Option<String>,
    #[allow(dead_code)]
    usage: Option<CopilotUsage>,
}

#[derive(Debug, Deserialize)]
struct CopilotChoice {
    message: ChatMessage,
    #[allow(dead_code)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct CopilotUsage {
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
    total_tokens: Option<u64>,
}

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
    fn test_copilot_request_format() {
        let schema = serde_json::json!({"type": "object", "properties": {}});
        let request = CopilotRequest {
            model: "gpt-4.1".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: Some("hi".to_string()),
            }],
            temperature: Some(0.0),
            max_tokens: Some(4096),
            response_format: Some(CopilotResponseFormat {
                r#type: "json_schema".to_string(),
                json_schema: Some(CopilotJsonSchema {
                    name: "test".to_string(),
                    strict: true,
                    schema,
                }),
            }),
        };

        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["model"], "gpt-4.1");
        assert_eq!(json["response_format"]["type"], "json_schema");
        assert!(json["response_format"]["json_schema"]["strict"]
            .as_bool()
            .unwrap());
    }

    #[test]
    fn test_copilot_response_parsing() {
        let body = r#"{"choices":[{"message":{"role":"assistant","content":"{\"a\":1}"},"finish_reason":"stop"}],"model":"gpt-4.1","usage":{"prompt_tokens":5,"completion_tokens":3,"total_tokens":8}}"#;
        let resp: CopilotResponse = serde_json::from_str(body).unwrap();
        assert_eq!(resp.choices.len(), 1);
        assert_eq!(
            resp.choices[0].message.content.as_deref().unwrap(),
            "{\"a\":1}"
        );
    }

    #[test]
    fn test_context_window_lookup() {
        assert_eq!(copilot_context_window("gpt-4.1"), 1_000_000);
        assert_eq!(copilot_context_window("claude-sonnet-4-6"), 200_000);
        assert_eq!(copilot_context_window("o4-mini"), 128_000);
        assert_eq!(copilot_context_window("some-unknown-model"), 128_000);
    }

    #[test]
    fn test_strip_markdown_json() {
        assert_eq!(strip_markdown_json(r#"{"a":1}"#), r#"{"a":1}"#);
        assert_eq!(strip_markdown_json("```json\n{\"a\":1}\n```"), r#"{"a":1}"#);
        assert_eq!(strip_markdown_json("```\n{\"a\":1}\n```"), r#"{"a":1}"#);
    }

    #[test]
    fn test_provider_name_and_model() {
        let provider = GitHubCopilotProvider::new("tok".to_string(), "gpt-4.1".to_string());
        assert_eq!(provider.name(), "github_copilot");
        assert_eq!(provider.model(), "gpt-4.1");
    }
}
