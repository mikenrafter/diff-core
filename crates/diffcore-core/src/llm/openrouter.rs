//! OpenRouter API client for diffcore LLM annotations.
//!
//! Routes requests through OpenRouter's unified API to access models from
//! any provider (Anthropic, OpenAI, Meta, Mistral, DeepSeek, Google, etc.).
//! Uses the OpenAI-compatible `/api/v1/chat/completions` endpoint with
//! `response_format: { type: "json_schema" }` for structured outputs.
//!
//! Model names use OpenRouter's `provider/model` format, e.g.:
//! - `anthropic/claude-sonnet-4-6`
//! - `openai/gpt-4.1`
//! - `meta-llama/llama-4-maverick`
//! - `deepseek/deepseek-r1`
//! - `google/gemini-2.5-flash`

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

const OPENROUTER_API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

/// OpenRouter unified API provider.
///
/// Proxies requests to any model available on OpenRouter using their
/// OpenAI-compatible chat completions endpoint. Supports structured outputs
/// via `json_schema` response format for models that advertise it.
#[derive(Debug, Clone)]
pub struct OpenRouterProvider {
    api_key: String,
    model: String,
    client: Client,
    /// Base URL (overridable for testing).
    base_url: String,
}

impl OpenRouterProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            api_key,
            model,
            client: Client::new(),
            base_url: OPENROUTER_API_URL.to_string(),
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
    /// OpenRouter's API is OpenAI-compatible but routes to many underlying providers.
    /// We always request `json_schema` structured outputs — OpenRouter will downgrade
    /// gracefully for models that don't support it natively.
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

        let response_format = OpenRouterResponseFormat {
            r#type: "json_schema".to_string(),
            json_schema: Some(OpenRouterJsonSchema {
                name: schema_name.to_string(),
                strict: true,
                schema,
            }),
        };

        let request = OpenRouterRequest {
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
            .header("HTTP-Referer", "https://github.com/diffcore/diffcore")
            .header("X-Title", "diffcore")
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
            return Err(LlmError::AuthError(
                "Invalid OpenRouter API key".to_string(),
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

        let api_response: OpenRouterResponse = serde_json::from_str(&body).map_err(|e| {
            LlmError::ParseResponse(format!(
                "Failed to parse OpenRouter response: {} — body: {}",
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
                "OpenRouter response contained no text content".to_string(),
            ));
        }

        Ok(text)
    }
}

/// Conservative context window estimate for OpenRouter models.
///
/// OpenRouter hosts hundreds of models with varying context windows.
/// We use a generous default since the user explicitly chose the model.
fn openrouter_context_window(model: &str) -> usize {
    // Well-known large-context models
    if model.contains("gemini-2") || model.contains("gpt-4.1") || model.contains("gpt-5") {
        1_000_000
    } else if model.contains("claude-sonnet-4")
        || model.contains("claude-opus-4")
        || model.contains("claude-3.5")
        || model.contains("claude-3-opus")
    {
        200_000
    } else if model.contains("deepseek") {
        128_000
    } else if model.contains("llama-4") || model.contains("llama-3") {
        128_000
    } else if model.contains("mistral-large") || model.contains("mixtral") {
        128_000
    } else if model.contains("qwen") {
        128_000
    } else {
        // Safe default for unknown models — most modern models support at least 32k
        128_000
    }
}

#[async_trait]
impl LlmProvider for OpenRouterProvider {
    fn name(&self) -> &str {
        "openrouter"
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn max_context_tokens(&self) -> usize {
        openrouter_context_window(&self.model)
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
    let cleaned = strip_markdown_json(text);
    serde_json::from_str(&cleaned).map_err(|e| {
        LlmError::ParseResponse(format!(
            "Failed to parse structured output: {} — response: {}",
            e,
            &cleaned[..cleaned.len().min(500)]
        ))
    })
}

/// Strip markdown code fences from JSON responses.
fn strip_markdown_json(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.starts_with("```json") {
        let after_fence = &trimmed[7..];
        if let Some(end) = after_fence.rfind("```") {
            return after_fence[..end].trim().to_string();
        }
    }
    if trimmed.starts_with("```") {
        let after_fence = &trimmed[3..];
        if let Some(end) = after_fence.rfind("```") {
            return after_fence[..end].trim().to_string();
        }
    }
    trimmed.to_string()
}

// ── OpenRouter API Types ──
// OpenAI-compatible with minor extensions (native_finish_reason).

#[derive(Debug, Serialize)]
struct OpenRouterRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<OpenRouterResponseFormat>,
}

#[derive(Debug, Serialize)]
struct OpenRouterResponseFormat {
    r#type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    json_schema: Option<OpenRouterJsonSchema>,
}

#[derive(Debug, Serialize)]
struct OpenRouterJsonSchema {
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
struct OpenRouterResponse {
    choices: Vec<OpenRouterChoice>,
    #[allow(dead_code)]
    model: Option<String>,
    #[allow(dead_code)]
    usage: Option<OpenRouterUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterChoice {
    message: ChatMessage,
    #[allow(dead_code)]
    finish_reason: Option<String>,
    #[allow(dead_code)]
    native_finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OpenRouterUsage {
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
    fn test_openrouter_request_format() {
        let schema = serde_json::json!({"type": "object", "properties": {}});
        let request = OpenRouterRequest {
            model: "anthropic/claude-sonnet-4-6".to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: Some("You are a reviewer".to_string()),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: Some("Review this".to_string()),
                },
            ],
            temperature: Some(0.0),
            max_tokens: Some(4096),
            response_format: Some(OpenRouterResponseFormat {
                r#type: "json_schema".to_string(),
                json_schema: Some(OpenRouterJsonSchema {
                    name: "test".to_string(),
                    strict: true,
                    schema,
                }),
            }),
        };

        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["model"], "anthropic/claude-sonnet-4-6");
        assert_eq!(json["response_format"]["type"], "json_schema");
        assert!(json["response_format"]["json_schema"]["strict"].as_bool().unwrap());
    }

    #[test]
    fn test_openrouter_response_parsing() {
        let body = r#"{"choices":[{"message":{"role":"assistant","content":"{\"key\":\"value\"}"},"finish_reason":"stop","native_finish_reason":"end_turn"}],"model":"anthropic/claude-sonnet-4-6","usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}}"#;
        let resp: OpenRouterResponse = serde_json::from_str(body).unwrap();
        assert_eq!(resp.choices.len(), 1);
        assert_eq!(
            resp.choices[0].message.content.as_deref().unwrap(),
            "{\"key\":\"value\"}"
        );
        assert_eq!(
            resp.choices[0].native_finish_reason.as_deref().unwrap(),
            "end_turn"
        );
    }

    #[test]
    fn test_context_window_lookup() {
        assert_eq!(
            openrouter_context_window("anthropic/claude-sonnet-4-6"),
            200_000
        );
        assert_eq!(
            openrouter_context_window("google/gemini-2.5-flash"),
            1_000_000
        );
        assert_eq!(
            openrouter_context_window("meta-llama/llama-4-maverick"),
            128_000
        );
        assert_eq!(
            openrouter_context_window("deepseek/deepseek-r1"),
            128_000
        );
        // Unknown model gets safe default
        assert_eq!(
            openrouter_context_window("some-unknown/model"),
            128_000
        );
    }

    #[test]
    fn test_strip_markdown_json() {
        assert_eq!(strip_markdown_json(r#"{"a":1}"#), r#"{"a":1}"#);
        assert_eq!(
            strip_markdown_json("```json\n{\"a\":1}\n```"),
            r#"{"a":1}"#
        );
        assert_eq!(strip_markdown_json("```\n{\"a\":1}\n```"), r#"{"a":1}"#);
    }

    #[test]
    fn test_provider_name_and_model() {
        let provider =
            OpenRouterProvider::new("key".to_string(), "meta-llama/llama-4-maverick".to_string());
        assert_eq!(provider.name(), "openrouter");
        assert_eq!(provider.model(), "meta-llama/llama-4-maverick");
    }
}
