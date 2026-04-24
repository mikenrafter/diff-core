/**
 * CommentsTab — right-panel review comment list for the current analysis.
 *
 * Groups comments by file (with a special "Group comments" bucket for
 * group-level notes), supports inline editing, and allows navigating to
 * the file/line of any comment.
 *
 * The `commentsByFile` memo lives here because it is only consumed in this
 * component. All other state comes from AppContext.
 */
import { useMemo } from "react";
import type { FlowGroup, ReviewComment } from "../../types";
import { shortPath } from "../../utils/pathUtils";
import { useAppContext } from "../../hooks/AppContext";

export function CommentsTab() {
  const {
    comments,
    openCommentInput,
    exportComments,
    activeCommentId,
    setActiveCommentId,
    analysis,
    openFileInTab,
    selectedFile,
    fileDiff,
    diffViewerRef,
    pendingScrollToCommentRef,
    editingCommentId,
    setEditingCommentId,
    editingCommentText,
    setEditingCommentText,
    updateComment,
    deleteComment,
  } = useAppContext();

  // Group comments by file path; group-level comments use a synthetic key.
  const commentsByFile = useMemo(() => {
    const map = new Map<string, ReviewComment[]>();
    for (const c of comments) {
      const key = c.file_path ?? `__group__${c.group_id}`;
      const arr = map.get(key);
      if (arr) arr.push(c);
      else map.set(key, [c]);
    }
    return map;
  }, [comments]);

  return (
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
                      onClick={(e) => { e.stopPropagation(); void deleteComment(comment.id); }}
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
                            if (editingCommentText.trim()) void updateComment(comment.id, editingCommentText.trim());
                          }
                          if (e.key === "Escape") { e.preventDefault(); setEditingCommentId(null); }
                        }}
                      />
                      <div className="comment-strip-edit-actions">
                        <span className="comment-strip-edit-hint">Cmd+Enter to save</span>
                        <button className="btn btn-comment-save" disabled={!editingCommentText.trim()} onClick={() => void updateComment(comment.id, editingCommentText.trim())}>Save</button>
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
}
