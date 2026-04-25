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
  LlmActivityEntry,
  RefinementResult,
  RefinementResponse,
  ReviewComment,
  CommentInput,
} from "./types";
import { LLM_PROVIDERS, DEFAULT_MODELS_BY_PROVIDER } from "./types";
import type { ModelInfo } from "./types";
import { type DiffViewerHandle, type EditedHunk } from "./components/DiffViewer";
import { type SourceFocusRequest } from "./components/SourceExplorer";
import { AISetupModal } from "./components/modals/AISetupModal";
import { CommentInputOverlay } from "./components/modals/CommentInputOverlay";
import { RegenDialog } from "./components/modals/RegenDialog";
import { MOCK_ANALYSIS, MOCK_DIFFS, MOCK_PASS1, MOCK_PASS2, MOCK_REPO_INFO, MOCK_LLM_SETTINGS, MOCK_REFINEMENT } from "./mock";
import { parseSymbolEndpoint, findLineContainingSymbol } from "./utils/pathUtils";
import { formatBranchStatus, formatCompareTargetLabel, COMPARE_TARGET_UNSTAGED, COMPARE_TARGET_STAGED } from "./utils/gitUtils";
import { computeToolEditHunks } from "./utils/groupUtils";
import { describeActivityEntry, summarizeActivityTimeline, providerSupportsToolActivity, buildMockActivityEntries } from "./utils/activityUtils";
import { resolveInteractiveProvider, resolveInteractiveModel, isApiProvider, PROVIDER_LABELS } from "./utils/llmUtils";
import type { SubscriptionProvider } from "./utils/llmUtils";
import { AppContext } from "./hooks/AppContext";
import { DiffContext } from "./hooks/DiffContext";
import { useEditorIntegration } from "./hooks/useEditorIntegration";
import { useActivityStream } from "./hooks/useActivityStream";
import { useManifestActions } from "./hooks/useManifestActions";
import { useCrossFileSearch } from "./hooks/useCrossFileSearch";
import { useDebouncedValue } from "./hooks/useDebouncedValue";
import { IS_TAURI, STATE_SAVE_RESTORE_ENABLED, tauriInvoke } from "./utils/tauriUtils";
import { HeaderBar } from "./components/panels/HeaderBar";
import { CenterPane } from "./components/panels/CenterPane";
import { RightPane } from "./components/panels/RightPane";
import { GroupsAndSettingsPane } from "./components/panels/GroupsAndSettingsPane";


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

const ACTIVITY_STREAM_LIMIT = 10;
// STATE_SAVE_RESTORE_ENABLED and IS_TAURI are imported from ./utils/tauriUtils

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

type FileShortStatus = {
  path: string;
  status: "A" | "M" | "D" | "R" | "C" | string;
};

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
  const [inspectedActivityId, setInspectedActivityId] = useState<string | null>(null);
  const activityLogRef = useRef<HTMLDivElement | null>(null);
  const [rightPanelTab, setRightPanelTab] = useState<RightPanelTab>("annotations");

  // Activity stream — must be called before any callbacks that reference its outputs
  const {
    activityJob, setActivityJob,
    activityEntries, setActivityEntries,
    activityError, setActivityError,
    activityViewMode, setActivityViewMode,
    closeActivityStream, runMockActivityJob, runStreamingJob,
  } = useActivityStream({
    onJobActive: () => setRightPanelTab("activity"),
    setInspectedActivityId,
  });
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
  const [fileStatusByPath, setFileStatusByPath] = useState<Record<string, string>>({});
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

  const [groupsPanelWidth, setGroupsPanelWidth] = useState(320);
  const groupsPanelDragging = useRef(false);
  const groupsPanelStartX = useRef(0);
  const groupsPanelStartWidth = useRef(0);
  const groupsPanelRafId = useRef(0);

  const [rightmostTab, setRightmostTab] = useState<"groups" | "settings">("groups");

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

  // Rightmost (groups/settings) drag resize handlers (rAF-throttled)
  useEffect(() => {
    const onMouseMove = (e: MouseEvent) => {
      if (!groupsPanelDragging.current) return;
      const clientX = e.clientX;
      cancelAnimationFrame(groupsPanelRafId.current);
      groupsPanelRafId.current = requestAnimationFrame(() => {
        const delta = groupsPanelStartX.current - clientX;
        const newWidth = Math.max(240, Math.min(900, groupsPanelStartWidth.current + delta));
        setGroupsPanelWidth(newWidth);
      });
    };
    const onMouseUp = () => {
      if (groupsPanelDragging.current) {
        groupsPanelDragging.current = false;
        cancelAnimationFrame(groupsPanelRafId.current);
        document.body.style.cursor = "";
        document.body.style.userSelect = "";
        document.querySelector(".panel-rightmost")?.classList.remove("panel-rightmost-dragging");
      }
    };
    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
    return () => {
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
    };
  }, []);

  const startGroupsPanelDrag = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    groupsPanelDragging.current = true;
    groupsPanelStartX.current = e.clientX;
    groupsPanelStartWidth.current = groupsPanelWidth;
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
    document.querySelector(".panel-rightmost")?.classList.add("panel-rightmost-dragging");
  }, [groupsPanelWidth]);

  // Demo mode: auto-load mock data on mount when not in Tauri
  const demoLoaded = useRef(false);
  const demoLlmSettingsRef = useRef<LlmSettings>(MOCK_LLM_SETTINGS);
  const aiSetupDismissed = useRef(false);

  // LLM settings IPC throttling / latest-wins guards
  const llmSettingsLoadRequestId = useRef(0);
  const [llmSettingsPendingSave, setLlmSettingsPendingSave] = useState<LlmSettings | null>(null);
  const [llmSettingsSaveInFlight, setLlmSettingsSaveInFlight] = useState(false);
  const llmSettingsPendingSaveDebounced = useDebouncedValue(llmSettingsPendingSave, 250);

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
    // latest-wins: prevent concurrent calls from thrashing state / UI
    const requestId = ++llmSettingsLoadRequestId.current;
    try {
      let settings: LlmSettings;
      if (IS_TAURI) {
        settings = await tauriInvoke<LlmSettings>("get_llm_settings", {
          repoPath: path,
        });
      } else {
        settings = demoLlmSettingsRef.current;
      }
      if (requestId !== llmSettingsLoadRequestId.current) return;
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
    // React-driven save pipeline: we enqueue the desired settings and let an effect
    // debounce + coalesce + serialize actual IPC writes.
    setLlmSettingsPendingSave(settings);
  }, [repoPath, loadLlmSettings]);

  // Persist LLM settings (debounced + serialized).
  useEffect(() => {
    const toSave = llmSettingsPendingSaveDebounced;
    if (!IS_TAURI || !toSave) return;
    if (llmSettingsSaveInFlight) return;

    let cancelled = false;
    setLlmSettingsSaveInFlight(true);

    (async () => {
      try {
        await tauriInvoke("save_llm_settings", {
          repoPath: repoPath || "",
          settings: toSave,
        });
        if (cancelled) return;
        // Clear only if we’re still saving the same object we debounced.
        setLlmSettingsPendingSave((current) => (current === toSave ? null : current));
        // Refresh once after saving; loadLlmSettings has latest-wins.
        void loadLlmSettings(repoPath || null);
      } catch {
        // Non-fatal: settings are still applied in-memory
      } finally {
        if (!cancelled) setLlmSettingsSaveInFlight(false);
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [
    IS_TAURI,
    llmSettingsPendingSaveDebounced,
    llmSettingsSaveInFlight,
    repoPath,
    loadLlmSettings,
  ]);

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
    const inspectorWidthRaw = window.localStorage.getItem("diffcore.inspectorWidth");
    const groupsWidthRaw = window.localStorage.getItem("diffcore.groupsWidth");
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

    const inspectorWidth = Number(inspectorWidthRaw);
    if (Number.isFinite(inspectorWidth) && inspectorWidth > 0) {
      setRightPanelWidth(Math.max(200, Math.min(800, inspectorWidth)));
    }
    const groupsWidth = Number(groupsWidthRaw);
    if (Number.isFinite(groupsWidth) && groupsWidth > 0) {
      setGroupsPanelWidth(Math.max(240, Math.min(900, groupsWidth)));
    }
  }, []);

  useEffect(() => {
    window.localStorage.setItem("diffcore.recentRepos", JSON.stringify(recentRepoPaths));
  }, [recentRepoPaths]);

  useEffect(() => {
    window.localStorage.setItem("diffcore.favoriteRepos", JSON.stringify(favoriteRepoPaths));
  }, [favoriteRepoPaths]);

  useEffect(() => {
    window.localStorage.setItem("diffcore.inspectorWidth", String(rightPanelWidth));
  }, [rightPanelWidth]);

  useEffect(() => {
    window.localStorage.setItem("diffcore.groupsWidth", String(groupsPanelWidth));
  }, [groupsPanelWidth]);

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

  const runAnalysis = useCallback(async (overrideRepoPath?: string) => {
    const effectiveRepoPath = (overrideRepoPath ?? repoPath).trim();
    if (!effectiveRepoPath) return;
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
          repoPath: effectiveRepoPath,
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
        tauriInvoke<RefinementResult | null>("get_cached_refinement", { repoPath: effectiveRepoPath || null }).then((cached) => {
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
        if (result.warnings.length > 0) {
          const attemptLabel = `${result.attempts_used} attempt${result.attempts_used === 1 ? "" : "s"}`;
          if (result.stop_reason === "parse_retries_exhausted") {
            showToast(`Refinement parse retries exhausted after ${attemptLabel}; using deterministic grouping`);
          } else if (result.stop_reason === "provider_failure") {
            showToast(`Refinement provider failed after ${attemptLabel}; using deterministic grouping`);
          } else {
            showToast("Refinement kept deterministic grouping");
          }
        } else {
          showToast("Refinement kept the existing grouping");
        }
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

  // ── Editor integration ────────────────────────────────────────────────────
  const {
    openWithDropdown, setOpenWithDropdown,
    lastEditor, availableEditors,
    openWithRef, editorOptions, openInEditor,
  } = useEditorIntegration({ selectedFile, buildAbsolutePath, showToast });

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

  // ── DiffViewer callback wrappers ─────────────────────────────────────────
  // These lift the ref-heavy DiffViewer event handlers out of the JSX so
  // CenterPane can call them via context without holding the internal refs.

  const handleDiffCommentRequest = useCallback(
    (startLine: number, endLine: number, selectedCode: string) => {
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
    },
    [openCommentInput],
  );

  const handleDiffEditorContentChange = useCallback(
    (newContent: string, _hunks: EditedHunk[]) => {
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
    },
    [comments, syncEditedFileAndComments, setFileDiff],
  );

  const handleDiffHunksChanged = useCallback(
    (hunks: EditedHunk[]) => {
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
    },
    [mapEditedHunksToReplayHunks],
  );

  // ── Composed feature hooks ────────────────────────────────────────────────
  // Manifest actions and cross-file search are placed here (after the callbacks
  // they depend on: handleSelectGroup, showToast, openFileInTab, etc.)

  const {
    watchedManifestPath, setWatchedManifestPath,
    importGroupsManifest, exportGroupsManifest, buildManifestAgentPrompt,
  } = useManifestActions({
    analysis, repoPath,
    setAnalysis, setRefinedGroups, setOriginalGroups, setShowRefined, setReviewedGroupIds,
    handleSelectGroup, showToast,
  });

  const {
    crossFileSearchOpen, setCrossFileSearchOpen,
    crossFileSearchQuery, setCrossFileSearchQuery,
    crossFileSearchLoading, crossFileSearchResults, crossFileSearchError,
    crossFileSearchInputRef, runCrossFileSearch, openCrossFileSearchResult,
  } = useCrossFileSearch({
    changedFilePathSet, selectedGroupRef, openFileInTab,
    repoPath, showToast, showUnchangedFiles, diffViewerRef,
    setSelectedFile, setFileDiff, setOpenTabs,
  });

  // ── Context value ─────────────────────────────────────────────────────────
  // All state and callbacks bundled for panel/tab/modal components to consume
  // via useAppContext() — eliminates prop-drilling across 3+ component levels.
  const contextValue = {
    analysis, setAnalysis, loading, error, setError, crashPanel, setCrashPanel, sortedGroups,
    selectedGroup, setSelectedGroup, selectedFile, setSelectedFile, fileDiff, setFileDiff,
    selectedFileChange, groupAnnotation, groupDeepAnalysis, analysisHash,
    overview, setOverview, deepAnalyses, annotating, deepAnalyzing, refining,
    annotationsEnabled, aiAccessReady,
    activityJob, activityEntries, activityError, activityViewMode, setActivityViewMode,
    inspectedActivityId, setInspectedActivityId, activityLogRef,
    activityTimeline, visibleActivityTimeline, activityStats,
    activityEventProvider, activitySupportsToolStreaming, activityIsDirectApi,
    showDirectApiBanner, directApiNoticeDismissed,
    repoPath, setRepoPath, baseRef, headRef, repoInfo,
    branchDropdownOpen, setBranchDropdownOpen,
    headBranchDropdownOpen, setHeadBranchDropdownOpen,
    recentCommits, showHeadCommits, setShowHeadCommits, showBaseCommits, setShowBaseCommits,
    baseBranches, headLabel, baseLabel, statusText, comparisonMode,
    includeUncommitted, setIncludeUncommitted, fileStatusByPath,
    recentRepoPaths, favoriteRepoPaths, setFavoriteRepoPaths, editsEnabled, repoInputRef,
    llmSettings, settingsOpen, setSettingsOpen,
    aiSetupOpen, aiSetupStep, setAiSetupStep, apiProviderDraft, setApiProviderDraft,
    apiKeyInput, setApiKeyInput, selectedApiModel, hasApiKey,
    recommendedSubscriptionProvider, providerModels, modelsLoading,
    ignorePaths, ignorePathInput, setIgnorePathInput,
    diffViewMode, setDiffViewMode, showUnchangedFiles, setShowUnchangedFiles,
    shouldRenderSideBySide, diffViewerRef,
    crossFileSearchOpen, setCrossFileSearchOpen, crossFileSearchQuery, setCrossFileSearchQuery,
    crossFileSearchLoading, crossFileSearchResults, crossFileSearchError, crossFileSearchInputRef,
    rightPanelTab, setRightPanelTab, rightPanelCollapsed, setRightPanelCollapsed, rightPanelWidth,
    groupsPanelWidth,
    rightmostTab, setRightmostTab,
    sourceFocusRequest,
    annotationSubTab, setAnnotationSubTab, graphGranularity, setGraphGranularity,
    replayActive, replayStep, replayVisited, replayHunkIndex, replayViewedHunkIds,
    currentReplayHunks, replayHunks, hasNextReplayHunk, hasPrevReplayHunk,
    infraExpanded, setInfraExpanded, infraShowAll, setInfraShowAll,
    infraSubGroupsExpanded, setInfraSubGroupsExpanded,
    originalGroups, refinedGroups, refinementResponse, showRefined, groupListTransitionState,
    refinementVerdict, resolvedRefinementProvider, resolvedRefinementModel,
    refinementProvider, refinementModel,
    openTabs, tabContextMenu, setTabContextMenu, contextMenu, setContextMenu,
    commentsCollapsed, setCommentsCollapsed,
    comments, commentInput, commentText, setCommentText, commentInputRef,
    activeCommentId, setActiveCommentId, editingCommentId, setEditingCommentId,
    editingCommentText, setEditingCommentText, pendingScrollToCommentRef, codeCommentsForSelectedFile, commentCountForGroup,
    commentsForFile,
    regenDialogOpen, setRegenDialogOpen, regenFeedbackText, setRegenFeedbackText,
    regenIncludePreviousOutput, setRegenIncludePreviousOutput,
    reviewedGroupIds,
    openWithDropdown, setOpenWithDropdown, lastEditor, availableEditors, openWithRef, editorOptions,
    updateAvailable, setUpdateAvailable, updating, setUpdating,
    toast, watchedManifestPath, setWatchedManifestPath,
    runAnalysis, runAnnotateOverview, runDeepAnalysis, runRefinement, toggleRefinedView,
    handleSelectGroup, handleSelectFile, handleSelectFileDebounced,
    openFileInTab, closeTab, closeOtherTabs, closeAllTabs,
    handleGraphNodeClick, handleEdgeEndpointClick, handleGraphEdgeClick,
    handleSourceNavigate, handleGoToDefinition, handleSelectBase, handleSelectHead,
    handleFileContextMenu, enterReplay, exitReplay, goToReplayStep, navigateReplayHunk,
    jumpToReplayHunk, commentOnCurrentReplayHunk,
    openCommentInput, submitComment, cancelComment, saveComment, deleteComment, updateComment,
    exportComments, copyPrDescription, toggleGroupReviewed, copyFilePath, copyFlowPaths,
    syncEditedFileAndComments,
    openAiSetup, dismissAiSetup, refreshAiAccess, dismissDirectApiNotice,
    activateSubscriptionProvider, activatePreferredActivityProvider, openApiKeyFallback,
    modelsForProvider, updateSetting, handleSaveApiKey, handleClearApiKey,
    fetchModelsForProvider, saveLlmSettings, resolvedPrimaryProvider, resolvedPrimaryModel,
    handleAddIgnorePath, handleRemoveIgnorePath, loadRepoInfo, browseForRepository, showToast,
    importGroupsManifest, exportGroupsManifest, buildManifestAgentPrompt,
    openInEditor, runCrossFileSearch, openCrossFileSearchResult, startRightPanelDrag, startGroupsPanelDrag,
    restoreLastSessionState,
    handleDiffCommentRequest, handleDiffEditorContentChange, handleDiffHunksChanged,
  };

  // Dedicated diff/Monaco context to isolate expensive rerenders from unrelated state changes.
  const diffContextValue = useMemo(() => ({
    fileDiff, selectedFile, selectedGroup,
    openTabs, handleSelectFile, closeTab, setTabContextMenu,
    openWithRef, openWithDropdown, setOpenWithDropdown,
    lastEditor, editorOptions, openInEditor,
    replayActive, replayStep, replayVisited,
    replayHunks, replayHunkIndex, replayViewedHunkIds,
    hasNextReplayHunk, hasPrevReplayHunk,
    navigateReplayHunk, commentOnCurrentReplayHunk,
    goToReplayStep, exitReplay,
    diffViewerRef, editsEnabled, shouldRenderSideBySide,
    codeCommentsForSelectedFile,
    setActiveCommentId, setRightPanelTab, rightPanelCollapsed, setRightPanelCollapsed,
    handleGoToDefinition,
    handleDiffCommentRequest, handleDiffEditorContentChange, handleDiffHunksChanged,
  }), [
    fileDiff, selectedFile, selectedGroup,
    openTabs, handleSelectFile, closeTab, setTabContextMenu,
    openWithRef, openWithDropdown, setOpenWithDropdown,
    lastEditor, editorOptions, openInEditor,
    replayActive, replayStep, replayVisited,
    replayHunks, replayHunkIndex, replayViewedHunkIds,
    hasNextReplayHunk, hasPrevReplayHunk,
    navigateReplayHunk, commentOnCurrentReplayHunk,
    goToReplayStep, exitReplay,
    diffViewerRef, editsEnabled, shouldRenderSideBySide,
    codeCommentsForSelectedFile,
    setActiveCommentId, setRightPanelTab, rightPanelCollapsed, setRightPanelCollapsed,
    handleGoToDefinition,
    handleDiffCommentRequest, handleDiffEditorContentChange, handleDiffHunksChanged,
  ]);

  return (
    <AppContext.Provider value={contextValue}>

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
      <HeaderBar />

      <AISetupModal />

      {/* SettingsPanel now lives in the rightmost tab */}
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
        {/* Left: Monaco Diff Viewer */}
        <DiffContext.Provider value={diffContextValue}>
          <CenterPane />
        </DiffContext.Provider>

        {/* Resize handle: diff | inspector */}
        <div className="panel-resize-handle" onMouseDown={startRightPanelDrag} />

        {/* Inner-right: Inspector tabs */}
        <RightPane />

        {/* Resize handle: inspector | groups/settings */}
        <div className="panel-resize-handle" onMouseDown={startGroupsPanelDrag} />

        {/* Rightmost: Flow groups + Settings tab */}
        <GroupsAndSettingsPane />
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

      <CommentInputOverlay />
      <RegenDialog />

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
    </AppContext.Provider>
  );
}
