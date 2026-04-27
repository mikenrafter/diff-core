/**
 * AppContext — single React context exposing all UI state and callbacks from
 * App.tsx to panel, tab, and modal components.
 *
 * WHY: The original App.tsx was ~6000 lines because every piece of state and
 * every callback lived there as a monolith. Breaking it into panels/tabs/modals
 * requires sharing state without prop-drilling through 3+ component levels.
 * A single context with a well-typed value is the chosen solution.
 *
 * HOW: App.tsx assembles the `AppContextValue` object from its state and
 * callbacks, then renders `<AppContext.Provider value={...}>` around the shell.
 * Components call `useAppContext()` and destructure only what they need.
 */
import type React from "react";
import { createContext, useContext } from "react";
import type { MutableRefObject } from "react";
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
  LlmActivityJob,
  RefinementResponse,
  ReviewComment,
  CommentInput,
  FileChange,
} from "../types";
import type { DiffViewerHandle, EditedHunk } from "../components/DiffViewer";
import type { SourceFocusRequest } from "../components/SourceExplorer";
import type { SubscriptionProvider } from "../utils/llmUtils";
import type { ActivityPresentation } from "../utils/activityUtils";

// ── Local UI types ──────────────────────────────────────────────────────────

export type OnboardingStep = "recommended" | "api";
export type RightPanelTab = "activity" | "annotations" | "source" | "comments";
export type ActivityViewMode = "stream" | "all";

/** A diffed code hunk tracked during the replay-flow walkthrough. */
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

/** How the two selected refs are being compared. */
export type CompareMode = "branch" | "unstaged_to_staged" | "staged_to_commit" | "invalid";

/** External editor launched from the "Open With" toolbar button. */
export type EditorId = "vscode" | "cursor" | "zed" | "vim" | "terminal";

/** A single line-match returned by the cross-file grep. */
export type CrossFileSearchMatch = {
  line_number: number;
  line_text: string;
};

/** A file with one or more line-matches from the cross-file grep. */
export type CrossFileSearchResult = {
  file_path: string;
  matches: CrossFileSearchMatch[];
};

/**
 * Summary of the LLM refinement pass shown in the activity hero and
 * annotations info sub-tab.
 */
export type RefinementVerdict = {
  provider: string;
  model: string;
  hadChanges: boolean;
  title: string;
  reasoning: string | null;
};

/** An activity-log entry enriched with a stable render ID and presentation. */
export type ActivityTimelineItem = {
  id: string;
  entry: LlmActivityEntry;
  presentation: ActivityPresentation;
};

/** Counts of each activity event kind shown in the activity hero. */
export type ActivityStats = {
  total: number;
  search: number;
  read: number;
  command: number;
};

// ── AppContextValue ─────────────────────────────────────────────────────────

/**
 * All UI state, callbacks, and derived values surfaced by App.tsx via React
 * context. Panel and tab components consume this via `useAppContext()` and
 * destructure only the fields they need.
 *
 * Fields are grouped by domain and prefixed with a comment banner for quick
 * navigation. Setters are included only where child components need to mutate
 * state directly (most mutations go through a named callback instead).
 */
export interface AppContextValue {
  // ─── Analysis ────────────────────────────────────────────────────────────
  analysis: AnalysisOutput | null;
  setAnalysis: React.Dispatch<React.SetStateAction<AnalysisOutput | null>>;
  loading: boolean;
  error: string | null;
  setError: React.Dispatch<React.SetStateAction<string | null>>;
  /** Test-only: when set, the named panel's ErrorBoundary will throw. */
  crashPanel: string | null;
  setCrashPanel: React.Dispatch<React.SetStateAction<string | null>>;
  sortedGroups: FlowGroup[];

  // ─── Group / file navigation ─────────────────────────────────────────────
  selectedGroup: FlowGroup | null;
  setSelectedGroup: React.Dispatch<React.SetStateAction<FlowGroup | null>>;
  selectedFile: string | null;
  setSelectedFile: React.Dispatch<React.SetStateAction<string | null>>;
  fileDiff: FileDiffContent | null;
  setFileDiff: React.Dispatch<React.SetStateAction<FileDiffContent | null>>;
  selectedFileChange: FileChange | null;
  groupAnnotation: Pass1GroupAnnotation | undefined;
  groupDeepAnalysis: Pass2Response | undefined;
  /** Weak hash of the current analysis used for comment scoping. */
  analysisHash: string;

  // ─── LLM annotations ─────────────────────────────────────────────────────
  overview: Pass1Response | null;
  setOverview: React.Dispatch<React.SetStateAction<Pass1Response | null>>;
  deepAnalyses: Record<string, Pass2Response>;
  annotating: boolean;
  deepAnalyzing: boolean;
  refining: boolean;
  annotationsEnabled: boolean;
  aiAccessReady: boolean;

  // ─── Activity stream ──────────────────────────────────────────────────────
  activityJob: LlmActivityJob | null;
  activityEntries: LlmActivityEntry[];
  activityError: string | null;
  activityViewMode: ActivityViewMode;
  setActivityViewMode: React.Dispatch<React.SetStateAction<ActivityViewMode>>;
  inspectedActivityId: string | null;
  setInspectedActivityId: React.Dispatch<React.SetStateAction<string | null>>;
  activityLogRef: MutableRefObject<HTMLDivElement | null>;
  activityTimeline: ActivityTimelineItem[];
  visibleActivityTimeline: ActivityTimelineItem[];
  activityStats: ActivityStats;
  /** The LLM provider that produced the most recent activity events. */
  activityEventProvider: string | null;
  activitySupportsToolStreaming: boolean;
  activityIsDirectApi: boolean;
  showDirectApiBanner: boolean;
  directApiNoticeDismissed: boolean;

  // ─── Repo / git ───────────────────────────────────────────────────────────
  repoPath: string;
  setRepoPath: React.Dispatch<React.SetStateAction<string>>;
  baseRef: string;
  headRef: string | null;
  repoInfo: RepoInfo | null;
  branchDropdownOpen: boolean;
  setBranchDropdownOpen: React.Dispatch<React.SetStateAction<boolean>>;
  headBranchDropdownOpen: boolean;
  setHeadBranchDropdownOpen: React.Dispatch<React.SetStateAction<boolean>>;
  recentCommits: CommitInfo[];
  showHeadCommits: boolean;
  setShowHeadCommits: React.Dispatch<React.SetStateAction<boolean>>;
  showBaseCommits: boolean;
  setShowBaseCommits: React.Dispatch<React.SetStateAction<boolean>>;
  baseBranches: BranchInfo[];
  headLabel: string;
  baseLabel: string;
  statusText: string | null;
  comparisonMode: CompareMode;
  includeUncommitted: boolean;
  setIncludeUncommitted: React.Dispatch<React.SetStateAction<boolean>>;
  fileStatusByPath: Record<string, string>;
  recentRepoPaths: string[];
  favoriteRepoPaths: string[];
  setFavoriteRepoPaths: React.Dispatch<React.SetStateAction<string[]>>;
  /** True when comparing unstaged→staged changes (edits are persisted live). */
  editsEnabled: boolean;
  repoInputRef: MutableRefObject<HTMLInputElement | null>;

  // ─── Pane layout ──────────────────────────────────────────────────────────
  rightmostTab: "groups" | "settings" | "ai";
  setRightmostTab: React.Dispatch<React.SetStateAction<"groups" | "settings" | "ai">>;
  groupsPanelWidth: number;

  // ─── LLM settings ─────────────────────────────────────────────────────────
  llmSettings: LlmSettings | null;
  settingsOpen: boolean;
  setSettingsOpen: React.Dispatch<React.SetStateAction<boolean>>;
  aiSetupOpen: boolean;
  aiSetupStep: OnboardingStep;
  setAiSetupStep: React.Dispatch<React.SetStateAction<OnboardingStep>>;
  apiProviderDraft: LlmProvider;
  setApiProviderDraft: React.Dispatch<React.SetStateAction<LlmProvider>>;
  apiKeyInput: string;
  setApiKeyInput: React.Dispatch<React.SetStateAction<string>>;
  /** The first model available for `apiProviderDraft`. */
  selectedApiModel: string;
  hasApiKey: boolean;
  recommendedSubscriptionProvider: SubscriptionProvider | null;
  providerModels: Record<string, string[]>;
  modelsLoading: string | null;
  /** Refresh available model list for a provider; force=true bypasses cache. */
  fetchModelsForProvider: (provider: string, force?: boolean) => Promise<void>;
  /** Persist LLM settings to backend storage and update local state. */
  saveLlmSettings: (settings: LlmSettings) => Promise<void>;
  /** Resolved primary LLM provider for the current machine (subscription or API). */
  resolvedPrimaryProvider: string | null;
  /** Model that will be used for the primary provider on this machine. */
  resolvedPrimaryModel: string | null;

  // ─── Ignore paths ─────────────────────────────────────────────────────────
  ignorePaths: string[];
  ignorePathInput: string;
  setIgnorePathInput: React.Dispatch<React.SetStateAction<string>>;

  // ─── Diff display ─────────────────────────────────────────────────────────
  diffViewMode: DiffViewMode;
  setDiffViewMode: React.Dispatch<React.SetStateAction<DiffViewMode>>;
  showUnchangedFiles: boolean;
  setShowUnchangedFiles: React.Dispatch<React.SetStateAction<boolean>>;
  shouldRenderSideBySide: boolean;
  diffViewerRef: MutableRefObject<DiffViewerHandle | null>;

  // ─── Cross-file search ────────────────────────────────────────────────────
  crossFileSearchOpen: boolean;
  setCrossFileSearchOpen: React.Dispatch<React.SetStateAction<boolean>>;
  crossFileSearchQuery: string;
  setCrossFileSearchQuery: React.Dispatch<React.SetStateAction<string>>;
  crossFileSearchLoading: boolean;
  crossFileSearchResults: CrossFileSearchResult[];
  crossFileSearchError: string | null;
  crossFileSearchInputRef: MutableRefObject<HTMLInputElement | null>;

  // ─── Right panel ──────────────────────────────────────────────────────────
  rightPanelTab: RightPanelTab;
  setRightPanelTab: React.Dispatch<React.SetStateAction<RightPanelTab>>;
  rightPanelCollapsed: boolean;
  setRightPanelCollapsed: React.Dispatch<React.SetStateAction<boolean>>;
  rightPanelWidth: number;
  startGroupsPanelDrag: (e: React.MouseEvent) => void;
  sourceFocusRequest: SourceFocusRequest | null;

  // ─── Annotation sub-tabs ──────────────────────────────────────────────────
  annotationSubTab: "info" | "graph" | "edges";
  setAnnotationSubTab: React.Dispatch<React.SetStateAction<"info" | "graph" | "edges">>;
  graphGranularity: "file" | "module_class_method";
  setGraphGranularity: React.Dispatch<React.SetStateAction<"file" | "module_class_method">>;

  // ─── Flow replay ──────────────────────────────────────────────────────────
  replayActive: boolean;
  replayStep: number;
  replayVisited: Set<string>;
  replayHunkIndex: number;
  replayViewedHunkIds: Set<string>;
  currentReplayHunks: ReplayHunk[];
  /** Alias for currentReplayHunks — used by hunk navigation. */
  replayHunks: ReplayHunk[];
  hasNextReplayHunk: boolean;
  hasPrevReplayHunk: boolean;
  hasNextFileInGroup: boolean;
  hasPrevFileInGroup: boolean;

  // ─── Infrastructure group ─────────────────────────────────────────────────
  infraExpanded: boolean;
  setInfraExpanded: React.Dispatch<React.SetStateAction<boolean>>;
  infraShowAll: boolean;
  setInfraShowAll: React.Dispatch<React.SetStateAction<boolean>>;
  infraSubGroupsExpanded: Set<string>;
  setInfraSubGroupsExpanded: React.Dispatch<React.SetStateAction<Set<string>>>;
  expandedGroupIds: Set<string>;
  setExpandedGroupIds: React.Dispatch<React.SetStateAction<Set<string>>>;

  // ─── Refinement ───────────────────────────────────────────────────────────
  originalGroups: FlowGroup[] | null;
  refinedGroups: FlowGroup[] | null;
  refinementResponse: RefinementResponse | null;
  showRefined: boolean;
  groupListTransitionState: "idle" | "fading-out" | "fading-in";
  refinementVerdict: RefinementVerdict | null;
  /** Resolved provider that will actually run refinement (respects subscription providers). */
  resolvedRefinementProvider: string | null;
  /** Resolved model that will actually run refinement. */
  resolvedRefinementModel: string | null;
  /** The provider that ran the most recent completed refinement pass. */
  refinementProvider: string | null;
  /** The model that ran the most recent completed refinement pass. */
  refinementModel: string | null;

  // ─── Tab bar ──────────────────────────────────────────────────────────────
  openTabs: Array<{ path: string; groupId: string }>;
  tabContextMenu: { x: number; y: number; tabPath: string } | null;
  setTabContextMenu: React.Dispatch<React.SetStateAction<{ x: number; y: number; tabPath: string } | null>>;

  // ─── Context menu ─────────────────────────────────────────────────────────
  contextMenu: { x: number; y: number; filePath: string } | null;
  setContextMenu: React.Dispatch<React.SetStateAction<{ x: number; y: number; filePath: string } | null>>;

  // ─── Comment strip (center panel legacy) ─────────────────────────────────
  /** Controls the collapsed state of the inline comment strip (currently hidden). */
  commentsCollapsed: boolean;
  setCommentsCollapsed: React.Dispatch<React.SetStateAction<boolean>>;

  // ─── Review comments ──────────────────────────────────────────────────────
  comments: ReviewComment[];
  commentInput: CommentInput | null;
  commentText: string;
  setCommentText: React.Dispatch<React.SetStateAction<string>>;
  commentInputRef: MutableRefObject<HTMLTextAreaElement | null>;
  activeCommentId: string | null;
  setActiveCommentId: React.Dispatch<React.SetStateAction<string | null>>;
  editingCommentId: string | null;
  setEditingCommentId: React.Dispatch<React.SetStateAction<string | null>>;
  editingCommentText: string;
  setEditingCommentText: React.Dispatch<React.SetStateAction<string>>;
  /** Ref used to queue a scroll-to-line after a file loads from a comment click. */
  pendingScrollToCommentRef: MutableRefObject<{ startLine: number; endLine?: number; commentId: string } | null>;
  codeCommentsForSelectedFile: ReviewComment[];
  commentCountForGroup: (groupId: string) => number;
  /** Get all comments for a given file path. */
  commentsForFile: (filePath: string) => ReviewComment[];

  // ─── Regen dialog ─────────────────────────────────────────────────────────
  regenDialogOpen: boolean;
  setRegenDialogOpen: React.Dispatch<React.SetStateAction<boolean>>;
  regenOperation: "summary" | "flow_analysis" | "refine_groups";
  setRegenOperation: React.Dispatch<React.SetStateAction<"summary" | "flow_analysis" | "refine_groups">>;
  regenFeedbackText: string;
  setRegenFeedbackText: React.Dispatch<React.SetStateAction<string>>;
  regenIncludePreviousOutput: boolean;
  setRegenIncludePreviousOutput: React.Dispatch<React.SetStateAction<boolean>>;

  // ─── Group review ─────────────────────────────────────────────────────────
  reviewedGroupIds: Set<string>;

  // ─── Editor integration ───────────────────────────────────────────────────
  openWithDropdown: boolean;
  setOpenWithDropdown: React.Dispatch<React.SetStateAction<boolean>>;
  lastEditor: EditorId;
  availableEditors: Set<EditorId> | null;
  openWithRef: MutableRefObject<HTMLDivElement | null>;
  editorOptions: Array<{ id: EditorId; label: string }>;

  // ─── App update ───────────────────────────────────────────────────────────
  updateAvailable: { version: string; body: string } | null;
  setUpdateAvailable: React.Dispatch<React.SetStateAction<{ version: string; body: string } | null>>;
  updating: boolean;
  setUpdating: React.Dispatch<React.SetStateAction<boolean>>;

  // ─── Toast ────────────────────────────────────────────────────────────────
  toast: string | null;

  // ─── Manifest watch ───────────────────────────────────────────────────────
  watchedManifestPath: string | null;
  setWatchedManifestPath: React.Dispatch<React.SetStateAction<string | null>>;

  // ─── Callbacks ────────────────────────────────────────────────────────────
  runAnalysis: (overrideRepoPath?: string) => Promise<void>;
  runAnnotateOverview: (opts?: { feedback?: string; includePreviousOutput?: boolean }) => Promise<void>;
  runDeepAnalysis: (opts?: { feedback?: string; includePreviousOutput?: boolean }) => Promise<void>;
  runRefinement: (opts?: { feedback?: string; includePreviousOutput?: boolean }) => Promise<void>;
  toggleRefinedView: (useRefined: boolean) => void;
  handleSelectGroup: (group: FlowGroup) => Promise<void>;
  handleSelectFile: (path: string) => Promise<void>;
  handleSelectFileDebounced: (path: string) => void;
  openFileInTab: (path: string, groupId: string) => void;
  closeTab: (tabPath: string) => void;
  closeOtherTabs: (keepPath: string) => void;
  closeAllTabs: () => void;
  handleGraphNodeClick: (path: string) => void;
  handleEdgeEndpointClick: (endpoint: string) => void;
  handleGraphEdgeClick: (sourceEndpoint: string, targetEndpoint: string) => void;
  handleSourceNavigate: (filePath: string, symbolName?: string) => Promise<void>;
  handleGoToDefinition: (word: string) => void;
  handleSelectBase: (refName: string) => void;
  handleSelectHead: (refName: string) => void;
  handleFileContextMenu: (e: React.MouseEvent, filePath: string) => void;
  enterReplay: () => void;
  exitReplay: () => void;
  goToReplayStep: (step: number) => void;
  navigateReplayHunk: (direction: 1 | -1) => void;
  jumpToReplayHunk: (index: number) => void;
  goToNextFileInGroup: () => void;
  goToPrevFileInGroup: () => void;
  openCommentInput: (overrideInput?: CommentInput) => void;
  submitComment: () => void;
  cancelComment: () => void;
  saveComment: (comment: ReviewComment) => Promise<void>;
  deleteComment: (commentId: string) => Promise<void>;
  updateComment: (commentId: string, newText: string) => Promise<void>;
  exportComments: () => Promise<void>;
  copyPrDescription: () => Promise<void>;
  toggleGroupReviewed: (groupId: string) => void;
  copyFilePath: (relativePath: string) => Promise<void>;
  copyFlowPaths: (group: FlowGroup) => Promise<void>;
  syncEditedFileAndComments: (
    filePath: string,
    groupId: string,
    newContent: string,
    hunks: EditedHunk[],
  ) => Promise<void>;
  openAiSetup: (step?: OnboardingStep) => void;
  dismissAiSetup: () => void;
  refreshAiAccess: () => Promise<void>;
  dismissDirectApiNotice: () => void;
  activateSubscriptionProvider: (provider: SubscriptionProvider) => Promise<void>;
  activatePreferredActivityProvider: () => Promise<void>;
  openApiKeyFallback: () => void;
  modelsForProvider: (provider: string) => string[];
  updateSetting: (field: keyof LlmSettings, value: string | boolean | number) => void;
  handleSaveApiKey: () => Promise<void>;
  handleClearApiKey: () => Promise<void>;
  handleAddIgnorePath: () => Promise<void>;
  handleRemoveIgnorePath: (pattern: string) => Promise<void>;
  loadRepoInfo: (path: string) => Promise<void>;
  browseForRepository: () => Promise<void>;
  showToast: (message: string) => void;
  importGroupsManifest: (manifestPath: string) => Promise<void>;
  exportGroupsManifest: () => Promise<string | null>;
  buildManifestAgentPrompt: (manifestPath: string) => string;
  openInEditor: (editor: EditorId) => Promise<void>;
  runCrossFileSearch: () => Promise<void>;
  openCrossFileSearchResult: (filePath: string, lineNumber: number) => Promise<void>;
  startRightPanelDrag: (e: React.MouseEvent) => void;

  // ─── Session restore ──────────────────────────────────────────────────────
  /** Restore the last saved session state (no-op when feature flag is off). */
  restoreLastSessionState: () => Promise<void>;

  // ─── DiffViewer callback wrappers ─────────────────────────────────────────
  /**
   * Called by DiffViewer when the user selects a code range for commenting.
   * Wraps the ref-based group/file lookup so CenterPane has no direct refs.
   */
  handleDiffCommentRequest: (
    startLine: number,
    endLine: number,
    selectedCode: string,
  ) => void;
  /**
   * Called by DiffViewer on every content edit. Debounces a sync write to
   * auto-comment placeholders after 2 s of idle time.
   */
  handleDiffEditorContentChange: (newContent: string, hunks: EditedHunk[]) => void;
  /**
   * Called by DiffViewer whenever the set of visible diff hunks changes.
   * Updates replay hunk state and handles any pending scroll requests.
   */
  handleDiffHunksChanged: (hunks: EditedHunk[]) => void;

  // ─── Monaco-derived hunk counts (UI truth) ────────────────────────────────
  /**
   * Per-file hunk counts as computed by Monaco for the currently-loaded content.
   * Used to display accurate counts even when git/libgit2 hunk splitting differs.
   */
  monacoHunkCounts: Map<string, number>;

  /** Per-file reviewed hunk counts (derived from replay viewed-hunk IDs). */
  reviewedHunksByFile: Map<string, number>;
}

// ── Context instance ────────────────────────────────────────────────────────

export const AppContext = createContext<AppContextValue | null>(null);

/**
 * Consume the app-wide context from any panel, tab, or modal component.
 * Throws if called outside an `<AppContext.Provider>` so missing wires are
 * caught immediately during development.
 */
export function useAppContext(): AppContextValue {
  const ctx = useContext(AppContext);
  if (!ctx) throw new Error("useAppContext must be used within an AppContext.Provider");
  return ctx;
}
