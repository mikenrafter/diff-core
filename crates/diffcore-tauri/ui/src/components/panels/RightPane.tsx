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
        onScrollToLine={(startLine: number, endLine: number) => {
          diffViewerRef.current?.scrollToLine(startLine, endLine);
        }}
      />
  );

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

  return (
    <>
      {/* Right panel drag handle */}
      <div
        className="panel-resize-handle"
        onMouseDown={startRightPanelDrag}
      />
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

    </>
  );
}
