/**
 * RegenDialog — overlay for regenerating annotations with user feedback.
 *
 * Accepts optional feedback text and an "include previous output" toggle
 * before triggering a new `runAnnotateOverview` pass. Renders only when
 * `regenDialogOpen` is truthy. All state from AppContext.
 */
import { useAppContext } from "../../hooks/AppContext";

export function RegenDialog() {
  const {
    regenDialogOpen,
    setRegenDialogOpen,
    regenFeedbackText,
    setRegenFeedbackText,
    regenIncludePreviousOutput,
    setRegenIncludePreviousOutput,
    runAnnotateOverview,
    annotating,
  } = useAppContext();

  if (!regenDialogOpen) return null;

  return (
    <div className="comment-overlay" onClick={() => setRegenDialogOpen(false)}>
      <div className="comment-input-panel" onClick={(e) => e.stopPropagation()}>
        <div className="comment-input-header">
          <span className="comment-input-scope">Regenerate with feedback/question</span>
          <button className="btn-close" onClick={() => setRegenDialogOpen(false)}>&times;</button>
        </div>
        <textarea
          className="comment-textarea"
          value={regenFeedbackText}
          onChange={(e) => setRegenFeedbackText(e.target.value)}
          placeholder="What should the next analysis focus on?"
          rows={5}
        />
        <label className="settings-toggle" style={{ display: "flex", alignItems: "center", gap: 8, marginTop: 8 }}>
          <input
            type="checkbox"
            checked={regenIncludePreviousOutput}
            onChange={(e) => setRegenIncludePreviousOutput(e.target.checked)}
          />
          <span>Include previous output as context</span>
        </label>
        <p className="settings-hint" style={{ marginTop: 8 }}>
          Your feedback and current review comments are always included in reanalysis context.
        </p>
        <div className="comment-input-footer">
          <button className="btn" onClick={() => setRegenDialogOpen(false)}>Cancel</button>
          <button
            className="btn btn-comment-save"
            onClick={() => {
              setRegenDialogOpen(false);
              void runAnnotateOverview({
                feedback: regenFeedbackText,
                includePreviousOutput: regenIncludePreviousOutput,
              });
            }}
            disabled={annotating}
          >
            {annotating ? "Regenerating..." : "Regenerate"}
          </button>
        </div>
      </div>
    </div>
  );
}
