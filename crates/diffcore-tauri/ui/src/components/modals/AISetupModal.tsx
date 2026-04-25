/**
 * AISetupModal — onboarding overlay for connecting an AI backend.
 *
 * Presents two paths:
 *  1. Subscription CLI (Codex CLI / Claude Code) — preferred; no key needed
 *  2. Direct API key fallback — for OpenAI, Anthropic, Gemini, etc.
 *
 * Renders only when `aiSetupOpen && llmSettings` are truthy. All state is
 * consumed from AppContext so no props are required.
 */
import type { LlmProvider } from "../../types";
import Dropdown from "../Dropdown";
import { PROVIDER_LABELS } from "../../utils/llmUtils";
import type { SubscriptionProvider } from "../../utils/llmUtils";
import { useAppContext } from "../../hooks/AppContext";

/** Direct API providers available in the fallback key flow. */
const API_PROVIDER_OPTIONS: LlmProvider[] = ["openai", "anthropic", "gemini", "openrouter", "github_copilot"];

/**
 * Subscription CLI backends that diffcore can delegate to.
 * Both expose a consistent install+login command pair so the card UI stays uniform.
 */
const SUBSCRIPTION_BACKENDS: Array<{
  provider: SubscriptionProvider;
  title: string;
  description: string;
  installCommand: string;
  loginCommand: string;
}> = [
  {
    provider: "codex",
    title: "Codex CLI",
    description: "Best path if you already use Codex. diffcore can reuse that login and let Codex inspect the repo directly.",
    installCommand: "npm install -g @openai/codex",
    loginCommand: "codex login",
  },
  {
    provider: "claude",
    title: "Claude Code",
    description: "Use your Claude Code subscription instead of pasting a separate Anthropic key into every repo.",
    installCommand: "brew install claude-code",
    loginCommand: "claude auth login",
  },
];

export function AISetupModal() {
  const {
    aiSetupOpen,
    llmSettings,
    aiSetupStep,
    apiProviderDraft,
    setApiProviderDraft,
    apiKeyInput,
    setApiKeyInput,
    aiAccessReady,
    recommendedSubscriptionProvider,
    refreshAiAccess,
    dismissAiSetup,
    activateSubscriptionProvider,
    openApiKeyFallback,
    handleSaveApiKey,
  } = useAppContext();

  if (!aiSetupOpen || !llmSettings) return null;

  return (
    <div className="ai-setup-overlay" onClick={dismissAiSetup}>
      <div
        className="ai-setup-modal"
        data-testid="ai-onboarding"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="ai-setup-header">
          <div>
            <h2>Set up AI access</h2>
            <p>
              Start with a repo-aware CLI if you already have one. Only fall back to an API key if you want direct API calls.
            </p>
          </div>
          <button className="btn-close" onClick={dismissAiSetup} title="Close onboarding">
            &times;
          </button>
        </div>
        <div className="ai-setup-body">
          <div className={`ai-setup-status ${aiAccessReady ? "ready" : "missing"}`}>
            <span className="api-key-dot" />
            <span>
              {aiAccessReady
                ? `Ready via ${llmSettings.api_key_source}`
                : "No active AI backend selected yet"}
            </span>
          </div>
          <p className="ai-setup-path">
            Shared config lives in <code>{llmSettings.global_config_path}</code>, so new repos reuse the same setup.
          </p>

          <section className="ai-setup-section">
            <div className="ai-setup-section-header">
              <h3>Use an existing subscription</h3>
              <p>If you already use Codex CLI or Claude Code, diffcore can reuse that login. No separate API key is required.</p>
            </div>
            <div className="ai-backend-grid">
              {SUBSCRIPTION_BACKENDS.map((backend) => {
                const available = backend.provider === "codex"
                  ? llmSettings.codex_available
                  : llmSettings.claude_available;
                const authenticated = backend.provider === "codex"
                  ? llmSettings.codex_authenticated
                  : llmSettings.claude_authenticated;
                const statusLabel = authenticated
                  ? "Ready"
                  : available
                    ? "Needs login"
                    : "Not found";
                const statusDetail = authenticated
                  ? `Use ${backend.title} as the primary backend for summaries and flow analysis.`
                  : available
                    ? `Finish setup with \`${backend.loginCommand}\`, then recheck.`
                    : `Install it with \`${backend.installCommand}\`, then recheck.`;

                return (
                  <article
                    key={backend.provider}
                    className={`ai-backend-card ${authenticated ? "ready" : available ? "login" : "missing"} ${llmSettings.provider === backend.provider ? "selected" : ""}`}
                    data-testid={`ai-card-${backend.provider}`}
                  >
                    <div className="ai-backend-header">
                      <div>
                        <h4>{backend.title}</h4>
                        <p>{backend.description}</p>
                      </div>
                      <span className={`ai-backend-badge ${authenticated ? "ready" : available ? "login" : "missing"}`}>
                        {statusLabel}
                      </span>
                    </div>
                    <p className="ai-backend-detail">{statusDetail}</p>
                    {!authenticated && (
                      <code className="ai-backend-command">
                        {available ? backend.loginCommand : backend.installCommand}
                      </code>
                    )}
                    <div className="ai-backend-actions">
                      {authenticated ? (
                        <button
                          className={`btn ${recommendedSubscriptionProvider === backend.provider ? "btn-primary" : ""}`}
                          onClick={() => void activateSubscriptionProvider(backend.provider)}
                        >
                          Use {backend.title}
                        </button>
                      ) : (
                        <button className="btn" onClick={() => void refreshAiAccess()}>
                          Recheck
                        </button>
                      )}
                    </div>
                  </article>
                );
              })}
            </div>
          </section>

          <section className={`ai-setup-section ai-api-section ${aiSetupStep === "api" ? "expanded" : ""}`}>
            <div className="ai-setup-section-header">
              <h3>Direct API fallback</h3>
              <p>Use this only when you want diffcore to talk to OpenAI, Anthropic, or Gemini directly.</p>
            </div>
            {aiSetupStep !== "api" ? (
              <button className="btn" onClick={openApiKeyFallback}>
                Use API key instead
              </button>
            ) : (
              <>
                <div className="settings-row" style={{ marginTop: 0 }}>
                  <label>Provider</label>
                </div>
                <Dropdown<LlmProvider>
                  value={apiProviderDraft}
                  onChange={(value) => setApiProviderDraft(value)}
                  options={API_PROVIDER_OPTIONS.map((provider) => ({
                    value: provider,
                    label: PROVIDER_LABELS[provider],
                  }))}
                  testId="api-provider-select"
                />
                <div className="api-key-input-row">
                  <input
                    type="password"
                    className="settings-input api-key-input"
                    placeholder={`Paste your ${PROVIDER_LABELS[apiProviderDraft]} key`}
                    value={apiKeyInput}
                    onChange={(e) => setApiKeyInput(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" && apiKeyInput.trim()) {
                        void handleSaveApiKey();
                      }
                    }}
                    data-testid="api-key-input"
                  />
                  <button
                    className="btn btn-save-key"
                    disabled={!apiKeyInput.trim()}
                    onClick={() => void handleSaveApiKey()}
                    data-testid="api-key-save"
                  >
                    Save and continue
                  </button>
                </div>
                <p className="settings-hint">
                  diffcore will save the provider choice and key globally, then reuse it for future repos.
                </p>
              </>
            )}
          </section>
        </div>
        <div className="ai-setup-footer">
          <button className="btn" onClick={dismissAiSetup}>
            Continue without AI
          </button>
          <button className="btn" onClick={() => void refreshAiAccess()}>
            Recheck setup
          </button>
        </div>
      </div>
    </div>
  );
}
