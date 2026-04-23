import type { LlmProvider, AnalysisOutput, FileDiffContent, DiffViewMode, Pass1Response, Pass2Response, ReviewComment, LlmActivityEntry } from "../types";

export const PROVIDER_LABELS: Record<LlmProvider, string> = {
  codex: "Codex CLI",
  claude: "Claude Code",
  anthropic: "Anthropic API",
  openai: "OpenAI API",
  gemini: "Gemini API",
  openrouter: "OpenRouter",
  github_copilot: "GitHub Copilot",
};

export type OnboardingStep = "recommended" | "api";
export type SubscriptionProvider = "codex" | "claude";
export type RightPanelTab = "activity" | "annotations" | "source" | "comments";
export type ActivityViewMode = "stream" | "all";
export type ReplayHunk = {
  id: string;
  filePath: string;
  startLine: number;
  endLine: number;
  originalStartLine: number;
  originalEndLine: number;
  isDeletionOnly: boolean;
  selectedCode: string | null;
};
export type ActivityKind =
  | "system"
  | "search"
  | "read"
  | "command"
  | "reasoning"
  | "result"
  | "warning"
  | "error";

export const API_PROVIDER_OPTIONS: LlmProvider[] = ["openai", "anthropic", "gemini", "openrouter", "github_copilot"];
export const ACTIVITY_STREAM_LIMIT = 10;
export const COMPARE_TARGET_UNSTAGED = "__DIFFCORE_UNSTAGED__";
export const COMPARE_TARGET_STAGED = "__DIFFCORE_STAGED__";
// TODO: re-enable app state save/restore after UX and reliability pass.
export const STATE_SAVE_RESTORE_ENABLED = false;

export type CompareMode = "branch" | "unstaged_to_staged" | "invalid";

export type PersistedAppState = {
  version: number;
  repoPath: string;
  baseRef: string;
  headRef: string | null;
  includeUncommitted: boolean;
  showUnchangedFiles: boolean;
  analysis: AnalysisOutput | null;
  selectedGroupId: string | null;
  selectedFile: string | null;
  fileDiff: FileDiffContent | null;
  openTabs: Array<{ path: string; groupId: string }>;
  comments: ReviewComment[];
  reviewedGroupIds: string[];
  rightPanelTab: RightPanelTab;
  annotationSubTab: "info" | "graph" | "edges";
  graphGranularity: "file" | "module_class_method";
  replayActive: boolean;
  replayStep: number;
  replayVisited: string[];
  replayHunkIndex: number;
  replayViewedHunkIds: string[];
  overview: Pass1Response | null;
  deepAnalyses: Record<string, Pass2Response>;
  activityEntries: LlmActivityEntry[];
  activityError: string | null;
  activityViewMode: ActivityViewMode;
  diffViewMode: DiffViewMode;
  recentRepoPaths: string[];
  favoriteRepoPaths: string[];
};

export type CrossFileSearchMatch = {
  line_number: number;
  line_text: string;
};

export type CrossFileSearchResult = {
  file_path: string;
  matches: CrossFileSearchMatch[];
};

export type FileShortStatus = {
  path: string;
  status: "A" | "M" | "D" | "R" | "C" | string;
};

export const SUBSCRIPTION_BACKENDS: Array<{
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
