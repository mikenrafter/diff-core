import type { LlmProvider } from "../../types";
import Dropdown from "../Dropdown";
import { useAppContext } from "../../hooks/AppContext";
import {
  AI_PROVIDER_TYPES,
  AI_PROVIDER_TYPE_LABELS,
  type AiProviderType,
  PROVIDER_LABELS,
  isApiProviderType,
  providerTypeFromProvider,
  runtimeProviderForProviderType,
} from "../../utils/llmUtils";

const CLI_HINTS: Partial<Record<AiProviderType, { install: string; login: string }>> = {
  cursor_cli: { install: "Install Cursor CLI in your PATH", login: "Sign in with cursor auth login" },
  codex_cli: { install: "npm install -g @openai/codex", login: "codex login" },
  claude_cli: { install: "brew install claude-code", login: "claude auth login" },
  qwen_cli: { install: "Install your preferred Qwen CLI adapter", login: "Authenticate with the adapter login flow" },
  gemini_cli: { install: "Install Gemini CLI tooling", login: "Authenticate with gemini auth login" },
  copilot_cli: { install: "Install GitHub Copilot CLI integration", login: "Authenticate with gh auth login" },
};

const API_KEY_HINTS: Partial<Record<AiProviderType, string>> = {
  cursor_api: "CURSOR_API_KEY",
  anthropic_api: "ANTHROPIC_API_KEY",
  openai_api: "OPENAI_API_KEY",
  alibaba_api: "DASHSCOPE_API_KEY",
  gemini_api: "GEMINI_API_KEY",
  copilot_api: "GITHUB_COPILOT_TOKEN",
  openrouter: "OPENROUTER_API_KEY",
  ollama_api: "OLLAMA_API_KEY",
};

const DIRECT_API_PROVIDER_TYPES = new Set<AiProviderType>([
  "cursor_api",
  "anthropic_api",
  "openai_api",
  "alibaba_api",
  "gemini_api",
  "copilot_api",
  "openrouter",
  "ollama_api",
]);

/**
 * AI tab for configuring provider type at the point of use.
 *
 * This keeps setup next to the analysis controls in the rightmost pane and
 * gives one place to switch between CLI-backed and direct-API workflows.
 */
export function AiOverlayTab() {
  const {
    llmSettings,
    updateSetting,
    modelsForProvider,
    fetchModelsForProvider,
    modelsLoading,
    apiKeyInput,
    setApiKeyInput,
    setApiProviderDraft,
    handleSaveApiKey,
    handleClearApiKey,
    refreshAiAccess,
  } = useAppContext();

  if (!llmSettings) {
    return (
      <div className="ai-overlay-tab">
        <div className="empty-state">AI settings are not available yet.</div>
      </div>
    );
  }

  const selectedProviderType = providerTypeFromProvider(llmSettings.provider);
  const runtimeProvider = runtimeProviderForProviderType(selectedProviderType);
  const isApi = isApiProviderType(selectedProviderType);
  const configuredModels = modelsForProvider(selectedProviderType as LlmProvider);
  const selectedModel = llmSettings.model || configuredModels[0] || "default";

  const applyProviderType = (providerType: AiProviderType) => {
    updateSetting("provider", providerType);
    void fetchModelsForProvider(providerType);

    if (DIRECT_API_PROVIDER_TYPES.has(providerType)) {
      setApiProviderDraft(providerType as LlmProvider);
    }
  };

  return (
    <div className="ai-overlay-tab">
      <div className="ai-overlay-section">
        <h3>AI Provider</h3>
        <p className="settings-hint">
          Choose a provider type for this repository. Provider aliases route through the nearest supported backend until native integrations land.
        </p>

        <div className="settings-row">
          <label>Provider type</label>
        </div>
        <Dropdown<AiProviderType>
          value={selectedProviderType}
          onChange={applyProviderType}
          options={AI_PROVIDER_TYPES.map((providerType) => ({
            value: providerType,
            label: AI_PROVIDER_TYPE_LABELS[providerType],
          }))}
          testId="ai-provider-type"
        />

        <div className="settings-row" style={{ marginTop: 12 }}>
          <label>Model</label>
        </div>
        <Dropdown
          value={selectedModel}
          onChange={(value) => updateSetting("model", value)}
          options={configuredModels.map((model) => ({ value: model, label: model }))}
          placeholder="Select model"
        />
        <button
          className="btn btn-small"
          style={{ marginTop: 8 }}
          disabled={modelsLoading === selectedProviderType}
          onClick={() => void fetchModelsForProvider(selectedProviderType, true)}
          title="Refresh model list"
        >
          {modelsLoading === selectedProviderType ? "Refreshing..." : "Refresh models"}
        </button>

        <div className="ai-overlay-runtime-note">
          Runtime backend: {PROVIDER_LABELS[runtimeProvider]}
        </div>
      </div>

      <div className="ai-overlay-section">
        <h3>{isApi ? "API Access" : "CLI Access"}</h3>
        <p className="ai-overlay-disclaimer">
          {isApi
            ? "API mode sends structured requests directly to hosted endpoints. Activity updates stay high-level, and tool-level file or shell traces are not streamed."
            : "CLI mode delegates analysis to a local agent workflow. When the selected integration supports it, activity can include file reads, searches, and command traces."}
        </p>

        {isApi ? (
          <>
            <div className={`api-key-status ${llmSettings.has_api_key ? "configured" : "missing"}`} style={{ marginBottom: 8 }}>
              <span className="api-key-dot" />
              <span>Key source: {llmSettings.api_key_source || "none"}</span>
            </div>
            <div className="api-key-input-row">
              <input
                type="password"
                className="settings-input api-key-input"
                placeholder="Paste API key"
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
                title="Save API key"
              >
                Save
              </button>
              {llmSettings.api_key_source === "~/.diffcore/config.toml" && (
                <button
                  className="btn btn-clear-key"
                  onClick={() => void handleClearApiKey()}
                  title="Clear stored API key"
                >
                  Clear
                </button>
              )}
            </div>
            <p className="settings-hint">
              Recommended env var: {API_KEY_HINTS[selectedProviderType] ?? "DIFFCORE_API_KEY"}
            </p>
          </>
        ) : (
          <>
            <div
              className={`api-key-status ${(llmSettings.codex_authenticated || llmSettings.claude_authenticated) ? "configured" : "missing"}`}
              style={{ marginBottom: 8 }}
            >
              <span className="api-key-dot" />
              <span>
                Codex CLI: {llmSettings.codex_authenticated ? "ready" : llmSettings.codex_available ? "installed, login required" : "not found"}
                {"  |  "}
                Claude CLI: {llmSettings.claude_authenticated ? "ready" : llmSettings.claude_available ? "installed, login required" : "not found"}
              </span>
            </div>
            <p className="settings-hint">
              Install: {CLI_HINTS[selectedProviderType]?.install ?? "Use the provider's CLI installer"}
            </p>
            <p className="settings-hint">
              Login: {CLI_HINTS[selectedProviderType]?.login ?? "Run the provider's auth/login command"}
            </p>
          </>
        )}

        <div className="ai-overlay-actions">
          <button className="btn" onClick={() => void refreshAiAccess()}>
            Recheck setup
          </button>
        </div>
      </div>
    </div>
  );
}
