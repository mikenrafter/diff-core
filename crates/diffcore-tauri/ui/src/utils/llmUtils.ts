import type { LlmProvider } from "../types";
import { LLM_PROVIDERS, DEFAULT_MODELS_BY_PROVIDER } from "../types";
import type { SubscriptionProvider } from "./constants";

export function resolveInteractiveProvider(
  configuredProvider: string | null,
  preferredProvider: SubscriptionProvider | null,
  isApiProvider: (p: string) => boolean,
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
