/**
 * LLM provider resolution utilities.
 *
 * Determines which provider/model to use given user settings and the machine
 * environment (codex/claude subscription vs. direct API key).
 */
import { LLM_PROVIDERS, DEFAULT_MODELS_BY_PROVIDER } from "../types";
import type { LlmProvider } from "../types";

export type SubscriptionProvider = "codex" | "claude";
export type AiProviderType =
  | "cursor_cli"
  | "cursor_api"
  | "claude_cli"
  | "anthropic_api"
  | "codex_cli"
  | "openai_api"
  | "qwen_cli"
  | "alibaba_api"
  | "gemini_cli"
  | "gemini_api"
  | "copilot_cli"
  | "copilot_api"
  | "openrouter"
  | "ollama_api";

export const AI_PROVIDER_TYPES: AiProviderType[] = [
  "cursor_cli",
  "cursor_api",
  "claude_cli",
  "anthropic_api",
  "codex_cli",
  "openai_api",
  "qwen_cli",
  "alibaba_api",
  "gemini_cli",
  "gemini_api",
  "copilot_cli",
  "copilot_api",
  "openrouter",
  "ollama_api",
];

/** Human-readable display labels for each LLM provider. */
export const PROVIDER_LABELS: Record<LlmProvider, string> = {
  codex: "Codex CLI",
  claude: "Claude Code",
  anthropic: "Anthropic API",
  openai: "OpenAI API",
  gemini: "Gemini API",
  openrouter: "OpenRouter",
  github_copilot: "GitHub Copilot",
  cursor_cli: "Cursor CLI",
  cursor_api: "Cursor API",
  claude_cli: "Claude CLI",
  anthropic_api: "Anthropic API",
  codex_cli: "Codex CLI",
  openai_api: "OpenAI API",
  qwen_cli: "Qwen CLI",
  alibaba_api: "Alibaba API",
  gemini_cli: "Gemini CLI",
  gemini_api: "Gemini API",
  copilot_cli: "Copilot CLI",
  copilot_api: "Copilot API",
  ollama_api: "Ollama API",
};

export const AI_PROVIDER_TYPE_LABELS: Record<AiProviderType, string> = {
  cursor_cli: "Cursor CLI",
  cursor_api: "Cursor API",
  claude_cli: "Claude CLI",
  anthropic_api: "Anthropic API",
  codex_cli: "Codex CLI",
  openai_api: "OpenAI API",
  qwen_cli: "Qwen CLI",
  alibaba_api: "Alibaba API",
  gemini_cli: "Gemini CLI",
  gemini_api: "Gemini API",
  copilot_cli: "Copilot CLI",
  copilot_api: "Copilot API",
  openrouter: "OpenRouter",
  ollama_api: "Ollama API",
};

const API_PROVIDER_SET = new Set<string>([
  "anthropic",
  "openai",
  "gemini",
  "openrouter",
  "github_copilot",
  "cursor_api",
  "anthropic_api",
  "openai_api",
  "alibaba_api",
  "gemini_api",
  "copilot_api",
  "ollama_api",
]);

export function isApiProviderType(providerType: AiProviderType): boolean {
  return providerType.endsWith("_api") || providerType === "openrouter";
}

/**
 * Map a UI provider-type choice to a runtime provider that is implemented today.
 *
 * Unsupported integrations intentionally route through the nearest compatible
 * provider until dedicated backends land.
 */
export function runtimeProviderForProviderType(providerType: AiProviderType): LlmProvider {
  switch (providerType) {
    case "cursor_cli":
    case "codex_cli":
      return "codex";
    case "claude_cli":
      return "claude";
    case "anthropic_api":
      return "anthropic";
    case "openai_api":
    case "cursor_api":
    case "ollama_api":
      return "openai";
    case "gemini_cli":
    case "gemini_api":
      return "gemini";
    case "qwen_cli":
    case "alibaba_api":
      return "alibaba_api";
    case "openrouter":
      return "openrouter";
    case "copilot_cli":
    case "copilot_api":
      return "github_copilot";
    default:
      return "codex";
  }
}

export function providerTypeFromProvider(provider: string | null | undefined): AiProviderType {
  if (provider && AI_PROVIDER_TYPES.includes(provider as AiProviderType)) {
    return provider as AiProviderType;
  }

  switch (provider) {
    case "codex":
      return "codex_cli";
    case "claude":
      return "claude_cli";
    case "anthropic":
      return "anthropic_api";
    case "openai":
      return "openai_api";
    case "gemini":
      return "gemini_api";
    case "openrouter":
      return "openrouter";
    case "alibaba_api":
      return "alibaba_api";
    case "github_copilot":
      return "copilot_api";
    default:
      return "codex_cli";
  }
}

export function isApiProvider(provider: string): boolean {
  return API_PROVIDER_SET.has(provider) || provider.endsWith("_api");
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
