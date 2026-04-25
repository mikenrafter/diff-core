/**
 * CommentInputOverlay — modal overlay for writing a new review comment.
 *
 * Shows scope context (code selection, file, or group), an optional code
 * preview, a textarea, and save/cancel controls. Keyboard shortcuts:
 *  - Enter (without Shift) → save
 *  - Escape → cancel
 *
 * Renders only when `commentInput` is truthy. All state from AppContext.
 */
import { shortPath } from "../../utils/pathUtils";
import { useAppContext } from "../../hooks/AppContext";

export function CommentInputOverlay() {
  const {
    commentInput,
    commentText,
    setCommentText,
    commentInputRef,
    submitComment,
    cancelComment,
  } = useAppContext();

  if (!commentInput) return null;

  return (
    <div className="comment-overlay" onClick={cancelComment}>
      <div className="comment-input-panel" onClick={(e) => e.stopPropagation()}>
        <div className="comment-input-header">
          <span className="comment-input-scope">
            {commentInput.type === "code" && commentInput.file_path
              ? `${shortPath(commentInput.file_path)}:${commentInput.start_line}-${commentInput.end_line}`
              : commentInput.type === "file" && commentInput.file_path
                ? shortPath(commentInput.file_path)
                : "Group comment"}
          </span>
          <button className="btn-close" onClick={cancelComment}>&times;</button>
        </div>
        {commentInput.selected_code && (
          <pre className="comment-input-code">{commentInput.selected_code}</pre>
        )}
        <textarea
          ref={commentInputRef}
          className="comment-textarea"
          placeholder="Add a review comment..."
          value={commentText}
          onChange={(e) => setCommentText(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              submitComment();
            }
            if (e.key === "Escape") {
              e.preventDefault();
              cancelComment();
            }
          }}
          rows={3}
        />
        <div className="comment-input-footer">
          <span className="comment-input-hint">Enter to save, Escape to cancel, Shift+Enter for newline</span>
          <button
            className="btn btn-comment-save"
            onClick={submitComment}
            disabled={!commentText.trim()}
          >
            Save
          </button>
        </div>
      </div>
    </div>
  );
}
