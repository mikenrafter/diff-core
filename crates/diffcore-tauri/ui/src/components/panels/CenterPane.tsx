import { memo } from "react";
import type { EditedHunk } from "../DiffViewer";
import DiffViewer from "../DiffViewer";
import ErrorBoundary from "../ErrorBoundary";
import { CrashTest } from "../CrashTest";
import { useDiffContext } from "../../hooks/DiffContext";
import { shortPath } from "../../utils/pathUtils";
import { editorIcons } from "../../utils/editorUtils";

/**
 * Center panel — Monaco diff viewer with tab bar, replay controls, and
 * the "Open With" editor toolbar. DiffViewer event handlers are delegated
 * to context callbacks to keep internal App.tsx refs out of this component.
 */
export const CenterPane = memo(function CenterPane() {
  const {
    fileDiff, selectedFile, selectedGroup,
    openWithRef, openWithDropdown, setOpenWithDropdown,
    lastEditor, editorOptions, openInEditor,
    replayActive, replayStep,
    exitReplay,
    openTabs, handleSelectFile, closeTab, setTabContextMenu,
    diffViewerRef, editsEnabled, shouldRenderSideBySide,
    diffSplitRatio, setDiffSplitRatio,
    codeCommentsForSelectedFile,
    setActiveCommentId, setRightPanelTab, rightPanelCollapsed, setRightPanelCollapsed,
    handleGoToDefinition,
    handleDiffCommentRequest, handleDiffEditorContentChange, handleDiffHunksChanged,
  } = useDiffContext();

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

                <span className="diff-toolbar-divider" aria-hidden="true" />

                {replayActive && selectedGroup && (
                  <span className="replay-badge">REPLAY</span>
                )}
                {replayActive && selectedGroup && selectedGroup.files[replayStep] && (
                  <span className="replay-file-role">
                    {selectedGroup.files[replayStep].role}
                  </span>
                )}

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
          {/* Hunk/file navigation lives in the flow-groups pane (left / embedded). */}
          <div className="panel-body diff-viewer">
            <ErrorBoundary panelName="Diff Viewer">
              <CrashTest panel="Diff Viewer" />
              <DiffViewer
                ref={diffViewerRef}
                fileDiff={fileDiff}
                editable={editsEnabled}
                renderSideBySide={shouldRenderSideBySide}
                splitViewRatio={diffSplitRatio}
                onSplitViewRatioChange={setDiffSplitRatio}
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
        </main>
  );
});
