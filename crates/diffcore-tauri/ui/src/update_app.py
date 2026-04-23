"""Update App.tsx to use extracted components and AppContext.Provider."""

with open("App.tsx", "r") as f:
    lines = f.readlines()

# Helper: 0-indexed slice of 1-indexed range [a, b] inclusive
def sl(a, b):
    return lines[a-1:b]

# ── 1. Extract commentsByFile useMemo (lines 3689–3699) ─────────────────────
comments_by_file = sl(3689, 3699)

# ── 2. New imports to add (after existing imports block, around line 77) ────
NEW_IMPORTS = """\
import { AppContext } from "./hooks/AppContext";
import type { AppContextValue } from "./hooks/AppContext";
import { HeaderBar } from "./components/panels/HeaderBar";
import { AISetupModal } from "./components/modals/AISetupModal";
import { SettingsPanel } from "./components/modals/SettingsPanel";
import { LeftPane } from "./components/panels/LeftPane";
import { CenterPane } from "./components/panels/CenterPane";
import { RightPane } from "./components/panels/RightPane";
import { CommentInputOverlay } from "./components/modals/CommentInputOverlay";
import { RegenDialog } from "./components/modals/RegenDialog";
"""

# ── 3. ctxValue block to insert before `return (` ────────────────────────────
CTX_VALUE = """\
  const ctxValue: AppContextValue = {
    // Core analysis
    analysis, setAnalysis, selectedGroup, setSelectedGroup,
    selectedFile, setSelectedFile, fileDiff, setFileDiff,
    loading, setLoading, error, setError, crashPanel, setCrashPanel,
    // LLM annotation
    overview, setOverview, deepAnalyses, setDeepAnalyses,
    annotating, setAnnotating, deepAnalyzing, setDeepAnalyzing,
    activityJob, setActivityJob, activityEntries, setActivityEntries,
    activityError, setActivityError, activityViewMode, setActivityViewMode,
    inspectedActivityId, setInspectedActivityId,
    rightPanelTab, setRightPanelTab,
    sourceFocusRequest, setSourceFocusRequest,
    regenDialogOpen, setRegenDialogOpen,
    regenFeedbackText, setRegenFeedbackText,
    regenIncludePreviousOutput, setRegenIncludePreviousOutput,
    // Repo + git
    repoPath, setRepoPath, baseRef, setBaseRef,
    headRef, setHeadRef, repoInfo, setRepoInfo,
    branchDropdownOpen, setBranchDropdownOpen,
    headBranchDropdownOpen, setHeadBranchDropdownOpen,
    recentCommits, setRecentCommits,
    showHeadCommits, setShowHeadCommits,
    showBaseCommits, setShowBaseCommits,
    // Diff mode
    diffViewMode, setDiffViewMode,
    directApiNoticeDismissed, setDirectApiNoticeDismissed,
    // Provider / model
    providerModels, setProviderModels,
    modelsLoading, setModelsLoading,
    hasApiKey, setHasApiKey,
    // Diff behavior
    includeUncommitted, setIncludeUncommitted,
    showUnchangedFiles, setShowUnchangedFiles,
    crossFileSearchOpen, setCrossFileSearchOpen,
    crossFileSearchQuery, setCrossFileSearchQuery,
    crossFileSearchLoading, setCrossFileSearchLoading,
    crossFileSearchResults, setCrossFileSearchResults,
    crossFileSearchError, setCrossFileSearchError,
    fileStatusByPath, setFileStatusByPath,
    repoQuickPickOpen, setRepoQuickPickOpen,
    recentRepoPaths, setRecentRepoPaths,
    favoriteRepoPaths, setFavoriteRepoPaths,
    // Derived comparison state
    comparisonMode, analysisDiffArgs, fileDiffArgs, editsEnabled,
    // LLM settings
    llmSettings, setLlmSettings,
    settingsOpen, setSettingsOpen,
    aiSetupOpen, setAiSetupOpen,
    aiSetupStep, setAiSetupStep,
    apiProviderDraft, setApiProviderDraft,
    apiKeyInput, setApiKeyInput,
    // Ignore paths
    ignorePaths, setIgnorePaths,
    ignorePathInput, setIgnorePathInput,
    // Review progress
    reviewedGroupIds, setReviewedGroupIds,
    // Replay
    replayActive, setReplayActive,
    replayStep, setReplayStep,
    replayVisited, setReplayVisited,
    replayHunkIndex, setReplayHunkIndex,
    replayViewedHunkIds, setReplayViewedHunkIds,
    currentReplayHunks, setCurrentReplayHunks,
    // Refinement
    originalGroups, setOriginalGroups,
    refinedGroups, setRefinedGroups,
    refinementResponse, setRefinementResponse,
    refinementProvider, setRefinementProvider,
    refinementModel, setRefinementModel,
    refinementHadChanges, setRefinementHadChanges,
    showRefined, setShowRefined,
    groupListTransitionState, setGroupListTransitionState,
    refining, setRefining,
    // Context menus
    contextMenu, setContextMenu,
    tabContextMenu, setTabContextMenu,
    // Tabs
    openTabs, setOpenTabs,
    // Comments
    comments, setComments,
    commentInput, setCommentInput,
    commentText, setCommentText,
    editingCommentId, setEditingCommentId,
    editingCommentText, setEditingCommentText,
    activeCommentId, setActiveCommentId,
    // UI state
    annotationSubTab, setAnnotationSubTab,
    graphGranularity, setGraphGranularity,
    commentsCollapsed, setCommentsCollapsed,
    infraExpanded, setInfraExpanded,
    infraShowAll, setInfraShowAll,
    infraSubGroupsExpanded, setInfraSubGroupsExpanded,
    rightPanelCollapsed, setRightPanelCollapsed,
    rightPanelWidth, setRightPanelWidth,
    watchedManifestPath, setWatchedManifestPath,
    updateAvailable, setUpdateAvailable,
    updating, setUpdating,
    toast, setToast,
    // Refs
    diffViewerRef, commentInputRef, repoInputRef,
    activityLogRef, crossFileSearchInputRef,
    pendingScrollToCommentRef, pendingReplayHunkScrollRef, pendingSymbolScrollRef,
    // Additional computed/derived
    headLabel, baseLabel, baseBranches, statusText,
    aiAccessReady, annotationsEnabled,
    replayHunks, hasPrevReplayHunk, hasNextReplayHunk,
    codeCommentsForSelectedFile,
    // Editor panel state
    lastEditor, setLastEditor,
    openWithDropdown, setOpenWithDropdown,
    openWithRef, editorIcons, editorOptions,
    // Additional refs
    selectedGroupRef, selectedFileRef,
    fileEditBaselineRef, pendingEditSync, latestEditPayloadRef,
    // Additional functions
    tauriInvoke, isApiProvider,
    restoreLastSessionState, fetchModelsForProvider, saveLlmSettings,
    commentCountForGroup, commentsForFile,
    mapEditedHunksToReplayHunks, commentOnCurrentReplayHunk, jumpToReplayHunk,
    // Named callbacks
    handleSelectFile, handleSelectFileDebounced,
    openFileInTab, handleGraphNodeClick, handleGraphEdgeClick,
    handleEdgeEndpointClick, handleSourceNavigate, handleGoToDefinition,
    resolveGroupFilePath, handleSelectGroup,
    runAnalysis, runAnnotateOverview, runDeepAnalysis,
    showToast, applyRefinementResult, runRefinement, toggleRefinedView,
    dismissDirectApiNotice, closeActivityStream,
    modelsForProvider, updateSetting, handleSaveApiKey, handleClearApiKey,
    handleAddIgnorePath, handleRemoveIgnorePath,
    openAiSetup, dismissAiSetup, refreshAiAccess,
    activateSubscriptionProvider, activatePreferredActivityProvider, openApiKeyFallback,
    toggleGroupReviewed, buildAbsolutePath, copyFilePath, copyFlowPaths,
    autoEditCommentText, syncEditedFileAndComments, copyPrDescription,
    loadComments, saveComment, deleteComment, updateComment,
    openCommentInput, submitComment, cancelComment, exportComments,
    importGroupsManifest, exportGroupsManifest, buildManifestAgentPrompt,
    shouldRenderSideBySide, openInEditor,
    openCrossFileSearchResult, runCrossFileSearch,
    handleFileContextMenu,
    enterReplay, exitReplay, goToReplayStep, navigateReplayHunk,
    handleSelectBase, handleSelectHead, startRightPanelDrag,
    browseForRepository, closeTab, closeOtherTabs, closeAllTabs,
    // Computed values
    sortedGroups, activityTimeline, visibleActivityTimeline,
    activityStats, refinementVerdict,
    commentsByGroupMap, commentsByFileMap, commentsByFile,
    changedFilePathSet, recommendedSubscriptionProvider,
    resolvedPrimaryProvider, resolvedRefinementProvider,
    resolvedPrimaryModel, resolvedRefinementModel,
    activityEventProvider, activitySupportsToolStreaming,
    activityIsDirectApi, showDirectApiBanner,
    groupAnnotation, groupDeepAnalysis, selectedFileChange, IS_TAURI,
  };

"""

# ── Build new file ────────────────────────────────────────────────────────────
out = []

# Section 1: lines 1–2603 (state + callbacks through commentsForFile)
out.extend(sl(1, 2603))

# Insert commentsByFile useMemo (moved from 3689)
out.append("\n  // Group comments by file for the Comments tab\n")
out.extend(comments_by_file)
out.append("\n")

# Section 2: lines 2604–3174 (rest of callbacks, computed values)
out.extend(sl(2604, 3174))

# Skip lines 3175–3825 (JSX var blocks + CrashTest)

# Insert ctxValue before return
out.append(CTX_VALUE)

# Line 3826: `  return (`
out.append("  return (\n")
# Line 3827: `    <div className="app">` — wrap with AppContext.Provider
out.append("    <AppContext.Provider value={ctxValue}>\n")
out.append("    <div className=\"app\">\n")

# Lines 3828–3860: update banner (keep as-is)
out.extend(sl(3828, 3860))

# Skip old header (3862–4175), replace with component
out.append("      <HeaderBar />\n")

# Skip aiSetupOpen block (4177–4336), replace
out.append("      {aiSetupOpen && llmSettings && <AISetupModal />}\n")

# Skip settingsOpen block (4339–4629), replace
out.append("      {settingsOpen && llmSettings && <SettingsPanel />}\n")

# Lines 4631–4643: error bar + panels div open
out.extend(sl(4631, 4643))

# Skip LeftPane (4644–5122), replace
out.append("        <LeftPane />\n")

# Skip CenterPane (5125–5555), replace
out.append("\n        <CenterPane />\n")

# Skip resize handle + RightPane (5557–5718), replace
out.append("\n        <RightPane />\n")

# Line 5719: `      </div>` closes .panels
out.extend(sl(5719, 5719))

# Lines 5720–5747: blank + keyboard hints
out.extend(sl(5720, 5747))

# Skip CommentInputOverlay (5750–5796), replace
out.append("\n      {commentInput && <CommentInputOverlay />}\n")

# Skip RegenDialog (5798–5841), replace
out.append("      {regenDialogOpen && <RegenDialog />}\n")

# Lines 5842–end: context menus, toast, closing tags
# But we need to modify the closing to add </AppContext.Provider>
# Lines 5842–5926: keep
out.extend(sl(5842, 5926))

# Replace the closing `    </div>\n  );\n}` with our wrapped version
out.append("    </div>\n")
out.append("    </AppContext.Provider>\n")
out.append("  );\n")
out.append("}\n")

# ── Add new imports after existing imports (insert after line 78) ─────────────
# Find last import line
import_end = 0
for i, line in enumerate(out):
    if line.startswith("import ") or line.strip().startswith("import "):
        import_end = i
# Insert new imports after the last import line
out.insert(import_end + 1, NEW_IMPORTS)

with open("App.tsx", "w") as f:
    f.writelines(out)

print(f"Done! New App.tsx has {len(out)} lines")
