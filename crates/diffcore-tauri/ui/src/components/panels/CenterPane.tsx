import type { EditedHunk } from "../DiffViewer";
import DiffViewer from "../DiffViewer";
import ErrorBoundary from "../ErrorBoundary";
import { CrashTest } from "../CrashTest";
import { useAppContext } from "../../hooks/AppContext";
import { shortPath } from "../../utils/pathUtils";
import { editorIcons } from "../../utils/editorUtils";

/**
 * Center panel — Monaco diff viewer with tab bar, replay controls, and
 * the "Open With" editor toolbar. DiffViewer event handlers are delegated
 * to context callbacks to keep internal App.tsx refs out of this component.
 */
export function CenterPane() {
  const {
    fileDiff, selectedFile, selectedGroup,
    openWithRef, openWithDropdown, setOpenWithDropdown,
    lastEditor, editorOptions, openInEditor,
    replayActive, replayStep, replayVisited,
    replayHunks, replayHunkIndex, replayViewedHunkIds,
    hasNextReplayHunk, hasPrevReplayHunk,
    navigateReplayHunk, commentOnCurrentReplayHunk,
    goToReplayStep, exitReplay,
    openTabs, handleSelectFile, closeTab, setTabContextMenu,
    diffViewerRef, editsEnabled, shouldRenderSideBySide,
    codeCommentsForSelectedFile,
    setActiveCommentId, setRightPanelTab, rightPanelCollapsed, setRightPanelCollapsed,
    handleGoToDefinition,
    handleDiffCommentRequest, handleDiffEditorContentChange, handleDiffHunksChanged,
    // Dead-code comment strip fields — kept for type-safety while the feature is hidden
    comments, commentsCollapsed, setCommentsCollapsed,
    activeCommentId, editingCommentId, setEditingCommentId,
    editingCommentText, setEditingCommentText,
    deleteComment, updateComment,
  } = useAppContext();

  return (
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
                onCommentRequest={handleDiffCommentRequest}
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
                onEditedContentChange={handleDiffEditorContentChange}
                onDiffHunksChange={(hunks: EditedHunk[]) => handleDiffHunksChanged(hunks)}
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
  );
}
