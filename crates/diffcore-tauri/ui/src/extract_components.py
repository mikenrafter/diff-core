"""Extract JSX sections from App.tsx into component files."""

with open("App.tsx", "r") as f:
    lines = f.readlines()

def L(n): 
    """Get line content (1-indexed)."""
    return lines[n-1]

def section_str(start_line, end_line):
    """Extract lines start_line..end_line (1-indexed, inclusive) as string."""
    return "".join(lines[start_line-1:end_line])

# ─── CrashTest component ────────────────────────────────────────────────────
crash_test = """\
import { useAppContext } from "../hooks/AppContext";

/** Test-only component that throws during render to exercise ErrorBoundary. */
export function CrashTest({ panel }: { panel: string }) {
  const { crashPanel } = useAppContext();
  if (crashPanel === panel) {
    throw new Error(`Test crash in ${panel}`);
  }
  return null;
}
"""
with open("components/CrashTest.tsx", "w") as f:
    f.write(crash_test)
print("Created components/CrashTest.tsx")

# ─── HeaderBar ──────────────────────────────────────────────────────────────
header_content = section_str(3862, 4175)
header_tsx = """\
import { useAppContext } from "../../hooks/AppContext";
import { COMPARE_TARGET_UNSTAGED, COMPARE_TARGET_STAGED, STATE_SAVE_RESTORE_ENABLED } from "../../utils/constants";
import { shortPath } from "../../utils/pathUtils";

export function HeaderBar() {
  const ctx = useAppContext();
  const {
    repoInputRef, repoPath, setRepoPath, loading, browseForRepository,
    setRepoQuickPickOpen, favoriteRepoPaths, setFavoriteRepoPaths,
    headBranchDropdownOpen, setHeadBranchDropdownOpen, setBranchDropdownOpen,
    headRef, handleSelectHead, baseBranches,
    showHeadCommits, setShowHeadCommits, recentCommits,
    repoInfo, branchDropdownOpen,
    baseRef, handleSelectBase,
    showBaseCommits, setShowBaseCommits,
    repoQuickPickOpen,
    headLabel, baseLabel,
    runAnalysis, comparisonMode,
    IS_TAURI, restoreLastSessionState,
    aiAccessReady, llmSettings, openAiSetup,
    settingsOpen, setSettingsOpen,
    analysis, reviewedGroupIds, sortedGroups,
    updating, setUpdating, updateAvailable, setUpdateAvailable,
    statusText,
  } = ctx;

  return (
""" + header_content + """\
  );
}
"""
with open("components/panels/HeaderBar.tsx", "w") as f:
    f.write(header_tsx)
print("Created components/panels/HeaderBar.tsx")

# ─── AISetupModal ────────────────────────────────────────────────────────────
ai_setup_content = section_str(4177, 4336)
ai_setup_tsx = """\
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
""" + ai_setup_content + """\
    </>
  );
}
"""
with open("components/modals/AISetupModal.tsx", "w") as f:
    f.write(ai_setup_tsx)
print("Created components/modals/AISetupModal.tsx")

# ─── SettingsPanel ───────────────────────────────────────────────────────────
settings_content = section_str(4339, 4629)
settings_tsx = """\
import Dropdown from "../Dropdown";
import { useAppContext } from "../../hooks/AppContext";
import { PROVIDER_LABELS } from "../../utils/constants";
import type { LlmProvider, DiffViewMode } from "../../types";

export function SettingsPanel() {
  const ctx = useAppContext();
  const {
    settingsOpen, setSettingsOpen, llmSettings,
    includeUncommitted, setIncludeUncommitted, saveLlmSettings,
    diffViewMode, setDiffViewMode, baseRef,
    openAiSetup, aiAccessReady, recommendedSubscriptionProvider,
    resolvedPrimaryProvider, resolvedRefinementProvider,
    resolvedPrimaryModel, resolvedRefinementModel,
    isApiProvider, updateSetting, modelsForProvider, modelsLoading,
    fetchModelsForProvider, handleSaveApiKey, handleClearApiKey,
    activatePreferredActivityProvider,
    apiKeyInput, setApiKeyInput,
    ignorePaths, ignorePathInput, setIgnorePathInput,
    handleAddIgnorePath, handleRemoveIgnorePath,
  } = ctx;

  // LLM_PROVIDERS is imported from types
  const LLM_PROVIDERS = ["openai", "anthropic", "gemini", "codex", "claude"] as const;

  return (
    <>
""" + settings_content + """\
    </>
  );
}
"""
with open("components/modals/SettingsPanel.tsx", "w") as f:
    f.write(settings_tsx)
print("Created components/modals/SettingsPanel.tsx")

# ─── LeftPane ────────────────────────────────────────────────────────────────
left_content = section_str(4644, 5122)
left_tsx = """\
import FileDisplay from "../FileDisplay";
import { useAppContext } from "../../hooks/AppContext";
import { PROVIDER_LABELS } from "../../utils/constants";
import { riskLevel, getGroupChangeIndicator, getFileMovedIndicator } from "../../utils/groupUtils";
import { resolveFileShortStatus } from "../../utils/gitUtils";
import { shortPath, truncateSearchResultLine } from "../../utils/pathUtils";
import type { InfraSubGroup } from "../../types";

export function LeftPane() {
  const ctx = useAppContext();
  const {
    comments, exportComments,
    showRefined, refinementProvider, refinementModel, refinementResponse,
    analysis, refining, refinedGroups, originalGroups, aiAccessReady,
    toggleRefinedView, runRefinement,
    resolvedRefinementProvider, resolvedRefinementModel,
    crossFileSearchOpen, setCrossFileSearchOpen,
    crossFileSearchQuery, setCrossFileSearchQuery,
    crossFileSearchLoading, crossFileSearchResults, setCrossFileSearchResults,
    crossFileSearchError, setCrossFileSearchError,
    showUnchangedFiles, setShowUnchangedFiles,
    runCrossFileSearch, openCrossFileSearchResult,
    groupListTransitionState, sortedGroups,
    selectedGroup, handleSelectGroup, selectedFile,
    reviewedGroupIds, toggleGroupReviewed, copyFlowPaths,
    openFileInTab, handleFileContextMenu,
    commentCountForGroup, commentsForFile,
    replayActive, replayVisited,
    fileStatusByPath, pendingScrollToCommentRef,
    rightPanelCollapsed, setRightPanelCollapsed,
    setRightPanelTab, setActiveCommentId,
    infraExpanded, setInfraExpanded, infraShowAll, setInfraShowAll,
    infraSubGroupsExpanded, setInfraSubGroupsExpanded,
    IS_TAURI, watchedManifestPath, setWatchedManifestPath,
    tauriInvoke, buildManifestAgentPrompt, exportGroupsManifest, showToast,
    repoPath, crossFileSearchInputRef, loading,
  } = ctx;

  return (
""" + left_content + """\
  );
}
"""
with open("components/panels/LeftPane.tsx", "w") as f:
    f.write(left_tsx)
print("Created components/panels/LeftPane.tsx")

# ─── CenterPane ──────────────────────────────────────────────────────────────
center_content = section_str(5125, 5555)
center_tsx = """\
import DiffViewer from "../DiffViewer";
import ErrorBoundary from "../ErrorBoundary";
import { CrashTest } from "../CrashTest";
import { useAppContext } from "../../hooks/AppContext";
import { shortPath } from "../../utils/pathUtils";
import { computeToolEditHunks } from "../../utils/pathUtils";
import type { EditedHunk } from "../DiffViewer";

export function CenterPane() {
  const ctx = useAppContext();
  const {
    fileDiff, selectedFile, editsEnabled, shouldRenderSideBySide,
    openWithRef, openWithDropdown, setOpenWithDropdown,
    lastEditor, setLastEditor, editorIcons, editorOptions,
    openInEditor,
    replayActive, selectedGroup, replayStep, replayHunkIndex,
    replayViewedHunkIds, replayHunks, hasPrevReplayHunk, hasNextReplayHunk,
    navigateReplayHunk, goToReplayStep, exitReplay, commentOnCurrentReplayHunk,
    openTabs, handleSelectFile, setTabContextMenu, closeTab,
    diffViewerRef, selectedGroupRef, selectedFileRef,
    openCommentInput, codeCommentsForSelectedFile,
    setActiveCommentId, setRightPanelTab, rightPanelCollapsed, setRightPanelCollapsed,
    handleGoToDefinition,
    fileEditBaselineRef, pendingEditSync, latestEditPayloadRef,
    syncEditedFileAndComments, mapEditedHunksToReplayHunks,
    setCurrentReplayHunks, setReplayHunkIndex, pendingReplayHunkScrollRef,
    comments, activeCommentId, commentsCollapsed, setCommentsCollapsed,
    setEditingCommentId, setEditingCommentText, editingCommentId, editingCommentText,
    deleteComment, updateComment,
    replayVisited,
  } = ctx;

  return (
""" + center_content + """\
  );
}
"""
with open("components/panels/CenterPane.tsx", "w") as f:
    f.write(center_tsx)
print("Created components/panels/CenterPane.tsx")

# ─── RightPane ────────────────────────────────────────────────────────────────
annotations_content = section_str(3175, 3443)
activity_content = section_str(3445, 3673)
# sourceTabContent is a single short reference — keep inline
comments_tab_content = section_str(3701, 3816)
right_aside_content = section_str(5564, 5718)

# Build sourceTabContent inline
source_tab_inline = """      <SourceExplorer
        fileDiff={fileDiff}
        selectedGroup={selectedGroup}
        selectedFileChange={selectedFileChange}
        focusRequest={sourceFocusRequest}
        onNavigateToSymbol={handleSourceNavigate}
        onScrollToLine={(startLine: number, endLine: number) => {
          diffViewerRef.current?.scrollToLine(startLine, endLine);
        }}
      />"""

right_tsx = """\
import { useMemo } from "react";
import FlowGraph from "../FlowGraph";
import ErrorBoundary from "../ErrorBoundary";
import FileDisplay from "../FileDisplay";
import Dropdown from "../Dropdown";
import SourceExplorer from "../SourceExplorer";
import { CrashTest } from "../CrashTest";
import { useAppContext } from "../../hooks/AppContext";
import { PROVIDER_LABELS, ACTIVITY_STREAM_LIMIT } from "../../utils/constants";
import { shortPath, symbolFilePath, shortSymbol } from "../../utils/pathUtils";
import { formatActivityTimestamp } from "../../utils/activityUtils";
import type { LlmProvider, ReviewComment, FlowGroup } from "../../types";

export function RightPane() {
  const ctx = useAppContext();
  const {
    selectedGroup, annotationSubTab, setAnnotationSubTab,
    llmSettings, updateSetting, modelsForProvider,
    setRegenDialogOpen, setRegenFeedbackText, setRegenIncludePreviousOutput,
    replayActive, enterReplay, exitReplay, replayStep, replayVisited,
    refinementVerdict, overview, groupAnnotation, groupDeepAnalysis,
    graphGranularity, setGraphGranularity,
    handleGraphNodeClick, handleGraphEdgeClick, handleEdgeEndpointClick,
    activityJob, activityTimeline, visibleActivityTimeline,
    activityEventProvider, activitySupportsToolStreaming, activityIsDirectApi,
    activityStats, activityError, activityViewMode, setActivityViewMode,
    showDirectApiBanner, PROVIDER_LABELS: _pLabels,
    activatePreferredActivityProvider, dismissDirectApiNotice,
    recommendedSubscriptionProvider, inspectedActivityId, setInspectedActivityId,
    activityLogRef, comments, commentsByFile,
    openCommentInput, exportComments, shortPath: _sp,
    analysis, openFileInTab, setActiveCommentId, selectedFile, fileDiff,
    diffViewerRef, pendingScrollToCommentRef, activeCommentId,
    editingCommentId, setEditingCommentId, editingCommentText, setEditingCommentText,
    deleteComment, updateComment,
    rightPanelCollapsed, setRightPanelCollapsed, rightPanelWidth,
    rightPanelTab, setRightPanelTab,
    annotating, deepAnalyzing, refining, aiAccessReady,
    openAiSetup, copyPrDescription,
    resolvedPrimaryProvider, resolvedPrimaryModel, annotationsEnabled,
    runAnnotateOverview, runDeepAnalysis,
    sourceFocusRequest, handleSourceNavigate, selectedFileChange,
    startRightPanelDrag,
  } = ctx;

""" + "  const annotationsTabContent = " + annotations_content + """;

  const activityTabContent = """ + activity_content + """;

  const sourceTabContent = (
""" + source_tab_inline + """
  );

  const commentsTabContent = """ + comments_tab_content + """;

  return (
    <>
      {/* Right panel drag handle */}
      <div
        className="panel-resize-handle"
        onMouseDown={startRightPanelDrag}
      />
""" + right_aside_content + """
    </>
  );
}
"""
with open("components/panels/RightPane.tsx", "w") as f:
    f.write(right_tsx)
print("Created components/panels/RightPane.tsx")

# ─── CommentInputOverlay ────────────────────────────────────────────────────
comment_overlay_content = section_str(5750, 5796)
comment_overlay_tsx = """\
import { useAppContext } from "../../hooks/AppContext";
import { shortPath } from "../../utils/pathUtils";

export function CommentInputOverlay() {
  const ctx = useAppContext();
  const {
    commentInput, commentInputRef, commentText, setCommentText,
    submitComment, cancelComment,
  } = ctx;

  return (
    <>
""" + comment_overlay_content + """\
    </>
  );
}
"""
with open("components/modals/CommentInputOverlay.tsx", "w") as f:
    f.write(comment_overlay_tsx)
print("Created components/modals/CommentInputOverlay.tsx")

# ─── RegenDialog ────────────────────────────────────────────────────────────
regen_content = section_str(5798, 5841)
regen_tsx = """\
import { useAppContext } from "../../hooks/AppContext";

export function RegenDialog() {
  const ctx = useAppContext();
  const {
    regenDialogOpen, setRegenDialogOpen,
    regenFeedbackText, setRegenFeedbackText,
    regenIncludePreviousOutput, setRegenIncludePreviousOutput,
    runAnnotateOverview, annotating,
  } = ctx;

  return (
    <>
""" + regen_content + """\
    </>
  );
}
"""
with open("components/modals/RegenDialog.tsx", "w") as f:
    f.write(regen_tsx)
print("Created components/modals/RegenDialog.tsx")

print("\nAll component files created!")
