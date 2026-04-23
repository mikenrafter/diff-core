import Dropdown from "../Dropdown";
import { useAppContext } from "../../hooks/AppContext";
import { SUBSCRIPTION_BACKENDS, API_PROVIDER_OPTIONS, PROVIDER_LABELS } from "../../utils/constants";
import type { LlmProvider } from "../../types";

export function AISetupModal() {
  const ctx = useAppContext();
  const {
    aiSetupOpen, llmSettings, dismissAiSetup,
    aiAccessReady, recommendedSubscriptionProvider,
    activateSubscriptionProvider, refreshAiAccess, openApiKeyFallback,
    aiSetupStep,
    apiProviderDraft, setApiProviderDraft,
    apiKeyInput, setApiKeyInput,
    handleSaveApiKey,
  } = ctx;

  return (
    <>
      {aiSetupOpen && llmSettings && (
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
                              onClick={() => activateSubscriptionProvider(backend.provider)}
                            >
                              Use {backend.title}
                            </button>
                          ) : (
                            <button className="btn" onClick={refreshAiAccess}>
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
                            handleSaveApiKey();
                          }
                        }}
                        data-testid="api-key-input"
                      />
                      <button
                        className="btn btn-save-key"
                        disabled={!apiKeyInput.trim()}
                        onClick={handleSaveApiKey}
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
              <button className="btn" onClick={refreshAiAccess}>
                Recheck setup
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
