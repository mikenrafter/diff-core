import { useAppContext } from "../../hooks/AppContext";
import { IS_TAURI, STATE_SAVE_RESTORE_ENABLED } from "../../utils/tauriUtils";

/**
 * Top navigation bar — repo path input, branch selectors, Analyze button, and
 * quick-access controls for settings and AI setup.
 */
export function HeaderBar() {
  const {
    repoInfo,
    restoreLastSessionState,
    aiAccessReady, llmSettings, openAiSetup,
    statusText,
    analysis, reviewedGroupIds, sortedGroups,
  } = useAppContext();

  return (
      <header className="top-bar">
        <div className="top-bar-left">
          <span className="logo">Diffcore</span>
        </div>
        <div className="top-bar-center" />
        <div className="top-bar-right">
          {IS_TAURI && STATE_SAVE_RESTORE_ENABLED && (
            <button
              className="btn"
              onClick={() => { void restoreLastSessionState(); }}
              title="Restore the latest saved application state"
            >
              Restore Session
            </button>
          )}
          {!aiAccessReady && llmSettings && (
            <button
              className="btn btn-ai-setup"
              onClick={() => openAiSetup("recommended")}
              title="Choose Codex CLI, Claude Code, or a direct API key"
            >
              Setup AI
            </button>
          )}
          {/* Branch status indicator */}
          {repoInfo && (
            <div className="repo-status">
              {repoInfo.current_branch && (
                <span className="current-branch" title="Current branch">
                  <span className="branch-icon">&#9741;</span>
                  {repoInfo.current_branch}
                </span>
              )}
              {statusText && (
                <span className="push-status" title="Tracking status">
                  {statusText}
                </span>
              )}
              {/* Worktree indicator */}
              {repoInfo.worktrees.length > 1 && (
                <span className="worktree-badge" title={`${repoInfo.worktrees.length} worktrees`}>
                  {repoInfo.worktrees.length} worktrees
                </span>
              )}
            </div>
          )}
          {analysis && (
            <span className="summary">
              {analysis.summary.total_files_changed} files,{" "}
              {analysis.summary.total_groups} groups
              {reviewedGroupIds.size > 0 && (
                <span className="reviewed-counter">
                  {" "}&middot; {reviewedGroupIds.size}/{sortedGroups.length} reviewed
                </span>
              )}
            </span>
          )}
        </div>
      </header>
  );
}
