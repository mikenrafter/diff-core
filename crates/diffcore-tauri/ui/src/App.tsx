import { useState, useCallback, useEffect, useRef, useMemo } from "react";
import type {
  AnalysisOutput,
  FlowGroup,
  FileDiffContent,
  DiffViewMode,
  Pass1Response,
  Pass1GroupAnnotation,
  Pass2Response,
  RepoInfo,
  BranchInfo,
  CommitInfo,
  LlmSettings,
  LlmProvider,
  AsyncLlmJobStart,
  LlmActivityEntry,
  LlmActivityJob,
  RefinementResult,
  RefinementResponse,
  ReviewComment,
  CommentInput,
  InfraSubGroup,
} from "./types";
import { LLM_PROVIDERS, DEFAULT_MODELS_BY_PROVIDER } from "./types";
import type { ModelInfo } from "./types";
import DiffViewer, { type DiffViewerHandle, type EditedHunk } from "./components/DiffViewer";
import FlowGraph from "./components/FlowGraph";
import SourceExplorer, { type SourceFocusRequest } from "./components/SourceExplorer";
import Dropdown from "./components/Dropdown";
import FileDisplay from "./components/FileDisplay";
// RiskHeatmap hidden (Phase 9.4) — component kept for future re-enablement
// import RiskHeatmap from "./components/RiskHeatmap";
import ErrorBoundary from "./components/ErrorBoundary";
import { buildManifestPrompt } from "./buildManifestPrompt";
import { MOCK_ANALYSIS, MOCK_DIFFS, MOCK_PASS1, MOCK_PASS2, MOCK_REPO_INFO, MOCK_LLM_SETTINGS, MOCK_REFINEMENT } from "./mock";
import { shortPath, shortSymbol, symbolFilePath, parseSymbolEndpoint, findLineContainingSymbol, truncateSearchResultLine } from "./utils/pathUtils";
import { resolveFileShortStatus, formatBranchStatus, formatCompareTargetLabel, COMPARE_TARGET_UNSTAGED, COMPARE_TARGET_STAGED } from "./utils/gitUtils";
import { riskLevel, getGroupChangeIndicator, getFileMovedIndicator, computeToolEditHunks } from "./utils/groupUtils";
import { describeActivityEntry, summarizeActivityTimeline, providerSupportsToolActivity, buildMockActivityEntries, formatActivityTimestamp } from "./utils/activityUtils";
import { resolveInteractiveProvider, resolveInteractiveModel, isApiProvider } from "./utils/llmUtils";
import type { SubscriptionProvider } from "./utils/llmUtils";

/** Detect if running inside Tauri (vs plain browser for demo/testing). */
const IS_TAURI = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** Lazy-import Tauri invoke only when in Tauri context. */
async function tauriInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd, args);
}

const PROVIDER_LABELS: Record<LlmProvider, string> = {
  codex: "Codex CLI",
  claude: "Claude Code",
  anthropic: "Anthropic API",
  openai: "OpenAI API",
  gemini: "Gemini API",
  openrouter: "OpenRouter",
  github_copilot: "GitHub Copilot",
};

type OnboardingStep = "recommended" | "api";
type RightPanelTab = "activity" | "annotations" | "source" | "comments";
type ActivityViewMode = "stream" | "all";
type ReplayHunk = {
  id: string;
  filePath: string;
  startLine: number;
  endLine: number;
  originalStartLine: number;
  originalEndLine: number;
  isDeletionOnly: boolean;
  selectedCode: string | null;
};

const API_PROVIDER_OPTIONS: LlmProvider[] = ["openai", "anthropic", "gemini", "openrouter", "github_copilot"];
const ACTIVITY_STREAM_LIMIT = 10;
// TODO: re-enable app state save/restore after UX and reliability pass.
const STATE_SAVE_RESTORE_ENABLED = false;

type CompareMode = "branch" | "unstaged_to_staged" | "invalid";

type PersistedAppState = {
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

type CrossFileSearchMatch = {
  line_number: number;
  line_text: string;
};

type CrossFileSearchResult = {
  file_path: string;
  matches: CrossFileSearchMatch[];
};

type FileShortStatus = {
  path: string;
  status: "A" | "M" | "D" | "R" | "C" | string;
};

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

/** Three-panel layout: flow groups | diff viewer | annotations */
export default function App() {
  const [analysis, setAnalysis] = useState<AnalysisOutput | null>(null);
  const [selectedGroup, setSelectedGroup] = useState<FlowGroup | null>(null);
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [fileDiff, setFileDiff] = useState<FileDiffContent | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Test-only: when set, the named panel's ErrorBoundary will catch a deliberate crash
  const [crashPanel, setCrashPanel] = useState<string | null>(null);

  // LLM annotation state
  const [overview, setOverview] = useState<Pass1Response | null>(null);
  const [deepAnalyses, setDeepAnalyses] = useState<Record<string, Pass2Response>>({});
  const [annotating, setAnnotating] = useState(false);
  const [deepAnalyzing, setDeepAnalyzing] = useState(false);
  // Counter to track concurrent deep analysis requests — prevents premature loading state clear
  const deepAnalyzingCount = useRef(0);
  const [activityJob, setActivityJob] = useState<LlmActivityJob | null>(null);
  const [activityEntries, setActivityEntries] = useState<LlmActivityEntry[]>([]);
  const [activityError, setActivityError] = useState<string | null>(null);
  const [activityViewMode, setActivityViewMode] = useState<ActivityViewMode>("stream");
  const [inspectedActivityId, setInspectedActivityId] = useState<string | null>(null);
  const activitySourceRef = useRef<EventSource | null>(null);
  const activityLogRef = useRef<HTMLDivElement | null>(null);
  const [rightPanelTab, setRightPanelTab] = useState<RightPanelTab>("annotations");
  const [sourceFocusRequest, setSourceFocusRequest] = useState<SourceFocusRequest | null>(null);
  const [regenDialogOpen, setRegenDialogOpen] = useState(false);
  const [regenFeedbackText, setRegenFeedbackText] = useState("");
  const [regenIncludePreviousOutput, setRegenIncludePreviousOutput] = useState(true);

  // Repo and git state
  const [repoPath, setRepoPath] = useState(IS_TAURI ? "" : "/demo/repo");
  const [baseRef, setBaseRef] = useState("main");
  const [headRef, setHeadRef] = useState<string | null>(null);
  const [repoInfo, setRepoInfo] = useState<RepoInfo | null>(null);
  const [branchDropdownOpen, setBranchDropdownOpen] = useState(false);
  const [headBranchDropdownOpen, setHeadBranchDropdownOpen] = useState(false);
  const [recentCommits, setRecentCommits] = useState<CommitInfo[]>([]);
  const [showHeadCommits, setShowHeadCommits] = useState(false);
  const [showBaseCommits, setShowBaseCommits] = useState(false);

  // Diff view mode: side-by-side, inline, or dynamic (per-file density)
  const [diffViewMode, setDiffViewMode] = useState<DiffViewMode>("side-by-side");

  // Whether the "Direct API mode" banner in the Activity tab has been
  // dismissed. Persisted globally (a single dismissal applies to every
  // direct-API provider) under "diffcore.directApiNotice.dismissed".
  const [directApiNoticeDismissed, setDirectApiNoticeDismissed] = useState(false);

  // Dynamic model lists (fetched from provider APIs, cached 24h)
  const [providerModels, setProviderModels] = useState<Record<string, string[]>>({});
  const [modelsLoading, setModelsLoading] = useState<string | null>(null);

  // LLM API key availability
  const [hasApiKey, setHasApiKey] = useState(!IS_TAURI); // Demo mode always has "key"

  // Diff behavior
  const [includeUncommitted, setIncludeUncommitted] = useState(true);
  const [showUnchangedFiles, setShowUnchangedFiles] = useState(false);
  const [crossFileSearchOpen, setCrossFileSearchOpen] = useState(false);
  const [crossFileSearchQuery, setCrossFileSearchQuery] = useState("");
  const [crossFileSearchLoading, setCrossFileSearchLoading] = useState(false);
  const [crossFileSearchResults, setCrossFileSearchResults] = useState<CrossFileSearchResult[]>([]);
  const [crossFileSearchError, setCrossFileSearchError] = useState<string | null>(null);
  const crossFileSearchInputRef = useRef<HTMLInputElement>(null);
  const [fileStatusByPath, setFileStatusByPath] = useState<Record<string, string>>({});
  const [repoQuickPickOpen, setRepoQuickPickOpen] = useState(false);
  const [recentRepoPaths, setRecentRepoPaths] = useState<string[]>([]);
  const [favoriteRepoPaths, setFavoriteRepoPaths] = useState<string[]>([]);

  const comparisonMode = useMemo<CompareMode>(() => {
    const sourceIsUnstaged = headRef === COMPARE_TARGET_UNSTAGED;
    const targetIsStaged = baseRef === COMPARE_TARGET_STAGED;
    const sourceIsSpecial = sourceIsUnstaged || headRef === COMPARE_TARGET_STAGED;
    const targetIsSpecial = targetIsStaged || baseRef === COMPARE_TARGET_UNSTAGED;

    if (sourceIsUnstaged && targetIsStaged) return "unstaged_to_staged";
    if (!sourceIsSpecial && !targetIsSpecial) return "branch";
    return "invalid";
  }, [headRef, baseRef]);

  const analysisDiffArgs = useMemo(() => {
    if (comparisonMode === "unstaged_to_staged") {
      return {
        base: null as string | null,
        head: null as string | null,
        staged: false,
        unstaged: true,
        prPreview: false,
        includeUncommitted: false,
      };
    }

    return {
      base: baseRef || "main",
      head: headRef || null,
      staged: false,
      unstaged: false,
      prPreview: true,
      // Branch comparison explicitly excludes uncommitted worktree changes.
      includeUncommitted: false,
    };
  }, [comparisonMode, baseRef, headRef]);

  const fileDiffArgs = useMemo(() => {
    if (comparisonMode === "unstaged_to_staged") {
      return {
        base: null as string | null,
        head: null as string | null,
        staged: false,
        unstaged: true,
        includeUncommitted: false,
      };
    }

    return {
      base: baseRef || "main",
      head: null as string | null,
      staged: false,
      unstaged: false,
      includeUncommitted: false,
    };
  }, [comparisonMode, baseRef]);

  const editsEnabled = comparisonMode === "unstaged_to_staged";

  // LLM settings
  const [llmSettings, setLlmSettings] = useState<LlmSettings | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [aiSetupOpen, setAiSetupOpen] = useState(false);
  const [aiSetupStep, setAiSetupStep] = useState<OnboardingStep>("recommended");
  const [apiProviderDraft, setApiProviderDraft] = useState<LlmProvider>("openai");
  const [apiKeyInput, setApiKeyInput] = useState("");

  // Ignore paths state
  const [ignorePaths, setIgnorePaths] = useState<string[]>([]);
  const [ignorePathInput, setIgnorePathInput] = useState("");

  // Flow review tick-off state (session-only)
  const [reviewedGroupIds, setReviewedGroupIds] = useState<Set<string>>(new Set());

  // Flow replay state
  const [replayActive, setReplayActive] = useState(false);
  const [replayStep, setReplayStep] = useState(0);
  const [replayVisited, setReplayVisited] = useState<Set<string>>(new Set());
  const [replayHunkIndex, setReplayHunkIndex] = useState(0);
  const [replayViewedHunkIds, setReplayViewedHunkIds] = useState<Set<string>>(new Set());
  const [currentReplayHunks, setCurrentReplayHunks] = useState<ReplayHunk[]>([]);

  // Refinement state
  const [originalGroups, setOriginalGroups] = useState<FlowGroup[] | null>(null);
  const [refinedGroups, setRefinedGroups] = useState<FlowGroup[] | null>(null);
  const [refinementResponse, setRefinementResponse] = useState<RefinementResponse | null>(null);
  const [refinementProvider, setRefinementProvider] = useState<string | null>(null);
  const [refinementModel, setRefinementModel] = useState<string | null>(null);
  const [refinementHadChanges, setRefinementHadChanges] = useState<boolean | null>(null);
  const [showRefined, setShowRefined] = useState(false);
  const [groupListTransitionState, setGroupListTransitionState] = useState<"idle" | "fading-out" | "fading-in">("idle");
  const groupListTransitionTimers = useRef<number[]>([]);
  const [refining, setRefining] = useState(false);

  // Context menu state (right-click on file items)
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number; filePath: string } | null>(null);

  // Multi-file tab bar state
  const [openTabs, setOpenTabs] = useState<Array<{ path: string; groupId: string }>>([]);
  const [tabContextMenu, setTabContextMenu] = useState<{ x: number; y: number; tabPath: string } | null>(null);

  // Review comments state
  const [comments, setComments] = useState<ReviewComment[]>([]);
  const [commentInput, setCommentInput] = useState<CommentInput | null>(null);
  const [commentText, setCommentText] = useState("");
  const commentInputRef = useRef<HTMLTextAreaElement>(null);
  const diffViewerRef = useRef<DiffViewerHandle>(null);
  const repoInputRef = useRef<HTMLInputElement>(null);
  const launchDirectoryCheckedRef = useRef(false);

  // Annotation sub-tab: "info" | "graph" | "edges"
  const [annotationSubTab, setAnnotationSubTab] = useState<"info" | "graph" | "edges">("info");
  const [graphGranularity, setGraphGranularity] = useState<"file" | "module_class_method">("file");
  const [commentsCollapsed, setCommentsCollapsed] = useState(false);
  const [activeCommentId, setActiveCommentId] = useState<string | null>(null);
  const pendingScrollToCommentRef = useRef<{ startLine: number; endLine?: number; commentId: string } | null>(null);
  const pendingReplayHunkScrollRef = useRef<{ filePath: string; targetHunkIndex: number; attempts: number; stepIndex: number; direction?: 1 | -1 } | null>(null);
  const pendingReplayResolveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pendingSymbolScrollRef = useRef<{ symbol: string } | null>(null);
  const [editingCommentId, setEditingCommentId] = useState<string | null>(null);
  const [editingCommentText, setEditingCommentText] = useState("");

  // Infrastructure group collapse state
  const [infraExpanded, setInfraExpanded] = useState(false);
  const [infraShowAll, setInfraShowAll] = useState(false);
  const [infraSubGroupsExpanded, setInfraSubGroupsExpanded] = useState<Set<string>>(new Set());

  // Right panel collapse/resize state
  const [rightPanelCollapsed, setRightPanelCollapsed] = useState(false);
  const [rightPanelWidth, setRightPanelWidth] = useState(320);
  const rightPanelDragging = useRef(false);
  const rightPanelStartX = useRef(0);
  const rightPanelStartWidth = useRef(0);
  const rightPanelRafId = useRef(0);

  // Groups manifest watching state
  const [watchedManifestPath, setWatchedManifestPath] = useState<string | null>(null);

  // Update notification state
  const [updateAvailable, setUpdateAvailable] = useState<{ version: string; body: string } | null>(null);
  const [updating, setUpdating] = useState(false);

  // Toast notification state (auto-dismiss)
  const [toast, setToast] = useState<string | null>(null);
  const toastTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Right panel drag resize handlers (rAF-throttled to prevent jank)
  useEffect(() => {
    const onMouseMove = (e: MouseEvent) => {
      if (!rightPanelDragging.current) return;
      const clientX = e.clientX;
      cancelAnimationFrame(rightPanelRafId.current);
      rightPanelRafId.current = requestAnimationFrame(() => {
        const delta = rightPanelStartX.current - clientX;
        const newWidth = Math.max(200, Math.min(800, rightPanelStartWidth.current + delta));
        setRightPanelWidth(newWidth);
      });
    };
    const onMouseUp = () => {
      if (rightPanelDragging.current) {
        rightPanelDragging.current = false;
        cancelAnimationFrame(rightPanelRafId.current);
        document.body.style.cursor = "";
        document.body.style.userSelect = "";
        document.querySelector(".panel-right")?.classList.remove("panel-right-dragging");
      }
    };
    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
    return () => {
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
    };
  }, []);

  const startRightPanelDrag = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    rightPanelDragging.current = true;
    rightPanelStartX.current = e.clientX;
    rightPanelStartWidth.current = rightPanelWidth;
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
    document.querySelector(".panel-right")?.classList.add("panel-right-dragging");
  }, [rightPanelWidth]);

  // Demo mode: auto-load mock data on mount when not in Tauri
  const demoLoaded = useRef(false);
  const demoLlmSettingsRef = useRef<LlmSettings>(MOCK_LLM_SETTINGS);
  const aiSetupDismissed = useRef(false);

  // Refs for keyboard nav to access latest state without re-registering listener
  const selectedGroupRef = useRef(selectedGroup);
  const selectedFileRef = useRef(selectedFile);
  const sortedGroupsRef = useRef<FlowGroup[]>([]);
  const replayActiveRef = useRef(replayActive);
  const replayStepRef = useRef(replayStep);
  selectedGroupRef.current = selectedGroup;
  selectedFileRef.current = selectedFile;
  replayActiveRef.current = replayActive;
  replayStepRef.current = replayStep;

  // Debounce refs for keyboard file navigation
  const pendingFileNav = useRef<ReturnType<typeof setTimeout> | null>(null);
  // Staleness guard: incremented on each file selection, stale responses are discarded
  const fileDiffGeneration = useRef(0);
  // Debounced editor-to-disk/comment sync state
  const pendingEditSync = useRef<ReturnType<typeof setTimeout> | null>(null);
  const latestEditPayloadRef = useRef<{
    filePath: string;
    groupId: string;
    newContent: string;
    hunks: EditedHunk[];
  } | null>(null);
  const fileEditBaselineRef = useRef<Map<string, string>>(new Map());
  const pendingAppStatePersist = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (pendingEditSync.current) {
        clearTimeout(pendingEditSync.current);
      }
      if (pendingAppStatePersist.current) {
        clearTimeout(pendingAppStatePersist.current);
      }
      if (pendingReplayResolveTimerRef.current) {
        clearTimeout(pendingReplayResolveTimerRef.current);
      }
    };
  }, []);

  const buildPersistedState = useCallback((): PersistedAppState => ({
    version: 1,
    repoPath,
    baseRef,
    headRef,
    includeUncommitted,
    showUnchangedFiles,
    analysis,
    selectedGroupId: selectedGroup?.id ?? null,
    selectedFile,
    fileDiff,
    openTabs,
    comments,
    reviewedGroupIds: Array.from(reviewedGroupIds),
    rightPanelTab,
    annotationSubTab,
    graphGranularity,
    replayActive,
    replayStep,
    replayVisited: Array.from(replayVisited),
    replayHunkIndex,
    replayViewedHunkIds: Array.from(replayViewedHunkIds),
    overview,
    deepAnalyses,
    activityEntries,
    activityError,
    activityViewMode,
    diffViewMode,
    recentRepoPaths,
    favoriteRepoPaths,
  }), [
    repoPath,
    baseRef,
    headRef,
    includeUncommitted,
    showUnchangedFiles,
    analysis,
    selectedGroup,
    selectedFile,
    fileDiff,
    openTabs,
    comments,
    reviewedGroupIds,
    rightPanelTab,
    annotationSubTab,
    graphGranularity,
    replayActive,
    replayStep,
    replayVisited,
    replayHunkIndex,
    replayViewedHunkIds,
    overview,
    deepAnalyses,
    activityEntries,
    activityError,
    activityViewMode,
    diffViewMode,
    recentRepoPaths,
    favoriteRepoPaths,
  ]);

  const restoreLastSessionState = useCallback(async () => {
    if (!STATE_SAVE_RESTORE_ENABLED) return;
    if (!IS_TAURI) return;
    try {
      const snapshot = await tauriInvoke<PersistedAppState | null>("load_last_app_state");
      if (!snapshot) {
        setToast("No saved session found");
        return;
      }

      setRepoPath(snapshot.repoPath || "");
      setBaseRef(snapshot.baseRef || "main");
      setHeadRef(snapshot.headRef ?? null);
      setIncludeUncommitted(snapshot.includeUncommitted ?? false);
      setShowUnchangedFiles(snapshot.showUnchangedFiles ?? false);
      setAnalysis(snapshot.analysis ?? null);
      setOverview(snapshot.overview ?? null);
      setDeepAnalyses(snapshot.deepAnalyses ?? {});
      setSelectedGroup(
        snapshot.analysis?.groups.find((g) => g.id === snapshot.selectedGroupId) ?? null,
      );
      setSelectedFile(snapshot.selectedFile ?? null);
      setFileDiff(snapshot.fileDiff ?? null);
      setOpenTabs(snapshot.openTabs ?? []);
      setComments(snapshot.comments ?? []);
      setReviewedGroupIds(new Set(snapshot.reviewedGroupIds ?? []));
      setRightPanelTab(snapshot.rightPanelTab ?? "annotations");
      setAnnotationSubTab(snapshot.annotationSubTab ?? "info");
      setGraphGranularity(snapshot.graphGranularity ?? "file");
      setReplayActive(snapshot.replayActive ?? false);
      setReplayStep(snapshot.replayStep ?? 0);
      setReplayVisited(new Set(snapshot.replayVisited ?? []));
      setReplayHunkIndex(snapshot.replayHunkIndex ?? 0);
      setReplayViewedHunkIds(new Set(snapshot.replayViewedHunkIds ?? []));
      setActivityEntries(snapshot.activityEntries ?? []);
      setActivityError(snapshot.activityError ?? null);
      setActivityViewMode(snapshot.activityViewMode ?? "stream");
      setDiffViewMode(snapshot.diffViewMode ?? "dynamic");
      setRecentRepoPaths(snapshot.recentRepoPaths ?? []);
      setFavoriteRepoPaths(snapshot.favoriteRepoPaths ?? []);
      setToast("Session restored");
    } catch {
      setToast("Failed to restore session");
    }
  }, []);

  /** Load LLM settings from backend. */
  const loadLlmSettings = useCallback(async (path: string | null) => {
    try {
      let settings: LlmSettings;
      if (IS_TAURI) {
        settings = await tauriInvoke<LlmSettings>("get_llm_settings", {
          repoPath: path,
        });
      } else {
        settings = demoLlmSettingsRef.current;
      }
      setLlmSettings(settings);
      setHasApiKey(settings.has_api_key);
      setIncludeUncommitted(settings.include_uncommitted);
      if (isApiProvider(settings.provider)) {
        setApiProviderDraft(settings.provider as LlmProvider);
      }
    } catch {
      // Non-fatal
    }
  }, []);

  /** Save LLM settings to backend. */
  const saveLlmSettings = useCallback(async (settings: LlmSettings) => {
    setLlmSettings(settings);
    setHasApiKey(settings.has_api_key);
    if (isApiProvider(settings.provider)) {
      setApiProviderDraft(settings.provider as LlmProvider);
    }
    if (!IS_TAURI) {
      demoLlmSettingsRef.current = settings;
      return;
    }
    try {
      await tauriInvoke("save_llm_settings", {
        repoPath: repoPath || "",
        settings,
      });
      // Re-check API key availability after save
      const updated = await tauriInvoke<LlmSettings>("get_llm_settings", {
        repoPath: repoPath || null,
      });
      setLlmSettings(updated);
      setHasApiKey(updated.has_api_key);
      if (isApiProvider(updated.provider)) {
        setApiProviderDraft(updated.provider as LlmProvider);
      }
    } catch {
      // Non-fatal: settings are still applied in-memory
    }
  }, [repoPath]);

  /** Load ignore paths from .diffcore.toml. */
  const loadIgnorePaths = useCallback(async (path: string | null) => {
    if (!IS_TAURI || !path) return;
    try {
      const paths = await tauriInvoke<string[]>("get_ignore_paths", { repoPath: path });
      setIgnorePaths(paths);
    } catch {
      // Non-fatal: ignore paths just won't be shown
    }
  }, []);

  /** Fetch available models from provider API and update providerModels state. */
  const fetchModelsForProvider = useCallback(async (provider: string, force = false) => {
    if (!IS_TAURI) {
      // In demo mode, use static fallback
      setProviderModels((prev) => ({
        ...prev,
        [provider]: DEFAULT_MODELS_BY_PROVIDER[provider as LlmProvider] ?? [],
      }));
      return;
    }
    setModelsLoading(provider);
    try {
      const models = await tauriInvoke<ModelInfo[]>("fetch_provider_models", {
        provider,
        forceRefresh: force,
      });
      setProviderModels((prev) => ({
        ...prev,
        [provider]: models.map((m) => m.id),
      }));
    } catch {
      // Fallback to static list on error
      setProviderModels((prev) => ({
        ...prev,
        [provider]: DEFAULT_MODELS_BY_PROVIDER[provider as LlmProvider] ?? [],
      }));
    } finally {
      setModelsLoading(null);
    }
  }, []);

  /** Fetch repository info (branches, worktrees, status). */
  const loadRepoInfo = useCallback(async (path: string) => {
    if (!path) return;
    try {
      let info: RepoInfo;
      if (IS_TAURI) {
        info = await tauriInvoke<RepoInfo>("get_repo_info", { repoPath: path });
      } else {
        await new Promise((r) => setTimeout(r, 100));
        info = MOCK_REPO_INFO;
      }
      setRepoInfo(info);
      if (IS_TAURI) {
        const commits = await tauriInvoke<CommitInfo[]>("list_commits", {
          repoPath: path,
          limit: 80,
        });
        setRecentCommits(commits);
      } else {
        setRecentCommits([]);
      }
      // Auto-set base ref to the detected default branch
      setBaseRef(info.default_branch);
      // Auto-set head ref to the current branch (what we're comparing FROM)
      setHeadRef(info.current_branch ?? "HEAD");
    } catch {
      // Non-fatal: we can still analyze without repo info
      setRepoInfo(null);
      setRecentCommits([]);
    }
    // Load LLM settings (includes API key check)
    loadLlmSettings(path);
    // Load ignore paths
    loadIgnorePaths(path);
  }, [loadLlmSettings, loadIgnorePaths]);

  const browseForRepository = useCallback(async () => {
    if (!IS_TAURI) return;
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const selected = await open({
        directory: true,
        multiple: false,
        title: "Select repository folder",
      });
      if (typeof selected === "string" && selected.length > 0) {
        setRepoPath(selected);
      }
    } catch {
      setToast("Failed to open folder picker");
    }
  }, []);

  useEffect(() => {
    if (!IS_TAURI || launchDirectoryCheckedRef.current || repoPath) return;
    launchDirectoryCheckedRef.current = true;
    tauriInvoke<string | null>("get_launch_directory")
      .then((dir) => {
        if (dir) setRepoPath(dir);
      })
      .catch(() => {});
  }, [repoPath]);

  // Load repo info when repo path changes
  useEffect(() => {
    if (repoPath) {
      loadRepoInfo(repoPath);
    } else {
      setRepoInfo(null);
      setRecentCommits([]);
    }
  }, [repoPath, loadRepoInfo]);

  useEffect(() => {
    const savedMode = window.localStorage.getItem("diffcore.diffViewMode");
    if (savedMode === "side-by-side" || savedMode === "inline" || savedMode === "dynamic") {
      setDiffViewMode(savedMode);
    }
  }, []);

  useEffect(() => {
    window.localStorage.setItem("diffcore.diffViewMode", diffViewMode);
  }, [diffViewMode]);

  useEffect(() => {
    const dismissed = window.localStorage.getItem("diffcore.directApiNotice.dismissed");
    if (dismissed === "1") {
      setDirectApiNoticeDismissed(true);
    }
  }, []);

  const dismissDirectApiNotice = useCallback(() => {
    setDirectApiNoticeDismissed(true);
    try {
      window.localStorage.setItem("diffcore.directApiNotice.dismissed", "1");
    } catch {
      // ignore quota / privacy-mode failures; in-memory dismissal still applies
    }
  }, []);

  useEffect(() => {
    const recentRaw = window.localStorage.getItem("diffcore.recentRepos");
    const favoriteRaw = window.localStorage.getItem("diffcore.favoriteRepos");
    if (recentRaw) {
      try {
        const parsed = JSON.parse(recentRaw);
        if (Array.isArray(parsed)) setRecentRepoPaths(parsed.filter((x) => typeof x === "string"));
      } catch {
        // ignore invalid saved recents
      }
    }
    if (favoriteRaw) {
      try {
        const parsed = JSON.parse(favoriteRaw);
        if (Array.isArray(parsed)) setFavoriteRepoPaths(parsed.filter((x) => typeof x === "string"));
      } catch {
        // ignore invalid saved favorites
      }
    }
  }, []);

  useEffect(() => {
    window.localStorage.setItem("diffcore.recentRepos", JSON.stringify(recentRepoPaths));
  }, [recentRepoPaths]);

  useEffect(() => {
    window.localStorage.setItem("diffcore.favoriteRepos", JSON.stringify(favoriteRepoPaths));
  }, [favoriteRepoPaths]);

  useEffect(() => {
    const path = repoPath.trim();
    if (!path) return;
    setRecentRepoPaths((prev) => {
      const deduped = [path, ...prev.filter((p) => p !== path)];
      return deduped.slice(0, 12);
    });
  }, [repoPath]);

  useEffect(() => {
    if (!repoPath) {
      loadLlmSettings(null);
    }
  }, [repoPath, loadLlmSettings]);

  useEffect(() => {
    if (!llmSettings) return;
    if (
      llmSettings.has_api_key
      || llmSettings.codex_authenticated
      || llmSettings.claude_authenticated
    ) {
      setAiSetupOpen(false);
      return;
    }
    if (aiSetupDismissed.current) return;
    setAiSetupOpen(true);
    setAiSetupStep("recommended");
  }, [llmSettings]);

  const closeActivityStream = useCallback(() => {
    if (activitySourceRef.current) {
      activitySourceRef.current.close();
      activitySourceRef.current = null;
    }
  }, []);

  useEffect(() => () => closeActivityStream(), [closeActivityStream]);

  useEffect(() => {
    if (activityJob || activityEntries.length > 0 || activityError) {
      setRightPanelTab("activity");
    }
  }, [activityEntries.length, activityError, activityJob]);

  const appendActivityEntry = useCallback((entry: LlmActivityEntry) => {
    setActivityEntries((prev) => [...prev, entry]);
  }, []);

  const runMockActivityJob = useCallback(
    async <T,>(
      job: LlmActivityJob,
      entries: Array<Omit<LlmActivityEntry, "timestamp_ms">>,
      result: T,
      onComplete: (value: T) => void,
    ) => {
      closeActivityStream();
      setActivityViewMode("stream");
      setInspectedActivityId(null);
      setActivityJob(job);
      setActivityEntries([]);
      setActivityError(null);
      for (const [index, entry] of entries.entries()) {
        await new Promise((resolve) => setTimeout(resolve, index === 0 ? 120 : 220));
        appendActivityEntry({ ...entry, timestamp_ms: Date.now() });
      }
      onComplete(result);
      setActivityJob(null);
    },
    [appendActivityEntry, closeActivityStream],
  );

  const runStreamingJob = useCallback(
    async <T,>(
      command: string,
      args: Record<string, unknown>,
      onComplete: (value: T) => void,
    ) => {
      closeActivityStream();
      setActivityViewMode("stream");
      setInspectedActivityId(null);
      setActivityEntries([]);
      setActivityError(null);

      const start = await tauriInvoke<AsyncLlmJobStart>(command, args);
      setActivityJob({
        job_id: start.job_id,
        operation: start.operation,
        provider: start.provider,
        model: start.model,
        title: start.title,
      });

      await new Promise<void>((resolve, reject) => {
        const source = new EventSource(start.stream_url);
        activitySourceRef.current = source;

        source.addEventListener("job_started", (event) => {
          try {
            const payload = JSON.parse((event as MessageEvent).data) as { title: string; provider: string; model: string; job_id: string; operation: string };
            setActivityJob({
              job_id: payload.job_id,
              operation: payload.operation,
              provider: payload.provider,
              model: payload.model,
              title: payload.title,
            });
          } catch {
            // Ignore malformed status events
          }
        });

        source.addEventListener("activity", (event) => {
          try {
            const payload = JSON.parse((event as MessageEvent).data) as { entry: LlmActivityEntry };
            appendActivityEntry(payload.entry);
          } catch {
            // Ignore malformed activity events
          }
        });

        source.addEventListener("completed", (event) => {
          closeActivityStream();
          try {
            const payload = JSON.parse((event as MessageEvent).data) as { result: T };
            onComplete(payload.result);
            setActivityJob(null);
            resolve();
          } catch (error) {
            const message = error instanceof Error ? error.message : String(error);
            setActivityError(message);
            reject(new Error(message));
          }
        });

        source.addEventListener("failed", (event) => {
          closeActivityStream();
          try {
            const payload = JSON.parse((event as MessageEvent).data) as { error: string };
            setActivityError(payload.error);
            appendActivityEntry({
              source: "diffcore",
              level: "error",
              message: payload.error,
              event_type: "job.failed",
              timestamp_ms: Date.now(),
            });
            setActivityJob(null);
            reject(new Error(payload.error));
          } catch {
            setActivityError("Activity stream failed");
            setActivityJob(null);
            reject(new Error("Activity stream failed"));
          }
        });

        source.onerror = () => {
          closeActivityStream();
          setActivityError("Activity stream disconnected");
          setActivityJob(null);
          reject(new Error("Activity stream disconnected"));
        };
      });
    },
    [appendActivityEntry, closeActivityStream],
  );

  const handleSelectFile = useCallback(
    async (path: string) => {
      setSelectedFile(path);
      setSourceFocusRequest(null);
      // Increment generation to mark any in-flight request as stale
      const generation = ++fileDiffGeneration.current;
      if (IS_TAURI) {
        if (!repoPath) return;
        try {
          const diff = await tauriInvoke<FileDiffContent>("get_file_diff", {
            repoPath,
            filePath: path,
            base: fileDiffArgs.base,
            head: fileDiffArgs.head,
            range: null,
            staged: fileDiffArgs.staged,
            unstaged: fileDiffArgs.unstaged,
            includeUncommitted: fileDiffArgs.includeUncommitted,
          });
          // Only apply if this is still the latest request
          if (generation === fileDiffGeneration.current) {
            fileEditBaselineRef.current.set(path, diff.new_content || "");
            setFileDiff(diff);
          }
        } catch (e) {
          if (generation === fileDiffGeneration.current) {
            setFileDiff(null);
            setError(`Failed to load diff for ${path}: ${String(e)}`);
          }
        }
      } else {
        if (generation === fileDiffGeneration.current) {
          const mock = MOCK_DIFFS[path] || null;
          if (mock) {
            fileEditBaselineRef.current.set(path, mock.new_content || "");
          }
          setFileDiff(mock);
        }
      }
    },
    [repoPath, fileDiffArgs],
  );

  /** Debounced file selection for keyboard navigation — updates highlight immediately,
   *  delays the expensive diff fetch by 150ms so rapid j/k presses only fetch the final file. */
  const handleSelectFileDebounced = useCallback(
    (path: string) => {
      // Immediately update highlight for visual feedback
      setSelectedFile(path);
      // Debounce the expensive diff fetch
      if (pendingFileNav.current) clearTimeout(pendingFileNav.current);
      pendingFileNav.current = setTimeout(() => {
        handleSelectFile(path);
      }, 150);
    },
    [handleSelectFile],
  );

  /** Open a file in a tab — adds to tabs if not already present, then selects it. */
  const openFileInTab = useCallback(
    (path: string, groupId: string) => {
      setOpenTabs((prev) => {
        if (prev.some((t) => t.path === path)) return prev;
        return [...prev, { path, groupId }];
      });
      handleSelectFile(path);
    },
    [handleSelectFile],
  );

  const replayHunks = currentReplayHunks;

  const mapEditedHunksToReplayHunks = useCallback((filePath: string, hunks: EditedHunk[]): ReplayHunk[] => {
    return hunks.map((h, index) => ({
      id: `monaco_hunk_${filePath}_${h.modifiedStartLine}_${h.modifiedEndLine}_${index}`,
      filePath,
      startLine: h.modifiedStartLine,
      endLine: h.modifiedEndLine,
      originalStartLine: h.originalStartLine,
      originalEndLine: h.originalEndLine,
      isDeletionOnly: !!h.isDeletionOnly,
      selectedCode: h.selectedCode || null,
    }));
  }, []);

  const collectVisibleReplayHunks = useCallback((filePath: string): ReplayHunk[] => {
    const fromEditor = diffViewerRef.current?.getDiffHunks?.() ?? [];
    return mapEditedHunksToReplayHunks(filePath, fromEditor);
  }, [mapEditedHunksToReplayHunks]);

  const jumpToReplayHunk = useCallback(
    (index: number) => {
      const group = selectedGroupRef.current;
      if (!group || replayHunks.length === 0) return;
      const clamped = Math.max(0, Math.min(index, replayHunks.length - 1));
      const hunk = replayHunks[clamped];
      setReplayHunkIndex(clamped);
      if (replayActiveRef.current) {
        setReplayViewedHunkIds((prev) => {
          const next = new Set(prev);
          next.add(hunk.id);
          return next;
        });
      }

      if (selectedFileRef.current !== hunk.filePath) {
        const fileStepIndex = group.files.findIndex((f) => f.path === hunk.filePath);
        pendingReplayHunkScrollRef.current = {
          filePath: hunk.filePath,
          targetHunkIndex: clamped,
          attempts: 0,
          stepIndex: Math.max(0, fileStepIndex),
        };
        openFileInTab(hunk.filePath, group.id);
      } else {
        diffViewerRef.current?.scrollToHunk?.({
          originalStartLine: hunk.originalStartLine,
          originalEndLine: hunk.originalEndLine,
          modifiedStartLine: hunk.startLine,
          modifiedEndLine: hunk.endLine,
          selectedCode: hunk.selectedCode ?? "",
          isDeletionOnly: hunk.isDeletionOnly,
        });
      }

      const fileStepIndex = group.files.findIndex((f) => f.path === hunk.filePath);
      if (fileStepIndex >= 0) {
        setReplayStep(fileStepIndex);
        setReplayVisited((prev) => {
          const next = new Set(prev);
          next.add(hunk.filePath);
          return next;
        });
      }
    },
    [replayHunks, openFileInTab],
  );

  const commentOnCurrentReplayHunk = useCallback(() => {
    const group = selectedGroupRef.current;
    if (!group || replayHunks.length === 0) return;
    const hunk = replayHunks[Math.max(0, Math.min(replayHunkIndex, replayHunks.length - 1))];
    jumpToReplayHunk(replayHunkIndex);
    setCommentInput({
      type: "code",
      group_id: group.id,
      file_path: hunk.filePath,
      start_line: hunk.startLine,
      end_line: hunk.endLine,
      selected_code: hunk.selectedCode ?? undefined,
    });
    setCommentText("");
    setTimeout(() => commentInputRef.current?.focus(), 50);
  }, [replayHunkIndex, replayHunks, jumpToReplayHunk]);

  // When fileDiff loads and a pending scroll-to-comment is queued, scroll the diff viewer
  useEffect(() => {
    if (fileDiff && pendingScrollToCommentRef.current) {
      const { startLine, endLine, commentId } = pendingScrollToCommentRef.current;
      pendingScrollToCommentRef.current = null;
      setTimeout(() => {
        diffViewerRef.current?.scrollToLine(startLine, endLine);
        const el = document.querySelector(`.comment-strip-item[data-comment-id="${commentId}"]`);
        el?.scrollIntoView({ behavior: "smooth", block: "nearest" });
      }, 100);
    }
  }, [fileDiff]);

  useEffect(() => {
    if (!fileDiff || !pendingReplayHunkScrollRef.current) return;
    const pending = pendingReplayHunkScrollRef.current;
    const { filePath, targetHunkIndex } = pending;
    if (fileDiff.path !== filePath) return;

    const tryResolve = () => {
      const visible = collectVisibleReplayHunks(filePath);
      if (visible.length === 0) {
        if (!pendingReplayHunkScrollRef.current || pendingReplayHunkScrollRef.current.filePath !== filePath) {
          return;
        }
        if (pending.attempts >= 8) {
          const group = selectedGroupRef.current;
          if (group && pending.direction != null) {
            const nextStep = pending.stepIndex + pending.direction;
            if (nextStep >= 0 && nextStep < group.files.length) {
              const nextFilePath = group.files[nextStep].path;
              setReplayStep(nextStep);
              setReplayVisited((prev) => {
                const next = new Set(prev);
                next.add(nextFilePath);
                return next;
              });
              setCurrentReplayHunks([]);
              setReplayHunkIndex(0);
              pendingReplayHunkScrollRef.current = {
                filePath: nextFilePath,
                targetHunkIndex: pending.direction > 0 ? 0 : -1,
                attempts: 0,
                stepIndex: nextStep,
                direction: pending.direction,
              };
              openFileInTab(nextFilePath, group.id);
              return;
            }
          }
          pendingReplayHunkScrollRef.current = null;
          return;
        }
        pending.attempts += 1;
        pendingReplayResolveTimerRef.current = setTimeout(tryResolve, 60);
        return;
      }

      pendingReplayHunkScrollRef.current = null;
      setCurrentReplayHunks(visible);
      const fallback = Math.max(0, Math.min(targetHunkIndex, visible.length - 1));
      const chosen = targetHunkIndex < 0 ? visible.length - 1 : fallback;
      const hunk = visible[Math.max(0, chosen)];
      setReplayHunkIndex(Math.max(0, chosen));
      if (hunk) {
        diffViewerRef.current?.scrollToHunk?.({
          originalStartLine: hunk.originalStartLine,
          originalEndLine: hunk.originalEndLine,
          modifiedStartLine: hunk.startLine,
          modifiedEndLine: hunk.endLine,
          selectedCode: hunk.selectedCode ?? "",
          isDeletionOnly: hunk.isDeletionOnly,
        });
      }
    };

    pendingReplayResolveTimerRef.current = setTimeout(tryResolve, 60);

    return () => {
      if (pendingReplayResolveTimerRef.current) {
        clearTimeout(pendingReplayResolveTimerRef.current);
      }
    };
  }, [fileDiff, collectVisibleReplayHunks, openFileInTab]);

  useEffect(() => {
    if (!fileDiff || !pendingSymbolScrollRef.current) return;
    const { symbol } = pendingSymbolScrollRef.current;
    pendingSymbolScrollRef.current = null;
    const targetLine = findLineContainingSymbol(fileDiff.new_content || fileDiff.old_content || "", symbol);
    if (targetLine != null) {
      setTimeout(() => {
        diffViewerRef.current?.scrollToLine(targetLine, targetLine);
      }, 100);
    }
  }, [fileDiff]);

  useEffect(() => {
    if (!fileDiff) {
      setCurrentReplayHunks([]);
      setReplayHunkIndex(0);
      return;
    }
    const timer = setTimeout(() => {
      const visible = collectVisibleReplayHunks(fileDiff.path);
      setCurrentReplayHunks(visible);
      const pending = pendingReplayHunkScrollRef.current;
      if (pending && pending.filePath === fileDiff.path && visible.length > 0) {
        const fallback = Math.max(0, Math.min(pending.targetHunkIndex, visible.length - 1));
        const chosen = pending.targetHunkIndex < 0 ? visible.length - 1 : fallback;
        setReplayHunkIndex(Math.max(0, chosen));
        return;
      }
      setReplayHunkIndex((prev) => {
        if (visible.length === 0) return 0;
        return Math.max(0, Math.min(prev, visible.length - 1));
      });
    }, 100);
    return () => clearTimeout(timer);
  }, [fileDiff, collectVisibleReplayHunks]);

  /** Close a single tab. If it was active, activate a neighbor. */
  const closeTab = useCallback(
    (tabPath: string) => {
      setOpenTabs((prev) => {
        const idx = prev.findIndex((t) => t.path === tabPath);
        const newTabs = prev.filter((t) => t.path !== tabPath);
        if (tabPath === selectedFileRef.current && newTabs.length > 0) {
          const newIdx = Math.min(idx, newTabs.length - 1);
          handleSelectFile(newTabs[newIdx].path);
        } else if (newTabs.length === 0) {
          setSelectedFile(null);
          setFileDiff(null);
        }
        return newTabs;
      });
    },
    [handleSelectFile],
  );

  /** Close all tabs except one. */
  const closeOtherTabs = useCallback(
    (keepPath: string) => {
      setOpenTabs((prev) => prev.filter((t) => t.path === keepPath));
      handleSelectFile(keepPath);
    },
    [handleSelectFile],
  );

  /** Close every tab. */
  const closeAllTabs = useCallback(() => {
    setOpenTabs([]);
    setSelectedFile(null);
    setFileDiff(null);
  }, []);

  /** Called when a node in the React Flow graph is clicked — opens the file without collapsing the graph. */
  const handleGraphNodeClick = useCallback(
    (path: string) => {
      handleSelectFile(path);
    },
    [handleSelectFile],
  );

  const resolveGroupFilePath = useCallback((fileLike: string, group: FlowGroup): string | null => {
    const direct = group.files.find((f) => f.path === fileLike);
    if (direct) return direct.path;
    const suffix = `/${fileLike}`;
    const bySuffix = group.files.find((f) => f.path.endsWith(suffix));
    return bySuffix?.path ?? null;
  }, []);

  const handleEdgeEndpointClick = useCallback(
    (endpoint: string) => {
      const group = selectedGroupRef.current;
      if (!group) return;
      const { filePath, symbol } = parseSymbolEndpoint(endpoint);
      const targetPath = resolveGroupFilePath(filePath, group);
      if (!targetPath) return;
      if (symbol) {
        pendingSymbolScrollRef.current = { symbol };
      }
      openFileInTab(targetPath, group.id);
    },
    [openFileInTab, resolveGroupFilePath],
  );

  const handleGraphEdgeClick = useCallback(
    (sourceEndpoint: string, targetEndpoint: string) => {
      const group = selectedGroupRef.current;
      if (!group) return;
      const source = parseSymbolEndpoint(sourceEndpoint);
      const target = parseSymbolEndpoint(targetEndpoint);
      const canUseTarget = resolveGroupFilePath(target.filePath, group) != null;
      const canUseSource = resolveGroupFilePath(source.filePath, group) != null;
      if (canUseTarget) {
        handleEdgeEndpointClick(targetEndpoint);
        return;
      }
      if (canUseSource) {
        handleEdgeEndpointClick(sourceEndpoint);
      }
    },
    [handleEdgeEndpointClick, resolveGroupFilePath],
  );

  const handleSourceNavigate = useCallback(
    async (filePath: string, symbolName?: string) => {
      setRightPanelTab("source");
      await handleSelectFile(filePath);
      if (symbolName) {
        setSourceFocusRequest({
          filePath,
          symbol: symbolName,
          token: Date.now(),
        });
      }
    },
    [handleSelectFile],
  );

  /** Handle "Go To Definition" from the diff viewer — look up the word in analysis edges and navigate. */
  const handleGoToDefinition = useCallback(
    (word: string) => {
      if (!analysis) return;
      // Search all groups for an edge whose "to" target contains this symbol name
      for (const group of analysis.groups) {
        for (const edge of group.edges) {
          // Edges are "file.ts::SymbolName" — check if the symbol part matches
          const toSymbol = edge.to.split("::").pop();
          if (toSymbol === word) {
            const toFile = edge.to.split("::")[0];
            // Check this file is in the changed files
            const file = group.files.find((f) => f.path === toFile || f.path.endsWith(`/${toFile}`));
            if (file) {
              openFileInTab(file.path, group.id);
              return;
            }
          }
          // Also check "from" side
          const fromSymbol = edge.from.split("::").pop();
          if (fromSymbol === word) {
            const fromFile = edge.from.split("::")[0];
            const file = group.files.find((f) => f.path === fromFile || f.path.endsWith(`/${fromFile}`));
            if (file) {
              openFileInTab(file.path, group.id);
              return;
            }
          }
        }
        // Also check symbols_changed in files
        for (const file of group.files) {
          if (file.symbols_changed.includes(word) && file.path !== selectedFile) {
            openFileInTab(file.path, group.id);
            return;
          }
        }
      }
    },
    [analysis, openFileInTab, selectedFile],
  );

  const handleSelectGroup = useCallback(
    async (group: FlowGroup) => {
      // Cancel any pending debounced file nav from the previous group
      if (pendingFileNav.current) clearTimeout(pendingFileNav.current);
      setSelectedGroup(group);
      // Exit replay mode when switching groups
      setReplayActive(false);
      setReplayStep(0);
      setReplayVisited(new Set());
      setReplayHunkIndex(0);
      setReplayViewedHunkIds(new Set());
      // Reset annotation sub-tab when switching groups
      setAnnotationSubTab("info");
      // Auto-select first file in group
      if (group.files.length > 0) {
        openFileInTab(group.files[0].path, group.id);
      } else {
        setSelectedFile(null);
        setFileDiff(null);
      }
    },
    [handleSelectFile],
  );

  const runAnalysis = useCallback(async () => {
    if (!repoPath) return;
    if (comparisonMode === "invalid") {
      setError("Unsupported compare targets. Use source=Unstaged changes and target=Staged changes, or regular branch/commit targets.");
      return;
    }
    setLoading(true);
    setError(null);
    // Reset LLM state on new analysis
    setOverview(null);
    setDeepAnalyses({});
    closeActivityStream();
    setActivityJob(null);
    setActivityEntries([]);
    setActivityError(null);
    setRightPanelTab("annotations");
    setSourceFocusRequest(null);
    // Reset refinement state
    setOriginalGroups(null);
    setRefinedGroups(null);
    setRefinementResponse(null);
    setRefinementProvider(null);
    setRefinementModel(null);
    setRefinementHadChanges(null);
    setShowRefined(false);
    // Reset review tick-off state
    setReviewedGroupIds(new Set());
    // Reset infrastructure group state
    setInfraExpanded(false);
    setInfraShowAll(false);
    // Reset comments
    setComments([]);
    setCommentInput(null);
    setCommentText("");
    // Reset tabs
    setOpenTabs([]);
    setFileStatusByPath({});
    try {
      let result: AnalysisOutput;
      if (IS_TAURI) {
        result = await tauriInvoke<AnalysisOutput>("analyze", {
          repoPath,
          base: analysisDiffArgs.base,
          head: analysisDiffArgs.head,
          range: null,
          staged: analysisDiffArgs.staged,
          unstaged: analysisDiffArgs.unstaged,
          prPreview: analysisDiffArgs.prPreview,
          includeUncommitted: analysisDiffArgs.includeUncommitted,
        });
      } else {
        // Demo mode: simulate short delay then return mock data
        await new Promise((r) => setTimeout(r, 400));
        result = MOCK_ANALYSIS;
      }
      setAnalysis(result);
      // If analysis came with annotations already (e.g., from --annotate), load them
      if (result.annotations) {
        setOverview(result.annotations);
      }
      // Auto-select first group
      if (result.groups.length > 0) {
        const sorted = [...result.groups].sort(
          (a, b) => a.review_order - b.review_order,
        );
        handleSelectGroup(sorted[0]);
      }
      // Check for cached refinement and auto-apply if found
      if (IS_TAURI) {
        tauriInvoke<RefinementResult | null>("get_cached_refinement", { repoPath: repoPath || null }).then((cached) => {
          if (cached) {
            applyRefinementResult(cached, { fromCache: true });
          }
        }).catch(() => {});
        tauriInvoke<FileShortStatus[]>("get_last_diff_file_statuses")
          .then((statuses) => {
            const map: Record<string, string> = {};
            for (const item of statuses) {
              map[item.path] = item.status;
            }
            setFileStatusByPath(map);
          })
          .catch(() => {});
      }
    } catch (e) {
      setError(String(e));
      // Re-focus the repo input so user can fix the path
      repoInputRef.current?.focus();
      repoInputRef.current?.select();
    } finally {
      setLoading(false);
    }
  }, [repoPath, comparisonMode, analysisDiffArgs, handleSelectGroup, closeActivityStream]);

  const recommendedSubscriptionProvider: SubscriptionProvider | null = llmSettings?.codex_authenticated
    ? "codex"
    : llmSettings?.claude_authenticated
      ? "claude"
      : null;
  const resolvedPrimaryProvider = resolveInteractiveProvider(
    llmSettings?.provider ?? null,
    recommendedSubscriptionProvider,
  );
  const resolvedPrimaryModel = resolveInteractiveModel(
    llmSettings?.model ?? null,
    llmSettings?.provider ?? null,
    resolvedPrimaryProvider,
  );
  const resolvedRefinementProvider = resolveInteractiveProvider(
    llmSettings?.refinement_provider ?? llmSettings?.provider ?? null,
    recommendedSubscriptionProvider,
  );
  const resolvedRefinementModel = resolveInteractiveModel(
    llmSettings?.refinement_model ?? llmSettings?.model ?? null,
    llmSettings?.refinement_provider ?? llmSettings?.provider ?? null,
    resolvedRefinementProvider,
  );
  const aiAccessReady = hasApiKey || !!recommendedSubscriptionProvider;
  const annotationsEnabled = (llmSettings?.annotations_enabled ?? false) || !!recommendedSubscriptionProvider;

  /** Run LLM Pass 1: overview annotation for all groups. */
  const runAnnotateOverview = useCallback(async (opts?: { feedback?: string; includePreviousOutput?: boolean }) => {
    setAnnotating(true);
    setError(null);
    try {
      const feedback = opts?.feedback?.trim() ?? "";
      const includePreviousOutput = opts?.includePreviousOutput ?? false;
      const previousSections: string[] = [];
      if (includePreviousOutput && overview) {
        previousSections.push(`Previous overview summary:\n${overview.overall_summary}`);
      }
      if (includePreviousOutput && selectedGroup && deepAnalyses[selectedGroup.id]) {
        previousSections.push(
          `Previous group deep analysis (${selectedGroup.name}):\n${deepAnalyses[selectedGroup.id].flow_narrative}`,
        );
      }
      const previousOutput = includePreviousOutput ? previousSections.join("\n\n") : "";
      const userComments = comments.map((c) => {
        if (c.file_path && c.start_line != null && c.end_line != null) {
          return `${c.file_path}:${c.start_line}-${c.end_line} :: ${c.text}`;
        }
        if (c.file_path) {
          return `${c.file_path} :: ${c.text}`;
        }
        return `group:${c.group_id} :: ${c.text}`;
      });

      if (IS_TAURI) {
        await runStreamingJob<Pass1Response>("start_annotate_overview", {
          repoPath: repoPath || null,
          llmProvider: resolvedPrimaryProvider,
          llmModel: resolvedPrimaryModel,
          userFeedback: feedback || null,
          includePreviousOutput,
          previousOutput: previousOutput || null,
          userComments,
        }, (result) => {
          setOverview(result);
        });
      } else {
        await runMockActivityJob<Pass1Response>(
          {
            job_id: "mock-overview",
            operation: "overview",
            provider: resolvedPrimaryProvider ?? "codex",
            model: resolvedPrimaryModel ?? "default",
            title: "Summarizing PR",
          },
          buildMockActivityEntries("overview", resolvedPrimaryProvider ?? "codex"),
          MOCK_PASS1,
          (result) => setOverview(result),
        );
      }
    } catch (e) {
      setError(`Annotation failed: ${String(e)}`);
    } finally {
      setAnnotating(false);
    }
  }, [comments, deepAnalyses, overview, repoPath, resolvedPrimaryModel, resolvedPrimaryProvider, runMockActivityJob, runStreamingJob, selectedGroup]);

  /** Run LLM Pass 2: deep analysis for the selected group. */
  const runDeepAnalysis = useCallback(async () => {
    if (!selectedGroup) return;
    deepAnalyzingCount.current += 1;
    setDeepAnalyzing(true);
    setError(null);
    try {
      if (IS_TAURI) {
        await runStreamingJob<Pass2Response>("start_annotate_group", {
          groupId: selectedGroup.id,
          repoPath,
          base: analysisDiffArgs.base,
          head: analysisDiffArgs.head,
          range: null,
          staged: analysisDiffArgs.staged,
          unstaged: analysisDiffArgs.unstaged,
          llmProvider: resolvedPrimaryProvider,
          llmModel: resolvedPrimaryModel,
        }, (result) => {
          setDeepAnalyses((prev) => ({ ...prev, [selectedGroup.id]: result }));
        });
      } else {
        await runMockActivityJob<Pass2Response>(
          {
            job_id: `mock-group-${selectedGroup.id}`,
            operation: "group",
            provider: resolvedPrimaryProvider ?? "codex",
            model: resolvedPrimaryModel ?? "default",
            title: `Analyzing ${selectedGroup.id}`,
          },
          buildMockActivityEntries("group", resolvedPrimaryProvider ?? "codex"),
          MOCK_PASS2[selectedGroup.id] || {
            group_id: selectedGroup.id,
            flow_narrative: "No deep analysis available for this group in demo mode.",
            file_annotations: [],
            cross_cutting_concerns: [],
          },
          (result) => setDeepAnalyses((prev) => ({ ...prev, [selectedGroup.id]: result })),
        );
      }
    } catch (e) {
      setError(`Deep analysis failed: ${String(e)}`);
    } finally {
      deepAnalyzingCount.current -= 1;
      // Only clear loading state when all concurrent deep analyses have completed
      if (deepAnalyzingCount.current <= 0) {
        deepAnalyzingCount.current = 0;
        setDeepAnalyzing(false);
      }
    }
  }, [selectedGroup, repoPath, analysisDiffArgs, resolvedPrimaryModel, resolvedPrimaryProvider, runMockActivityJob, runStreamingJob]);

  /** Show a toast notification that auto-dismisses. */
  const showToast = useCallback((message: string) => {
    if (toastTimer.current) clearTimeout(toastTimer.current);
    setToast(message);
    toastTimer.current = setTimeout(() => setToast(null), 3500);
  }, []);

  // Persist a full UI snapshot (including activity logs) to the app-state log folder.
  useEffect(() => {
    if (!STATE_SAVE_RESTORE_ENABLED) return;
    if (!IS_TAURI) return;
    if (pendingAppStatePersist.current) {
      clearTimeout(pendingAppStatePersist.current);
    }
    pendingAppStatePersist.current = setTimeout(() => {
      const snapshot = buildPersistedState();
      tauriInvoke<string>("save_app_state", { snapshot }).catch(() => {});
    }, 2000);
    return () => {
      if (pendingAppStatePersist.current) {
        clearTimeout(pendingAppStatePersist.current);
      }
    };
  }, [buildPersistedState]);

  // Auto-restore the latest snapshot once when running in Tauri.
  useEffect(() => {
    if (!STATE_SAVE_RESTORE_ENABLED) return;
    if (!IS_TAURI) return;
    restoreLastSessionState();
  }, [restoreLastSessionState]);

  const applyRefinementResult = useCallback((result: RefinementResult, opts?: { fromCache?: boolean }) => {
    if (!analysis) return;

    if (!originalGroups) {
      setOriginalGroups(analysis.groups);
    }

    setRefinedGroups(result.refined_groups);
    setRefinementResponse(result.refinement_response);
    setRefinementProvider(result.provider);
    setRefinementModel(result.model);
    setRefinementHadChanges(result.had_changes);

    if (result.had_changes) {
      setShowRefined(true);
      setAnalysis((prev) =>
        prev
          ? {
              ...prev,
              groups: result.refined_groups,
              infrastructure_group: result.infrastructure_group ?? prev.infrastructure_group,
            }
          : prev,
      );
      setReviewedGroupIds(new Set());
      if (opts?.fromCache) {
        showToast("Restored cached refinement");
      } else {
        showToast("Groups updated by refinement \u2014 review state reset");
      }
      const sorted = [...result.refined_groups].sort(
        (a, b) => a.review_order - b.review_order,
      );
      if (sorted.length > 0) {
        handleSelectGroup(sorted[0]);
      }
    } else {
      setShowRefined(false);
      if (!opts?.fromCache) {
        showToast("Refinement kept the existing grouping");
      }
    }

    // Cache the result for future sessions (non-blocking, fire-and-forget)
    if (IS_TAURI && !opts?.fromCache) {
      tauriInvoke("store_refinement_cache", { result, repoPath: repoPath || null }).catch(() => {});
    }
  }, [analysis, originalGroups, handleSelectGroup, showToast]);

  /** Run LLM refinement pass on the current analysis groups. */
  const runRefinement = useCallback(async () => {
    if (!analysis) return;
    setRefining(true);
    setError(null);
    try {
      if (IS_TAURI) {
        await runStreamingJob<RefinementResult>("start_refine_groups", {
          repoPath: repoPath || null,
          llmProvider: resolvedRefinementProvider,
          llmModel: resolvedRefinementModel,
        }, (result) => {
          applyRefinementResult(result);
        });
      } else {
        await runMockActivityJob<RefinementResult>(
          {
            job_id: "mock-refinement",
            operation: "refinement",
            provider: resolvedRefinementProvider ?? "claude",
            model: resolvedRefinementModel ?? "default",
            title: "Refining groups",
          },
          buildMockActivityEntries("refinement", resolvedRefinementProvider ?? "claude"),
          MOCK_REFINEMENT,
          (result) => applyRefinementResult(result),
        );
      }
    } catch (e) {
      setError(`Refinement failed: ${String(e)}`);
    } finally {
      setRefining(false);
    }
  }, [analysis, repoPath, resolvedRefinementModel, resolvedRefinementProvider, runMockActivityJob, runStreamingJob, applyRefinementResult]);

  /** Toggle between original and refined groups. */
  const toggleRefinedView = useCallback(
    (useRefined: boolean) => {
      if (!analysis) return;
      const groups = useRefined ? refinedGroups : originalGroups;
      if (!groups || useRefined === showRefined) return;

      groupListTransitionTimers.current.forEach((timer) => window.clearTimeout(timer));
      groupListTransitionTimers.current = [];
      setGroupListTransitionState("fading-out");

      const swapTimer = window.setTimeout(() => {
        setShowRefined(useRefined);
        setAnalysis((prev) =>
          prev ? { ...prev, groups } : prev,
        );
        const sorted = [...groups].sort(
          (a, b) => a.review_order - b.review_order,
        );
        if (sorted.length > 0) {
          handleSelectGroup(sorted[0]);
        }
        setGroupListTransitionState("fading-in");

        const settleTimer = window.setTimeout(() => {
          setGroupListTransitionState("idle");
        }, 220);
        groupListTransitionTimers.current.push(settleTimer);
      }, 150);

      groupListTransitionTimers.current.push(swapTimer);
    },
    [analysis, refinedGroups, originalGroups, handleSelectGroup, showRefined],
  );

  useEffect(() => () => {
    groupListTransitionTimers.current.forEach((timer) => window.clearTimeout(timer));
  }, []);

  // Auto-load demo data when not in Tauri
  useEffect(() => {
    if (!IS_TAURI && !demoLoaded.current) {
      demoLoaded.current = true;
      runAnalysis();
    }
  }, [runAnalysis]);

  // Test API for Playwright — only available in demo/browser mode
  useEffect(() => {
    if (IS_TAURI) return;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (window as any).__TEST_API__ = {
      setRepoInfo: (data: RepoInfo | null) => setRepoInfo(data),
      setLlmSettings: (data: LlmSettings) => {
        demoLlmSettingsRef.current = data;
        setLlmSettings(data);
        setHasApiKey(data.has_api_key);
        if (isApiProvider(data.provider)) {
          setApiProviderDraft(data.provider as LlmProvider);
        }
      },
      setAnalysis: (data: AnalysisOutput | null) => { setAnalysis(data); if (data && data.groups.length > 0) { const sorted = [...data.groups].sort((a, b) => a.review_order - b.review_order); handleSelectGroup(sorted[0]); } },
      setActivityEntries: (entries: LlmActivityEntry[]) => {
        setActivityJob(null);
        setActivityError(null);
        setActivityViewMode("stream");
        setInspectedActivityId(null);
        setActivityEntries(entries);
      },
      setError: (msg: string | null) => setError(msg),
      clearAnalysis: () => { setAnalysis(null); setSelectedGroup(null); setSelectedFile(null); setFileDiff(null); setOverview(null); setDeepAnalyses({}); setOriginalGroups(null); setRefinedGroups(null); setRefinementResponse(null); setRefinementProvider(null); setRefinementModel(null); setRefinementHadChanges(null); setShowRefined(false); setReviewedGroupIds(new Set()); setComments([]); setCommentInput(null); setCommentText(""); setRightPanelTab("annotations"); setSourceFocusRequest(null); setActivityJob(null); setActivityEntries([]); setActivityError(null); setActivityViewMode("stream"); setInspectedActivityId(null); },
      openAiSetup: (step: OnboardingStep = "recommended") => openAiSetup(step),
      dismissAiSetup: () => dismissAiSetup(),
      getAiSetupState: () => ({ open: aiSetupOpen, step: aiSetupStep }),
      enterReplay: () => enterReplay(),
      exitReplay: () => exitReplay(),
      getReplayState: () => ({ active: replayActive, step: replayStep, visited: Array.from(replayVisited) }),
      toggleGroupReviewed: (id: string) => toggleGroupReviewed(id),
      getReviewedGroupIds: () => Array.from(reviewedGroupIds),
      getActivityEntries: () => activityEntries,
      getActivityJob: () => activityJob,
      crashPanel: (name: string | null) => setCrashPanel(name),
      copyFilePath: (path: string) => copyFilePath(path),
      getToast: () => toast,
      getComments: () => comments,
      openCommentInput: (input?: CommentInput) => openCommentInput(input),
      submitComment: () => submitComment(),
      cancelComment: () => cancelComment(),
      setCommentText: (text: string) => setCommentText(text),
      deleteComment: (id: string) => deleteComment(id),
      exportComments: () => exportComments(),
      getCommentInput: () => commentInput,
      getSelectedFile: () => selectedFile,
      getSelectedGroup: () => selectedGroup,
      getHeadRef: () => headRef,
      setHeadRef: (ref: string) => setHeadRef(ref),
      getBaseRef: () => baseRef,
    };
    return () => { delete (window as any).__TEST_API__; };
  });

  // Close branch dropdowns when clicking outside
  useEffect(() => {
    if (!branchDropdownOpen && !headBranchDropdownOpen) return;
    function handleClick(e: MouseEvent) {
      const target = e.target as HTMLElement;
      if (!target.closest(".branch-dropdown-wrapper")) {
        setBranchDropdownOpen(false);
        setHeadBranchDropdownOpen(false);
      }
    }
    window.addEventListener("click", handleClick);
    return () => window.removeEventListener("click", handleClick);
  }, [branchDropdownOpen, headBranchDropdownOpen]);

  // Close settings panel on Escape
  useEffect(() => {
    if (!settingsOpen) return;
    function handleKey(e: KeyboardEvent) {
      if (e.key === "Escape") setSettingsOpen(false);
    }
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, [settingsOpen]);

  // Close context menu on click or Escape
  useEffect(() => {
    if (!contextMenu) return;
    function handleClick() {
      setContextMenu(null);
    }
    function handleKey(e: KeyboardEvent) {
      if (e.key === "Escape") setContextMenu(null);
    }
    window.addEventListener("click", handleClick);
    window.addEventListener("keydown", handleKey);
    return () => {
      window.removeEventListener("click", handleClick);
      window.removeEventListener("keydown", handleKey);
    };
  }, [contextMenu]);

  // Close tab context menu on click or Escape
  useEffect(() => {
    if (!tabContextMenu) return;
    function handleClick() {
      setTabContextMenu(null);
    }
    function handleKey(e: KeyboardEvent) {
      if (e.key === "Escape") setTabContextMenu(null);
    }
    window.addEventListener("click", handleClick);
    window.addEventListener("keydown", handleKey);
    return () => {
      window.removeEventListener("click", handleClick);
      window.removeEventListener("keydown", handleKey);
    };
  }, [tabContextMenu]);

  /** Get models for a provider — dynamic if available, static fallback otherwise. */
  const modelsForProvider = useCallback((provider: string): string[] => {
    return providerModels[provider] ?? DEFAULT_MODELS_BY_PROVIDER[provider as LlmProvider] ?? [];
  }, [providerModels]);

  /** Update a single LLM setting field and persist. */
  const updateSetting = useCallback(
    (field: keyof LlmSettings, value: string | boolean | number) => {
      if (!llmSettings) return;
      const updated = { ...llmSettings, [field]: value };
      // When provider changes, reset model to default for that provider and sync refinement
      if (field === "provider") {
        const provider = value as LlmProvider;
        const models = modelsForProvider(provider);
        updated.model = models[0] ?? "";
        updated.refinement_provider = provider;
        updated.refinement_model = models[0] ?? "";
        // Trigger dynamic model fetch for this provider
        fetchModelsForProvider(provider);
        if (isApiProvider(provider)) {
          setApiProviderDraft(provider);
        }
      }
      if (field === "refinement_provider") {
        const provider = value as LlmProvider;
        const models = modelsForProvider(provider);
        updated.refinement_model = models[0] ?? "";
        fetchModelsForProvider(provider);
      }
      saveLlmSettings(updated);
    },
    [llmSettings, saveLlmSettings, modelsForProvider, fetchModelsForProvider],
  );

  const selectedApiModel = useMemo(
    () => modelsForProvider(apiProviderDraft)[0] ?? "default",
    [apiProviderDraft, modelsForProvider],
  );

  /** Save an API key to the shared diffcore config and refresh settings. */
  const handleSaveApiKey = useCallback(async () => {
    const key = apiKeyInput.trim();
    if (!key || !llmSettings) return;
    const updated: LlmSettings = {
      ...llmSettings,
      annotations_enabled: true,
      provider: apiProviderDraft,
      model: selectedApiModel,
      api_key_source: "~/.diffcore/config.toml",
      has_api_key: true,
      refinement_provider: apiProviderDraft,
      refinement_model: selectedApiModel,
    };
    try {
      await saveLlmSettings(updated);
      if (IS_TAURI) {
        await tauriInvoke("save_api_key", { repoPath: repoPath || "", apiKey: key });
      } else {
        demoLlmSettingsRef.current = updated;
      }
      setApiKeyInput("");
      setAiSetupOpen(false);
      // Refresh settings to pick up the new key
      await loadLlmSettings(repoPath || null);
    } catch {
      setError("Failed to save API key");
    }
  }, [apiKeyInput, apiProviderDraft, llmSettings, repoPath, loadLlmSettings, saveLlmSettings, selectedApiModel]);

  /** Clear the stored API key from the shared diffcore config and refresh settings. */
  const handleClearApiKey = useCallback(async () => {
    try {
      if (IS_TAURI) {
        await tauriInvoke("clear_api_key", { repoPath: repoPath || "" });
      } else if (llmSettings) {
        const updated: LlmSettings = {
          ...llmSettings,
          api_key_source: "none",
          has_api_key: false,
          annotations_enabled: false,
        };
        demoLlmSettingsRef.current = updated;
        setLlmSettings(updated);
        setHasApiKey(false);
      }
      setApiKeyInput("");
      // Refresh settings to reflect removal
      await loadLlmSettings(repoPath || null);
    } catch {
      setError("Failed to clear API key");
    }
  }, [llmSettings, repoPath, loadLlmSettings]);

  /** Add an ignore path pattern and persist to .diffcore.toml. */
  const handleAddIgnorePath = useCallback(async () => {
    const pattern = ignorePathInput.trim();
    if (!pattern || !repoPath) return;
    if (ignorePaths.includes(pattern)) {
      setIgnorePathInput("");
      return;
    }
    const updated = [...ignorePaths, pattern];
    setIgnorePaths(updated);
    setIgnorePathInput("");
    if (IS_TAURI) {
      try {
        await tauriInvoke("save_ignore_paths", { repoPath, paths: updated });
      } catch {
        setError("Failed to save ignore paths");
      }
    }
  }, [ignorePathInput, repoPath, ignorePaths]);

  /** Remove an ignore path pattern and persist to .diffcore.toml. */
  const handleRemoveIgnorePath = useCallback(async (pattern: string) => {
    if (!repoPath) return;
    const updated = ignorePaths.filter((p) => p !== pattern);
    setIgnorePaths(updated);
    if (IS_TAURI) {
      try {
        await tauriInvoke("save_ignore_paths", { repoPath, paths: updated });
      } catch {
        setError("Failed to save ignore paths");
      }
    }
  }, [repoPath, ignorePaths]);

  // Sort groups by review_order (memoized to avoid re-sorting on every render)
  const sortedGroups = useMemo(
    () => analysis
      ? [...analysis.groups].sort((a, b) => a.review_order - b.review_order)
      : [],
    [analysis],
  );
  sortedGroupsRef.current = sortedGroups;

  // Get the Pass 1 annotation for the currently selected group
  const groupAnnotation: Pass1GroupAnnotation | undefined = overview?.groups.find(
    (g) => g.id === selectedGroup?.id,
  );

  // Get the Pass 2 deep analysis for the currently selected group
  const groupDeepAnalysis: Pass2Response | undefined = selectedGroup
    ? deepAnalyses[selectedGroup.id]
    : undefined;
  const selectedFileChange = useMemo(
    () => selectedGroup?.files.find((file) => file.path === selectedFile) ?? null,
    [selectedGroup, selectedFile],
  );

  const activityTimeline = useMemo(
    () => activityEntries.map((entry, index) => ({
      id: `${entry.timestamp_ms}-${index}`,
      entry,
      presentation: describeActivityEntry(entry),
    })),
    [activityEntries],
  );
  const recentActivityTimeline = useMemo(
    () => activityTimeline.slice(-ACTIVITY_STREAM_LIMIT),
    [activityTimeline],
  );
  const visibleActivityTimeline = activityViewMode === "stream"
    ? recentActivityTimeline
    : activityTimeline;
  const activityStats = useMemo(() => summarizeActivityTimeline(activityTimeline), [activityTimeline]);
  const activityEventProvider = useMemo(() => {
    if (activityJob?.provider) return activityJob.provider;
    for (let index = activityTimeline.length - 1; index >= 0; index -= 1) {
      const source = activityTimeline[index]?.entry.source;
      if (source && LLM_PROVIDERS.includes(source as LlmProvider)) {
        return source as LlmProvider;
      }
    }
    return resolvedRefinementProvider ?? resolvedPrimaryProvider ?? null;
  }, [activityJob?.provider, activityTimeline, resolvedPrimaryProvider, resolvedRefinementProvider]);
  const activitySupportsToolStreaming = providerSupportsToolActivity(activityEventProvider);
  // Direct-API mode: a hosted-API provider (OpenAI / Anthropic / Gemini) is the
  // active activity source. These providers only emit high-level progress, so
  // the search/reads/commands tiles are always 0 and the banner explaining
  // that is worth surfacing once.
  const activityIsDirectApi = activityEventProvider != null && isApiProvider(activityEventProvider);
  const showDirectApiBanner = activityIsDirectApi && !directApiNoticeDismissed;
  const refinementVerdict = useMemo(() => {
    if (!refinementResponse || !refinementProvider) return null;
    return {
      provider: refinementProvider,
      model: refinementModel ?? "default",
      hadChanges: refinementHadChanges === true,
      title: refinementHadChanges
        ? "Applied structural changes"
        : "Kept the current grouping",
      reasoning: refinementResponse.reasoning?.trim() || null,
    };
  }, [refinementHadChanges, refinementModel, refinementProvider, refinementResponse]);

  const openAiSetup = useCallback((step: OnboardingStep = "recommended") => {
    aiSetupDismissed.current = false;
    setAiSetupStep(step);
    setAiSetupOpen(true);
    setSettingsOpen(false);
  }, []);

  const dismissAiSetup = useCallback(() => {
    aiSetupDismissed.current = true;
    setAiSetupOpen(false);
  }, []);

  const refreshAiAccess = useCallback(async () => {
    await loadLlmSettings(repoPath || null);
  }, [loadLlmSettings, repoPath]);

  // Check for updates on startup
  useEffect(() => {
    if (!IS_TAURI) return;
    (async () => {
      try {
        const { check } = await import("@tauri-apps/plugin-updater");
        const update = await check();
        if (update) {
          setUpdateAvailable({ version: update.version, body: update.body ?? "" });
        }
      } catch {
        // Update check failed silently — non-fatal
      }
    })();
  }, []);

  // Auto-scroll disabled — users control scroll position manually.

  useEffect(() => {
    if (visibleActivityTimeline.length === 0) {
      setInspectedActivityId(null);
      return;
    }

    if (activityViewMode === "stream") {
      setInspectedActivityId(visibleActivityTimeline[visibleActivityTimeline.length - 1]?.id ?? null);
      return;
    }

    setInspectedActivityId((current) => {
      if (current && visibleActivityTimeline.some((item) => item.id === current)) {
        return current;
      }
      return visibleActivityTimeline[visibleActivityTimeline.length - 1]?.id ?? null;
    });
  }, [activityViewMode, visibleActivityTimeline]);

  const activateSubscriptionProvider = useCallback(async (provider: SubscriptionProvider) => {
    if (!llmSettings) return;
    const model = modelsForProvider(provider)[0] ?? "default";
    const updated: LlmSettings = {
      ...llmSettings,
      annotations_enabled: true,
      provider,
      model,
      api_key_source: provider === "codex" ? "Codex CLI login" : "Claude Code subscription",
      has_api_key: true,
      refinement_provider: provider,
      refinement_model: model,
    };
    await saveLlmSettings(updated);
    setAiSetupOpen(false);
  }, [llmSettings, saveLlmSettings]);

  const activatePreferredActivityProvider = useCallback(async () => {
    if (!recommendedSubscriptionProvider || !llmSettings) return;
    const model = modelsForProvider(recommendedSubscriptionProvider)[0] ?? "default";
    const updated: LlmSettings = {
      ...llmSettings,
      annotations_enabled: true,
      provider: recommendedSubscriptionProvider,
      model,
      api_key_source: recommendedSubscriptionProvider === "codex" ? "Codex CLI login" : "Claude Code subscription",
      has_api_key: true,
      refinement_provider: recommendedSubscriptionProvider,
      refinement_model: model,
    };
    await saveLlmSettings(updated);
    showToast(`Using ${PROVIDER_LABELS[recommendedSubscriptionProvider]} for live AI jobs`);
  }, [llmSettings, recommendedSubscriptionProvider, saveLlmSettings, showToast]);

  const openApiKeyFallback = useCallback(() => {
    if (llmSettings && isApiProvider(llmSettings.provider)) {
      setApiProviderDraft(llmSettings.provider as LlmProvider);
    } else {
      setApiProviderDraft("openai");
    }
    openAiSetup("api");
  }, [llmSettings, openAiSetup]);

  /** Toggle reviewed state for a flow group. */
  const toggleGroupReviewed = useCallback((groupId: string) => {
    setReviewedGroupIds((prev) => {
      const next = new Set(prev);
      if (next.has(groupId)) {
        next.delete(groupId);
      } else {
        next.add(groupId);
      }
      return next;
    });
  }, []);

  /** Build the absolute file path from repo path + relative path. */
  const buildAbsolutePath = useCallback(
    (relativePath: string): string => {
      if (!repoPath) return relativePath;
      const base = repoPath.endsWith("/") ? repoPath.slice(0, -1) : repoPath;
      return `${base}/${relativePath}`;
    },
    [repoPath],
  );

  /** Copy a file's absolute path to clipboard. */
  const copyFilePath = useCallback(
    async (relativePath: string) => {
      const absPath = buildAbsolutePath(relativePath);
      try {
        await navigator.clipboard.writeText(absPath);
        showToast("Path copied to clipboard");
      } catch {
        showToast("Failed to copy path");
      }
    },
    [buildAbsolutePath, showToast],
  );

  /** Copy all file paths in a flow group to clipboard (absolute, one per line, flow order). */
  const copyFlowPaths = useCallback(
    async (group: FlowGroup) => {
      const paths = group.files
        .map((f) => buildAbsolutePath(f.path))
        .join("\n");
      try {
        await navigator.clipboard.writeText(paths);
        showToast(`${group.files.length} file paths copied to clipboard`);
      } catch {
        showToast("Failed to copy paths");
      }
    },
    [buildAbsolutePath, showToast],
  );

  /** Build readable auto-comment text for an edited hunk. */
  const autoEditCommentText = useCallback((hunk: EditedHunk): string => {
    const isDeletionOnly = hunk.modifiedStartLine === 0 || hunk.modifiedEndLine === 0;
    if (isDeletionOnly) {
      return `Edited hunk: deleted original lines ${hunk.originalStartLine}-${hunk.originalEndLine}.`;
    }
    return `Edited hunk: original ${hunk.originalStartLine}-${hunk.originalEndLine} -> modified ${hunk.modifiedStartLine}-${hunk.modifiedEndLine}.`;
  }, []);

  /** Persist edited file content to disk and replace auto-generated hunk comments. */
  const syncEditedFileAndComments = useCallback(async (
    filePath: string,
    groupId: string,
    newContent: string,
    hunks: EditedHunk[],
  ) => {
    if (IS_TAURI) {
      const absoluteFilePath = buildAbsolutePath(filePath);
      try {
        await tauriInvoke("save_file_content", {
          filePath: absoluteFilePath,
          content: newContent,
        });
      } catch (e) {
        showToast(`Failed to save edits to disk: ${String(e)}`);
        return;
      }
    }

    const existingAutoComments = comments.filter(
      (c) => c.type === "code" && c.file_path === filePath && c.id.startsWith("auto_edit_hunk_"),
    );

    const nextAutoComments: ReviewComment[] = hunks.map((hunk, index) => {
      const hasModifiedRange = hunk.modifiedStartLine > 0 && hunk.modifiedEndLine > 0;
      const startLine = hasModifiedRange
        ? hunk.modifiedStartLine
        : Math.max(1, hunk.originalStartLine);
      const endLine = hasModifiedRange
        ? Math.max(hunk.modifiedStartLine, hunk.modifiedEndLine)
        : Math.max(startLine, hunk.originalEndLine);

      return {
        id: `auto_edit_hunk_${filePath}_${index}`,
        type: "code",
        group_id: groupId,
        file_path: filePath,
        start_line: startLine,
        end_line: endLine,
        selected_code: hunk.selectedCode || null,
        text: autoEditCommentText(hunk),
        created_at: new Date().toISOString(),
      };
    });

    setComments((prev) => {
      const retained = prev.filter(
        (c) => !(c.type === "code" && c.file_path === filePath && c.id.startsWith("auto_edit_hunk_")),
      );
      return [...retained, ...nextAutoComments];
    });

    if (IS_TAURI && repoPath) {
      try {
        for (const existing of existingAutoComments) {
          await tauriInvoke("delete_comment_cached", { repoPath, commentId: existing.id });
        }
        for (const comment of nextAutoComments) {
          await tauriInvoke("save_comment_cached", { repoPath, comment });
        }
      } catch {
        // Non-fatal: local UI state still reflects current edits.
      }
    }
  }, [autoEditCommentText, buildAbsolutePath, comments, repoPath, showToast]);

  /** Export the overview as a PR-description-style brief. */
  const copyPrDescription = useCallback(async () => {
    if (!overview) {
      showToast("Generate a summary first");
      return;
    }

    const orderedGroups = overview.suggested_review_order
      .map((id) => overview.groups.find((group) => group.id === id))
      .filter((group): group is Pass1GroupAnnotation => Boolean(group));
    const fallbackGroups = overview.groups.filter(
      (group) => !orderedGroups.some((ordered) => ordered.id === group.id),
    );

    const lines = [
      "# Summary",
      "",
      overview.overall_summary,
      "",
      "# Review Flow",
      "",
      ...[...orderedGroups, ...fallbackGroups].flatMap((group) => [
        `- ${group.name}: ${group.summary}`,
      ]),
    ];

    try {
      await navigator.clipboard.writeText(lines.join("\n"));
      showToast("PR description copied to clipboard");
    } catch {
      showToast("Failed to copy PR description");
    }
  }, [overview, showToast]);

  /** Compute a simple hash of the analysis for comment scoping. */
  const analysisHash = analysis
    ? `${analysis.diff_source.base_sha ?? ""}:${analysis.diff_source.head_sha ?? ""}:${analysis.summary.total_files_changed}`
    : "";

  /** Load comments from backend — uses branch-based cache. */
  const loadComments = useCallback(async () => {
    if (!repoPath) return;
    try {
      if (IS_TAURI) {
        // Primary: load from branch-based cache
        const result = await tauriInvoke<ReviewComment[]>("load_comments_cached", {
          repoPath,
        });
        if (result.length > 0) {
          setComments(result);
          return;
        }
        // Fallback: try legacy analysis-hash-based comments and migrate them
        if (analysisHash) {
          const legacy = await tauriInvoke<ReviewComment[]>("load_comments", {
            repoPath,
            analysisHash,
          });
          if (legacy.length > 0) {
            setComments(legacy);
            // Migrate legacy comments to branch-based cache
            for (const c of legacy) {
              await tauriInvoke("save_comment_cached", { repoPath, comment: c }).catch(() => {});
            }
            return;
          }
        }
      }
    } catch {
      // Non-fatal: comments will just be empty
    }
  }, [repoPath, analysisHash]);

  // Load comments when analysis changes
  useEffect(() => {
    if (analysis) {
      loadComments();
    }
  }, [analysis, loadComments]);

  /** Save a comment (persist to branch-based cache). */
  const saveComment = useCallback(
    async (comment: ReviewComment) => {
      setComments((prev) => [...prev, comment]);
      if (IS_TAURI && repoPath) {
        try {
          await tauriInvoke("save_comment_cached", {
            repoPath,
            comment,
          });
        } catch {
          // Already saved in state, persistence failure is non-fatal
        }
      }
    },
    [repoPath],
  );

  /** Delete a comment by ID. */
  const deleteComment = useCallback(
    async (commentId: string) => {
      setComments((prev) => prev.filter((c) => c.id !== commentId));
      if (IS_TAURI && repoPath) {
        try {
          await tauriInvoke("delete_comment_cached", {
            repoPath,
            commentId,
          });
        } catch {
          // Non-fatal
        }
      }
    },
    [repoPath],
  );

  /** Update a comment's text by ID. */
  const updateComment = useCallback(
    async (commentId: string, newText: string) => {
      setComments((prev) =>
        prev.map((c) => (c.id === commentId ? { ...c, text: newText } : c)),
      );
      setEditingCommentId(null);
      if (IS_TAURI && repoPath) {
        try {
          await tauriInvoke("update_comment_cached", {
            repoPath,
            commentId,
            newText,
          });
        } catch {
          // Non-fatal — optimistic update already applied
        }
      }
    },
    [repoPath],
  );

  /** Open the comment input — context-sensitive based on current selection. */
  const openCommentInput = useCallback(
    (overrideInput?: CommentInput) => {
      const group = selectedGroupRef.current;
      const file = selectedFileRef.current;
      if (!group) return;

      const input: CommentInput = overrideInput ?? (file
        ? { type: "file", group_id: group.id, file_path: file }
        : { type: "group", group_id: group.id });

      setCommentInput(input);
      setCommentText("");
      // Focus the textarea after render
      setTimeout(() => commentInputRef.current?.focus(), 50);
    },
    [],
  );

  /** Submit the current comment. */
  const submitComment = useCallback(() => {
    if (!commentInput || !commentText.trim()) return;

    const comment: ReviewComment = {
      id: `comment_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`,
      type: commentInput.type,
      group_id: commentInput.group_id,
      file_path: commentInput.file_path ?? null,
      start_line: commentInput.start_line ?? null,
      end_line: commentInput.end_line ?? null,
      selected_code: commentInput.selected_code ?? null,
      text: commentText.trim(),
      created_at: new Date().toISOString(),
    };

    saveComment(comment);
    setCommentInput(null);
    setCommentText("");
    showToast("Comment saved");
  }, [commentInput, commentText, saveComment, showToast]);

  /** Cancel the current comment input. */
  const cancelComment = useCallback(() => {
    setCommentInput(null);
    setCommentText("");
  }, []);

  /** Export all comments as formatted text, copy to clipboard. */
  const exportComments = useCallback(async () => {
    if (comments.length === 0) {
      showToast("No comments to copy");
      return;
    }

    // Build formatted output locally (works in both Tauri and demo mode)
    const base = repoPath.endsWith("/") ? repoPath.slice(0, -1) : repoPath;
    let output = "";

    // Group comments by group_id, then sort by type for clean output
    for (const comment of comments) {
      switch (comment.type) {
        case "code": {
          if (comment.file_path) {
            const absPath = `${base}/${comment.file_path}`;
            if (comment.start_line != null && comment.end_line != null) {
              output += `${absPath}:${comment.start_line}-${comment.end_line}\n`;
            } else {
              output += `${absPath}\n`;
            }
            if (comment.selected_code) {
              output += "```\n";
              output += comment.selected_code;
              if (!comment.selected_code.endsWith("\n")) output += "\n";
              output += "```\n";
            }
            output += `> ${comment.text}\n\n`;
          }
          break;
        }
        case "file": {
          if (comment.file_path) {
            const absPath = `${base}/${comment.file_path}`;
            output += `${absPath}\n`;
            output += `> ${comment.text}\n\n`;
          }
          break;
        }
        case "group": {
          // Find the group name for better export
          const group = analysis?.groups.find((g) => g.id === comment.group_id);
          const label = group ? group.name : comment.group_id;
          output += `Flow: "${label}"\n`;
          output += `> ${comment.text}\n\n`;
          break;
        }
      }
    }

    try {
      await navigator.clipboard.writeText(output);
      showToast(`${comments.length} comment${comments.length === 1 ? "" : "s"} copied to clipboard`);
    } catch {
      showToast("Failed to copy comments");
    }
  }, [comments, repoPath, analysis, showToast]);

  /** Import a groups manifest JSON and apply it to the current analysis. */
  const importGroupsManifest = useCallback(async (manifestPath: string) => {
    if (!IS_TAURI) return;
    try {
      const updated = await tauriInvoke<AnalysisOutput>("import_groups_manifest", { manifestPath });
      setAnalysis(updated);
      // Reset state for new groupings
      setRefinedGroups(null);
      setOriginalGroups(null);
      setShowRefined(false);
      setReviewedGroupIds(new Set());
      if (updated.groups.length > 0) {
        const sorted = [...updated.groups].sort((a, b) => a.review_order - b.review_order);
        handleSelectGroup(sorted[0]);
      }
      showToast(`Loaded ${updated.groups.length} groups from manifest`);
    } catch (e) {
      showToast(`Failed to import manifest: ${String(e)}`);
    }
  }, [handleSelectGroup, showToast]);

  /** Export current groups as an editable manifest JSON. */
  const exportGroupsManifest = useCallback(async () => {
    if (!IS_TAURI || !analysis) return;
    try {
      // Default path: .diffcore/groups.json in repo
      const outputPath = repoPath
        ? `${repoPath}/.diffcore/groups.json`
        : "groups.json";
      await tauriInvoke("export_groups_manifest", { outputPath });
      showToast(`Groups manifest exported to ${outputPath}`);
      return outputPath;
    } catch (e) {
      showToast(`Failed to export manifest: ${String(e)}`);
      return null;
    }
  }, [analysis, repoPath, showToast]);

  /** Build an agent prompt for iterative manifest refinement. */
  const buildManifestAgentPrompt = useCallback((manifestPath: string): string => {
    return buildManifestPrompt({
      manifestPath,
      repoPath: repoPath || ".",
      groupCount: analysis?.groups.length ?? 0,
      fileCount: analysis?.summary.total_files_changed ?? 0,
      infraCount: analysis?.infrastructure_group?.files.length ?? 0,
    });
  }, [analysis, repoPath]);

  // Listen for manifest-changed events from the file watcher
  useEffect(() => {
    if (!IS_TAURI || !watchedManifestPath) return;
    let cancelled = false;

    (async () => {
      const { listen } = await import("@tauri-apps/api/event");
      const unlisten = await listen<string>("manifest-changed", (event) => {
        if (!cancelled) {
          importGroupsManifest(event.payload);
        }
      });
      return unlisten;
    })().then((unlisten) => {
      if (cancelled && unlisten) unlisten();
      // Store unlisten for cleanup
      return unlisten;
    });

    return () => { cancelled = true; };
  }, [watchedManifestPath, importGroupsManifest]);

  /** Pre-indexed comment counts by group for O(1) lookup. */
  const commentsByGroupMap = useMemo(() => {
    const map = new Map<string, number>();
    for (const c of comments) {
      map.set(c.group_id, (map.get(c.group_id) ?? 0) + 1);
    }
    return map;
  }, [comments]);

  /** Pre-indexed comments by file path for O(1) lookup. */
  const commentsByFileMap = useMemo(() => {
    const map = new Map<string, ReviewComment[]>();
    for (const c of comments) {
      if (c.file_path) {
        const arr = map.get(c.file_path);
        if (arr) arr.push(c);
        else map.set(c.file_path, [c]);
      }
    }
    return map;
  }, [comments]);

  /** Get the count of comments for a specific group. */
  const commentCountForGroup = useCallback(
    (groupId: string): number => commentsByGroupMap.get(groupId) ?? 0,
    [commentsByGroupMap],
  );

  /** Get comments for the currently selected file. */
  const commentsForFile = useCallback(
    (filePath: string): ReviewComment[] => commentsByFileMap.get(filePath) ?? [],
    [commentsByFileMap],
  );

  /**
   * Compute whether to render side-by-side for the current file.
   *
   * In "dynamic" mode, uses change density to decide: sparse edits or full-file
   * rewrites get side-by-side; dense targeted changes get inline.
   */
  const shouldRenderSideBySide = useMemo((): boolean => {
    if (diffViewMode === "side-by-side") return true;
    if (diffViewMode === "inline") return false;
    // Dynamic: compute per-file density
    if (!fileDiff) return true;
    const oldLines = (fileDiff.old_content || "").split("\n").length;
    const newLines = (fileDiff.new_content || "").split("\n").length;
    const maxLines = Math.max(oldLines, newLines, 1);
    // Find the selected file's change stats from the active group
    const fileChange = selectedGroup?.files.find((f) => f.path === selectedFile);
    if (!fileChange) return true;
    const totalChanged = fileChange.changes.additions + fileChange.changes.deletions;
    const density = totalChanged / maxLines;
    return  density > (2/3);
  }, [diffViewMode, fileDiff, selectedGroup, selectedFile]);

  /** Code-level comments for the selected file, passed to DiffViewer. */
  const codeCommentsForSelectedFile = useMemo(
    () => selectedFile
      ? comments.filter((c) => c.type === "code" && c.file_path === selectedFile)
      : [],
    [comments, selectedFile],
  );

  /** Open the current file in an external editor. */
  type EditorId = "vscode" | "cursor" | "zed" | "vim" | "terminal";
  const [openWithDropdown, setOpenWithDropdown] = useState(false);
  const [lastEditor, setLastEditor] = useState<EditorId>("vscode");
  const [availableEditors, setAvailableEditors] = useState<Set<EditorId> | null>(null);
  const openWithRef = useRef<HTMLDivElement>(null);

  const editorIcons: Record<EditorId, string> = {
    vscode: `<svg width="16" height="16" viewBox="0 0 256 256" xmlns="http://www.w3.org/2000/svg"><path d="M180.3 4.5l-56 43.2L59 4.2a8.3 8.3 0 0 0-10.2 1.5L5.1 47.4a8 8 0 0 0 0 11.2L44 96l-39 37.4a8 8 0 0 0 0 11.2l43.7 41.7a8.3 8.3 0 0 0 10.2 1.5l65.3-43.5 56 43.2a12.2 12.2 0 0 0 7 2.5c2 0 4-.5 5.8-1.6l47.5-23a12 12 0 0 0 6.5-10.6V33.2c0-4.4-2.5-8.5-6.5-10.6L193.1 0c-4-2-8.8-1.3-12.8 2.5v2zM192 52.8v150.4L123 128z" fill="#007ACC"/></svg>`,
    cursor: `<svg width="16" height="16" viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg"><path d="M1 1l6.5 14L10 9l6-2.5z" stroke="#cdd6f4" stroke-width="1.5" stroke-linejoin="round" fill="none"/><path d="M10 9l4.5 4.5" stroke="#cdd6f4" stroke-width="1.5" stroke-linecap="round"/></svg>`,
    zed: `<svg width="16" height="16" viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg"><path d="M2 3h12L2 13h12" stroke="#cdd6f4" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>`,
    vim: `<svg width="16" height="16" viewBox="0 0 544 544" xmlns="http://www.w3.org/2000/svg"><polygon points="272,16 16,272 144,272 272,144 272,272 400,272 528,272 272,16" fill="#019833"/><polygon points="272,528 528,272 400,272 272,400 272,272 144,272 16,272 272,528" fill="#33cc33"/></svg>`,
    terminal: `<svg width="16" height="16" viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg"><rect x="1" y="2" width="14" height="12" rx="2" stroke="#cdd6f4" stroke-width="1.2"/><path d="M4 6l2.5 2L4 10" stroke="#a6e3a1" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/><path d="M8.5 10H12" stroke="#6c7086" stroke-width="1.2" stroke-linecap="round"/></svg>`,
  };

  const allEditorOptions: { id: EditorId; label: string }[] = [
    { id: "vscode", label: "VS Code" },
    { id: "cursor", label: "Cursor" },
    { id: "zed", label: "Zed" },
    { id: "vim", label: "Vim" },
    { id: "terminal", label: "Terminal" },
  ];

  const editorOptions = availableEditors
    ? allEditorOptions.filter((opt) => availableEditors.has(opt.id))
    : allEditorOptions;

  // Detect which editors are installed
  useEffect(() => {
    if (!IS_TAURI) return;
    tauriInvoke<Record<string, boolean>>("check_editors_available").then(
      (result) => {
        const available = new Set<EditorId>();
        for (const [id, isAvailable] of Object.entries(result)) {
          if (isAvailable) available.add(id as EditorId);
        }
        setAvailableEditors(available);
        // If current lastEditor is not available, switch to first available
        if (!available.has(lastEditor)) {
          const first = allEditorOptions.find((opt) => available.has(opt.id));
          if (first) setLastEditor(first.id);
        }
      },
      () => {
        // On error, show all editors (graceful fallback)
      },
    );
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const openInEditor = useCallback(
    async (editor: EditorId) => {
      const file = selectedFile;
      if (!file) {
        showToast("No file selected");
        return;
      }
      const absPath = buildAbsolutePath(file);
      setLastEditor(editor);
      setOpenWithDropdown(false);
      if (!IS_TAURI) {
        showToast(`Would open ${absPath} in ${editor}`);
        return;
      }
      try {
        await tauriInvoke("open_in_editor", { editor, filePath: absPath });
      } catch (e: any) {
        const msg = typeof e === "string" ? e : e?.message || String(e);
        showToast(msg);
      }
    },
    [selectedFile, buildAbsolutePath, showToast],
  );

  // Close "Open With" dropdown when clicking outside
  useEffect(() => {
    if (!openWithDropdown) return;
    function handleClick(e: MouseEvent) {
      if (openWithRef.current && !openWithRef.current.contains(e.target as Node)) {
        setOpenWithDropdown(false);
      }
    }
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, [openWithDropdown]);

  const changedFilePathSet = useMemo(() => {
    const set = new Set<string>();
    if (!analysis) return set;
    for (const group of analysis.groups) {
      for (const file of group.files) {
        set.add(file.path);
      }
    }
    for (const file of analysis.infrastructure_group?.files ?? []) {
      set.add(file);
    }
    return set;
  }, [analysis]);

  const openCrossFileSearchResult = useCallback(async (filePath: string, lineNumber: number) => {
    const group = selectedGroupRef.current;
    const changed = changedFilePathSet.has(filePath);

    if (changed && group) {
      openFileInTab(filePath, group.id);
      setTimeout(() => {
        diffViewerRef.current?.scrollToLine(lineNumber, lineNumber);
      }, 120);
      return;
    }

    if (IS_TAURI && repoPath) {
      try {
        const content = await tauriInvoke<FileDiffContent>("get_workspace_file_content", {
          repoPath,
          filePath,
        });
        setSelectedFile(filePath);
        setFileDiff(content);
        setOpenTabs((prev) => {
          if (prev.some((t) => t.path === filePath)) return prev;
          return [...prev, { path: filePath, groupId: group?.id ?? "__workspace_search__" }];
        });
        setTimeout(() => {
          diffViewerRef.current?.scrollToLine(lineNumber, lineNumber);
        }, 120);
        return;
      } catch (e) {
        showToast(`Unable to open ${filePath}: ${String(e)}`);
      }
    }
  }, [changedFilePathSet, openFileInTab, repoPath, showToast]);

  const runCrossFileSearch = useCallback(async () => {
    const query = crossFileSearchQuery.trim();
    if (!query) {
      setCrossFileSearchResults([]);
      setCrossFileSearchError(null);
      return;
    }

    setCrossFileSearchLoading(true);
    setCrossFileSearchError(null);
    try {
      if (IS_TAURI && repoPath) {
        const results = await tauriInvoke<CrossFileSearchResult[]>("cross_file_search", {
          repoPath,
          query,
          showUnchangedFiles,
          maxResults: 200,
        });
        setCrossFileSearchResults(results);
      } else {
        // Demo fallback: search known mock diff files.
        const needle = query.toLowerCase();
        const results: CrossFileSearchResult[] = Object.entries(MOCK_DIFFS)
          .map(([path, diff]) => {
            const lines = (diff?.new_content || "").split("\n");
            const matches: CrossFileSearchMatch[] = [];
            lines.forEach((line, idx) => {
              if (line.toLowerCase().includes(needle)) {
                matches.push({ line_number: idx + 1, line_text: line });
              }
            });
            return { file_path: path, matches };
          })
          .filter((r) => r.matches.length > 0);
        setCrossFileSearchResults(results);
      }
    } catch (e) {
      setCrossFileSearchResults([]);
      setCrossFileSearchError(String(e));
    } finally {
      setCrossFileSearchLoading(false);
    }
  }, [crossFileSearchQuery, repoPath, showUnchangedFiles]);

  useEffect(() => {
    if (!crossFileSearchOpen) return;
    const timer = setTimeout(() => {
      crossFileSearchInputRef.current?.focus();
      crossFileSearchInputRef.current?.select();
    }, 0);
    return () => clearTimeout(timer);
  }, [crossFileSearchOpen]);

  /** Handle right-click context menu on a file item. */
  const handleFileContextMenu = useCallback(
    (e: React.MouseEvent, filePath: string) => {
      e.preventDefault();
      e.stopPropagation();
      setContextMenu({ x: e.clientX, y: e.clientY, filePath });
    },
    [],
  );

  /** Enter flow replay mode for the currently selected group. */
  const enterReplay = useCallback(() => {
    const group = selectedGroupRef.current;
    if (!group || group.files.length === 0) return;
    setReplayActive(true);
    setReplayStep(0);
    setReplayVisited(new Set([group.files[0].path]));
    setReplayHunkIndex(0);
    setReplayViewedHunkIds(new Set());
    handleSelectFile(group.files[0].path);
  }, [handleSelectFile]);

  /** Exit flow replay mode. */
  const exitReplay = useCallback(() => {
    setReplayActive(false);
    setReplayStep(0);
    setReplayVisited(new Set());
    setReplayHunkIndex(0);
    setReplayViewedHunkIds(new Set());
  }, []);

  /** Move to a specific replay step. */
  const goToReplayStep = useCallback(
    (step: number) => {
      const group = selectedGroupRef.current;
      if (!group) return;
      const clamped = Math.max(0, Math.min(step, group.files.length - 1));
      setReplayStep(clamped);
      const filePath = group.files[clamped].path;
      pendingReplayHunkScrollRef.current = { filePath, targetHunkIndex: 0, attempts: 0, stepIndex: clamped };
      setReplayHunkIndex(0);
      setCurrentReplayHunks([]);
      setReplayVisited((prev) => {
        const next = new Set(prev);
        next.add(filePath);
        return next;
      });
      handleSelectFile(filePath);
    },
    [handleSelectFile],
  );

  const hasNextReplayHunk = (
    replayHunkIndex < replayHunks.length - 1
    || !!selectedGroup?.files[replayStep + 1]
  );
  const hasPrevReplayHunk = (
    replayHunkIndex > 0
    || !!selectedGroup?.files[replayStep - 1]
  );

  const navigateReplayHunk = useCallback(
    (direction: 1 | -1) => {
      const group = selectedGroupRef.current;
      if (!group || group.files.length === 0) return;
      const hunks = replayHunks;

      if (direction > 0) {
        if (replayHunkIndex < hunks.length - 1) {
          jumpToReplayHunk(replayHunkIndex + 1);
          return;
        }
        if (replayStep < group.files.length - 1) {
          const nextStep = replayStep + 1;
          const nextFilePath = group.files[nextStep].path;
          setReplayStep(nextStep);
          setReplayVisited((prev) => {
            const next = new Set(prev);
            next.add(nextFilePath);
            return next;
          });
          setCurrentReplayHunks([]);
          setReplayHunkIndex(0);
          pendingReplayHunkScrollRef.current = { filePath: nextFilePath, targetHunkIndex: 0, attempts: 0, stepIndex: nextStep, direction: 1 };
          openFileInTab(nextFilePath, group.id);
        }
        return;
      }

      if (replayHunkIndex > 0) {
        jumpToReplayHunk(replayHunkIndex - 1);
        return;
      }
      if (replayStep > 0) {
        const prevStep = replayStep - 1;
        const prevFilePath = group.files[prevStep].path;
        setReplayStep(prevStep);
        setReplayVisited((prev) => {
          const next = new Set(prev);
          next.add(prevFilePath);
          return next;
        });
        setCurrentReplayHunks([]);
        setReplayHunkIndex(0); 
        pendingReplayHunkScrollRef.current = { filePath: prevFilePath, targetHunkIndex: -1, attempts: 0, stepIndex: prevStep, direction: -1 };
        openFileInTab(prevFilePath, group.id);
      }
    },
    [replayHunkIndex, replayHunks, replayStep, jumpToReplayHunk, openFileInTab],
  );

  // Keyboard navigation: j/k = next/prev file, J/K = next/prev group, r = replay, l/h = hunk next/prev
  // Vim keys and arrow keys are supported
  // Registered on capture phase so shortcuts work even when Monaco editor has focus.
  // Let Monaco pass through if edit mode is enabled.
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      // Skip if user is typing in an input field — but not Monaco's internal textarea if immutable
      const target = e.target as HTMLElement;
      const isInImmutableMonaco = !!target.closest(".monaco-editor") && !editsEnabled;
      if (
        !isInImmutableMonaco &&
        (target.tagName === "INPUT" ||
          target.tagName === "TEXTAREA" ||
          target.tagName === "SELECT")
      ) {
        return;
      }

      // Cmd/Ctrl+W: close active tab (intercept before Monaco check)
      if ((e.metaKey || e.ctrlKey) && e.key === "w") {
        e.preventDefault();
        e.stopPropagation();
        const file = selectedFileRef.current;
        if (file) closeTab(file);
        return;
      }

      if (e.key === "F") {
        e.preventDefault();
        e.stopPropagation();
        setCrossFileSearchOpen(true);
        return;
      }

      if (e.key === "f") {
        e.preventDefault();
        e.stopPropagation();
        diffViewerRef.current?.openFindWidget();
        return;
      }

      // When Monaco has focus, only intercept known app shortcut keys.
      // Let other keys (arrows, Page Up/Down, etc.) pass through to Monaco for scrolling.
      if (isInImmutableMonaco) {
        const appKeys = new Set(["j", "k", "J", "K", "r", "x", "y", "Y", "c", "C", "f", "F"]);
        if (!appKeys.has(e.key)) {
          return;
        }
      }

      // Helper: prevent default AND stop propagation (needed to prevent Monaco
      // from showing "Cannot edit in read-only editor" for intercepted keys).
      const consume = () => {
        e.preventDefault();
        e.stopPropagation();
      };

      const groups = sortedGroupsRef.current;
      const group = selectedGroupRef.current;
      const file = selectedFileRef.current;
      const isReplaying = replayActiveRef.current;
      const step = replayStepRef.current;

      // Replay mode keys
      if (isReplaying && group) {
        if (e.key === "Escape" || e.key === "r") {
          consume();
          exitReplay();
          return;
        }
        if (e.key === "n" || e.key === "ArrowRight" || e.key === " ") {
          consume();
          if (step < group.files.length - 1) {
            goToReplayStep(step + 1);
          }
          return;
        }
        if (e.key === "p" || e.key === "ArrowLeft") {
          consume();
          if (step > 0) {
            goToReplayStep(step - 1);
          }
          return;
        }
        // Block other navigation while replaying
        if (["j", "k", "J", "K"].includes(e.key)) {
          consume();
          return;
        }
      }

      // Normal mode: r enters replay
      if (e.key === "r" && group && group.files.length > 0) {
        consume();
        enterReplay();
        return;
      }

      // x toggles reviewed state on the currently selected group
      if (e.key === "x" && group) {
        consume();
        toggleGroupReviewed(group.id);
        return;
      }

      // y copies the absolute path of the currently selected file
      if (e.key === "y" && !e.shiftKey && file) {
        consume();
        copyFilePath(file);
        return;
      }

      // Y (shift+y) copies all file paths in the current group
      if (e.key === "Y" && group) {
        consume();
        copyFlowPaths(group);
        return;
      }

      // h navigates to the previous hunk
      if (e.key === "h") {
        consume();
        if (replayActiveRef.current) {
          navigateReplayHunk(-1);
        } else {
          // TODO
          // diffViewerRef.current?.scrollToPreviousHunk();
        }
        return;
      }

      // l navigates to the next hunk
      if (e.key === "l") {
        consume();
        if (replayActiveRef.current) {
          navigateReplayHunk(1);
        } else {
          // TODO
          // diffViewerRef.current?.scrollToNextHunk();
        }
        return;
      }

      // c opens context-sensitive comment input — if text is selected in Monaco, include it
      if (e.key === "c" && !e.shiftKey && group) {
        consume();
        // Try to grab the current selection from Monaco's modified (right-side) editor
        const monacoEditors = (window as any).monaco?.editor?.getEditors?.();
        if (monacoEditors && file) {
          for (const ed of monacoEditors) {
            const sel = ed.getSelection?.();
            if (sel && sel.startLineNumber !== sel.endLineNumber) {
              const model = ed.getModel?.();
              if (model) {
                const startLine = Math.min(sel.startLineNumber, sel.endLineNumber);
                const endLine = Math.max(sel.startLineNumber, sel.endLineNumber);
                const lines: string[] = [];
                for (let i = startLine; i <= endLine; i++) {
                  lines.push(model.getLineContent(i));
                }
                openCommentInput({
                  type: "code",
                  group_id: group.id,
                  file_path: file,
                  start_line: startLine,
                  end_line: endLine,
                  selected_code: lines.join("\n"),
                });
                return;
              }
            }
          }
        }
        openCommentInput();
        return;
      }

      // C (shift+c) copies all comments
      if (e.key === "C" && group) {
        consume();
        exportComments();
        return;
      }

      if (groups.length === 0 || !group) return;

      const groupIdx = groups.findIndex((g) => g.id === group.id);
      if (groupIdx === -1) return;

      if (e.key === "j") {
        // Next file in current group (debounced to avoid IPC storms)
        consume();
        const fileIdx = group.files.findIndex((f) => f.path === file);
        if (fileIdx < group.files.length - 1) {
          handleSelectFileDebounced(group.files[fileIdx + 1].path);
        }
      } else if (e.key === "k") {
        // Previous file in current group (debounced)
        consume();
        const fileIdx = group.files.findIndex((f) => f.path === file);
        if (fileIdx > 0) {
          handleSelectFileDebounced(group.files[fileIdx - 1].path);
        }
      } else if (e.key === "J") {
        // Next group
        consume();
        if (groupIdx < groups.length - 1) {
          handleSelectGroup(groups[groupIdx + 1]);
        }
      } else if (e.key === "K") {
        // Previous group
        consume();
        if (groupIdx > 0) {
          handleSelectGroup(groups[groupIdx - 1]);
        }
      }
    }

    window.addEventListener("keydown", handleKeyDown, true);
    return () => window.removeEventListener("keydown", handleKeyDown, true);
  }, [handleSelectFile, handleSelectFileDebounced, handleSelectGroup, enterReplay, exitReplay, goToReplayStep, toggleGroupReviewed, copyFilePath, copyFlowPaths, openCommentInput, exportComments, closeTab]);

  const handleSelectBase = useCallback((refName: string) => {
    setBaseRef(refName);
    setBranchDropdownOpen(false);
    setShowBaseCommits(false);
  }, []);

  const handleSelectHead = useCallback((refName: string) => {
    setHeadRef(refName);
    setHeadBranchDropdownOpen(false);
    setShowHeadCommits(false);
  }, []);

  // Derived: all branches for the dropdowns
  const baseBranches: BranchInfo[] = repoInfo?.branches ?? [];
  const headLabel = formatCompareTargetLabel(headRef, recentCommits) ?? "HEAD";
  const baseLabel = formatCompareTargetLabel(baseRef, recentCommits) ?? baseRef;

  // Status display
  const statusText = formatBranchStatus(repoInfo);

  const annotationsTabContent = selectedGroup ? (
    <div className="annotations-scroll-container">
      {/* Annotation sub-tabs */}
      <div className="annotation-subtabs">
        <button
          className={`annotation-subtab ${annotationSubTab === "info" ? "active" : ""}`}
          onClick={() => setAnnotationSubTab("info")}
        >
          Info
        </button>
        <button
          className={`annotation-subtab ${annotationSubTab === "graph" ? "active" : ""}`}
          onClick={() => setAnnotationSubTab("graph")}
          disabled={selectedGroup.edges.length === 0}
        >
          Graph
        </button>
        <button
          className={`annotation-subtab ${annotationSubTab === "edges" ? "active" : ""}`}
          onClick={() => setAnnotationSubTab("edges")}
          disabled={selectedGroup.edges.length === 0}
        >
          Edges
          {selectedGroup.edges.length > 0 && (
            <span className="annotation-subtab-count">{selectedGroup.edges.length}</span>
          )}
        </button>
      </div>

      {annotationSubTab === "info" && (
        <>
          <div className="annotation-section" data-testid="annotations-panel">
            <h3>Flow Group</h3>
            <p className="group-detail-name">{selectedGroup.name}</p>
            {selectedGroup.entrypoint && (
              <p className="entrypoint-info">
                Entrypoint: {selectedGroup.entrypoint.symbol} (
                {selectedGroup.entrypoint.entrypoint_type})
              </p>
            )}
            <p>
              Risk: <strong>{selectedGroup.risk_score.toFixed(2)}</strong>{" "}
              | Files: <strong>{selectedGroup.files.length}</strong> |
              Review order: <strong>#{selectedGroup.review_order}</strong>
            </p>
            {llmSettings && (
              <>
                <div className="settings-row" style={{ marginTop: 8, alignItems: "center" }}>
                  <label style={{ fontSize: 12, color: "var(--text-secondary)" }}>
                    Model ({PROVIDER_LABELS[llmSettings.provider as LlmProvider]})
                  </label>
                  <div style={{ marginLeft: "auto", maxWidth: 260, flex: "0 1 260px" }}>
                    <Dropdown
                      value={llmSettings.model}
                      onChange={(value) => updateSetting("model", value)}
                      options={modelsForProvider(llmSettings.provider).map((m) => ({ value: m, label: m }))}
                      placeholder="Select model"
                    />
                  </div>
                </div>
                <button
                  className="btn"
                  style={{ marginTop: 8 }}
                  onClick={() => {
                    setRegenDialogOpen(true);
                    setRegenFeedbackText("");
                    setRegenIncludePreviousOutput(true);
                  }}
                  title="Regenerate annotations with additional guidance"
                >
                  Regenerate with feedback/question
                </button>
              </>
            )}
            {selectedGroup.files.length > 1 && !replayActive && (
              <button
                className="btn btn-replay"
                onClick={enterReplay}
                title="Step through files in data flow order (r)"
              >
                &#9654; Replay Flow
              </button>
            )}
            {replayActive && (
              <button
                className="btn btn-replay-exit"
                onClick={exitReplay}
                title="Exit replay mode (Esc)"
              >
                &#10005; Exit Replay
              </button>
            )}
          </div>

          {refinementVerdict && (
            <div className="annotation-section refinement-verdict-section" data-testid="refinement-verdict">
              <h3>Refinement Verdict</h3>
              <p className="refinement-verdict-title">{refinementVerdict.title}</p>
              <p className="refinement-verdict-meta">
                {PROVIDER_LABELS[refinementVerdict.provider as LlmProvider] ?? refinementVerdict.provider}/{refinementVerdict.model}
              </p>
              {refinementVerdict.reasoning && (
                <p className="refinement-verdict-reasoning">{refinementVerdict.reasoning}</p>
              )}
            </div>
          )}

          {overview && !groupAnnotation && (
            <div className="annotation-section llm-section">
              <h3>LLM Overview</h3>
              <p className="llm-summary">{overview.overall_summary}</p>
            </div>
          )}

          {groupAnnotation && (
            <div className="annotation-section llm-section">
              <h3>LLM Summary</h3>
              <p className="llm-summary">{groupAnnotation.summary}</p>
              <p className="llm-rationale">
                <strong>Review rationale:</strong> {groupAnnotation.review_order_rationale}
              </p>
              {groupAnnotation.risk_flags.length > 0 && (
                <div className="risk-flags">
                  {groupAnnotation.risk_flags.map((flag, i) => (
                    <span key={i} className="risk-flag">{flag}</span>
                  ))}
                </div>
              )}
            </div>
          )}

          {overview && groupAnnotation && (
            <div className="annotation-section llm-section">
              <h3>Overall Summary</h3>
              <p className="llm-summary">{overview.overall_summary}</p>
            </div>
          )}

          {groupDeepAnalysis && (
            <>
              <div className="annotation-section llm-section">
                <h3>Flow Narrative</h3>
                <p className="llm-narrative">{groupDeepAnalysis.flow_narrative}</p>
              </div>

              {groupDeepAnalysis.file_annotations.length > 0 && (
                <div className="annotation-section llm-section">
                  <h3>File Annotations</h3>
                  {groupDeepAnalysis.file_annotations.map((fa, i) => (
                    <div key={i} className="file-annotation">
                      <div className="file-annotation-header">
                        <span className="file-annotation-path">{shortPath(fa.file)}</span>
                        <span className="file-annotation-role">{fa.role_in_flow}</span>
                      </div>
                      <p className="file-annotation-changes">{fa.changes_summary}</p>
                      {fa.risks.length > 0 && (
                        <div className="file-annotation-list">
                          <span className="annotation-label risk-label">Risks:</span>
                          <ul>
                            {fa.risks.map((r, j) => (
                              <li key={j}>{r}</li>
                            ))}
                          </ul>
                        </div>
                      )}
                      {fa.suggestions.length > 0 && (
                        <div className="file-annotation-list">
                          <span className="annotation-label suggestion-label">Suggestions:</span>
                          <ul>
                            {fa.suggestions.map((s, j) => (
                              <li key={j}>{s}</li>
                            ))}
                          </ul>
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              )}

              {groupDeepAnalysis.cross_cutting_concerns.length > 0 && (
                <div className="annotation-section llm-section">
                  <h3>Cross-Cutting Concerns</h3>
                  <ul className="concerns-list">
                    {groupDeepAnalysis.cross_cutting_concerns.map((c, i) => (
                      <li key={i}>{c}</li>
                    ))}
                  </ul>
                </div>
              )}
            </>
          )}
        </>
      )}

      {annotationSubTab === "graph" && selectedGroup.edges.length > 0 && (
        <div className="annotation-section flow-graph-section flow-graph-full">
          <div className="settings-row" style={{ marginBottom: 8, alignItems: "center" }}>
            <label style={{ fontSize: 12, color: "var(--text-secondary)" }}>Granularity</label>
            <div style={{ marginLeft: "auto", maxWidth: 220, flex: "0 1 220px" }}>
              <Dropdown<"file" | "module_class_method">
                value={graphGranularity}
                onChange={(value) => setGraphGranularity(value)}
                options={[
                  { value: "file", label: "file" },
                  { value: "module_class_method", label: "module/class/method", description: "preview" },
                ]}
              />
            </div>
          </div>
          {graphGranularity === "module_class_method" && (
            <p className="settings-hint" style={{ marginBottom: 8 }}>
              Preview mode: symbol-level graph is not available yet; rendering file-level graph as fallback.
            </p>
          )}
          <ErrorBoundary panelName="Flow Graph">
            <CrashTest panel="Flow Graph" />
            <FlowGraph
              edges={selectedGroup.edges}
              files={selectedGroup.files}
              onNodeClick={handleGraphNodeClick}
              onEdgeClick={handleGraphEdgeClick}
              replayNodeId={replayActive && selectedGroup.files[replayStep] ? selectedGroup.files[replayStep].path : null}
            />
          </ErrorBoundary>
        </div>
      )}

      {annotationSubTab === "edges" && selectedGroup.edges.length > 0 && (
        <div className="annotation-section edges-section">
          <ul className="edge-list">
            {selectedGroup.edges.map((edge, i) => {
              const fromFile = symbolFilePath(edge.from);
              const fromSymbol = shortSymbol(edge.from);
              const toLabel = shortSymbol(edge.to);
              return (
                <li key={i} className="edge-item file-item edge-item-row">
                  <FileDisplay
                    path={fromFile}
                    roleBadge={edge.edge_type}
                    hideChanges
                    prefix={(
                      <span className="edge-target">
                        <span className="edge-from-symbol" title={edge.from}>
                          {fromSymbol}
                        </span>
                        <span className="edge-arrow" aria-hidden="true">&rarr;</span>
                        <button
                          className="edge-endpoint edge-to"
                          onClick={() => handleEdgeEndpointClick(edge.to)}
                          title={edge.to}
                        >
                          {toLabel}
                        </button>
                      </span>
                    )}
                  />
                </li>
              );
            })}
          </ul>
        </div>
      )}
    </div>
  ) : (
    <div className="empty-state">
      Select a group to see annotations.
    </div>
  );

  const activityTabContent = (
    <div className="activity-tab-shell">
      <div className="activity-tab-region activity-tab-region-hero">
      <div className="annotation-section activity-hero-section" data-testid="activity-panel">
        <div className="activity-hero">
          <div>
            <p className="activity-hero-eyebrow">AI Activity</p>
            <p className="activity-hero-title">
              {activityJob
                ? activityJob.title
                : activityTimeline.length > 0
                  ? "Latest AI run"
                  : "No AI activity yet"}
            </p>
            <p className="activity-hero-subtitle">
              {activityJob
                ? `${PROVIDER_LABELS[activityJob.provider as LlmProvider] ?? activityJob.provider}/${activityJob.model}`
                : activityTimeline.length > 0 && activityEventProvider
                  ? `Latest stream captured from ${PROVIDER_LABELS[activityEventProvider as LlmProvider] ?? activityEventProvider}${activitySupportsToolStreaming ? " with live repo access." : "."}`
                  : activityEventProvider
                    ? `${PROVIDER_LABELS[activityEventProvider as LlmProvider] ?? activityEventProvider}${activitySupportsToolStreaming ? " can stream repo activity live." : " is running in direct API mode."}`
                  : "Run Summarize PR, Analyze This Flow, or Refine to inspect AI work."}
            </p>
          </div>
          {activityJob ? (
            <span className="activity-live-badge">Live</span>
          ) : activityTimeline.length > 0 ? (
            <span className="activity-live-badge activity-live-idle">Saved</span>
          ) : null}
        </div>

        {(activityJob || activityTimeline.length > 0) && (
          <div
            className={`activity-stats ${activityIsDirectApi ? "activity-stats-events-only" : ""}`}
            data-testid="activity-stats"
          >
            <div className="activity-stat">
              <span className="activity-stat-value">{activityStats.total}</span>
              <span className="activity-stat-label">events</span>
            </div>
            {/*
              Search / reads / commands tiles are hidden in direct-API mode
              because hosted APIs (OpenAI / Anthropic / Gemini) don't emit
              tool events — leaving them visible just shows three stale 0s.
            */}
            {!activityIsDirectApi && (
              <>
                <div className="activity-stat">
                  <span className="activity-stat-value">{activityStats.search}</span>
                  <span className="activity-stat-label">search</span>
                </div>
                <div className="activity-stat">
                  <span className="activity-stat-value">{activityStats.read}</span>
                  <span className="activity-stat-label">reads</span>
                </div>
                <div className="activity-stat">
                  <span className="activity-stat-value">{activityStats.command}</span>
                  <span className="activity-stat-label">commands</span>
                </div>
              </>
            )}
          </div>
        )}

        {activitySupportsToolStreaming && activityEventProvider && (
          <div className="activity-callout activity-callout-positive">
            <p className="activity-callout-title">Live repo activity enabled</p>
            <p className="activity-callout-body">
              {PROVIDER_LABELS[activityEventProvider as LlmProvider] ?? activityEventProvider} can inspect the repo directly,
              so file reads, searches, and shell commands stream here while you wait.
            </p>
          </div>
        )}

        {refinementVerdict && (
          <div className={`activity-callout ${refinementVerdict.hadChanges ? "activity-callout-positive" : ""}`} data-testid="activity-refinement-verdict">
            <p className="activity-callout-title">{refinementVerdict.title}</p>
            <p className="activity-callout-body">
              {PROVIDER_LABELS[refinementVerdict.provider as LlmProvider] ?? refinementVerdict.provider}/{refinementVerdict.model}
            </p>
            {refinementVerdict.reasoning && (
              <p className="activity-callout-body">{refinementVerdict.reasoning}</p>
            )}
          </div>
        )}
      </div>
      </div>

      <div className="activity-tab-divider" role="presentation" aria-hidden="true" />

      <div className="activity-tab-region activity-tab-region-events">
      <div className="annotation-section activity-section" data-testid="activity-log-panel">
        {showDirectApiBanner && (
          <div className="activity-events-banner" data-testid="activity-direct-api-note" role="status">
            <div className="activity-events-banner-body">
              <p className="activity-events-banner-title">Direct API mode</p>
              <p className="activity-events-banner-text">
                OpenAI, Anthropic, and Gemini only emit high-level progress.
                File reads, grep searches, and shell steps appear when Diffcore routes
                the job through Codex CLI or Claude Code.
              </p>
              {recommendedSubscriptionProvider && (
                <button
                  className="btn btn-sm activity-events-banner-action"
                  onClick={activatePreferredActivityProvider}
                >
                  Use {PROVIDER_LABELS[recommendedSubscriptionProvider]}
                </button>
              )}
            </div>
            <button
              type="button"
              className="activity-events-banner-dismiss"
              onClick={dismissDirectApiNotice}
              aria-label="Dismiss direct API mode notice"
              title="Dismiss"
            >
              &times;
            </button>
          </div>
        )}
        <div className="activity-log-header">
          <div>
            <h3>{activityViewMode === "stream" ? "Live Stream" : "All Events"}</h3>
            <p className="activity-log-subtitle">
              {activityViewMode === "stream"
                ? activityTimeline.length > ACTIVITY_STREAM_LIMIT
                  ? `Showing the latest ${ACTIVITY_STREAM_LIMIT} of ${activityTimeline.length} events.`
                  : "Newest activity lands at the bottom of the stream."
                : `${activityTimeline.length} events captured for the current run.`}
            </p>
          </div>
          <div className="activity-log-actions">
            {activityError && <span className="activity-error-inline">{activityError}</span>}
            <div className="activity-view-switch" role="tablist" aria-label="Activity views">
              <button
                className={`activity-view-tab ${activityViewMode === "stream" ? "active" : ""}`}
                type="button"
                role="tab"
                aria-selected={activityViewMode === "stream"}
                data-testid="activity-view-stream-tab"
                onClick={() => setActivityViewMode("stream")}
              >
                Stream
                <span className="activity-view-count">{Math.min(activityTimeline.length, ACTIVITY_STREAM_LIMIT)}</span>
              </button>
              <button
                className={`activity-view-tab ${activityViewMode === "all" ? "active" : ""}`}
                type="button"
                role="tab"
                aria-selected={activityViewMode === "all"}
                data-testid="activity-view-all-tab"
                onClick={() => setActivityViewMode("all")}
              >
                All events
                <span className="activity-view-count">{activityTimeline.length}</span>
              </button>
            </div>
          </div>
        </div>
        <div className="activity-feed-shell">
          <div
            ref={activityLogRef}
            className={`activity-log activity-log-rich activity-log-${activityViewMode}`}
            data-testid="activity-log"
            data-activity-view={activityViewMode}
          >
            {visibleActivityTimeline.length === 0 && (
              <div className="activity-empty">
                Activity will appear here once an AI job starts.
              </div>
            )}
            {visibleActivityTimeline.map(({ id, entry, presentation }, index) => {
              const isExpanded = inspectedActivityId === id;
              const hasExtraContent = Boolean(
                presentation.eventTypeLabel
                || presentation.detail
                || presentation.payloadText,
              );

              return (
                <div
                  key={id}
                  className={`activity-card activity-card-${presentation.kind} activity-${entry.level} ${activityViewMode === "stream" && index === visibleActivityTimeline.length - 1 ? "activity-card-latest" : ""} ${isExpanded ? "activity-card-expanded" : ""}`}
                  data-testid="activity-entry"
                >
                  <button
                    type="button"
                    className="activity-card-trigger"
                    aria-pressed={isExpanded}
                    onClick={() => setInspectedActivityId(id)}
                  >
                    <div className="activity-card-meta">
                      <span className={`activity-kind-badge activity-kind-${presentation.kind}`}>{presentation.badge}</span>
                      <span className="activity-source-pill">{presentation.sourceLabel}</span>
                      <span className="activity-time">{formatActivityTimestamp(entry.timestamp_ms)}</span>
                    </div>
                    <div className="activity-card-body">
                      <p className="activity-card-title">{presentation.title}</p>
                      {presentation.subject && (
                        <p className="activity-card-subject">{presentation.subject}</p>
                      )}
                      {hasExtraContent && (
                        <div className="activity-card-hints">
                          {presentation.eventTypeLabel && (
                            <span className="activity-card-hint" title={presentation.eventTypeLabel}>Event</span>
                          )}
                          {presentation.detail && (
                            <span className="activity-card-hint" title={presentation.detail}>{presentation.detailLabel ?? "Detail"}</span>
                          )}
                          {presentation.payloadText && (
                            <span className="activity-card-hint" title={presentation.payloadText}>
                              Payload{presentation.payloadSummary ? ` · ${presentation.payloadSummary}` : ""}
                            </span>
                          )}
                        </div>
                      )}
                    </div>
                  </button>
                </div>
              );
            })}
          </div>

        </div>
      </div>
      </div>
    </div>
  );

  const sourceTabContent = (
    <SourceExplorer
      fileDiff={fileDiff}
      selectedGroup={selectedGroup}
      selectedFileChange={selectedFileChange}
      focusRequest={sourceFocusRequest}
      onNavigateToSymbol={handleSourceNavigate}
      onScrollToLine={(startLine, endLine) => {
        diffViewerRef.current?.scrollToLine(startLine, endLine);
      }}
    />
  );

  // Group comments by file for the Comments tab
  const commentsByFile = useMemo(() => {
    const map = new Map<string, ReviewComment[]>();
    // Group-level comments go under a special key
    for (const c of comments) {
      const key = c.file_path ?? `__group__${c.group_id}`;
      const arr = map.get(key);
      if (arr) arr.push(c);
      else map.set(key, [c]);
    }
    return map;
  }, [comments]);

  const commentsTabContent = (
    <div className="comments-tab">
      <div className="comments-tab-header">
        <button className="btn btn-sm" onClick={() => openCommentInput()} title="Add comment (c)">
          + Comment
        </button>
        {comments.length > 0 && (
          <button className="btn btn-sm btn-copy-comments" onClick={exportComments} title="Copy all comments (Shift+C)">
            Copy All
          </button>
        )}
      </div>
      {comments.length === 0 ? (
        <div className="comments-tab-empty">
          <p>No comments yet</p>
          <p className="comments-tab-hint">Press <kbd>c</kbd> to comment on a file or select code lines in the diff viewer</p>
        </div>
      ) : (
        <div className="comments-tab-list">
          {Array.from(commentsByFile.entries()).map(([fileKey, fileComments]) => (
            <div key={fileKey} className="comments-tab-file-group">
              <div className="comments-tab-file-header">
                <span className="comments-tab-file-path">
                  {fileKey.startsWith("__group__") ? "Group comments" : shortPath(fileKey)}
                </span>
                <span className="comments-tab-file-count">{fileComments.length}</span>
              </div>
              {fileComments.map((comment) => (
                <div
                  key={comment.id}
                  data-comment-id={comment.id}
                  className={`comments-tab-card ${activeCommentId === comment.id ? "comments-tab-card-active" : ""}`}
                  onClick={() => {
                    setActiveCommentId(comment.id);
                    if (comment.file_path) {
                      const group = analysis?.groups.find((g: FlowGroup) => g.id === comment.group_id);
                      if (group) openFileInTab(comment.file_path, group.id);
                    }
                    if (comment.start_line != null) {
                      // If file is already loaded, scroll immediately; otherwise queue
                      if (comment.file_path === selectedFile && fileDiff) {
                        setTimeout(() => {
                          diffViewerRef.current?.scrollToLine(comment.start_line!, comment.end_line ?? undefined);
                        }, 50);
                      } else if (comment.file_path) {
                        pendingScrollToCommentRef.current = {
                          startLine: comment.start_line,
                          endLine: comment.end_line ?? undefined,
                          commentId: comment.id,
                        };
                      }
                    }
                  }}
                  role="button"
                  tabIndex={0}
                >
                  <div className="comments-tab-card-meta">
                    <span className={`comment-strip-badge comment-strip-badge-${comment.type}`}>{comment.type}</span>
                    {comment.start_line != null && comment.end_line != null && (
                      <span className="comments-tab-card-lines">L{comment.start_line}-{comment.end_line}</span>
                    )}
                    <button
                      className="comment-strip-edit"
                      onClick={(e) => {
                        e.stopPropagation();
                        setEditingCommentId(comment.id);
                        setEditingCommentText(comment.text);
                      }}
                      title="Edit"
                    >
                      &#9998;
                    </button>
                    <button
                      className="comment-strip-delete"
                      onClick={(e) => { e.stopPropagation(); deleteComment(comment.id); }}
                      title="Delete"
                    >
                      &times;
                    </button>
                  </div>
                  {comment.selected_code && (
                    <pre className="comment-strip-code">{comment.selected_code}</pre>
                  )}
                  {editingCommentId === comment.id ? (
                    <div className="comment-strip-edit-container" onClick={(e) => e.stopPropagation()}>
                      <textarea
                        className="comment-strip-edit-textarea"
                        value={editingCommentText}
                        onChange={(e) => setEditingCommentText(e.target.value)}
                        autoFocus
                        rows={3}
                        onKeyDown={(e) => {
                          if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
                            e.preventDefault();
                            if (editingCommentText.trim()) updateComment(comment.id, editingCommentText.trim());
                          }
                          if (e.key === "Escape") { e.preventDefault(); setEditingCommentId(null); }
                        }}
                      />
                      <div className="comment-strip-edit-actions">
                        <span className="comment-strip-edit-hint">Cmd+Enter to save</span>
                        <button className="btn btn-comment-save" disabled={!editingCommentText.trim()} onClick={() => updateComment(comment.id, editingCommentText.trim())}>Save</button>
                        <button className="btn btn-comment-cancel" onClick={() => setEditingCommentId(null)}>Cancel</button>
                      </div>
                    </div>
                  ) : (
                    <p className="comments-tab-card-text">{comment.text}</p>
                  )}
                </div>
              ))}
            </div>
          ))}
        </div>
      )}
    </div>
  );

  /** Test-only component that throws during render to exercise ErrorBoundary. */
  function CrashTest({ panel }: { panel: string }) {
    if (crashPanel === panel) {
      throw new Error(`Test crash in ${panel}`);
    }
    return null;
  }

  return (
    <div className="app">
      {/* Update banner */}
      {updateAvailable && (
        <div className="update-banner">
          <span>Diffcore v{updateAvailable.version} is available</span>
          <button
            className="btn btn-refine"
            disabled={updating}
            onClick={async () => {
              setUpdating(true);
              try {
                const { check } = await import("@tauri-apps/plugin-updater");
                const update = await check();
                if (update) {
                  await update.downloadAndInstall();
                  const { relaunch } = await import("@tauri-apps/plugin-process");
                  await relaunch();
                }
              } catch {
                setUpdating(false);
              }
            }}
          >
            {updating ? "Updating..." : "Update & Restart"}
          </button>
          <button
            className="update-dismiss"
            onClick={() => setUpdateAvailable(null)}
            title="Dismiss"
          >
            &times;
          </button>
        </div>
      )}
      {/* Top bar */}
      <header className="top-bar">
        <div className="top-bar-left">
          <span className="logo">Diffcore</span>
        </div>
        <div className="top-bar-center">
          <input
            ref={repoInputRef}
            className="input repo-input"
            type="text"
            placeholder="Repository path..."
            value={repoPath}
            onChange={(e) => setRepoPath(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && repoPath && !loading) {
                (e.target as HTMLInputElement).blur();
                runAnalysis();
              }
            }}
          />
          <button
            className="btn"
            onClick={() => { void browseForRepository(); }}
            title="Browse for a repository folder"
          >
            Browse
          </button>
          <button
            className="btn"
            onClick={() => setRepoQuickPickOpen((v) => !v)}
            title="Quick-pick recent and favorite repositories"
          >
            Recent
          </button>
          <button
            className="btn"
            onClick={() => {
              const path = repoPath.trim();
              if (!path) return;
              setFavoriteRepoPaths((prev) => (
                prev.includes(path) ? prev.filter((p) => p !== path) : [path, ...prev]
              ));
            }}
            title="Pin or unpin current repository"
          >
            {favoriteRepoPaths.includes(repoPath.trim()) ? "Unpin" : "Pin"}
          </button>

          {/* Branch comparison: head (source) → base (target) */}
          <div className="branch-comparison">
            {/* Head (source) branch dropdown */}
            <div className="branch-dropdown-wrapper" data-testid="head-branch-dropdown">
              <button
                className="btn branch-dropdown-trigger"
                onClick={() => {
                  setHeadBranchDropdownOpen(!headBranchDropdownOpen);
                  setBranchDropdownOpen(false);
                }}
                title="Select source branch (what you're comparing)"
              >
                <span className="branch-label">source</span>
                <span className="branch-icon">&#9741;</span>
                <span className="branch-name">{headLabel}</span>
                <span className="dropdown-arrow">&#9662;</span>
              </button>
              {headBranchDropdownOpen && (
                <ul className="branch-dropdown">
                  <li
                    className={`branch-option ${headRef === COMPARE_TARGET_UNSTAGED ? "selected" : ""}`}
                    onClick={() => handleSelectHead(COMPARE_TARGET_UNSTAGED)}
                  >
                    <span className="branch-option-name">Unstaged changes</span>
                  </li>
                  <li
                    className={`branch-option ${headRef === COMPARE_TARGET_STAGED ? "selected" : ""}`}
                    onClick={() => handleSelectHead(COMPARE_TARGET_STAGED)}
                  >
                    <span className="branch-option-name">Staged changes</span>
                  </li>
                  {baseBranches.map((b) => (
                    <li
                      key={b.name}
                      className={`branch-option ${b.name === headRef ? "selected" : ""} ${b.is_current ? "current" : ""}`}
                      onClick={() => handleSelectHead(b.name)}
                    >
                      <span className="branch-option-name">{b.name}</span>
                      {b.is_current && <span className="branch-current-badge">current</span>}
                      {b.has_upstream && <span className="branch-upstream-badge">tracked</span>}
                    </li>
                  ))}
                  {baseBranches.length === 0 && (
                    <li className="branch-option disabled">No branches found</li>
                  )}
                  {recentCommits.length > 0 && (
                    <>
                      <li
                        className="branch-option"
                        onClick={() => setShowHeadCommits((open) => !open)}
                        title="Show or hide recent commits"
                      >
                        <span className="branch-option-name">
                          {showHeadCommits ? "Hide recent commits" : "Show recent commits"}
                        </span>
                      </li>
                      {showHeadCommits && recentCommits.map((commit) => (
                        <li
                          key={`head-${commit.sha}`}
                          className={`branch-option ${commit.sha === headRef ? "selected" : ""}`}
                          onClick={() => handleSelectHead(commit.sha)}
                          title={`${commit.sha} · ${commit.author}`}
                        >
                          <span className="branch-option-name">{commit.short_sha} {commit.summary}</span>
                        </li>
                      ))}
                    </>
                  )}
                </ul>
              )}
            </div>

            <span className="branch-arrow" title="compared against">&#8594;</span>

            {/* Base (target) branch dropdown */}
            <div className="branch-dropdown-wrapper" data-testid="base-branch-dropdown">
              <button
                className="btn branch-dropdown-trigger"
                onClick={() => {
                  setBranchDropdownOpen(!branchDropdownOpen);
                  setHeadBranchDropdownOpen(false);
                }}
                title="Select target branch (what you're comparing against)"
              >
                <span className="branch-label">target</span>
                <span className="branch-icon">&#9741;</span>
                <span className="branch-name">{baseLabel}</span>
                <span className="dropdown-arrow">&#9662;</span>
              </button>
              {branchDropdownOpen && (
                <ul className="branch-dropdown">
                  <li
                    className={`branch-option ${baseRef === COMPARE_TARGET_UNSTAGED ? "selected" : ""}`}
                    onClick={() => handleSelectBase(COMPARE_TARGET_UNSTAGED)}
                  >
                    <span className="branch-option-name">Unstaged changes</span>
                  </li>
                  <li
                    className={`branch-option ${baseRef === COMPARE_TARGET_STAGED ? "selected" : ""}`}
                    onClick={() => handleSelectBase(COMPARE_TARGET_STAGED)}
                  >
                    <span className="branch-option-name">Staged changes</span>
                  </li>
                  {baseBranches.map((b) => (
                    <li
                      key={b.name}
                      className={`branch-option ${b.name === baseRef ? "selected" : ""} ${b.is_current ? "current" : ""}`}
                      onClick={() => handleSelectBase(b.name)}
                    >
                      <span className="branch-option-name">{b.name}</span>
                      {b.is_current && <span className="branch-current-badge">current</span>}
                      {b.has_upstream && <span className="branch-upstream-badge">tracked</span>}
                    </li>
                  ))}
                  {baseBranches.length === 0 && (
                    <li className="branch-option disabled">No branches found</li>
                  )}
                  {recentCommits.length > 0 && (
                    <>
                      <li
                        className="branch-option"
                        onClick={() => setShowBaseCommits((open) => !open)}
                        title="Show or hide recent commits"
                      >
                        <span className="branch-option-name">
                          {showBaseCommits ? "Hide recent commits" : "Show recent commits"}
                        </span>
                      </li>
                      {showBaseCommits && recentCommits.map((commit) => (
                        <li
                          key={`base-${commit.sha}`}
                          className={`branch-option ${commit.sha === baseRef ? "selected" : ""}`}
                          onClick={() => handleSelectBase(commit.sha)}
                          title={`${commit.sha} · ${commit.author}`}
                        >
                          <span className="branch-option-name">{commit.short_sha} {commit.summary}</span>
                        </li>
                      ))}
                    </>
                  )}
                </ul>
              )}
            </div>

            {/* Worktree indicator — shown when NOT a worktree (regular repo) */}
            {repoInfo && !repoInfo.is_worktree && (
              <span className="branch-repo-badge" title="Regular repository (not a worktree)">repo</span>
            )}
            {repoInfo?.is_worktree && (
              <span className="branch-worktree-badge" title="Linked worktree">worktree</span>
            )}
          </div>

          {repoQuickPickOpen && (
            <div className="branch-dropdown" style={{ maxHeight: 220, overflowY: "auto", minWidth: 320 }}>
              {favoriteRepoPaths.length > 0 && (
                <>
                  <li className="branch-option disabled">Favorites</li>
                  {favoriteRepoPaths.map((path) => (
                    <li
                      key={`fav-${path}`}
                      className="branch-option"
                      onClick={() => {
                        setRepoPath(path);
                        setRepoQuickPickOpen(false);
                      }}
                      title={path}
                    >
                      <span className="branch-option-name">★ {shortPath(path)}</span>
                    </li>
                  ))}
                </>
              )}
              {recentRepoPaths.length > 0 && (
                <>
                  <li className="branch-option disabled">Recent</li>
                  {recentRepoPaths.map((path) => (
                    <li
                      key={`recent-${path}`}
                      className="branch-option"
                      onClick={() => {
                        setRepoPath(path);
                        setRepoQuickPickOpen(false);
                      }}
                      title={path}
                    >
                      <span className="branch-option-name">{shortPath(path)}</span>
                    </li>
                  ))}
                </>
              )}
              {favoriteRepoPaths.length === 0 && recentRepoPaths.length === 0 && (
                <li className="branch-option disabled">No recent repositories</li>
              )}
            </div>
          )}

          <button
            className="btn btn-primary"
            onClick={runAnalysis}
            disabled={loading || !repoPath || comparisonMode === "invalid"}
          >
            {loading ? "Analyzing..." : "Analyze"}
          </button>
        </div>
        <div className="top-bar-right">
          {IS_TAURI && STATE_SAVE_RESTORE_ENABLED && (
            <button
              className="btn"
              onClick={() => { void restoreLastSessionState(); }}
              title="Restore the latest saved application state"
            >
              Restore Session
            </button>
          )}
          {!aiAccessReady && llmSettings && (
            <button
              className="btn btn-ai-setup"
              onClick={() => openAiSetup("recommended")}
              title="Choose Codex CLI, Claude Code, or a direct API key"
            >
              Setup AI
            </button>
          )}
          {/* Settings gear icon */}
          <button
            className="btn btn-settings"
            onClick={() => setSettingsOpen(!settingsOpen)}
            title="Settings"
          >
            &#9881;
          </button>
          {/* Branch status indicator */}
          {repoInfo && (
            <div className="repo-status">
              {repoInfo.current_branch && (
                <span className="current-branch" title="Current branch">
                  <span className="branch-icon">&#9741;</span>
                  {repoInfo.current_branch}
                </span>
              )}
              {statusText && (
                <span className="push-status" title="Tracking status">
                  {statusText}
                </span>
              )}
              {/* Worktree indicator */}
              {repoInfo.worktrees.length > 1 && (
                <span className="worktree-badge" title={`${repoInfo.worktrees.length} worktrees`}>
                  {repoInfo.worktrees.length} worktrees
                </span>
              )}
            </div>
          )}
          {analysis && (
            <span className="summary">
              {analysis.summary.total_files_changed} files,{" "}
              {analysis.summary.total_groups} groups
              {reviewedGroupIds.size > 0 && (
                <span className="reviewed-counter">
                  {" "}&middot; {reviewedGroupIds.size}/{sortedGroups.length} reviewed
                </span>
              )}
            </span>
          )}
        </div>
      </header>

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

      {/* Settings Panel Overlay */}
      {settingsOpen && llmSettings && (
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
                        saveLlmSettings({ ...llmSettings, include_uncommitted: val });
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
                  onClick={() => fetchModelsForProvider(llmSettings.provider, true)}
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
                            handleSaveApiKey();
                          }
                        }}
                      />
                      <button
                        className="btn btn-save-key"
                        disabled={!apiKeyInput.trim()}
                        onClick={handleSaveApiKey}
                        title="Save API key to ~/.diffcore/config.toml"
                      >
                        Save
                      </button>
                      {llmSettings.api_key_source === "~/.diffcore/config.toml" && (
                        <button
                          className="btn btn-clear-key"
                          onClick={handleClearApiKey}
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
                      <button className="btn" onClick={activatePreferredActivityProvider}>
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
                          onClick={() => handleRemoveIgnorePath(pattern)}
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
                        handleAddIgnorePath();
                      }
                    }}
                  />
                  <button
                    className="btn btn-save-key"
                    disabled={!ignorePathInput.trim()}
                    onClick={handleAddIgnorePath}
                  >
                    Add
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Error display */}
      {error && (
        <div className="error-bar">
          <span>{error}</span>
          <button className="btn-close" onClick={() => setError(null)}>
            &times;
          </button>
        </div>
      )}

      {/* Three-panel layout */}
      <div className="panels">
        {/* Left panel: Flow Groups */}
        <aside className="panel panel-left">
          <div className="panel-header">
            <span>Flow Groups</span>
            {comments.length > 0 && (
              <button
                className="btn btn-copy-comments"
                onClick={exportComments}
                title="Copy all comments to clipboard (Shift+C)"
              >
                Copy Comments ({comments.length})
              </button>
            )}
            {showRefined && refinementProvider && (
              <span className="refined-badge" title={`Refined by ${refinementProvider}/${refinementModel}`}>
                Refined by {refinementModel}
              </span>
            )}
          </div>
          <div className="panel-body">
            {/* Refinement banner — shown after analysis when LLM access is available */}
            {analysis && !refinedGroups && !refining && aiAccessReady && (
              <div className="refinement-banner">
                <span>AI can improve these groupings</span>
                <button
                  className="btn btn-refine"
                  onClick={runRefinement}
                  title={`Refine groupings using ${resolvedRefinementProvider ?? "anthropic"} (${resolvedRefinementModel ?? "default"})`}
                >
                  Refine
                </button>
              </div>
            )}

            {/* Manifest export & watch — allows CLI/agent refinement loop */}
            {analysis && IS_TAURI && !watchedManifestPath && (
              <div className="refinement-banner">
                <span>Edit groups via CLI</span>
                <button
                  className="btn btn-refine"
                  onClick={async () => {
                    // Build prompt and copy FIRST (synchronous relative to user gesture)
                    // so clipboard access isn't lost after awaits
                    const defaultPath = repoPath
                      ? `${repoPath}/.diffcore/groups.json`
                      : "groups.json";
                    const prompt = buildManifestAgentPrompt(defaultPath);
                    const clipboardOk = await navigator.clipboard.writeText(prompt).then(() => true).catch(() => false);

                    const path = await exportGroupsManifest();
                    if (path) {
                      // If the actual path differs from default, re-copy with correct path
                      if (path !== defaultPath) {
                        const correctedPrompt = buildManifestAgentPrompt(path);
                        navigator.clipboard.writeText(correctedPrompt).catch(() => {});
                      }
                      await tauriInvoke("watch_manifest", { manifestPath: path }).catch(() => {});
                      setWatchedManifestPath(path);
                      showToast(clipboardOk
                        ? "Watching manifest — agent prompt copied to clipboard"
                        : `Watching ${path} for changes`
                      );
                    }
                  }}
                  title="Export groups as JSON manifest and watch for changes"
                >
                  Export &amp; Watch
                </button>
              </div>
            )}
            {watchedManifestPath && (
              <div className="manifest-watch-banner">
                <div className="manifest-watch-banner-header">
                  <span className="manifest-watch-indicator">Live</span>
                  <span>Agent prompt copied to clipboard</span>
                </div>
                <p className="manifest-watch-hint">
                  Paste into Claude Code or your terminal agent to start refining groups. The UI updates in real-time.
                </p>
                <button
                  className="btn btn-refine"
                  style={{ alignSelf: "flex-start", fontSize: 10 }}
                  onClick={() => {
                    const prompt = buildManifestAgentPrompt(watchedManifestPath);
                    navigator.clipboard.writeText(prompt).then(() => {
                      showToast("Agent prompt copied to clipboard");
                    }).catch(() => {});
                  }}
                >
                  Copy prompt again
                </button>
              </div>
            )}

            {/* Refinement loading state */}
            {refining && (
              <div className="refinement-loading">
                <span className="refine-spinner" />
                <span>
                  Refining with {resolvedRefinementProvider ?? "anthropic"}/{resolvedRefinementModel ?? "..."}
                </span>
              </div>
            )}

            {/* Original/Refined toggle — shown when refined groups exist */}
            {refinedGroups && originalGroups && (
              <div className="refinement-toggle">
                <button
                  className={`toggle-btn ${!showRefined ? "active" : ""}`}
                  onClick={() => toggleRefinedView(false)}
                >
                  Original
                </button>
                <button
                  className={`toggle-btn ${showRefined ? "active" : ""}`}
                  onClick={() => toggleRefinedView(true)}
                >
                  Refined
                </button>
              </div>
            )}

            <div className="refinement-banner" style={{ flexDirection: "column", alignItems: "stretch", gap: 8 }}>
              <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                <span>Cross-file search</span>
                <button
                  className="btn btn-refine"
                  style={{ fontSize: 10 }}
                  onClick={() => setCrossFileSearchOpen((v) => !v)}
                  title="Toggle cross-file search (F)"
                >
                  {crossFileSearchOpen ? "Hide" : "Show"}
                </button>
              </div>
              {crossFileSearchOpen && (
                <>
                  <input
                    ref={crossFileSearchInputRef}
                    className="input"
                    placeholder="Search across files..."
                    value={crossFileSearchQuery}
                    onChange={(e) => setCrossFileSearchQuery(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") {
                        e.preventDefault();
                        void runCrossFileSearch();
                      }
                      if (e.key === "Escape") {
                        e.preventDefault();
                        setCrossFileSearchOpen(false);
                      }
                    }}
                    style={{ width: "100%" }}
                  />
                  <label className="settings-toggle" style={{ display: "flex", alignItems: "center", gap: 8 }}>
                    <input
                      type="checkbox"
                      checked={showUnchangedFiles}
                      onChange={(e) => setShowUnchangedFiles(e.target.checked)}
                    />
                    <span>show unchanged files</span>
                  </label>
                  <div className="comment-input-footer" style={{ justifyContent: "space-between" }}>
                    <span className="comment-input-hint">
                      {showUnchangedFiles ? "Searching changed + unchanged files" : "Searching changed files only"}
                    </span>
                    <button className="btn btn-comment-save" onClick={() => { void runCrossFileSearch(); }}>
                      {crossFileSearchLoading ? "Searching..." : "Search"}
                    </button>
                  </div>
                  {crossFileSearchError && (
                    <div className="error-banner">{crossFileSearchError}</div>
                  )}
                  <div style={{ maxHeight: 220, overflow: "auto" }}>
                    {crossFileSearchResults.length === 0 && !crossFileSearchLoading && crossFileSearchQuery.trim() && !crossFileSearchError && (
                      <div className="comment-input-hint">No matches</div>
                    )}
                    {crossFileSearchResults.slice(0, 60).map((result) => (
                      <div key={result.file_path} style={{ marginBottom: 8 }}>
                        <div style={{ fontWeight: 600, fontSize: 12, opacity: 0.9 }}>{shortPath(result.file_path)}</div>
                        {result.matches.slice(0, 4).map((m) => (
                          <button
                            key={`${result.file_path}:${m.line_number}:${m.line_text}`}
                            className="comment-strip-item"
                            style={{ width: "100%", textAlign: "left", marginTop: 4 }}
                            onClick={() => {
                              void openCrossFileSearchResult(result.file_path, m.line_number);
                            }}
                          >
                            <strong>{m.line_number}</strong>: {truncateSearchResultLine(m.line_text)}
                          </button>
                        ))}
                      </div>
                    ))}
                  </div>
                </>
              )}
            </div>

            <div
              className={`group-list group-list-${groupListTransitionState}`}
              data-testid="group-list"
              data-group-list-transition={groupListTransitionState}
            >
              {sortedGroups.map((group) => {
                const changeIndicator = showRefined
                  ? getGroupChangeIndicator(group, refinementResponse)
                  : null;

                return (
                  <div
                    key={group.id}
                    className={`group-item ${selectedGroup?.id === group.id ? "selected" : ""} ${changeIndicator ? "refined-change" : ""} ${reviewedGroupIds.has(group.id) ? "group-reviewed" : ""}`}
                    onClick={() => handleSelectGroup(group)}
                  >
                    <div className="group-header">
                      <span
                        className={`group-review-check ${reviewedGroupIds.has(group.id) ? "checked" : ""}`}
                        title={reviewedGroupIds.has(group.id) ? "Mark as unreviewed" : "Mark as reviewed"}
                        onClick={(e) => {
                          e.stopPropagation();
                          toggleGroupReviewed(group.id);
                        }}
                      >
                        {reviewedGroupIds.has(group.id) ? "\u2713" : ""}
                      </span>
                      <span className="group-name">{group.name}</span>
                      <button
                        className="copy-flow-btn"
                        title="Copy all file paths in this flow"
                        onClick={(e) => {
                          e.stopPropagation();
                          copyFlowPaths(group);
                        }}
                      >
                        &#128203;
                      </button>
                      {commentCountForGroup(group.id) > 0 && (
                        <span className="comment-count-badge" title={`${commentCountForGroup(group.id)} comment${commentCountForGroup(group.id) === 1 ? "" : "s"}`}>
                          {commentCountForGroup(group.id)}
                        </span>
                      )}
                      <span className="risk-badge" data-risk={riskLevel(group.risk_score)}>
                        {group.risk_score.toFixed(2)}
                      </span>
                    </div>
                    {changeIndicator && (
                      <div className="change-indicator" title={changeIndicator.reason}>
                        <span className={`change-tag change-${changeIndicator.type}`}>
                          {changeIndicator.label}
                        </span>
                      </div>
                    )}
                    {selectedGroup?.id === group.id && (
                      <ul className={`file-list ${reviewedGroupIds.has(group.id) ? "file-list-collapsed" : ""}`}>
                        {group.files.map((file) => {
                          const fileMoved = showRefined
                            ? getFileMovedIndicator(file.path, refinementResponse)
                            : null;
                          const fileCommentCount = commentsForFile(file.path).length;
                          const status = resolveFileShortStatus(file.path, fileStatusByPath, file.changes.additions, file.changes.deletions);

                          return (
                            <li
                              key={file.path}
                              className={`file-item file-item-two-line ${selectedFile === file.path ? "selected" : ""} ${fileMoved ? "file-moved" : ""}`}
                              onClick={(e) => {
                                e.stopPropagation();
                                openFileInTab(file.path, group.id);
                              }}
                              onContextMenu={(e) => handleFileContextMenu(e, file.path)}
                            >
                              <FileDisplay
                                path={file.path}
                                gitStatus={status}
                                roleBadge={file.role}
                                additions={file.changes.additions}
                                deletions={file.changes.deletions}
                                reviewedInReplay={replayActive && replayVisited.has(file.path)}
                                variant="two-line"
                                movedFrom={fileMoved?.from}
                                prefix={fileCommentCount > 0 ? (
                                  <button
                                    className="file-comment-btn"
                                    title={`${fileCommentCount} comment${fileCommentCount === 1 ? "" : "s"} — click to view`}
                                    onClick={(e) => {
                                      e.stopPropagation();
                                      openFileInTab(file.path, group.id);
                                      setRightPanelTab("comments");
                                      if (rightPanelCollapsed) setRightPanelCollapsed(false);
                                      const fc = commentsForFile(file.path);
                                      const first = fc.find((c) => c.start_line != null);
                                      if (first) {
                                        pendingScrollToCommentRef.current = {
                                          startLine: first.start_line!,
                                          endLine: first.end_line ?? undefined,
                                          commentId: first.id,
                                        };
                                        setActiveCommentId(first.id);
                                      }
                                    }}
                                  >
                                    <span className="file-comment-icon">&#128172;</span>
                                    <span className="file-comment-count">{fileCommentCount}</span>
                                  </button>
                                ) : null}
                              />
                            </li>
                          );
                        })}
                      </ul>
                    )}
                  </div>
                );
              })}
              {/* Infrastructure group — collapsed by default, shows count, with sub-groups */}
              {analysis?.infrastructure_group && analysis.infrastructure_group.files.length > 0 && (() => {
                // Build path → FileChange lookup from the enriched file_changes list
                // so sub-group and flat-list renderers can pass real stats.
                const infraChangesMap = new Map(
                  (analysis.infrastructure_group.file_changes ?? []).map((fc) => [fc.path, fc]),
                );
                return (
                <div className="group-item infra-group">
                  <div
                    className="group-header"
                    style={{ cursor: "pointer" }}
                    onClick={() => setInfraExpanded((prev) => !prev)}
                  >
                    <span className="group-name">
                      Ungrouped
                    </span>
                    <span className="risk-badge" data-risk="low">
                      {analysis.infrastructure_group.files.length} files
                    </span>
                    <span style={{ marginLeft: 4, fontSize: 10, opacity: 0.6 }}>
                      {infraExpanded ? "\u25B2" : "\u25BC"}
                    </span>
                  </div>
                  {infraExpanded && (
                    <>
                      {analysis.infrastructure_group.sub_groups && analysis.infrastructure_group.sub_groups.length > 0 ? (
                        analysis.infrastructure_group.sub_groups.map((sg: InfraSubGroup) => {
                          const isSubExpanded = infraSubGroupsExpanded.has(sg.name);
                          return (
                            <div key={sg.name} className="infra-sub-group">
                              <div
                                className="infra-sub-group-header"
                                onClick={(e) => {
                                  e.stopPropagation();
                                  setInfraSubGroupsExpanded((prev) => {
                                    const next = new Set(prev);
                                    if (next.has(sg.name)) {
                                      next.delete(sg.name);
                                    } else {
                                      next.add(sg.name);
                                    }
                                    return next;
                                  });
                                }}
                              >
                                <span style={{ fontSize: 10, opacity: 0.6, marginRight: 4 }}>
                                  {isSubExpanded ? "\u25BC" : "\u25B6"}
                                </span>
                                <span className="infra-sub-group-name">{sg.name}</span>
                                <span className="infra-sub-group-count">
                                  {sg.files.length} file{sg.files.length === 1 ? "" : "s"}
                                </span>
                              </div>
                              {isSubExpanded && (
                                <ul className="file-list">
                                  {sg.files.map((f) => {
                                    // Prefer per-file stats from the sub-group's file_changes;
                                    // fall back to the top-level infraChangesMap.
                                    const fc = sg.file_changes?.find((c) => c.path === f)
                                      ?? infraChangesMap.get(f);
                                    const status = resolveFileShortStatus(
                                      f, fileStatusByPath,
                                      fc?.changes.additions ?? 1,
                                      fc?.changes.deletions ?? 1,
                                    );
                                    return (
                                    <li
                                      key={f}
                                      className={`file-item ${selectedFile === f ? "selected" : ""}`}
                                      onClick={(e) => {
                                        e.stopPropagation();
                                        openFileInTab(f, "infra");
                                      }}
                                    >
                                      <FileDisplay
                                        path={f}
                                        gitStatus={status}
                                        additions={fc?.changes.additions}
                                        deletions={fc?.changes.deletions}
                                        hideChanges
                                      />
                                    </li>
                                    );
                                  })}
                                </ul>
                              )}
                            </div>
                          );
                        })
                      ) : (
                        <ul className="file-list">
                          {(infraShowAll
                            ? analysis.infrastructure_group.files
                            : analysis.infrastructure_group.files.slice(0, 50)
                          ).map((f) => {
                            const fc = infraChangesMap.get(f);
                            const status = resolveFileShortStatus(
                              f, fileStatusByPath,
                              fc?.changes.additions ?? 1,
                              fc?.changes.deletions ?? 1,
                            );
                            return (
                            <li
                              key={f}
                              className={`file-item ${selectedFile === f ? "selected" : ""}`}
                              onClick={(e) => {
                                e.stopPropagation();
                                openFileInTab(f, "infra");
                              }}
                            >
                              <FileDisplay
                                path={f}
                                gitStatus={status}
                                additions={fc?.changes.additions}
                                deletions={fc?.changes.deletions}
                                hideChanges
                              />
                            </li>
                            );
                          })}
                          {!infraShowAll && analysis.infrastructure_group.files.length > 50 && (
                            <li
                              className="file-item"
                              style={{ opacity: 0.7, cursor: "pointer", textAlign: "center" }}
                              onClick={(e) => { e.stopPropagation(); setInfraShowAll(true); }}
                            >
                              Show all {analysis.infrastructure_group.files.length} files...
                            </li>
                          )}
                        </ul>
                      )}
                    </>
                  )}
                </div>
                );
              })()}
            </div>
            {loading && (
              <div className="empty-state loading-state">
                <span className="spinner" />
                Analyzing repository...
              </div>
            )}
            {!analysis && !loading && (
              <div className="empty-state">
                Enter a repository path and click Analyze to start.
              </div>
            )}
          </div>
          {/* Sticky footer bar — always visible when comments exist */}
          {comments.length > 0 && (
            <div className="panel-footer">
              <span className="panel-footer-count">{comments.length} comment{comments.length === 1 ? "" : "s"}</span>
              <button
                className="btn btn-copy-comments-footer"
                onClick={exportComments}
                title="Copy all comments to clipboard (Shift+C)"
              >
                Copy All Comments
              </button>
              <span className="panel-footer-hint">Shift+C</span>
            </div>
          )}
        </aside>

        {/* Center panel: Monaco Diff Viewer */}
        <main className="panel panel-center">
          <div className="panel-header">
            <span className={fileDiff ? "panel-header-filepath" : "panel-header-title"} title={fileDiff?.path}>{fileDiff ? fileDiff.path : "Diff Viewer"}</span>
            {fileDiff && (
              <div className="editor-toolbar diff-toolbar" ref={openWithRef}>
                <button
                  className="editor-btn open-with-btn"
                  onClick={() => openInEditor(lastEditor)}
                  title={`Open in ${editorOptions.find((e) => e.id === lastEditor)?.label ?? lastEditor}`}
                >
                  Open With
                </button>
                <button
                  className="editor-btn open-with-arrow"
                  onClick={() => setOpenWithDropdown(!openWithDropdown)}
                  aria-label="Choose editor"
                >
                  {openWithDropdown ? "\u25B2" : "\u25BC"}
                </button>
                {openWithDropdown && (
                  <div className="open-with-dropdown">
                    {editorOptions.map((opt) => (
                      <button
                        key={opt.id}
                        className={`open-with-option ${opt.id === lastEditor ? "active" : ""}`}
                        onClick={() => openInEditor(opt.id)}
                      >
                        <span
                          className="editor-icon"
                          dangerouslySetInnerHTML={{ __html: editorIcons[opt.id] }}
                        />
                        {opt.label}
                      </button>
                    ))}
                  </div>
                )}

                {/* Replay + hunk controls. Always rendered to keep the toolbar
                    layout stable; replay-only controls are disabled when
                    replay is inactive. Hunk navigation and "+ Hunk Comment"
                    work any time a diff is open because `currentReplayHunks`
                    is populated by `onDiffHunksChange` regardless of replay
                    state. */}
                <span className="diff-toolbar-divider" aria-hidden="true" />

                {replayActive && selectedGroup && (
                  <span className="replay-badge">REPLAY</span>
                )}
                {replayActive && selectedGroup && (
                  <span className="replay-step-label">
                    Step {replayStep + 1} of {selectedGroup.files.length}
                  </span>
                )}
                {replayActive && selectedGroup && selectedGroup.files[replayStep] && (
                  <span className="replay-file-role">
                    {selectedGroup.files[replayStep].role}
                  </span>
                )}

                {selectedGroup && selectedGroup.files.length > 0 && (
                  <div className="replay-progress" data-disabled={!replayActive || undefined}>
                    {selectedGroup.files.map((f, i) => (
                      <button
                        key={f.path}
                        type="button"
                        className={`replay-dot ${i === replayStep && replayActive ? "active" : ""} ${replayVisited.has(f.path) ? "visited" : ""}`}
                        onClick={() => goToReplayStep(i)}
                        disabled={!replayActive}
                        title={replayActive ? shortPath(f.path) : "Replay-only navigation"}
                      />
                    ))}
                  </div>
                )}

                <span className="replay-hunk-status">
                  Hunk {replayHunks.length === 0 ? 0 : Math.min(replayHunkIndex + 1, replayHunks.length)}/{replayHunks.length}
                  {replayActive && replayHunks[replayHunkIndex] && replayViewedHunkIds.has(replayHunks[replayHunkIndex].id) ? " viewed" : ""}
                </span>

                <button
                  type="button"
                  className="btn replay-btn replay-comment-btn"
                  onClick={commentOnCurrentReplayHunk}
                  disabled={replayHunks.length === 0}
                  title={replayHunks.length === 0 ? "No hunks in this file" : "Comment on current hunk"}
                >
                  + Hunk Comment
                </button>
                <button
                  type="button"
                  className="btn replay-btn replay-hunk-btn"
                  onClick={() => navigateReplayHunk(-1)}
                  disabled={!hasPrevReplayHunk}
                  title="Jump to previous hunk"
                >
                  &#9664;&nbsp;Hunk
                </button>
                <button
                  type="button"
                  className="btn replay-btn replay-hunk-btn"
                  onClick={() => navigateReplayHunk(1)}
                  disabled={!hasNextReplayHunk}
                  title="Jump to next hunk"
                >
                  Hunk&nbsp;&#9654;
                </button>
                <button
                  type="button"
                  className="btn replay-btn"
                  onClick={() => goToReplayStep(replayStep - 1)}
                  disabled={!replayActive || replayStep === 0}
                  title={replayActive ? "Previous file (p / Left Arrow)" : "Replay-only navigation"}
                >
                  &#9664;&nbsp;File
                </button>
                <button
                  type="button"
                  className="btn replay-btn"
                  onClick={() => goToReplayStep(replayStep + 1)}
                  disabled={!replayActive || !selectedGroup || replayStep >= selectedGroup.files.length - 1}
                  title={replayActive ? "Next file (n / Right Arrow / Space)" : "Replay-only navigation"}
                >
                  File&nbsp;&#9654;
                </button>
                <button
                  type="button"
                  className="btn replay-btn replay-exit"
                  onClick={exitReplay}
                  disabled={!replayActive}
                  title={replayActive ? "Exit replay (Esc)" : "Replay-only"}
                >
                  &#10005;
                </button>
              </div>
            )}
          </div>
          {/* File tab bar */}
          {openTabs.length > 0 && (
            <div className="tab-bar" role="tablist">
              {openTabs.map((tab) => {
                const basename = tab.path.split("/").pop() || tab.path;
                const hasDuplicate = openTabs.filter((t) => (t.path.split("/").pop() || t.path) === basename).length > 1;
                const displayName = hasDuplicate ? shortPath(tab.path) : basename;
                return (
                  <div
                    key={tab.path}
                    className={`file-tab ${selectedFile === tab.path ? "active" : ""}`}
                    onClick={() => handleSelectFile(tab.path)}
                    onAuxClick={(e) => { if (e.button === 1) { e.preventDefault(); closeTab(tab.path); } }}
                    onContextMenu={(e) => {
                      e.preventDefault();
                      setTabContextMenu({ x: e.clientX, y: e.clientY, tabPath: tab.path });
                    }}
                    title={tab.path}
                    role="tab"
                    aria-selected={selectedFile === tab.path}
                  >
                    <span className="file-tab-name">{displayName}</span>
                    <button
                      className="file-tab-close"
                      onClick={(e) => { e.stopPropagation(); closeTab(tab.path); }}
                      aria-label={`Close ${basename}`}
                    >
                      &times;
                    </button>
                  </div>
                );
              })}
            </div>
          )}
          {/* Replay bar — controls were merged into the diff toolbar above
              (`.editor-toolbar.diff-toolbar`) so they remain visible (and
              partially enabled) at all times, not just during replay. */}
          <div className="panel-body diff-viewer">
            <ErrorBoundary panelName="Diff Viewer">
              <CrashTest panel="Diff Viewer" />
              <DiffViewer
                ref={diffViewerRef}
                fileDiff={fileDiff}
                editable={editsEnabled}
                renderSideBySide={shouldRenderSideBySide}
                onCommentRequest={(startLine: number, endLine: number, selectedCode: string) => {
                  const group = selectedGroupRef.current;
                  const file = selectedFileRef.current;
                  if (group && file) {
                    openCommentInput({
                      type: "code",
                      group_id: group.id,
                      file_path: file,
                      start_line: startLine,
                      end_line: endLine,
                      selected_code: selectedCode,
                    });
                  }
                }}
                codeComments={codeCommentsForSelectedFile}
                onGlyphClick={(commentId) => {
                  setActiveCommentId(commentId);
                  setRightPanelTab("comments");
                  if (rightPanelCollapsed) setRightPanelCollapsed(false);
                  // Scroll the comment card into view in the comments tab
                  setTimeout(() => {
                    const el = document.querySelector(`.comments-tab-card[data-comment-id="${commentId}"]`);
                    el?.scrollIntoView({ behavior: "smooth", block: "nearest" });
                  }, 100);
                }}
                onGoToDefinition={handleGoToDefinition}
                onEditedContentChange={(newContent: string, _hunks: EditedHunk[]) => {
                  const file = selectedFileRef.current;
                  const group = selectedGroupRef.current;
                  if (!file || !group) return;

                  setFileDiff((prev) => {
                    if (!prev || prev.path !== file) return prev;
                    return { ...prev, new_content: newContent };
                  });

                  const baseline = fileEditBaselineRef.current.get(file) ?? "";
                  const toolEditHunks = computeToolEditHunks(baseline, newContent);
                  const hasExistingAutoComments = comments.some(
                    (c) => c.type === "code" && c.file_path === file && c.id.startsWith("auto_edit_hunk_"),
                  );

                  // Ignore mount/view noise: only sync when there are true tool edits
                  // or when existing auto-comments need cleanup after revert.
                  if (toolEditHunks.length === 0 && !hasExistingAutoComments && newContent === baseline) {
                    return;
                  }

                  latestEditPayloadRef.current = {
                    filePath: file,
                    groupId: group.id,
                    newContent,
                    hunks: toolEditHunks,
                  };

                  if (pendingEditSync.current) {
                    clearTimeout(pendingEditSync.current);
                  }
                  pendingEditSync.current = setTimeout(() => {
                    const payload = latestEditPayloadRef.current;
                    if (!payload) return;
                    void syncEditedFileAndComments(
                      payload.filePath,
                      payload.groupId,
                      payload.newContent,
                      payload.hunks,
                    );
                  }, 2000);
                }}
                onDiffHunksChange={(hunks: EditedHunk[]) => {
                  const file = selectedFileRef.current;
                  if (!file) return;
                  const visible = mapEditedHunksToReplayHunks(file, hunks);
                  setCurrentReplayHunks(visible);

                  const pending = pendingReplayHunkScrollRef.current;
                  if (pending && pending.filePath === file && visible.length > 0) {
                    const fallback = Math.max(0, Math.min(pending.targetHunkIndex, visible.length - 1));
                    const chosen = pending.targetHunkIndex < 0 ? visible.length - 1 : fallback;
                    const hunk = visible[Math.max(0, chosen)];
                    pendingReplayHunkScrollRef.current = null;
                    setReplayHunkIndex(Math.max(0, chosen));
                    if (hunk) {
                      diffViewerRef.current?.scrollToHunk?.({
                        originalStartLine: hunk.originalStartLine,
                        originalEndLine: hunk.originalEndLine,
                        modifiedStartLine: hunk.startLine,
                        modifiedEndLine: hunk.endLine,
                        selectedCode: hunk.selectedCode ?? "",
                        isDeletionOnly: hunk.isDeletionOnly,
                      });
                    }
                  }
                }}
              />
            </ErrorBoundary>
          </div>
          {/* Comments moved to right panel tab */}
          {false && selectedFile && (() => {
            const fileComments = comments.filter(
              (c) => c.file_path === selectedFile && selectedGroup && c.group_id === selectedGroup.id,
            );
            if (fileComments.length === 0) return null;
            return (
              <div className={`comment-strip ${commentsCollapsed ? "comment-strip-collapsed" : ""}`}>
                {/* Header bar with "Comments" label and collapse toggle */}
                <div className="comment-strip-header">
                  <button
                    className="comment-strip-toggle"
                    onClick={() => setCommentsCollapsed(!commentsCollapsed)}
                    aria-expanded={!commentsCollapsed}
                  >
                    <span className="section-toggle-icon">{commentsCollapsed ? "\u25B6" : "\u25BC"}</span>
                    <span>Comments</span>
                    <span className="comment-strip-count">{fileComments.length}</span>
                  </button>
                </div>
                {/* Collapsible body */}
                <div className="comment-strip-body">
                  {/* Left nav — compact pill list for quick toggling */}
                  <div className="comment-strip-nav">
                    {fileComments.map((comment, i) => (
                      <button
                        key={comment.id}
                        className={`comment-strip-nav-item comment-strip-nav-${comment.type} ${activeCommentId === comment.id ? "comment-strip-nav-active" : ""}`}
                        onClick={() => {
                          setActiveCommentId(comment.id);
                          if (comment.start_line != null) {
                            diffViewerRef.current?.scrollToLine(comment.start_line, comment.end_line ?? undefined);
                          }
                          // Scroll corresponding card into view
                          setTimeout(() => {
                            const el = document.querySelector(`.comment-strip-item[data-comment-id="${comment.id}"]`);
                            el?.scrollIntoView({ behavior: "smooth", block: "nearest" });
                          }, 50);
                        }}
                        title={comment.text.slice(0, 60) + (comment.text.length > 60 ? "..." : "")}
                      >
                        <span className="comment-strip-nav-num">{i + 1}</span>
                        {comment.start_line != null && (
                          <span className="comment-strip-nav-line">L{comment.start_line}</span>
                        )}
                      </button>
                    ))}
                  </div>
                  {/* Right detail — scrollable comment cards */}
                  <div className="comment-strip-detail">
                    {fileComments.map((comment) => (
                      <div
                        key={comment.id}
                        data-comment-id={comment.id}
                        className={`comment-strip-item ${activeCommentId === comment.id ? "comment-strip-item-active" : ""}`}
                        onClick={() => {
                          setActiveCommentId(comment.id);
                          if (comment.start_line != null) {
                            diffViewerRef.current?.scrollToLine(comment.start_line, comment.end_line ?? undefined);
                          }
                        }}
                        role={comment.start_line != null ? "button" : undefined}
                        tabIndex={comment.start_line != null ? 0 : undefined}
                      >
                        <div className="comment-strip-meta">
                          <span className={`comment-strip-badge comment-strip-badge-${comment.type}`}>{comment.type}</span>
                          {comment.file_path && (
                            <span className="comment-strip-filepath">{shortPath(comment.file_path)}</span>
                          )}
                          {comment.start_line != null && comment.end_line != null && (
                            <span className="comment-strip-lines">:{comment.start_line}-{comment.end_line}</span>
                          )}
                          <button
                            className="comment-strip-edit"
                            onClick={(e) => {
                              e.stopPropagation();
                              setEditingCommentId(comment.id);
                              setEditingCommentText(comment.text);
                            }}
                            title="Edit comment"
                          >
                            &#9998;
                          </button>
                          <button
                            className="comment-strip-delete"
                            onClick={(e) => { e.stopPropagation(); deleteComment(comment.id); }}
                            title="Delete comment"
                          >
                            &times;
                          </button>
                        </div>
                        {comment.selected_code && (
                          <pre className="comment-strip-code">{comment.selected_code}</pre>
                        )}
                        {editingCommentId === comment.id ? (
                          <div className="comment-strip-edit-container" onClick={(e) => e.stopPropagation()}>
                            <textarea
                              className="comment-strip-edit-textarea"
                              value={editingCommentText}
                              onChange={(e) => setEditingCommentText(e.target.value)}
                              autoFocus
                              rows={3}
                              onKeyDown={(e) => {
                                if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
                                  e.preventDefault();
                                  if (editingCommentText.trim()) {
                                    updateComment(comment.id, editingCommentText.trim());
                                  }
                                }
                                if (e.key === "Escape") {
                                  e.preventDefault();
                                  setEditingCommentId(null);
                                }
                              }}
                            />
                            <div className="comment-strip-edit-actions">
                              <span className="comment-strip-edit-hint">Cmd+Enter to save, Escape to cancel</span>
                              <button
                                className="btn btn-comment-save"
                                disabled={!editingCommentText.trim()}
                                onClick={() => updateComment(comment.id, editingCommentText.trim())}
                              >
                                Save
                              </button>
                              <button
                                className="btn btn-comment-cancel"
                                onClick={() => setEditingCommentId(null)}
                              >
                                Cancel
                              </button>
                            </div>
                          </div>
                        ) : (
                          <p
                            className="comment-strip-text"
                            onDoubleClick={(e) => {
                              e.stopPropagation();
                              setEditingCommentId(comment.id);
                              setEditingCommentText(comment.text);
                            }}
                            title="Double-click to edit"
                          >
                            {comment.text}
                          </p>
                        )}
                      </div>
                    ))}
                  </div>
                </div>
              </div>
            );
          })()}
        </main>

        {/* Right panel drag handle */}
        <div
          className="panel-resize-handle"
          onMouseDown={startRightPanelDrag}
        />

        {/* Right panel: Activity + Annotations */}
        <aside
          className={`panel panel-right ${rightPanelCollapsed ? "panel-right-collapsed" : ""}`}
          style={rightPanelCollapsed ? undefined : { width: rightPanelWidth }}
        >
          <div className="panel-header panel-header-tabs" data-testid="right-panel-tabs" role="tablist" aria-label="Right panel views">
            <button
              className="panel-collapse-btn"
              onClick={() => setRightPanelCollapsed(!rightPanelCollapsed)}
              title={rightPanelCollapsed ? "Expand panel" : "Collapse panel"}
            >
              {rightPanelCollapsed ? "\u25C0" : "\u25B6"}
            </button>
            {!rightPanelCollapsed && (
              <>
                <button
                  className={`panel-tab ${rightPanelTab === "activity" ? "active" : ""}`}
                  onClick={() => setRightPanelTab("activity")}
                  data-testid="activity-tab"
                  role="tab"
                  aria-selected={rightPanelTab === "activity"}
                  title="LLM"
                >
                  <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                    <polyline points="1,8 4,8 6,3 8,13 10,6 12,8 15,8" />
                  </svg>
                  {(activityJob || activityTimeline.length > 0) && (
                    <span className="panel-tab-count">{activityTimeline.length || 1}</span>
                  )}
                </button>
                <button
                  className={`panel-tab ${rightPanelTab === "comments" ? "active" : ""}`}
                  onClick={() => setRightPanelTab("comments")}
                  data-testid="comments-tab"
                  role="tab"
                  aria-selected={rightPanelTab === "comments"}
                  title="Comments"
                >
                  <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                    <path d="M2,2 h12 a1,1 0 0 1 1,1 v7 a1,1 0 0 1 -1,1 h-7 l-3,3 v-3 h-2 a1,1 0 0 1 -1,-1 v-7 a1,1 0 0 1 1,-1 z" />
                  </svg>
                  {comments.length > 0 && (
                    <span className="panel-tab-count">{comments.length}</span>
                  )}
                </button>
                <button
                  className={`panel-tab ${rightPanelTab === "annotations" ? "active" : ""}`}
                  onClick={() => setRightPanelTab("annotations")}
                  data-testid="annotations-tab"
                  role="tab"
                  aria-selected={rightPanelTab === "annotations"}
                  title="Info"
                >
                  <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                    <rect x="2" y="1" width="12" height="14" rx="1.5" />
                    <line x1="5" y1="5" x2="11" y2="5" />
                    <line x1="5" y1="8" x2="11" y2="8" />
                    <line x1="5" y1="11" x2="9" y2="11" />
                  </svg>
                </button>
                <button
                  className={`panel-tab ${rightPanelTab === "source" ? "active" : ""}`}
                  onClick={() => setRightPanelTab("source")}
                  data-testid="source-tab"
                  role="tab"
                  aria-selected={rightPanelTab === "source"}
                  title="Subsystem Source"
                >
                  <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                    <polyline points="5,3 1,8 5,13" />
                    <polyline points="11,3 15,8 11,13" />
                    <line x1="9" y1="2" x2="7" y2="14" />
                  </svg>
                </button>
              </>
            )}
          </div>
          {!rightPanelCollapsed && (
          <div className="panel-body panel-body-right">
            {rightPanelTab === "activity"
              ? activityTabContent
              : rightPanelTab === "source"
                ? sourceTabContent
                : rightPanelTab === "comments"
                  ? commentsTabContent
                  : annotationsTabContent}

            {(annotating || deepAnalyzing || refining) && rightPanelTab === "activity" && (
              <div className="annotation-section llm-loading">
                <span className="spinner" />
                {annotating
                  ? "Generating overview..."
                  : deepAnalyzing
                    ? "Analyzing flow group..."
                    : "Refining groups..."}
              </div>
            )}

            {!aiAccessReady && llmSettings && (
              <div className="annotation-section llm-loading llm-setup-cta">
                <span>Connect {PROVIDER_LABELS.codex}, {PROVIDER_LABELS.claude}, or a direct API key to unlock summaries and refinement.</span>
                <button className="btn" onClick={() => openAiSetup("recommended")}>
                  Setup AI
                </button>
              </div>
            )}

            {selectedGroup && rightPanelTab === "annotations" && (
              <div className="annotation-section annotation-actions">
                {overview && !annotating && (
                  <button
                    className="btn btn-copy-comments-footer"
                    onClick={copyPrDescription}
                    title="Copy the generated summary as a PR description"
                  >
                    Copy PR Description
                  </button>
                )}
                {!overview && !annotating && !refinedGroups && (
                  <button
                    className={`btn btn-summarize ${!aiAccessReady ? "no-api-key" : ""}`}
                    onClick={() => { void runAnnotateOverview(); }}
                    disabled={annotating || !aiAccessReady || !annotationsEnabled}
                    title={
                      aiAccessReady
                        ? `Run LLM Pass 1 via ${resolvedPrimaryProvider ?? "codex"} (${resolvedPrimaryModel ?? "default"}): generate an overview summary of all flow groups.`
                        : "AI setup required — choose Codex CLI, Claude Code, or a direct API key"
                    }
                  >
                    {aiAccessReady ? "Summarize PR" : "Summarize PR (Setup required)"}
                  </button>
                )}
                {!groupDeepAnalysis && !deepAnalyzing && (
                  <button
                    className={`btn btn-analyze-flow ${!aiAccessReady ? "no-api-key" : ""}`}
                    onClick={runDeepAnalysis}
                    disabled={deepAnalyzing || !aiAccessReady || !annotationsEnabled}
                    title={
                      aiAccessReady
                        ? `Run LLM Pass 2 via ${resolvedPrimaryProvider ?? "codex"} (${resolvedPrimaryModel ?? "default"}): deep analysis of this flow group.`
                        : "AI setup required — choose Codex CLI, Claude Code, or a direct API key"
                    }
                  >
                    {aiAccessReady ? "Analyze This Flow" : "Analyze Flow (Setup required)"}
                  </button>
                )}
                {aiAccessReady && resolvedPrimaryProvider && (
                  <span className="llm-provider-badge">
                    {PROVIDER_LABELS[resolvedPrimaryProvider as LlmProvider]}/{resolvedPrimaryModel ?? "default"}
                  </span>
                )}
              </div>
            )}
          </div>
          )}
        </aside>
      </div>

      {/* Keyboard shortcuts bar */}
      {analysis && (
        <footer className="keyboard-hints">
          {replayActive ? (
            <>
              <span><kbd>n</kbd> / <kbd>&#8594;</kbd> / <kbd>Space</kbd> next step</span>
              <span><kbd>p</kbd> / <kbd>&#8592;</kbd> prev step</span>
              <span><kbd>Esc</kbd> exit replay</span>
            </>
          ) : (
            <>
              <span><kbd>j</kbd> next file</span>
              <span><kbd>k</kbd> prev file</span>
              <span><kbd>J</kbd> next group</span>
              <span><kbd>K</kbd> prev group</span>
              <span><kbd>x</kbd> mark reviewed</span>
              <span><kbd>y</kbd> copy path</span>
              <span><kbd>Y</kbd> copy flow</span>
              <span><kbd>c</kbd> comment</span>
              <span><kbd>C</kbd> copy comments</span>
              <span><kbd>r</kbd> replay flow</span>
              <span><kbd>f</kbd> search</span>
              <span><kbd>F</kbd> cross-file search</span>
            </>
          )}
        </footer>
      )}

      {/* Comment input overlay */}
      {commentInput && (
        <div className="comment-overlay" onClick={cancelComment}>
          <div className="comment-input-panel" onClick={(e) => e.stopPropagation()}>
            <div className="comment-input-header">
              <span className="comment-input-scope">
                {commentInput.type === "code" && commentInput.file_path
                  ? `${shortPath(commentInput.file_path)}:${commentInput.start_line}-${commentInput.end_line}`
                  : commentInput.type === "file" && commentInput.file_path
                    ? shortPath(commentInput.file_path)
                    : "Group comment"}
              </span>
              <button className="btn-close" onClick={cancelComment}>&times;</button>
            </div>
            {commentInput.selected_code && (
              <pre className="comment-input-code">{commentInput.selected_code}</pre>
            )}
            <textarea
              ref={commentInputRef}
              className="comment-textarea"
              placeholder="Add a review comment..."
              value={commentText}
              onChange={(e) => setCommentText(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && !e.shiftKey) {
                  e.preventDefault();
                  submitComment();
                }
                if (e.key === "Escape") {
                  e.preventDefault();
                  cancelComment();
                }
              }}
              rows={3}
            />
            <div className="comment-input-footer">
              <span className="comment-input-hint">Enter to save, Escape to cancel, Shift+Enter for newline</span>
              <button
                className="btn btn-comment-save"
                onClick={submitComment}
                disabled={!commentText.trim()}
              >
                Save
              </button>
            </div>
          </div>
        </div>
      )}

      {regenDialogOpen && (
        <div className="comment-overlay" onClick={() => setRegenDialogOpen(false)}>
          <div className="comment-input-panel" onClick={(e) => e.stopPropagation()}>
            <div className="comment-input-header">
              <span className="comment-input-scope">Regenerate with feedback/question</span>
              <button className="btn-close" onClick={() => setRegenDialogOpen(false)}>&times;</button>
            </div>
            <textarea
              className="comment-textarea"
              value={regenFeedbackText}
              onChange={(e) => setRegenFeedbackText(e.target.value)}
              placeholder="What should the next analysis focus on?"
              rows={5}
            />
            <label className="settings-toggle" style={{ display: "flex", alignItems: "center", gap: 8, marginTop: 8 }}>
              <input
                type="checkbox"
                checked={regenIncludePreviousOutput}
                onChange={(e) => setRegenIncludePreviousOutput(e.target.checked)}
              />
              <span>Include previous output as context</span>
            </label>
            <p className="settings-hint" style={{ marginTop: 8 }}>
              Your feedback and current review comments are always included in reanalysis context.
            </p>
            <div className="comment-input-footer">
              <button className="btn" onClick={() => setRegenDialogOpen(false)}>Cancel</button>
              <button
                className="btn btn-comment-save"
                onClick={() => {
                  setRegenDialogOpen(false);
                  void runAnnotateOverview({
                    feedback: regenFeedbackText,
                    includePreviousOutput: regenIncludePreviousOutput,
                  });
                }}
                disabled={annotating}
              >
                {annotating ? "Regenerating..." : "Regenerate"}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Context menu (right-click on file) */}
      {contextMenu && (
        <div
          className="context-menu"
          style={{ top: contextMenu.y, left: contextMenu.x }}
          onClick={(e) => e.stopPropagation()}
        >
          <button
            className="context-menu-item"
            onClick={() => {
              copyFilePath(contextMenu.filePath);
              setContextMenu(null);
            }}
          >
            Copy File Path
          </button>
          <button
            className="context-menu-item"
            onClick={() => {
              const group = selectedGroupRef.current;
              if (group) {
                openCommentInput({ type: "file", group_id: group.id, file_path: contextMenu.filePath });
              }
              setContextMenu(null);
            }}
          >
            Add Comment
          </button>
        </div>
      )}

      {/* Tab context menu (right-click on tab) */}
      {tabContextMenu && (
        <div
          className="context-menu"
          style={{ top: tabContextMenu.y, left: tabContextMenu.x }}
          onClick={(e) => e.stopPropagation()}
        >
          <button
            className="context-menu-item"
            onClick={() => {
              copyFilePath(tabContextMenu.tabPath);
              setTabContextMenu(null);
            }}
          >
            Copy File Path
          </button>
          <button
            className="context-menu-item"
            onClick={() => {
              closeTab(tabContextMenu.tabPath);
              setTabContextMenu(null);
            }}
          >
            Close
          </button>
          <button
            className="context-menu-item"
            onClick={() => {
              closeOtherTabs(tabContextMenu.tabPath);
              setTabContextMenu(null);
            }}
          >
            Close Others
          </button>
          <button
            className="context-menu-item"
            onClick={() => {
              closeAllTabs();
              setTabContextMenu(null);
            }}
          >
            Close All
          </button>
        </div>
      )}

      {/* Toast notification */}
      {toast && (
        <div className="toast">
          <span>{toast}</span>
          <button className="toast-close" onClick={() => setToast(null)} aria-label="Dismiss">&times;</button>
        </div>
      )}
    </div>
  );
}
