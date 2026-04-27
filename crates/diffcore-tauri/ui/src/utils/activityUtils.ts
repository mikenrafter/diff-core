/**
 * Activity presentation utilities.
 *
 * These pure functions format LLM activity stream entries for display in the
 * Activity tab. No React or hooks — safe to import anywhere.
 */
import type { LlmActivityEntry } from "../types";
import { shortPath } from "./pathUtils";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type ActivityKind =
  | "system"
  | "search"
  | "read"
  | "command"
  | "reasoning"
  | "result"
  | "warning"
  | "error";

export interface ActivityPresentation {
  kind: ActivityKind;
  badge: string;
  title: string;
  detail?: string;
  detailLabel?: string;
  subject?: string;
  eventTypeLabel?: string;
  payloadText?: string;
  payloadSummary?: string;
  sourceLabel: string;
}

// ---------------------------------------------------------------------------
// Provider label map — kept here so activitySourceLabel doesn't need to
// import from App.tsx. Consumers that already import PROVIDER_LABELS from
// App.tsx can continue to do so; this copy is the canonical utility-layer one.
// ---------------------------------------------------------------------------

const PROVIDER_DISPLAY: Record<string, string> = {
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

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

export function formatActivityPayload(payload: unknown): string | null {
  if (payload == null) return null;
  if (typeof payload === "string") return payload;
  try {
    const serialized = JSON.stringify(payload, null, 2);
    return serialized.length > 2_000 ? `${serialized.slice(0, 2_000)}\n...` : serialized;
  } catch {
    return String(payload);
  }
}

export function summarizeActivityPayload(payload: unknown): string | null {
  if (payload == null) return null;
  if (Array.isArray(payload)) {
    return `${payload.length} item${payload.length === 1 ? "" : "s"}`;
  }
  if (typeof payload === "object") {
    return `${Object.keys(payload as Record<string, unknown>).length} fields`;
  }
  return typeof payload;
}

export function extractPathLikeToken(text: string): string | null {
  const matches = text.match(/(?:\/|\.{1,2}\/)?[A-Za-z0-9._@-]+(?:\/[A-Za-z0-9._@-]+)+/g);
  return matches && matches.length > 0 ? matches[matches.length - 1] : null;
}

export function extractActivitySubject(detail: string | null | undefined): string | null {
  if (!detail) return null;
  const pathLike = extractPathLikeToken(detail);
  if (pathLike) {
    return shortPath(pathLike);
  }
  const normalized = detail.replace(/\s+/g, " ").trim();
  if (!normalized) return null;
  return normalized.length > 42 ? `${normalized.slice(0, 39)}...` : normalized;
}

export function humanizeActivityTool(toolName: string): string {
  return toolName
    .replace(/[_-]+/g, " ")
    .replace(/([a-z])([A-Z])/g, "$1 $2")
    .trim();
}

export function isSearchCommand(command: string): boolean {
  const lower = command.toLowerCase();
  return lower.includes("rg")
    || lower.includes("grep")
    || lower.includes("fd")
    || lower.includes("find")
    || lower.includes("glob")
    || lower.includes("search");
}

export function isReadCommand(command: string): boolean {
  const lower = command.toLowerCase();
  return lower.includes("cat")
    || lower.includes("sed")
    || lower.includes("head")
    || lower.includes("tail")
    || lower.includes("read")
    || lower.includes("open")
    || lower.includes("view");
}

export function extractCommandFromActivity(message: string): string | null {
  const match = message.match(/(?:is running|finished)\s+(.+)$/i);
  return match?.[1]?.trim() ?? null;
}

export function extractActivityDetail(message: string): string | null {
  const match = message.match(/:\s+(.+)$/);
  return match?.[1]?.trim() ?? null;
}

export function formatActivityTimestamp(timestampMs: number): string {
  return new Date(timestampMs).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

export function activitySourceLabel(source: string): string {
  if (source in PROVIDER_DISPLAY) {
    return PROVIDER_DISPLAY[source];
  }
  if (source.toLowerCase() === "diffcore") {
    return "Diffcore";
  }
  return source.toUpperCase();
}

function canonicalToolProvider(provider: string | null | undefined): string | null {
  if (!provider) return null;
  switch (provider) {
    case "codex_cli":
    case "cursor_cli":
      return "codex";
    case "claude_cli":
      return "claude";
    default:
      return provider;
  }
}

export function providerSupportsToolActivity(provider: string | null | undefined): boolean {
  const canonical = canonicalToolProvider(provider);
  return canonical === "codex" || canonical === "claude";
}

// ---------------------------------------------------------------------------
// Presentation builder
// ---------------------------------------------------------------------------

export function withActivityMetadata(
  entry: LlmActivityEntry,
  presentation: ActivityPresentation,
): ActivityPresentation {
  return {
    ...presentation,
    eventTypeLabel: entry.event_type ?? undefined,
    payloadText: formatActivityPayload(entry.payload) ?? undefined,
    payloadSummary: summarizeActivityPayload(entry.payload) ?? undefined,
  };
}

export function describeActivityEntry(entry: LlmActivityEntry): ActivityPresentation {
  const sourceLabel = activitySourceLabel(entry.source);
  const message = entry.message.trim();
  const lowerMessage = message.toLowerCase();
  const eventType = entry.event_type?.toLowerCase() ?? "";
  const command = extractCommandFromActivity(message);
  const messageDetail = extractActivityDetail(message);

  if (entry.level === "error") {
    return withActivityMetadata(entry, { kind: "error", badge: "ERROR", title: message, sourceLabel });
  }

  if (entry.level === "warning") {
    return withActivityMetadata(entry, { kind: "warning", badge: "WARN", title: message, sourceLabel });
  }

  if (command) {
    if (isSearchCommand(command) || eventType.includes("search") || eventType.includes("grep")) {
      return withActivityMetadata(entry, {
        kind: "search",
        badge: "SEARCH",
        title: lowerMessage.includes("finished") ? "Repo search finished" : "Searching the repo",
        detail: command,
        detailLabel: "Command",
        subject: extractActivitySubject(command) ?? undefined,
        sourceLabel,
      });
    }
    if (isReadCommand(command) || eventType.includes("read") || eventType.includes("open") || eventType.includes("view")) {
      return withActivityMetadata(entry, {
        kind: "read",
        badge: "READ",
        title: lowerMessage.includes("finished") ? "Finished reading files" : "Reading files",
        detail: command,
        detailLabel: "Command",
        subject: extractActivitySubject(command) ?? undefined,
        sourceLabel,
      });
    }
    return withActivityMetadata(entry, {
      kind: "command",
      badge: "CMD",
      title: lowerMessage.includes("finished") ? "Shell command finished" : "Running shell command",
      detail: command,
      detailLabel: "Command",
      subject: extractActivitySubject(command) ?? undefined,
      sourceLabel,
    });
  }

  if (eventType.includes("tool_use")) {
    const toolName = eventType.split(".").pop() ?? "tool";
    const detail = messageDetail ?? humanizeActivityTool(toolName);
    const subject = extractActivitySubject(detail);
    if (isSearchCommand(toolName)) {
      return withActivityMetadata(entry, { kind: "search", badge: "SEARCH", title: `${sourceLabel} searched the repo`, detail, detailLabel: "Tool input", subject: subject ?? undefined, sourceLabel });
    }
    if (isReadCommand(toolName)) {
      return withActivityMetadata(entry, { kind: "read", badge: "READ", title: `${sourceLabel} inspected files`, detail, detailLabel: "Tool input", subject: subject ?? undefined, sourceLabel });
    }
    return withActivityMetadata(entry, {
      kind: "command",
      badge: "TOOL",
      title: `${sourceLabel} used ${humanizeActivityTool(toolName)}`,
      detail,
      detailLabel: "Tool input",
      subject: subject ?? undefined,
      sourceLabel,
    });
  }

  if (eventType.includes("search") || eventType.includes("grep") || lowerMessage.includes("searching the repo")) {
    return withActivityMetadata(entry, {
      kind: "search",
      badge: "SEARCH",
      title: lowerMessage.includes("finished") ? "Repo search finished" : "Searching the repo",
      detail: messageDetail ?? undefined,
      detailLabel: "Step detail",
      subject: extractActivitySubject(messageDetail) ?? undefined,
      sourceLabel,
    });
  }

  if (
    eventType.includes("read")
    || eventType.includes("open")
    || eventType.includes("view")
    || lowerMessage.includes("inspecting a file")
    || lowerMessage.includes("reading file")
  ) {
    return withActivityMetadata(entry, {
      kind: "read",
      badge: "READ",
      title: lowerMessage.includes("finished") ? "Finished reading files" : "Reading files",
      detail: messageDetail ?? undefined,
      detailLabel: "Step detail",
      subject: extractActivitySubject(messageDetail) ?? undefined,
      sourceLabel,
    });
  }

  if (lowerMessage.includes("reasoning") || eventType.includes("thinking")) {
    return withActivityMetadata(entry, {
      kind: "reasoning",
      badge: "THINK",
      title: `${sourceLabel} is reasoning`,
      detail: message === "Claude is reasoning" ? undefined : message,
      detailLabel: "Reasoning",
      sourceLabel,
    });
  }

  if (
    lowerMessage.includes("structured response ready")
    || lowerMessage.includes("prepared structured output")
    || lowerMessage.includes("writing pr-ready summary")
    || lowerMessage.includes("completed the response")
    || lowerMessage.includes("finished its turn")
  ) {
    return withActivityMetadata(entry, {
      kind: "result",
      badge: "RESULT",
      title: message,
      sourceLabel,
    });
  }

  if (lowerMessage.includes("preparing")) {
    return withActivityMetadata(entry, { kind: "system", badge: "PLAN", title: message, sourceLabel });
  }

  if (lowerMessage.includes("using ")) {
    return withActivityMetadata(entry, { kind: "system", badge: "MODEL", title: message, sourceLabel });
  }

  return withActivityMetadata(entry, { kind: "system", badge: "STEP", title: message, sourceLabel });
}

export function summarizeActivityTimeline(
  timeline: Array<{ presentation: ActivityPresentation }>,
): { total: number; search: number; read: number; command: number } {
  return timeline.reduce(
    (summary, item) => {
      summary.total += 1;
      if (item.presentation.kind === "search") summary.search += 1;
      if (item.presentation.kind === "read") summary.read += 1;
      if (item.presentation.kind === "command") summary.command += 1;
      return summary;
    },
    { total: 0, search: 0, read: 0, command: 0 },
  );
}

// ---------------------------------------------------------------------------
// Mock activity builder (used in demo / non-Tauri mode)
// ---------------------------------------------------------------------------

export function buildMockActivityEntries(
  operation: "overview" | "group" | "refinement",
  provider: string,
): Array<Omit<LlmActivityEntry, "timestamp_ms">> {
  const canonical = canonicalToolProvider(provider);
  const toolBacked = providerSupportsToolActivity(provider);
  const source = toolBacked ? provider : "diffcore";
  const providerName = canonical === "claude" ? "Claude" : "Codex";

  const sharedStart: Array<Omit<LlmActivityEntry, "timestamp_ms">> = [
    {
      source: "diffcore",
      level: "info",
      message: `Preparing ${operation === "group" ? "deep analysis" : operation} request`,
      event_type: "diffcore.prepare",
      payload: { operation, stage: "prepare" },
    },
  ];

  if (!toolBacked) {
    return [
      ...sharedStart,
      {
        source: "diffcore",
        level: "info",
        message: "Direct API mode only streams high-level progress. Switch to Codex CLI or Claude Code for file reads, grep, and shell activity.",
        event_type: "diffcore.direct_api",
      },
      {
        source: provider,
        level: "info",
        message: operation === "refinement" ? "Reviewing current groups and producing a structured verdict" : "Submitting structured request to the API provider",
        event_type: "provider.request",
      },
      {
        source: provider,
        level: "info",
        message: operation === "refinement" ? "Refinement rationale: keep the current grouping because the changed files already form coherent review flows." : "Structured response ready",
        event_type: "provider.result",
      },
    ];
  }

  if (operation === "overview") {
    return [
      ...sharedStart,
      {
        source,
        level: "info",
        message: `${providerName} is running rg --files crates/diffcore-tauri/ui/src`,
        event_type: "stdout.command_execution",
        payload: {
          command: "rg --files crates/diffcore-tauri/ui/src",
          cwd: "crates/diffcore-tauri/ui/src",
        },
      },
      {
        source,
        level: "info",
        message: `${providerName} is running sed -n '1,240p' crates/diffcore-tauri/ui/src/App.tsx`,
        event_type: "stdout.command_execution",
        payload: {
          command: "sed -n '1,240p' crates/diffcore-tauri/ui/src/App.tsx",
          path: "crates/diffcore-tauri/ui/src/App.tsx",
        },
      },
      {
        source,
        level: "info",
        message: "Writing PR-ready summary",
        event_type: "provider.summary",
      },
    ];
  }

  if (operation === "group") {
    return [
      ...sharedStart,
      {
        source,
        level: "info",
        message: `${providerName} is running rg "UserService|CreateUserInput" -n crates/diffcore-tauri/ui/src`,
        event_type: "stdout.command_execution",
        payload: {
          command: `rg "UserService|CreateUserInput" -n crates/diffcore-tauri/ui/src`,
          query: "UserService|CreateUserInput",
        },
      },
      {
        source,
        level: "info",
        message: `${providerName} is running sed -n '1,220p' crates/diffcore-tauri/ui/src/mock.ts`,
        event_type: "stdout.command_execution",
        payload: {
          command: "sed -n '1,220p' crates/diffcore-tauri/ui/src/mock.ts",
          path: "crates/diffcore-tauri/ui/src/mock.ts",
        },
      },
      {
        source,
        level: "info",
        message: "Assembling cross-cutting concerns",
        event_type: "provider.cross_cutting",
      },
    ];
  }

  return [
    ...sharedStart,
    {
      source,
      level: "info",
      message: `${providerName} is running rg --files /demo/repo | head -n 40`,
      event_type: "stdout.command_execution",
      payload: {
        command: "rg --files /demo/repo | head -n 40",
        cwd: "/demo/repo",
      },
    },
    {
      source,
      level: "info",
      message: `${providerName} is running sed -n '1,220p' /demo/repo/src/routes/users.ts`,
      event_type: "stdout.command_execution",
      payload: {
        command: "sed -n '1,220p' /demo/repo/src/routes/users.ts",
        path: "/demo/repo/src/routes/users.ts",
      },
    },
    {
      source,
      level: "info",
      message: "Refinement rationale: keep the current grouping because the changed files already form coherent review flows.",
      event_type: "refinement.reasoning",
    },
  ];
}
