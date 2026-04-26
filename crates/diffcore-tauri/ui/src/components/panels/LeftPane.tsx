import type { InfraSubGroup } from "../../types";
import { useAppContext } from "../../hooks/AppContext";
import FileDisplay from "../FileDisplay";
import { riskLevel, getGroupChangeIndicator, getFileMovedIndicator } from "../../utils/groupUtils";
import { resolveFileShortStatus } from "../../utils/gitUtils";
import { shortPath, truncateSearchResultLine } from "../../utils/pathUtils";
import { IS_TAURI, tauriInvoke } from "../../utils/tauriUtils";

/**
 * Left panel — flow group list with file navigation, refinement controls,
 * cross-file search, and the infrastructure group accordion.
 */
export function LeftPane({ embedded = false }: { embedded?: boolean }) {
  const {
    analysis, loading, sortedGroups, selectedGroup, selectedFile,
    handleSelectGroup, openFileInTab, reviewedGroupIds, toggleGroupReviewed,
    replayActive, replayVisited,
    comments, exportComments,
    monacoHunkCounts,
    reviewedHunksByFile,
    showRefined, refinedGroups, originalGroups, refinementProvider, refinementModel,
    refinementResponse, refining, runRefinement, toggleRefinedView,
    resolvedRefinementProvider, resolvedRefinementModel,
    commentCountForGroup, commentsForFile,
    setRightPanelTab, rightPanelCollapsed, setRightPanelCollapsed,
    crossFileSearchOpen, setCrossFileSearchOpen,
    crossFileSearchQuery, setCrossFileSearchQuery,
    crossFileSearchLoading, crossFileSearchResults, crossFileSearchError, crossFileSearchInputRef,
    runCrossFileSearch, openCrossFileSearchResult,
    showUnchangedFiles, setShowUnchangedFiles,
    watchedManifestPath, setWatchedManifestPath,
    groupListTransitionState, fileStatusByPath,
    handleFileContextMenu, copyFlowPaths, buildManifestAgentPrompt, exportGroupsManifest,
    infraExpanded, setInfraExpanded,
    infraShowAll, setInfraShowAll,
    infraSubGroupsExpanded, setInfraSubGroupsExpanded,
    expandedGroupIds, setExpandedGroupIds,
    aiAccessReady, repoPath, showToast,
    pendingScrollToCommentRef, setActiveCommentId,
    fileDiff,
    hasNextReplayHunk, hasPrevReplayHunk, navigateReplayHunk,
    hasNextFileInGroup, hasPrevFileInGroup, goToNextFileInGroup, goToPrevFileInGroup,
  } = useAppContext();

  const Wrapper = embedded ? "div" : "aside";
  const wrapperClassName = embedded ? "groups-pane-embedded" : "panel panel-left";

  return (
        <Wrapper className={wrapperClassName}>
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

            {/* Hunk / file navigation — same banner pattern as cross-file search */}
            {analysis && selectedGroup && fileDiff && (
              <div className="refinement-banner left-pane-hunk-file-nav">
                <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", width: "100%" }}>
                  <span>Hunk &amp; file navigation</span>
                </div>
                <div className="left-pane-hunk-file-nav-buttons">
                  <button
                    type="button"
                    className="btn btn-refine"
                    style={{ fontSize: 10 }}
                    onClick={() => navigateReplayHunk(-1)}
                    disabled={!hasPrevReplayHunk}
                    title="Previous hunk"
                  >
                    &lt; hunk
                  </button>
                  <button
                    type="button"
                    className="btn btn-refine"
                    style={{ fontSize: 10 }}
                    onClick={() => navigateReplayHunk(1)}
                    disabled={!hasNextReplayHunk}
                    title="Next hunk"
                  >
                    hunk &gt;
                  </button>
                  <button
                    type="button"
                    className="btn btn-refine"
                    style={{ fontSize: 10 }}
                    onClick={goToPrevFileInGroup}
                    disabled={!hasPrevFileInGroup}
                    title="Previous file in group"
                  >
                    &lt; file
                  </button>
                  <button
                    type="button"
                    className="btn btn-refine"
                    style={{ fontSize: 10 }}
                    onClick={goToNextFileInGroup}
                    disabled={!hasNextFileInGroup}
                    title="Next file in group"
                  >
                    file &gt;
                  </button>
                </div>
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
                const expanded = expandedGroupIds.has(group.id);

                return (
                  <div
                    key={group.id}
                    className={`group-item ${selectedGroup?.id === group.id ? "selected" : ""} ${changeIndicator ? "refined-change" : ""} ${reviewedGroupIds.has(group.id) ? "group-reviewed" : ""}`}
                    onClick={() => handleSelectGroup(group)}
                  >
                    <div
                      className="group-header"
                      onClick={(e) => {
                        e.stopPropagation();
                        setExpandedGroupIds((prev) => {
                          const next = new Set(prev);
                          if (next.has(group.id)) next.delete(group.id);
                          else next.add(group.id);
                          return next;
                        });
                      }}
                      title="Toggle flow group"
                    >
                      <span className="group-expand-indicator">
                        {expanded ? "\u25BC" : "\u25B6"}
                      </span>
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
                    {expanded && (
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
                                hunks={monacoHunkCounts.get(file.path) ?? file.changes.hunks}
                                reviewedHunks={reviewedHunksByFile.get(file.path) ?? 0}
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
        </Wrapper>
  );
}
