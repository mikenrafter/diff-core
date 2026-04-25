/**
 * SettingsPanel — settings overlay exposed from the toolbar gear icon.
 *
 * Covers four sections:
 *  1. Diff Behavior — uncommitted changes toggle, diff view mode
 *  2. AI Access     — provider/model selection, API key, subscription hints
 *  3. Annotations   — LLM annotation on/off toggle
 *  4. Refinement    — LLM refinement on/off, provider, model, max iterations
 *  5. Exclude Paths — glob patterns to omit from analysis
 *
 * Renders only when `settingsOpen && llmSettings` are truthy. All state is
 * consumed from AppContext so no props are required.
 */
import type { LlmProvider, DiffViewMode, LlmSettings } from "../../types";
import { LLM_PROVIDERS } from "../../types";
import Dropdown from "../Dropdown";
import { PROVIDER_LABELS, isApiProvider } from "../../utils/llmUtils";
import { useAppContext } from "../../hooks/AppContext";
import { useEffect, useMemo, useState } from "react";
import { logger, type LogLevel } from "../../utils/logger";
import { useLogger } from "../../hooks/useLogger";

export function SettingsPanel() {
  const {
    settingsOpen,
    llmSettings,
    setSettingsOpen,
    includeUncommitted,
    setIncludeUncommitted,
    saveLlmSettings,
    baseRef,
    diffViewMode,
    setDiffViewMode,
    aiAccessReady,
    openAiSetup,
    recommendedSubscriptionProvider,
    resolvedPrimaryProvider,
    resolvedPrimaryModel,
    resolvedRefinementProvider,
    resolvedRefinementModel,
    apiKeyInput,
    setApiKeyInput,
    handleSaveApiKey,
    handleClearApiKey,
    activatePreferredActivityProvider,
    modelsForProvider,
    fetchModelsForProvider,
    modelsLoading,
    updateSetting,
    ignorePaths,
    ignorePathInput,
    setIgnorePathInput,
    handleAddIgnorePath,
    handleRemoveIgnorePath,
  } = useAppContext();

  const logState = useLogger();
  const [logQuery, setLogQuery] = useState("");
  const filteredLogs = useMemo(() => {
    const q = logQuery.trim().toLowerCase();
    if (!q) return logState.entries;
    return logState.entries.filter((e) => {
      const hay = `${e.level} ${e.category} ${e.event} ${e.message ?? ""}`.toLowerCase();
      return hay.includes(q);
    });
  }, [logState.entries, logQuery]);

  useEffect(() => {
    logger.log({ level: "info", category: "ui", event: "settings_open" });
    return () => {
      logger.log({ level: "info", category: "ui", event: "settings_close" });
    };
  }, []);

  if (!settingsOpen || !llmSettings) return null;

  return (
    <div className="settings-overlay" onClick={() => setSettingsOpen(false)}>
      <div className="settings-panel" onClick={(e) => e.stopPropagation()}>
        <div className="settings-header">
          <h2>Settings</h2>
          <button className="btn-close" onClick={() => setSettingsOpen(false)}>
            &times;
          </button>
        </div>
        <div className="settings-body">
          {/* Diff Behavior */}
          <div className="settings-section">
            <h3>Diff Behavior</h3>
            <label className="settings-toggle" style={{ display: "flex", alignItems: "center", gap: 8, cursor: "pointer" }}>
              <input
                type="checkbox"
                checked={includeUncommitted}
                onChange={(e) => {
                  const val = e.target.checked;
                  setIncludeUncommitted(val);
                  if (llmSettings) {
                    void saveLlmSettings({ ...llmSettings, include_uncommitted: val } as LlmSettings);
                  }
                }}
              />
              <span>Include uncommitted changes</span>
            </label>
            <p className="settings-hint">
              When enabled, branch comparisons include both committed and uncommitted
              working tree changes (equivalent to <code>git diff {baseRef || "main"}</code>).
            </p>
            <label className="settings-toggle" style={{ display: "flex", alignItems: "center", gap: 8, marginTop: 12 }}>
              <span>Diff view mode</span>
              <div style={{ marginLeft: "auto", maxWidth: 200, flex: "0 1 200px" }}>
                <Dropdown<DiffViewMode>
                  value={diffViewMode}
                  onChange={(value) => setDiffViewMode(value)}
                  options={[
                    { value: "side-by-side", label: "Side-by-side" },
                    { value: "inline", label: "Inline" },
                    { value: "dynamic", label: "Dynamic" },
                  ]}
                />
              </div>
            </label>
            <p className="settings-hint">
              Side-by-side shows old/new in two columns. Inline shows a unified view.
              Dynamic picks per-file based on change density.
            </p>
          </div>

          {/* Debug logs */}
          <div className="settings-section">
            <h3>Debug Logs</h3>
            <label className="settings-toggle" style={{ display: "flex", alignItems: "center", gap: 8 }}>
              <input
                type="checkbox"
                checked={logState.enabled}
                onChange={(e) => logger.setEnabled(e.target.checked)}
              />
              <span>Enable in-app logging (IPC timings + UI events)</span>
            </label>
            <div className="settings-row" style={{ marginTop: 10, display: "flex", gap: 8, alignItems: "center" }}>
              <label style={{ minWidth: 90 }}>Min level</label>
              <div style={{ flex: 1 }}>
                <Dropdown<LogLevel>
                  value={logState.minLevel}
                  onChange={(v) => logger.setMinLevel(v)}
                  options={[
                    { value: "debug", label: "debug" },
                    { value: "info", label: "info" },
                    { value: "warn", label: "warn" },
                    { value: "error", label: "error" },
                  ]}
                />
              </div>
              <button className="btn btn-small" onClick={() => logger.clear()} disabled={!logState.enabled}>
                Clear
              </button>
              <button
                className="btn btn-small"
                disabled={!logState.enabled || logState.entries.length === 0}
                onClick={() => {
                  const payload = JSON.stringify(logState.entries.slice().reverse(), null, 2);
                  navigator.clipboard.writeText(payload).catch(() => {});
                  logger.log({ level: "info", category: "ui", event: "logs_copied", data: { count: logState.entries.length } });
                }}
                title="Copy logs as JSON"
              >
                Copy JSON
              </button>
            </div>
            <div className="settings-row" style={{ marginTop: 8 }}>
              <input
                className="settings-input"
                placeholder="Filter logs (e.g. analyze, get_repo_info, settings)..."
                value={logQuery}
                onChange={(e) => setLogQuery(e.target.value)}
                style={{ width: "100%" }}
                disabled={!logState.enabled}
              />
            </div>
            <p className="settings-hint">
              Tip: the worst offenders usually show up as <code>ipc</code> events with high <code>duration_ms</code>.
            </p>
            <div style={{ maxHeight: 220, overflow: "auto", fontFamily: "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace", fontSize: 11, border: "1px solid rgba(255,255,255,0.08)", borderRadius: 6, padding: 8 }}>
              {!logState.enabled && (
                <div style={{ opacity: 0.7 }}>Logging is disabled.</div>
              )}
              {logState.enabled && filteredLogs.length === 0 && (
                <div style={{ opacity: 0.7 }}>No log entries.</div>
              )}
              {logState.enabled && filteredLogs.slice(0, 120).map((e) => (
                <div key={e.id} style={{ display: "flex", gap: 8, padding: "2px 0" }}>
                  <span style={{ opacity: 0.7, minWidth: 70 }}>
                    {new Date(e.ts).toLocaleTimeString()}
                  </span>
                  <span style={{ minWidth: 42 }}>{e.level}</span>
                  <span style={{ minWidth: 46 }}>{e.category}</span>
                  <span style={{ flex: 1, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
                    {e.event}{e.message ? ` — ${e.message}` : ""}
                  </span>
                  {typeof e.duration_ms === "number" && (
                    <span style={{ minWidth: 60, textAlign: "right", opacity: 0.9 }}>
                      {e.duration_ms.toFixed(1)}ms
                    </span>
                  )}
                </div>
              ))}
            </div>
          </div>
          {/* LLM Access / Onboarding */}
          <div className="settings-section">
            <div className="settings-section-title-row">
              <h3>AI Access</h3>
              <button className="btn btn-inline-setup" onClick={() => openAiSetup("recommended")}>
                Open setup flow
              </button>
            </div>
            <div className={`api-key-status ${aiAccessReady ? "configured" : "missing"}`}>
              <span className="api-key-dot" />
              <span>
                {aiAccessReady
                  ? `Ready via ${recommendedSubscriptionProvider
                    ? PROVIDER_LABELS[recommendedSubscriptionProvider]
                    : llmSettings.api_key_source}`
                  : "Not configured yet"}
              </span>
            </div>
            <p className="settings-hint">
              Saved globally in <code>{llmSettings.global_config_path}</code>, so new projects reuse the same setup.
            </p>
            <p className="settings-hint">
              Prefer Codex CLI or Claude Code if you already use them. Direct API keys are the fallback path.
            </p>
            {resolvedPrimaryProvider && (
              <p className="settings-hint">
                Effective summary backend on this machine: <strong>{PROVIDER_LABELS[resolvedPrimaryProvider as LlmProvider]}/{resolvedPrimaryModel ?? "default"}</strong>
              </p>
            )}
            {llmSettings.refinement_enabled && resolvedRefinementProvider && (
              <p className="settings-hint">
                Effective refinement backend on this machine: <strong>{PROVIDER_LABELS[resolvedRefinementProvider as LlmProvider]}/{resolvedRefinementModel ?? "default"}</strong>
              </p>
            )}
            {recommendedSubscriptionProvider && isApiProvider(llmSettings.provider) && (
              <p className="settings-hint">
                Diffcore will prefer {PROVIDER_LABELS[recommendedSubscriptionProvider]} for live jobs on this machine,
                so file reads, greps, and shell commands can stream into the Activity tab while API keys stay available
                as fallback.
              </p>
            )}
            <p className="settings-hint">
              Codex CLI: {llmSettings.codex_authenticated ? "ready" : llmSettings.codex_available ? "installed, needs login" : "not found"}
              {" "}· Claude Code: {llmSettings.claude_authenticated ? "ready" : llmSettings.claude_available ? "installed, needs login" : "not found"}
            </p>
            <div className="settings-row">
              <label>Primary backend</label>
            </div>
            <Dropdown<LlmProvider>
              value={llmSettings.provider as LlmProvider}
              onChange={(value) => updateSetting("provider", value)}
              options={LLM_PROVIDERS.map((p) => ({ value: p, label: PROVIDER_LABELS[p] }))}
            />
            <div className="settings-row" style={{ marginTop: 12 }}>
              <label>Model</label>
            </div>
            <Dropdown
              value={llmSettings.model}
              onChange={(value) => updateSetting("model", value)}
              options={modelsForProvider(llmSettings.provider).map((m) => ({ value: m, label: m }))}
              placeholder="Select model"
            />
            <button
              className="btn btn-small"
              style={{ marginTop: 4 }}
              disabled={modelsLoading === llmSettings.provider}
              onClick={() => void fetchModelsForProvider(llmSettings.provider, true)}
              title="Refresh model list from provider API"
            >
              {modelsLoading === llmSettings.provider ? "Refreshing…" : "⟳ Refresh models"}
            </button>
            {!isApiProvider(resolvedPrimaryProvider ?? llmSettings.provider) && (
              <p className="settings-hint">
                No API key needed here. diffcore will call {PROVIDER_LABELS[(resolvedPrimaryProvider ?? llmSettings.provider) as LlmProvider]}
                {" "}inside the repo so it can inspect the filesystem before producing structured output.
              </p>
            )}
            {isApiProvider(llmSettings.provider) && (
              <>
                <div className="api-key-input-row">
                  <input
                    type="password"
                    className="settings-input api-key-input"
                    placeholder="Paste your API key"
                    value={apiKeyInput}
                    onChange={(e) => setApiKeyInput(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" && apiKeyInput.trim()) {
                        void handleSaveApiKey();
                      }
                    }}
                  />
                  <button
                    className="btn btn-save-key"
                    disabled={!apiKeyInput.trim()}
                    onClick={() => void handleSaveApiKey()}
                    title="Save API key to ~/.diffcore/config.toml"
                  >
                    Save
                  </button>
                  {llmSettings.api_key_source === "~/.diffcore/config.toml" && (
                    <button
                      className="btn btn-clear-key"
                      onClick={() => void handleClearApiKey()}
                      title="Remove stored API key"
                    >
                      Clear
                    </button>
                  )}
                </div>
                <p className="settings-hint">
                  New users can usually skip keys by using Codex CLI or Claude Code if they are already signed in.
                  If you prefer direct API calls, paste a key above, set <code>DIFFCORE_API_KEY</code>, a provider-specific
                  env var (<code>ANTHROPIC_API_KEY</code>, <code>OPENAI_API_KEY</code>, <code>GEMINI_API_KEY</code>), or
                  configure <code>key_cmd</code> in <code>~/.diffcore/config.toml</code>.
                </p>
                {recommendedSubscriptionProvider && (
                  <button className="btn" onClick={() => void activatePreferredActivityProvider()}>
                    Use {PROVIDER_LABELS[recommendedSubscriptionProvider]} instead
                  </button>
                )}
              </>
            )}
          </div>

          {/* Annotations Toggle */}
          <div className="settings-section">
            <h3>Annotations</h3>
            <label className="settings-toggle">
              <input
                type="checkbox"
                checked={llmSettings.annotations_enabled}
                onChange={(e) => updateSetting("annotations_enabled", e.target.checked)}
              />
              <span>Enable LLM annotations</span>
            </label>
            <p className="settings-hint">
              When enabled, diffcore can generate a PR-ready summary plus deeper flow analysis.
            </p>
          </div>

          {/* Refinement Section */}
          <div className="settings-section settings-refinement">
            <h3>Refinement</h3>
            <label className="settings-toggle">
              <input
                type="checkbox"
                checked={llmSettings.refinement_enabled}
                onChange={(e) => updateSetting("refinement_enabled", e.target.checked)}
              />
              <span>Enable LLM refinement</span>
            </label>
            <p className="settings-hint">
              Refines deterministic groupings using an LLM pass. This is still useful with Codex CLI or Claude Code:
              the provider changes, but the refinement step is what decides whether the deterministic groups should be
              split, merged, re-ranked, or kept as-is.
            </p>
            {llmSettings.refinement_enabled && (
              <>
                <div className="settings-row">
                  <label>Provider</label>
                  <Dropdown<LlmProvider>
                    value={llmSettings.refinement_provider as LlmProvider}
                    onChange={(value) => updateSetting("refinement_provider", value)}
                    options={LLM_PROVIDERS.map((p) => ({ value: p, label: PROVIDER_LABELS[p] }))}
                  />
                </div>
                <div className="settings-row">
                  <label>Model</label>
                  <Dropdown
                    value={llmSettings.refinement_model}
                    onChange={(value) => updateSetting("refinement_model", value)}
                    options={modelsForProvider(llmSettings.refinement_provider).map((m) => ({ value: m, label: m }))}
                    placeholder="Select model"
                  />
                </div>
                <div className="settings-row">
                  <label>Max iterations</label>
                  <input
                    type="number"
                    className="settings-number"
                    min={1}
                    max={10}
                    value={llmSettings.refinement_max_iterations}
                    onChange={(e) =>
                      updateSetting("refinement_max_iterations", Math.max(1, parseInt(e.target.value) || 1))
                    }
                  />
                </div>
              </>
            )}
          </div>

          {/* Exclude Paths Section */}
          <div className="settings-section">
            <h3>Exclude Paths</h3>
            <p className="settings-hint" style={{ marginTop: 0, marginBottom: 8 }}>
              Glob patterns for files/folders to exclude from analysis. Matched against repo-relative paths.
            </p>
            {ignorePaths.length > 0 && (
              <div className="ignore-paths-list">
                {ignorePaths.map((pattern) => (
                  <span key={pattern} className="ignore-path-tag">
                    <code>{pattern}</code>
                    <button
                      className="ignore-path-remove"
                      onClick={() => void handleRemoveIgnorePath(pattern)}
                      title={`Remove ${pattern}`}
                    >
                      &times;
                    </button>
                  </span>
                ))}
              </div>
            )}
            <div className="ignore-path-input-row">
              <input
                type="text"
                className="settings-input ignore-path-input"
                placeholder="e.g. dist/**, **/*.generated.ts"
                value={ignorePathInput}
                onChange={(e) => setIgnorePathInput(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && ignorePathInput.trim()) {
                    void handleAddIgnorePath();
                  }
                }}
              />
              <button
                className="btn btn-save-key"
                disabled={!ignorePathInput.trim()}
                onClick={() => void handleAddIgnorePath()}
              >
                Add
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
