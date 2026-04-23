/**
 * LLM provider resolution utilities.
 *
 * Determines which provider/model to use given user settings and the machine
 * environment (codex/claude subscription vs. direct API key).
 */
import { LLM_PROVIDERS, DEFAULT_MODELS_BY_PROVIDER } from "../types";
import type { LlmProvider } from "../types";

export type SubscriptionProvider = "codex" | "claude";

export function isApiProvider(provider: string): boolean {
  return (
    provider === "anthropic"
    || provider === "openai"
    || provider === "gemini"
    || provider === "openrouter"
    || provider === "github_copilot"
  );
}

export function resolveInteractiveProvider(
  configuredProvider: string | null,
  preferredProvider: SubscriptionProvider | null,
): LlmProvider | null {
  if (preferredProvider && configuredProvider && isApiProvider(configuredProvider)) {
    return preferredProvider;
  }
  if (configuredProvider && LLM_PROVIDERS.includes(configuredProvider as LlmProvider)) {
    return configuredProvider as LlmProvider;
  }
  return preferredProvider;
}

export function resolveInteractiveModel(
  configuredModel: string | null,
  configuredProvider: string | null,
  resolvedProvider: LlmProvider | null,
): string | null {
  if (!resolvedProvider) return configuredModel;
  if (configuredProvider === resolvedProvider && configuredModel) {
    return configuredModel;
  }
  return DEFAULT_MODELS_BY_PROVIDER[resolvedProvider]?.[0] ?? configuredModel ?? "default";
}
