//! LLM annotation and refinement commands.

use std::path::PathBuf;
use std::sync::Arc;

use log::warn;

use crate::activity_stream::{ActivityEntry, JobHandle};
use diffcore_core::cache;
use diffcore_core::llm;
use diffcore_core::llm::refinement;
use diffcore_core::llm::schema::{Pass1Response, Pass2Response, RefinementResponse};
use diffcore_core::types::AnalysisOutput;

use super::{AppState, CommandError};

fn build_pass1_request(
    analysis: &AnalysisOutput,
    reanalysis_context: Option<&str>,
) -> llm::schema::Pass1Request {
    let flow_groups: Vec<llm::schema::Pass1GroupInput> = analysis
        .groups
        .iter()
        .map(|g| llm::schema::Pass1GroupInput {
            id: g.id.clone(),
            name: g.name.clone(),
            entrypoint: g
                .entrypoint
                .as_ref()
                .map(|ep| format!("{}::{}", ep.file, ep.symbol)),
            files: g.files.iter().map(|f| f.path.clone()).collect(),
            risk_score: g.risk_score,
            edge_summary: g
                .edges
                .iter()
                .map(|e| format!("{} -> {}", e.from, e.to))
                .collect::<Vec<_>>()
                .join(", "),
        })
        .collect();

    let mut diff_summary = format!(
            "{} files changed across {} groups",
            analysis.summary.total_files_changed, analysis.summary.total_groups,
        );

    if let Some(context) = reanalysis_context {
        let trimmed = context.trim();
        if !trimmed.is_empty() {
            diff_summary.push_str("\n\n## Reanalysis Context\n");
            diff_summary.push_str(trimmed);
        }
    }

    llm::schema::Pass1Request {
        diff_summary,
        flow_groups,
        graph_summary: format!(
            "{} groups, {} total files",
            analysis.summary.total_groups, analysis.summary.total_files_changed,
        ),
    }
}

fn build_overview_reanalysis_context(
    user_feedback: Option<String>,
    include_previous_output: Option<bool>,
    previous_output: Option<String>,
    user_comments: Option<Vec<String>>,
) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();

    if let Some(feedback) = user_feedback {
        let trimmed = feedback.trim();
        if !trimmed.is_empty() {
            parts.push(format!("User feedback/question:\n{}", trimmed));
        }
    }

    if let Some(comments) = user_comments {
        let non_empty: Vec<String> = comments
            .into_iter()
            .map(|c| c.trim().to_string())
            .filter(|c| !c.is_empty())
            .collect();
        if !non_empty.is_empty() {
            parts.push(format!("Review comments:\n- {}", non_empty.join("\n- ")));
        }
    }

    if include_previous_output.unwrap_or(false) {
        if let Some(previous) = previous_output {
            let trimmed = previous.trim();
            if !trimmed.is_empty() {
                parts.push(format!("Previous output to consider:\n{}", trimmed));
            }
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n\n"))
    }
}

fn build_pass2_request(
    analysis: &AnalysisOutput,
    group_id: &str,
    repo_path: &str,
    base: Option<String>,
    head: Option<String>,
    range: Option<String>,
    staged: bool,
    unstaged: bool,
    include_uncommitted: bool,
) -> Result<llm::schema::Pass2Request, CommandError> {
    let group = analysis
        .groups
        .iter()
        .find(|g| g.id == group_id)
        .ok_or_else(|| CommandError::Analysis(format!("Group '{}' not found", group_id)))?
        .clone();

    let repo_path_buf = PathBuf::from(repo_path);
    let repo_path_buf = std::fs::canonicalize(&repo_path_buf)
        .map_err(|e| CommandError::Io(format!("Invalid repo path: {}", e)))?;
    let repo = git2::Repository::discover(&repo_path_buf)
        .map_err(|e| CommandError::Git(format!("Not a git repository: {}", e)))?;

    let (diff_result, _) = super::extract_diff(&repo, base, head, range, staged, unstaged, false, include_uncommitted)?;

    let files: Vec<llm::schema::Pass2FileInput> = group
        .files
        .iter()
        .map(|f| {
            let file_diff = diff_result.files.iter().find(|d| d.path() == f.path);
            let diff_text = file_diff
                .map(|d| {
                    let old = d.old_content.as_deref().unwrap_or("");
                    let new = d.new_content.as_deref().unwrap_or("");
                    format!(
                        "--- a/{}\n+++ b/{}\n{}",
                        f.path,
                        f.path,
                        super::simple_unified_diff(old, new)
                    )
                })
                .unwrap_or_default();
            let new_content = file_diff.and_then(|d| d.new_content.clone());

            llm::schema::Pass2FileInput {
                path: f.path.clone(),
                diff: diff_text,
                new_content,
                role: format!("{:?}", f.role),
            }
        })
        .collect();

    let graph_context = group
        .edges
        .iter()
        .map(|e| format!("{} --{:?}--> {}", e.from, e.edge_type, e.to))
        .collect::<Vec<_>>()
        .join("\n");

    Ok(llm::schema::Pass2Request {
        group_id: group.id.clone(),
        group_name: group.name.clone(),
        files,
        graph_context,
    })
}

fn make_activity_callback(
    job: JobHandle,
) -> Arc<dyn Fn(llm::ActivityUpdate) + Send + Sync + 'static> {
    Arc::new(move |update| {
        let job = job.clone();
        tauri::async_runtime::spawn(async move {
            job.emit(ActivityEntry {
                source: update.source,
                level: update.level,
                message: update.message,
                event_type: update.event_type,
                payload: update.payload,
                timestamp_ms: update.timestamp_ms,
            })
            .await;
        });
    })
}

async fn emit_diffcore_activity(job: &JobHandle, message: impl Into<String>) {
    job.emit(ActivityEntry::info("diffcore", message, None))
        .await;
}

async fn emit_diffcore_warning(
    job: &JobHandle,
    source: impl Into<String>,
    message: impl Into<String>,
    event_type: Option<String>,
    payload: Option<serde_json::Value>,
) {
    job.emit(ActivityEntry {
        source: source.into(),
        level: "warning".to_string(),
        message: message.into(),
        event_type,
        payload,
        timestamp_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or(0),
    })
    .await;
}

async fn emit_direct_api_activity_notice(job: &JobHandle, provider: &str) {
    if super::provider_supports_tool_activity(provider) {
        return;
    }

    emit_diffcore_activity(
        job,
        "Direct API mode only shows high-level progress. Switch to Codex CLI or Claude Code to stream file reads, greps, and shell activity.",
    )
    .await;
}

fn refinement_reasoning_excerpt(reasoning: &str) -> Option<String> {
    let trimmed = reasoning.split_whitespace().collect::<Vec<_>>().join(" ");
    if trimmed.is_empty() {
        return None;
    }

    let sentence_end = trimmed.find(". ").map(|index| index + 1);
    let excerpt = sentence_end
        .map(|index| trimmed[..index].trim().to_string())
        .unwrap_or_else(|| trimmed.chars().take(220).collect::<String>());

    if excerpt.is_empty() {
        None
    } else if excerpt.chars().count() < trimmed.chars().count() && sentence_end.is_none() {
        Some(format!("{}...", excerpt))
    } else {
        Some(excerpt)
    }
}

fn refinement_operations_summary(response: &RefinementResponse) -> String {
    let mut parts = Vec::new();

    if !response.splits.is_empty() {
        parts.push(format!(
            "{} split{}",
            response.splits.len(),
            if response.splits.len() == 1 { "" } else { "s" }
        ));
    }
    if !response.merges.is_empty() {
        parts.push(format!(
            "{} merge{}",
            response.merges.len(),
            if response.merges.len() == 1 { "" } else { "s" }
        ));
    }
    if !response.re_ranks.is_empty() {
        parts.push(format!(
            "{} re-rank{}",
            response.re_ranks.len(),
            if response.re_ranks.len() == 1 {
                ""
            } else {
                "s"
            }
        ));
    }
    if !response.reclassifications.is_empty() {
        parts.push(format!(
            "{} reclassification{}",
            response.reclassifications.len(),
            if response.reclassifications.len() == 1 {
                ""
            } else {
                "s"
            }
        ));
    }

    if parts.is_empty() {
        "no structural changes".to_string()
    } else {
        parts.join(", ")
    }
}

fn empty_refinement_response(reasoning: String) -> RefinementResponse {
    RefinementResponse {
        splits: vec![],
        merges: vec![],
        re_ranks: vec![],
        reclassifications: vec![],
        reasoning,
    }
}

fn fallback_refinement_result(
    analysis: &AnalysisOutput,
    provider: String,
    model: String,
    message: String,
) -> RefinementResult {
    RefinementResult {
        refined_groups: analysis.groups.clone(),
        infrastructure_group: analysis.infrastructure_group.clone(),
        refinement_response: empty_refinement_response(message.clone()),
        provider,
        model,
        had_changes: false,
        warnings: vec![refinement::fallback_warning(message)],
        stop_reason: refinement::RefinementIterationStopReason::ProviderFailure,
        attempts_used: 1,
        parse_failures: 0,
    }
}

async fn run_overview_with_activity(
    request: llm::schema::Pass1Request,
    llm_config: diffcore_core::config::LlmConfig,
    workdir: Option<PathBuf>,
    job: JobHandle,
) -> Result<Pass1Response, CommandError> {
    emit_diffcore_activity(&job, "Preparing overview request").await;
    let provider = llm::create_provider_for_workdir(&llm_config, workdir.as_deref())
        .map_err(|e| CommandError::Llm(format!("{}", e)))?;
    let provider_name = provider.name().to_string();
    let provider_model = provider.model().to_string();
    emit_diffcore_activity(
        &job,
        format!("Using {} / {}", provider_name, provider_model),
    )
    .await;
    emit_direct_api_activity_notice(&job, &provider_name).await;

    llm::with_activity_callback(make_activity_callback(job), async {
        provider.annotate_overview(&request).await
    })
    .await
    .map_err(|e| CommandError::Llm(format!("{}", e)))
}

async fn run_group_with_activity(
    request: llm::schema::Pass2Request,
    llm_config: diffcore_core::config::LlmConfig,
    workdir: Option<PathBuf>,
    job: JobHandle,
) -> Result<Pass2Response, CommandError> {
    emit_diffcore_activity(&job, "Preparing deep analysis request").await;
    let provider = llm::create_provider_for_workdir(&llm_config, workdir.as_deref())
        .map_err(|e| CommandError::Llm(format!("{}", e)))?;
    let provider_name = provider.name().to_string();
    let provider_model = provider.model().to_string();
    emit_diffcore_activity(
        &job,
        format!("Using {} / {}", provider_name, provider_model),
    )
    .await;
    emit_direct_api_activity_notice(&job, &provider_name).await;

    llm::with_activity_callback(make_activity_callback(job), async {
        provider.annotate_group(&request).await
    })
    .await
    .map_err(|e| CommandError::Llm(format!("{}", e)))
}

async fn run_refinement_with_activity(
    analysis: AnalysisOutput,
    refinement_llm_config: diffcore_core::config::LlmConfig,
    workdir: Option<PathBuf>,
    job: JobHandle,
) -> Result<RefinementResult, CommandError> {
    emit_diffcore_activity(&job, "Preparing refinement request").await;
    let provider = llm::create_provider_for_workdir(&refinement_llm_config, workdir.as_deref())
        .map_err(|e| CommandError::Llm(format!("{}", e)))?;
    let provider_name = provider.name().to_string();
    let provider_model = provider.model().to_string();
    emit_diffcore_activity(
        &job,
        format!("Using {} / {}", provider_name, provider_model),
    )
    .await;
    emit_direct_api_activity_notice(&job, &provider_name).await;

    let analysis_json = serde_json::to_string_pretty(&analysis)
        .map_err(|e| CommandError::Llm(format!("Failed to serialize analysis: {}", e)))?;
    let diff_summary = format!(
        "{} files changed across {} groups",
        analysis.summary.total_files_changed, analysis.summary.total_groups,
    );
    let configured_provider_name = refinement_llm_config
        .provider
        .clone()
        .unwrap_or_else(|| "anthropic".to_string());
    let configured_model_name = refinement_llm_config
        .model
        .clone()
        .unwrap_or_else(|| super::default_model_for_provider(&configured_provider_name).to_string());
    let outcome = llm::with_activity_callback(make_activity_callback(job.clone()), async {
        refinement::run_refinement_iterations(
            provider.as_ref(),
            &analysis.groups,
            analysis.infrastructure_group.as_ref(),
            &analysis_json,
            &diff_summary,
            refinement_llm_config.refinement.max_iterations,
        )
        .await
    })
    .await;

    if outcome.parse_failures > 0
        && outcome.fallback_message.is_none()
        && outcome.attempts_used > 1
    {
        emit_diffcore_activity(
            &job,
            format!(
                "Refinement recovered after parse retry on attempt {}/{}",
                outcome.attempts_used,
                refinement_llm_config.refinement.max_iterations.max(1)
            ),
        )
        .await;
    }

    if let Some(fallback_message) = &outcome.fallback_message {
        emit_diffcore_warning(
            &job,
            configured_provider_name.clone(),
            fallback_message.clone(),
            Some("refinement.fallback".to_string()),
            Some(serde_json::json!({
                "attempts": outcome.attempts_used,
                "parse_failures": outcome.parse_failures,
                "reason": format!("{:?}", outcome.stop_reason),
            })),
        )
        .await;

        let mut result = fallback_refinement_result(
            &analysis,
            configured_provider_name,
            configured_model_name,
            fallback_message.clone(),
        );
        result.stop_reason = outcome.stop_reason;
        result.attempts_used = outcome.attempts_used;
        result.parse_failures = outcome.parse_failures;
        result.warnings = outcome.warnings;
        result.refinement_response = outcome.refinement_response;
        return Ok(result);
    }

    if let Some(reasoning) = refinement_reasoning_excerpt(&outcome.refinement_response.reasoning) {
        job.emit(ActivityEntry::info(
            provider_name.clone(),
            format!("Refinement rationale: {}", reasoning),
            Some("refinement.reasoning".to_string()),
        ))
        .await;
    }

    let provider_name = configured_provider_name;
    let model_name = configured_model_name;

    if !outcome.had_changes {
        emit_diffcore_activity(&job, "Refinement kept the current grouping").await;
        return Ok(RefinementResult {
            refined_groups: analysis.groups.clone(),
            infrastructure_group: analysis.infrastructure_group.clone(),
            refinement_response: outcome.refinement_response,
            provider: provider_name,
            model: model_name,
            had_changes: false,
            warnings: outcome.warnings,
            stop_reason: outcome.stop_reason,
            attempts_used: outcome.attempts_used,
            parse_failures: outcome.parse_failures,
        });
    }

    for warning in &outcome.warnings {
        let mut entry = ActivityEntry::info(
            provider_name.clone(),
            format!("Refinement warning: {}", warning.message),
            Some(warning.event_type().to_string()),
        );
        if matches!(
            &warning.action,
            diffcore_core::llm::refinement::RefinementWarningAction::Dropped { .. }
        ) {
            entry.level = "warning".to_string();
        }
        entry.payload = Some(warning.audit_payload());
        job.emit(entry).await;
    }

    emit_diffcore_activity(
        &job,
        format!(
            "Refinement proposed {}",
            refinement_operations_summary(&outcome.refinement_response)
        ),
    )
    .await;

    Ok(RefinementResult {
        refined_groups: outcome.refined_groups,
        infrastructure_group: outcome.infrastructure_group,
        refinement_response: outcome.refinement_response,
        provider: provider_name,
        model: model_name,
        had_changes: true,
        warnings: outcome.warnings,
        stop_reason: outcome.stop_reason,
        attempts_used: outcome.attempts_used,
        parse_failures: outcome.parse_failures,
    })
}

#[tauri::command]
pub fn start_annotate_overview(
    repo_path: Option<String>,
    llm_provider: Option<String>,
    llm_model: Option<String>,
    user_feedback: Option<String>,
    include_previous_output: Option<bool>,
    previous_output: Option<String>,
    user_comments: Option<Vec<String>>,
    state: tauri::State<'_, AppState>,
) -> Result<AsyncLlmJobStart, CommandError> {
    let analysis = super::load_cached_analysis(&state)?;
    let (mut config, workdir) = super::load_config_from_path(repo_path.as_deref());
    if let Some(provider) = llm_provider {
        config.llm.provider = Some(provider);
    }
    if let Some(model) = llm_model {
        config.llm.model = Some(model);
    }

    let provider_name = config
        .llm
        .provider
        .clone()
        .unwrap_or_else(|| "anthropic".to_string());
    let model_name = config
        .llm
        .model
        .clone()
        .unwrap_or_else(|| super::default_model_for_provider(&provider_name).to_string());
    let (job, start) =
        state.create_llm_job("overview", &provider_name, &model_name, "Summarizing PR")?;
    let llm_config = config.llm.clone();
    let reanalysis_context = build_overview_reanalysis_context(
        user_feedback,
        include_previous_output,
        previous_output,
        user_comments,
    );
    let request = build_pass1_request(&analysis, reanalysis_context.as_deref());

    tauri::async_runtime::spawn(async move {
        match run_overview_with_activity(request, llm_config, workdir, job.clone()).await {
            Ok(response) => match serde_json::to_value(&response) {
                Ok(value) => job.complete("overview", value).await,
                Err(error) => {
                    job.fail(format!("Failed to serialize overview response: {}", error))
                        .await
                }
            },
            Err(error) => job.fail(error.to_string()).await,
        }
    });

    Ok(start)
}

#[tauri::command]
pub fn start_annotate_group(
    group_id: String,
    repo_path: String,
    base: Option<String>,
    head: Option<String>,
    range: Option<String>,
    staged: bool,
    unstaged: bool,
    include_uncommitted: Option<bool>,
    llm_provider: Option<String>,
    llm_model: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<AsyncLlmJobStart, CommandError> {
    let analysis = super::load_cached_analysis(&state)?;
    let request = build_pass2_request(
        &analysis, &group_id, &repo_path, base, head, range, staged, unstaged, include_uncommitted.unwrap_or(true),
    )?;
    let (mut config, workdir) = super::load_config_from_path(Some(&repo_path));
    if let Some(provider) = llm_provider {
        config.llm.provider = Some(provider);
    }
    if let Some(model) = llm_model {
        config.llm.model = Some(model);
    }

    let provider_name = config
        .llm
        .provider
        .clone()
        .unwrap_or_else(|| "anthropic".to_string());
    let model_name = config
        .llm
        .model
        .clone()
        .unwrap_or_else(|| super::default_model_for_provider(&provider_name).to_string());
    let (job, start) = state.create_llm_job(
        "group",
        &provider_name,
        &model_name,
        &format!("Analyzing {}", group_id),
    )?;
    let llm_config = config.llm.clone();

    tauri::async_runtime::spawn(async move {
        match run_group_with_activity(request, llm_config, workdir, job.clone()).await {
            Ok(response) => match serde_json::to_value(&response) {
                Ok(value) => job.complete("group", value).await,
                Err(error) => {
                    job.fail(format!("Failed to serialize group response: {}", error))
                        .await
                }
            },
            Err(error) => job.fail(error.to_string()).await,
        }
    });

    Ok(start)
}

#[tauri::command]
pub fn start_refine_groups(
    repo_path: Option<String>,
    llm_provider: Option<String>,
    llm_model: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<AsyncLlmJobStart, CommandError> {
    let analysis = super::load_cached_analysis(&state)?;
    let (mut config, workdir) = super::load_config_from_path(repo_path.as_deref());
    if let Some(provider) = llm_provider {
        config.llm.refinement.provider = Some(provider.clone());
        if config.llm.provider.is_none() {
            config.llm.provider = Some(provider);
        }
    }
    if let Some(model) = llm_model {
        config.llm.refinement.model = Some(model.clone());
        if config.llm.model.is_none() {
            config.llm.model = Some(model);
        }
    }

    let refinement_llm_config = diffcore_core::config::LlmConfig {
        provider: config
            .llm
            .refinement
            .provider
            .clone()
            .or(config.llm.provider.clone()),
        model: config
            .llm
            .refinement
            .model
            .clone()
            .or(config.llm.model.clone()),
        key_cmd: config
            .llm
            .refinement
            .key_cmd
            .clone()
            .or(config.llm.key_cmd.clone()),
        key: config.llm.key.clone(),
        annotations_enabled: config.llm.annotations_enabled,
        refinement: config.llm.refinement.clone(),
    };

    let provider_name = refinement_llm_config
        .provider
        .clone()
        .unwrap_or_else(|| "anthropic".to_string());
    let model_name = refinement_llm_config
        .model
        .clone()
        .unwrap_or_else(|| super::default_model_for_provider(&provider_name).to_string());
    let (job, start) =
        state.create_llm_job("refinement", &provider_name, &model_name, "Refining groups")?;

    tauri::async_runtime::spawn(async move {
        match run_refinement_with_activity(analysis, refinement_llm_config, workdir, job.clone())
            .await
        {
            Ok(response) => match serde_json::to_value(&response) {
                Ok(value) => job.complete("refinement", value).await,
                Err(error) => {
                    job.fail(format!(
                        "Failed to serialize refinement response: {}",
                        error
                    ))
                    .await
                }
            },
            Err(error) => job.fail(error.to_string()).await,
        }
    });

    Ok(start)
}

/// Run LLM Pass 1 (overview annotation) on the cached analysis.
///
/// Returns structured overview with per-group summaries, risk flags,
/// and suggested review order. The result is also stored in the cached
/// analysis output's `annotations` field.
#[tauri::command]
pub async fn annotate_overview(
    repo_path: Option<String>,
    llm_provider: Option<String>,
    llm_model: Option<String>,
    user_feedback: Option<String>,
    include_previous_output: Option<bool>,
    previous_output: Option<String>,
    user_comments: Option<Vec<String>>,
    state: tauri::State<'_, AppState>,
) -> Result<Pass1Response, CommandError> {
    // Get the cached analysis to build the request
    let analysis = {
        let last = state
            .last_analysis
            .lock()
            .map_err(|e| CommandError::Analysis(format!("Lock poisoned: {}", e)))?;
        last.clone().ok_or_else(|| {
            CommandError::Analysis("No analysis available. Run analyze first.".into())
        })?
    };

    // Load config from the repo directory (not default)
    let (mut config, workdir) = super::load_config_from_path(repo_path.as_deref());

    // Apply frontend overrides if provided
    if let Some(p) = llm_provider {
        config.llm.provider = Some(p);
    }
    if let Some(m) = llm_model {
        config.llm.model = Some(m);
    }

    // Create LLM provider
    let provider = llm::create_provider_for_workdir(&config.llm, workdir.as_deref())
        .map_err(|e| CommandError::Llm(format!("{}", e)))?;

    let reanalysis_context = build_overview_reanalysis_context(
        user_feedback,
        include_previous_output,
        previous_output,
        user_comments,
    );
    let request = build_pass1_request(&analysis, reanalysis_context.as_deref());

    let response = provider
        .annotate_overview(&request)
        .await
        .map_err(|e| CommandError::Llm(format!("{}", e)))?;

    // Store the annotations in the cached analysis
    match state.last_analysis.lock() {
        Ok(mut last) => {
            if let Some(ref mut a) = *last {
                a.annotations = Some(serde_json::to_value(&response).map_err(|e| {
                    CommandError::Llm(format!("Failed to serialize response: {}", e))
                })?);
            }
        }
        Err(e) => warn!(
            "Failed to update last_analysis annotations (lock poisoned): {}",
            e
        ),
    }

    Ok(response)
}

/// Run LLM Pass 2 (deep analysis) on a specific group.
///
/// Returns per-file annotations, flow narrative, and cross-cutting concerns.
#[tauri::command]
pub async fn annotate_group(
    group_id: String,
    repo_path: String,
    base: Option<String>,
    head: Option<String>,
    range: Option<String>,
    staged: bool,
    unstaged: bool,
    include_uncommitted: Option<bool>,
    llm_provider: Option<String>,
    llm_model: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<Pass2Response, CommandError> {
    // Get the cached analysis to find the group
    let analysis = {
        let last = state
            .last_analysis
            .lock()
            .map_err(|e| CommandError::Analysis(format!("Lock poisoned: {}", e)))?;
        last.clone().ok_or_else(|| {
            CommandError::Analysis("No analysis available. Run analyze first.".into())
        })?
    };

    let group = analysis
        .groups
        .iter()
        .find(|g| g.id == group_id)
        .ok_or_else(|| CommandError::Analysis(format!("Group '{}' not found", group_id)))?
        .clone();

    // Get file diffs for Pass 2 context
    let repo_path_buf = PathBuf::from(&repo_path);
    let repo_path_buf = std::fs::canonicalize(&repo_path_buf)
        .map_err(|e| CommandError::Io(format!("Invalid repo path: {}", e)))?;
    let repo = git2::Repository::discover(&repo_path_buf)
        .map_err(|e| CommandError::Git(format!("Not a git repository: {}", e)))?;

    let (diff_result, _) = super::extract_diff(&repo, base, head, range, staged, unstaged, false, include_uncommitted.unwrap_or(true))?;

    // Build Pass 2 file inputs with diffs
    let files: Vec<llm::schema::Pass2FileInput> = group
        .files
        .iter()
        .map(|f| {
            let file_diff = diff_result.files.iter().find(|d| d.path() == f.path);
            let diff_text = file_diff
                .map(|d| {
                    // Build a simple unified diff representation
                    let old = d.old_content.as_deref().unwrap_or("");
                    let new = d.new_content.as_deref().unwrap_or("");
                    format!(
                        "--- a/{}\n+++ b/{}\n{}",
                        f.path,
                        f.path,
                        super::simple_unified_diff(old, new)
                    )
                })
                .unwrap_or_default();
            let new_content = file_diff.and_then(|d| d.new_content.clone());

            llm::schema::Pass2FileInput {
                path: f.path.clone(),
                diff: diff_text,
                new_content,
                role: format!("{:?}", f.role),
            }
        })
        .collect();

    // Build graph context
    let graph_context = group
        .edges
        .iter()
        .map(|e| format!("{} --{:?}--> {}", e.from, e.edge_type, e.to))
        .collect::<Vec<_>>()
        .join("\n");

    let (mut config, workdir) = super::load_config_from_path(Some(&repo_path));
    if let Some(p) = llm_provider {
        config.llm.provider = Some(p);
    }
    if let Some(m) = llm_model {
        config.llm.model = Some(m);
    }
    let provider = llm::create_provider_for_workdir(&config.llm, workdir.as_deref())
        .map_err(|e| CommandError::Llm(format!("{}", e)))?;

    let request = llm::schema::Pass2Request {
        group_id: group.id.clone(),
        group_name: group.name.clone(),
        files,
        graph_context,
    };

    let response = provider
        .annotate_group(&request)
        .await
        .map_err(|e| CommandError::Llm(format!("{}", e)))?;

    Ok(response)
}

/// Run LLM refinement pass on the cached analysis groups.
///
/// Takes the deterministic groups (v1) and asks an LLM to suggest structural
/// improvements: splits, merges, re-ranks, and reclassifications. Applies the
/// refinement operations and returns the result containing both the refined
/// groups and the raw refinement response (for change indicators in the UI).
///
/// Falls back to returning the original groups if refinement produces no changes
/// or validation fails.
#[tauri::command]
pub async fn refine_groups(
    repo_path: Option<String>,
    llm_provider: Option<String>,
    llm_model: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<RefinementResult, CommandError> {
    // Get the cached analysis
    let analysis = {
        let last = state
            .last_analysis
            .lock()
            .map_err(|e| CommandError::Analysis(format!("Lock poisoned: {}", e)))?;
        last.clone().ok_or_else(|| {
            CommandError::Analysis("No analysis available. Run analyze first.".into())
        })?
    };

    // Load config, applying frontend overrides
    let (mut config, workdir) = super::load_config_from_path(repo_path.as_deref());
    // Use refinement-specific provider/model if set, otherwise fall back to overrides
    if let Some(p) = llm_provider {
        config.llm.refinement.provider = Some(p.clone());
        if config.llm.provider.is_none() {
            config.llm.provider = Some(p);
        }
    }
    if let Some(m) = llm_model {
        config.llm.refinement.model = Some(m.clone());
        if config.llm.model.is_none() {
            config.llm.model = Some(m);
        }
    }

    // Build LLM config for the refinement provider
    let refinement_llm_config = diffcore_core::config::LlmConfig {
        provider: config
            .llm
            .refinement
            .provider
            .clone()
            .or(config.llm.provider.clone()),
        model: config
            .llm
            .refinement
            .model
            .clone()
            .or(config.llm.model.clone()),
        key_cmd: config
            .llm
            .refinement
            .key_cmd
            .clone()
            .or(config.llm.key_cmd.clone()),
        key: config.llm.key.clone(),
        annotations_enabled: config.llm.annotations_enabled,
        refinement: config.llm.refinement.clone(),
    };

    let provider = llm::create_provider_for_workdir(&refinement_llm_config, workdir.as_deref())
        .map_err(|e| CommandError::Llm(format!("{}", e)))?;

    // Serialize analysis for the refinement request
    let analysis_json = serde_json::to_string_pretty(&analysis)
        .map_err(|e| CommandError::Llm(format!("Failed to serialize analysis: {}", e)))?;

    let diff_summary = format!(
        "{} files changed across {} groups",
        analysis.summary.total_files_changed, analysis.summary.total_groups,
    );

    let provider_name = refinement_llm_config
        .provider
        .unwrap_or_else(|| "anthropic".to_string());
    let model_name = refinement_llm_config
        .model
        .unwrap_or_else(|| super::default_model_for_provider(&provider_name).to_string());
    let outcome = refinement::run_refinement_iterations(
        provider.as_ref(),
        &analysis.groups,
        analysis.infrastructure_group.as_ref(),
        &analysis_json,
        &diff_summary,
        refinement_llm_config.refinement.max_iterations,
    )
    .await;

    if let Some(fallback_message) = &outcome.fallback_message {
        warn!("{}", fallback_message);
        let mut result = fallback_refinement_result(
            &analysis,
            provider_name,
            model_name,
            fallback_message.clone(),
        );
        result.stop_reason = outcome.stop_reason;
        result.attempts_used = outcome.attempts_used;
        result.parse_failures = outcome.parse_failures;
        result.warnings = outcome.warnings;
        result.refinement_response = outcome.refinement_response;
        return Ok(result);
    }

    if !outcome.had_changes {
        return Ok(RefinementResult {
            refined_groups: analysis.groups.clone(),
            infrastructure_group: analysis.infrastructure_group.clone(),
            refinement_response: outcome.refinement_response,
            provider: provider_name,
            model: model_name,
            had_changes: false,
            warnings: outcome.warnings,
            stop_reason: outcome.stop_reason,
            attempts_used: outcome.attempts_used,
            parse_failures: outcome.parse_failures,
        });
    }

    for w in &outcome.warnings {
        warn!(
            "Refinement warning [{}]: {}",
            w.event_type(),
            w.message
        );
    }

    // Update cached analysis with refined groups
    match state.last_analysis.lock() {
        Ok(mut last) => {
            if let Some(ref mut a) = *last {
                a.groups = outcome.refined_groups.clone();
                a.infrastructure_group = outcome.infrastructure_group.clone();
            }
        }
        Err(e) => warn!(
            "Failed to update last_analysis with refinement (lock poisoned): {}",
            e
        ),
    }

    Ok(RefinementResult {
        refined_groups: outcome.refined_groups,
        infrastructure_group: outcome.infrastructure_group,
        refinement_response: outcome.refinement_response,
        provider: provider_name,
        model: model_name,
        had_changes: true,
        warnings: outcome.warnings,
        stop_reason: outcome.stop_reason,
        attempts_used: outcome.attempts_used,
        parse_failures: outcome.parse_failures,
    })
}

/// Result of a refinement pass, including both the refined groups and
/// the raw refinement operations (for UI change indicators).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RefinementResult {
    /// The refined flow groups (v2) — or original groups if no changes.
    pub refined_groups: Vec<diffcore_core::types::FlowGroup>,
    /// The refined infrastructure group.
    pub infrastructure_group: Option<diffcore_core::types::InfrastructureGroup>,
    /// The raw refinement response with split/merge/re-rank/reclassify operations.
    pub refinement_response: RefinementResponse,
    /// Which provider performed the refinement.
    pub provider: String,
    /// Which model performed the refinement.
    pub model: String,
    /// Whether the refinement actually produced changes.
    pub had_changes: bool,
    /// Non-fatal warnings from the lenient apply path: repaired IDs and
    /// dropped operations. Empty in the common case.
    #[serde(default)]
    pub warnings: Vec<diffcore_core::llm::refinement::RefinementWarning>,
    /// Deterministic runtime stop reason for the refinement loop.
    #[serde(default = "default_refinement_stop_reason")]
    pub stop_reason: diffcore_core::llm::refinement::RefinementIterationStopReason,
    /// Number of provider attempts used for this run.
    #[serde(default = "default_refinement_attempts_used")]
    pub attempts_used: u32,
    /// Number of parse failures encountered before completion/fallback.
    #[serde(default)]
    pub parse_failures: u32,
}

fn default_refinement_stop_reason() -> diffcore_core::llm::refinement::RefinementIterationStopReason {
    diffcore_core::llm::refinement::RefinementIterationStopReason::NoOp
}

fn default_refinement_attempts_used() -> u32 {
    1
}

/// Load cached refinement result for the current analysis.
///
/// Tries two keys: (1) diff-hash key (exact match), (2) branch-based key (same branch
/// across worktrees, even with different uncommitted changes).
#[tauri::command]
pub fn get_cached_refinement(
    repo_path: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<Option<RefinementResult>, CommandError> {
    // Try diff-hash key first (exact content match)
    let diff_key = state.last_cache_key.lock().ok().and_then(|k| k.clone());
    if let Some(ref key) = diff_key {
        if let Some(json) = cache::load_cached_refinement(key) {
            if let Ok(result) = serde_json::from_str::<RefinementResult>(&json) {
                return Ok(Some(result));
            }
        }
    }

    // Fallback: try branch-based key (works across worktrees on same branch)
    if let Some(ref repo) = repo_path {
        if let Ok(branch_key) = super::comments::comment_cache_key(repo) {
            let branch_refine_key = format!("branch_{}", branch_key);
            if let Some(json) = cache::load_cached_refinement(&branch_refine_key) {
                if let Ok(result) = serde_json::from_str::<RefinementResult>(&json) {
                    return Ok(Some(result));
                }
            }
        }
    }

    Ok(None)
}

/// Store a refinement result in the global cache (~/.diffcore/cache/refinements/).
///
/// Stores under both diff-hash key and branch-based key for cross-worktree access.
#[tauri::command]
pub fn store_refinement_cache(
    result: RefinementResult,
    repo_path: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<(), CommandError> {
    let json = match serde_json::to_string(&result) {
        Ok(j) => j,
        Err(e) => {
            warn!("Failed to serialize refinement for caching: {}", e);
            return Ok(());
        }
    };

    // Store under diff-hash key
    if let Some(cache_key) = state.last_cache_key.lock().ok().and_then(|k| k.clone()) {
        cache::store_cached_refinement(&cache_key, &json);
    }

    // Also store under branch-based key for cross-worktree access
    if let Some(ref repo) = repo_path {
        if let Ok(branch_key) = super::comments::comment_cache_key(repo) {
            let branch_refine_key = format!("branch_{}", branch_key);
            cache::store_cached_refinement(&branch_refine_key, &json);
        }
    }

    Ok(())
}

/// Background LLM job registration payload returned before SSE streaming begins.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AsyncLlmJobStart {
    pub job_id: String,
    pub stream_url: String,
    pub operation: String,
    pub provider: String,
    pub model: String,
    pub title: String,
}
