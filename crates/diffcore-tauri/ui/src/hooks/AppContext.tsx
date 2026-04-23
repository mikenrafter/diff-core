import type React from "react";
import { createContext, useContext } from "react";
import type {
  AnalysisOutput,
  FlowGroup,
  FileDiffContent,
  DiffViewMode,
  Pass1Response,
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
} from "../types";
import type { DiffViewerHandle, EditedHunk } from "../components/DiffViewer";
import type { SourceFocusRequest } from "../components/SourceExplorer";
import type {
  OnboardingStep,
  SubscriptionProvider,
  RightPanelTab,
  ActivityViewMode,
  ReplayHunk,
  CompareMode,
  CrossFileSearchResult,
} from "../utils/constants";
import type { ActivityPresentation } from "../utils/activityUtils";

// Callback type shorthand
// eslint-disable-next-line @typescript-eslint/no-explicit-any
type AnyFn = (...args: any[]) => any;

export interface AppContextValue {
  // ── Core analysis ──
  analysis: AnalysisOutput | null;
  setAnalysis: (v: AnalysisOutput | null) => void;
  selectedGroup: FlowGroup | null;
  setSelectedGroup: (v: FlowGroup | null) => void;
  selectedFile: string | null;
  setSelectedFile: (v: string | null) => void;
  fileDiff: FileDiffContent | null;
  setFileDiff: (v: FileDiffContent | null) => void;
  loading: boolean;
  setLoading: (v: boolean) => void;
  error: string | null;
  setError: (v: string | null) => void;
  crashPanel: string | null;
  setCrashPanel: (v: string | null) => void;

  // ── LLM annotation ──
  overview: Pass1Response | null;
  setOverview: (v: Pass1Response | null) => void;
  deepAnalyses: Record<string, Pass2Response>;
  setDeepAnalyses: AnyFn;
  annotating: boolean;
  setAnnotating: (v: boolean) => void;
  deepAnalyzing: boolean;
  setDeepAnalyzing: (v: boolean) => void;
  activityJob: LlmActivityJob | null;
  setActivityJob: (v: LlmActivityJob | null) => void;
  activityEntries: LlmActivityEntry[];
  setActivityEntries: AnyFn;
  activityError: string | null;
  setActivityError: (v: string | null) => void;
  activityViewMode: ActivityViewMode;
  setActivityViewMode: (v: ActivityViewMode) => void;
  inspectedActivityId: string | null;
  setInspectedActivityId: (v: string | null) => void;
  rightPanelTab: RightPanelTab;
  setRightPanelTab: (v: RightPanelTab) => void;
  sourceFocusRequest: SourceFocusRequest | null;
  setSourceFocusRequest: (v: SourceFocusRequest | null) => void;
  regenDialogOpen: boolean;
  setRegenDialogOpen: (v: boolean) => void;
  regenFeedbackText: string;
  setRegenFeedbackText: (v: string) => void;
  regenIncludePreviousOutput: boolean;
  setRegenIncludePreviousOutput: (v: boolean) => void;

  // ── Repo + git ──
  repoPath: string;
  setRepoPath: (v: string) => void;
  baseRef: string;
  setBaseRef: (v: string) => void;
  headRef: string | null;
  setHeadRef: (v: string | null) => void;
  repoInfo: RepoInfo | null;
  setRepoInfo: (v: RepoInfo | null) => void;
  branchDropdownOpen: boolean;
  setBranchDropdownOpen: (v: boolean) => void;
  headBranchDropdownOpen: boolean;
  setHeadBranchDropdownOpen: (v: boolean) => void;
  recentCommits: CommitInfo[];
  setRecentCommits: (v: CommitInfo[]) => void;
  showHeadCommits: boolean;
  setShowHeadCommits: (v: boolean) => void;
  showBaseCommits: boolean;
  setShowBaseCommits: (v: boolean) => void;

  // ── Diff mode ──
  diffViewMode: DiffViewMode;
  setDiffViewMode: (v: DiffViewMode) => void;
  directApiNoticeDismissed: boolean;
  setDirectApiNoticeDismissed: (v: boolean) => void;

  // ── Provider / model ──
  providerModels: Record<string, string[]>;
  setProviderModels: AnyFn;
  modelsLoading: string | null;
  setModelsLoading: (v: string | null) => void;
  hasApiKey: boolean;
  setHasApiKey: (v: boolean) => void;

  // ── Diff behavior ──
  includeUncommitted: boolean;
  setIncludeUncommitted: (v: boolean) => void;
  showUnchangedFiles: boolean;
  setShowUnchangedFiles: (v: boolean) => void;
  crossFileSearchOpen: boolean;
  setCrossFileSearchOpen: (v: boolean) => void;
  crossFileSearchQuery: string;
  setCrossFileSearchQuery: (v: string) => void;
  crossFileSearchLoading: boolean;
  setCrossFileSearchLoading: (v: boolean) => void;
  crossFileSearchResults: CrossFileSearchResult[];
  setCrossFileSearchResults: (v: CrossFileSearchResult[]) => void;
  crossFileSearchError: string | null;
  setCrossFileSearchError: (v: string | null) => void;
  fileStatusByPath: Record<string, string>;
  setFileStatusByPath: AnyFn;
  repoQuickPickOpen: boolean;
  setRepoQuickPickOpen: (v: boolean) => void;
  recentRepoPaths: string[];
  setRecentRepoPaths: (v: string[]) => void;
  favoriteRepoPaths: string[];
  setFavoriteRepoPaths: (v: string[]) => void;

  // ── Derived comparison state ──
  comparisonMode: CompareMode;
  analysisDiffArgs: {
    base: string | null;
    head: string | null;
    staged: boolean;
    unstaged: boolean;
    prPreview: boolean;
    includeUncommitted: boolean;
  };
  fileDiffArgs: {
    base: string | null;
    head: string | null;
    staged: boolean;
    unstaged: boolean;
    includeUncommitted: boolean;
  };
  editsEnabled: boolean;

  // ── LLM settings ──
  llmSettings: LlmSettings | null;
  setLlmSettings: AnyFn;
  settingsOpen: boolean;
  setSettingsOpen: (v: boolean) => void;
  aiSetupOpen: boolean;
  setAiSetupOpen: (v: boolean) => void;
  aiSetupStep: OnboardingStep;
  setAiSetupStep: (v: OnboardingStep) => void;
  apiProviderDraft: LlmProvider;
  setApiProviderDraft: (v: LlmProvider) => void;
  apiKeyInput: string;
  setApiKeyInput: (v: string) => void;

  // ── Ignore paths ──
  ignorePaths: string[];
  setIgnorePaths: (v: string[]) => void;
  ignorePathInput: string;
  setIgnorePathInput: (v: string) => void;

  // ── Review progress ──
  reviewedGroupIds: Set<string>;
  setReviewedGroupIds: AnyFn;

  // ── Replay ──
  replayActive: boolean;
  setReplayActive: (v: boolean) => void;
  replayStep: number;
  setReplayStep: (v: number) => void;
  replayVisited: Set<string>;
  setReplayVisited: AnyFn;
  replayHunkIndex: number;
  setReplayHunkIndex: (v: number) => void;
  replayViewedHunkIds: Set<string>;
  setReplayViewedHunkIds: AnyFn;
  currentReplayHunks: ReplayHunk[];
  setCurrentReplayHunks: (v: ReplayHunk[]) => void;

  // ── Refinement ──
  originalGroups: FlowGroup[] | null;
  setOriginalGroups: (v: FlowGroup[] | null) => void;
  refinedGroups: FlowGroup[] | null;
  setRefinedGroups: (v: FlowGroup[] | null) => void;
  refinementResponse: RefinementResponse | null;
  setRefinementResponse: (v: RefinementResponse | null) => void;
  refinementProvider: string | null;
  setRefinementProvider: (v: string | null) => void;
  refinementModel: string | null;
  setRefinementModel: (v: string | null) => void;
  refinementHadChanges: boolean | null;
  setRefinementHadChanges: (v: boolean | null) => void;
  showRefined: boolean;
  setShowRefined: (v: boolean) => void;
  groupListTransitionState: "idle" | "fading-out" | "fading-in";
  setGroupListTransitionState: (v: "idle" | "fading-out" | "fading-in") => void;
  refining: boolean;
  setRefining: (v: boolean) => void;

  // ── Context menus ──
  contextMenu: { x: number; y: number; filePath: string } | null;
  setContextMenu: (v: { x: number; y: number; filePath: string } | null) => void;
  tabContextMenu: { x: number; y: number; tabPath: string } | null;
  setTabContextMenu: (v: { x: number; y: number; tabPath: string } | null) => void;

  // ── Tabs ──
  openTabs: Array<{ path: string; groupId: string }>;
  setOpenTabs: AnyFn;

  // ── Comments ──
  comments: ReviewComment[];
  setComments: AnyFn;
  commentInput: CommentInput | null;
  setCommentInput: (v: CommentInput | null) => void;
  commentText: string;
  setCommentText: (v: string) => void;
  editingCommentId: string | null;
  setEditingCommentId: (v: string | null) => void;
  editingCommentText: string;
  setEditingCommentText: (v: string) => void;
  activeCommentId: string | null;
  setActiveCommentId: (v: string | null) => void;

  // ── UI state ──
  annotationSubTab: "info" | "graph" | "edges";
  setAnnotationSubTab: (v: "info" | "graph" | "edges") => void;
  graphGranularity: "file" | "module_class_method";
  setGraphGranularity: (v: "file" | "module_class_method") => void;
  commentsCollapsed: boolean;
  setCommentsCollapsed: (v: boolean) => void;
  infraExpanded: boolean;
  setInfraExpanded: (v: boolean) => void;
  infraShowAll: boolean;
  setInfraShowAll: (v: boolean) => void;
  infraSubGroupsExpanded: Set<string>;
  setInfraSubGroupsExpanded: AnyFn;
  rightPanelCollapsed: boolean;
  setRightPanelCollapsed: (v: boolean) => void;
  rightPanelWidth: number;
  setRightPanelWidth: (v: number) => void;
  watchedManifestPath: string | null;
  setWatchedManifestPath: (v: string | null) => void;
  updateAvailable: { version: string; body: string } | null;
  setUpdateAvailable: (v: { version: string; body: string } | null) => void;
  updating: boolean;
  setUpdating: (v: boolean) => void;
  toast: string | null;
  setToast: (v: string | null) => void;

  // ── Refs ──
  diffViewerRef: React.RefObject<DiffViewerHandle>;
  commentInputRef: React.RefObject<HTMLTextAreaElement>;
  repoInputRef: React.RefObject<HTMLInputElement>;
  activityLogRef: React.RefObject<HTMLDivElement>;
  crossFileSearchInputRef: React.RefObject<HTMLInputElement>;
  pendingScrollToCommentRef: React.MutableRefObject<{ startLine: number; endLine?: number; commentId: string } | null>;
  pendingReplayHunkScrollRef: React.MutableRefObject<{ filePath: string; targetHunkIndex: number; attempts: number; stepIndex: number; direction?: 1 | -1 } | null>;
  pendingSymbolScrollRef: React.MutableRefObject<{ symbol: string } | null>;

  // ── Additional computed/derived ──
  headLabel: string;
  baseLabel: string;
  baseBranches: BranchInfo[];
  statusText: string | null;
  aiAccessReady: boolean;
  annotationsEnabled: boolean;
  replayHunks: ReplayHunk[];
  hasPrevReplayHunk: boolean;
  hasNextReplayHunk: boolean;
  codeCommentsForSelectedFile: ReviewComment[];

  // ── Editor panel state ──
  lastEditor: string;
  setLastEditor: (v: string) => void;
  openWithDropdown: boolean;
  setOpenWithDropdown: (v: boolean) => void;
  openWithRef: React.RefObject<HTMLDivElement>;
  editorIcons: Record<string, string>;
  editorOptions: Array<{ id: string; label: string }>;

  // ── Additional refs ──
  selectedGroupRef: React.MutableRefObject<FlowGroup | null>;
  selectedFileRef: React.MutableRefObject<string | null>;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  fileEditBaselineRef: React.MutableRefObject<Map<string, any>>;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  pendingEditSync: React.MutableRefObject<any>;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  latestEditPayloadRef: React.MutableRefObject<any>;

  // ── Additional functions ──
  tauriInvoke: AnyFn;
  isApiProvider: (provider: string) => boolean;
  restoreLastSessionState: AnyFn;
  fetchModelsForProvider: AnyFn;
  saveLlmSettings: AnyFn;
  commentCountForGroup: (groupId: string) => number;
  commentsForFile: (filePath: string) => unknown[];
  mapEditedHunksToReplayHunks: AnyFn;
  commentOnCurrentReplayHunk: AnyFn;
  jumpToReplayHunk: AnyFn;

  // ── Named callbacks ──
  handleSelectFile: AnyFn;
  handleSelectFileDebounced: AnyFn;
  openFileInTab: AnyFn;
  handleGraphNodeClick: AnyFn;
  handleGraphEdgeClick: AnyFn;
  handleEdgeEndpointClick: AnyFn;
  handleSourceNavigate: AnyFn;
  handleGoToDefinition: AnyFn;
  resolveGroupFilePath: AnyFn;
  handleSelectGroup: AnyFn;
  runAnalysis: AnyFn;
  runAnnotateOverview: AnyFn;
  runDeepAnalysis: AnyFn;
  showToast: AnyFn;
  applyRefinementResult: AnyFn;
  runRefinement: AnyFn;
  toggleRefinedView: AnyFn;
  dismissDirectApiNotice: AnyFn;
  closeActivityStream: AnyFn;
  modelsForProvider: AnyFn;
  updateSetting: AnyFn;
  handleSaveApiKey: AnyFn;
  handleClearApiKey: AnyFn;
  handleAddIgnorePath: AnyFn;
  handleRemoveIgnorePath: AnyFn;
  openAiSetup: AnyFn;
  dismissAiSetup: AnyFn;
  refreshAiAccess: AnyFn;
  activateSubscriptionProvider: AnyFn;
  activatePreferredActivityProvider: AnyFn;
  openApiKeyFallback: AnyFn;
  toggleGroupReviewed: AnyFn;
  buildAbsolutePath: AnyFn;
  copyFilePath: AnyFn;
  copyFlowPaths: AnyFn;
  autoEditCommentText: AnyFn;
  syncEditedFileAndComments: AnyFn;
  copyPrDescription: AnyFn;
  loadComments: AnyFn;
  saveComment: AnyFn;
  deleteComment: AnyFn;
  updateComment: AnyFn;
  openCommentInput: AnyFn;
  submitComment: AnyFn;
  cancelComment: AnyFn;
  exportComments: AnyFn;
  importGroupsManifest: AnyFn;
  exportGroupsManifest: AnyFn;
  buildManifestAgentPrompt: AnyFn;
  shouldRenderSideBySide: AnyFn;
  openInEditor: AnyFn;
  openCrossFileSearchResult: AnyFn;
  runCrossFileSearch: AnyFn;
  handleFileContextMenu: AnyFn;
  enterReplay: AnyFn;
  exitReplay: AnyFn;
  goToReplayStep: AnyFn;
  navigateReplayHunk: AnyFn;
  handleSelectBase: AnyFn;
  handleSelectHead: AnyFn;
  startRightPanelDrag: AnyFn;
  browseForRepository: AnyFn;
  closeTab: AnyFn;
  closeOtherTabs: AnyFn;
  closeAllTabs: AnyFn;

  // ── Computed values ──
  sortedGroups: FlowGroup[];
  activityTimeline: Array<{ id: string; entry: LlmActivityEntry; presentation: ActivityPresentation }>;
  visibleActivityTimeline: Array<{ id: string; entry: LlmActivityEntry; presentation: ActivityPresentation }>;
  activityStats: { total: number; search: number; read: number; command: number };
  refinementVerdict: {
    title: string;
    provider: string;
    model: string;
    reasoning?: string | null;
    hadChanges: boolean;
  } | null;
  commentsByGroupMap: Map<string, ReviewComment[]>;
  commentsByFileMap: Map<string, ReviewComment[]>;
  commentsByFile: Map<string, ReviewComment[]>;
  changedFilePathSet: Set<string>;
  recommendedSubscriptionProvider: SubscriptionProvider | null;
  resolvedPrimaryProvider: LlmProvider | null;
  resolvedRefinementProvider: LlmProvider | null;
  resolvedPrimaryModel: string | null;
  resolvedRefinementModel: string | null;
  activityEventProvider: string | null;
  activitySupportsToolStreaming: boolean;
  activityIsDirectApi: boolean;
  showDirectApiBanner: boolean;
  groupAnnotation: unknown;
  groupDeepAnalysis: unknown;
  selectedFileChange: unknown;
  IS_TAURI: boolean;
}

export const AppContext = createContext<AppContextValue | null>(null);

export function useAppContext(): AppContextValue {
  const ctx = useContext(AppContext);
  if (!ctx) {
    throw new Error("useAppContext must be used within AppContext.Provider");
  }
  return ctx;
}
