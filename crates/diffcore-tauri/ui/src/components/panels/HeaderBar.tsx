import { useEffect, useRef, useState } from "react";
import { useAppContext } from "../../hooks/AppContext";
import { shortPath } from "../../utils/pathUtils";
import { COMPARE_TARGET_STAGED, COMPARE_TARGET_UNSTAGED } from "../../utils/gitUtils";
import { IS_TAURI, STATE_SAVE_RESTORE_ENABLED } from "../../utils/tauriUtils";

/**
 * Top navigation bar — repo path input, branch selectors, Analyze button, and
 * quick-access controls for settings and AI setup.
 */
export function HeaderBar() {
  const {
    repoPath, setRepoPath, repoInputRef, browseForRepository, loading,
    repoQuickPickOpen, setRepoQuickPickOpen, recentRepoPaths,
    favoriteRepoPaths, setFavoriteRepoPaths,
    headBranchDropdownOpen, setHeadBranchDropdownOpen,
    headRef, headLabel,
    branchDropdownOpen, setBranchDropdownOpen,
    baseRef, baseLabel,
    handleSelectHead, handleSelectBase,
    baseBranches, recentCommits,
    showHeadCommits, setShowHeadCommits,
    showBaseCommits, setShowBaseCommits,
    repoInfo, comparisonMode, runAnalysis,
    restoreLastSessionState,
    aiAccessReady, llmSettings, openAiSetup,
    settingsOpen, setSettingsOpen,
    statusText,
    analysis, reviewedGroupIds, sortedGroups,
  } = useAppContext();

  // Local draft state so we don't run repo probes on every keystroke.
  const [repoPathDraft, setRepoPathDraft] = useState(repoPath);
  const commitTimerRef = useRef<number | null>(null);

  useEffect(() => {
    setRepoPathDraft(repoPath);
  }, [repoPath]);

  const commitRepoPath = (nextRaw: string) => {
    const next = nextRaw.trim();
    if (next === repoPath) return;
    setRepoPath(next);
  };

  return (
      <header className="top-bar">
        <div className="top-bar-left">
          <span className="logo">Diffcore</span>
        </div>
        <div className="top-bar-center">
          <input
            ref={repoInputRef}
            className="input repo-input"
            type="text"
            placeholder="Repository path..."
            value={repoPathDraft}
            onChange={(e) => {
              const next = e.target.value;
              setRepoPathDraft(next);

              if (commitTimerRef.current) {
                window.clearTimeout(commitTimerRef.current);
                commitTimerRef.current = null;
              }

              // If user clears the input, commit immediately (so UI resets).
              if (next.trim().length === 0) {
                setRepoPath("");
                return;
              }

              // Debounce committing to app state so expensive repo checks run after idle.
              commitTimerRef.current = window.setTimeout(() => {
                commitRepoPath(next);
                commitTimerRef.current = null;
              }, 450);
            }}
            onBlur={() => {
              if (commitTimerRef.current) {
                window.clearTimeout(commitTimerRef.current);
                commitTimerRef.current = null;
              }
              commitRepoPath(repoPathDraft);
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter" && repoPathDraft.trim() && !loading) {
                (e.target as HTMLInputElement).blur();
                const path = repoPathDraft.trim();
                // Ensure repo info is loaded for the committed path.
                commitRepoPath(path);
                void runAnalysis(path);
              }
            }}
          />
          <button
            className="btn"
            onClick={() => { void browseForRepository(); }}
            title="Browse for a repository folder"
          >
            Browse
          </button>
          <button
            className="btn"
            onClick={() => setRepoQuickPickOpen((v) => !v)}
            title="Quick-pick recent and favorite repositories"
          >
            Recent
          </button>
          <button
            className="btn"
            onClick={() => {
              const path = repoPathDraft.trim();
              if (!path) return;
              commitRepoPath(path);
              setFavoriteRepoPaths((prev) => (
                prev.includes(path) ? prev.filter((p) => p !== path) : [path, ...prev]
              ));
            }}
            title="Pin or unpin current repository"
          >
            {favoriteRepoPaths.includes(repoPathDraft.trim()) ? "Unpin" : "Pin"}
          </button>

          {/* Branch comparison: head (source) → base (target) */}
          <div className="branch-comparison">
            {/* Head (source) branch dropdown */}
            <div className="branch-dropdown-wrapper" data-testid="head-branch-dropdown">
              <button
                className="btn branch-dropdown-trigger"
                onClick={() => {
                  setHeadBranchDropdownOpen(!headBranchDropdownOpen);
                  setBranchDropdownOpen(false);
                }}
                title="Select source branch (what you're comparing)"
              >
                <span className="branch-label">source</span>
                <span className="branch-icon">&#9741;</span>
                <span className="branch-name">{headLabel}</span>
                <span className="dropdown-arrow">&#9662;</span>
              </button>
              {headBranchDropdownOpen && (
                <ul className="branch-dropdown">
                  <li
                    className={`branch-option ${headRef === COMPARE_TARGET_UNSTAGED ? "selected" : ""}`}
                    onClick={() => handleSelectHead(COMPARE_TARGET_UNSTAGED)}
                  >
                    <span className="branch-option-name">Unstaged changes</span>
                  </li>
                  <li
                    className={`branch-option ${headRef === COMPARE_TARGET_STAGED ? "selected" : ""}`}
                    onClick={() => handleSelectHead(COMPARE_TARGET_STAGED)}
                  >
                    <span className="branch-option-name">Staged changes</span>
                  </li>
                  {baseBranches.map((b) => (
                    <li
                      key={b.name}
                      className={`branch-option ${b.name === headRef ? "selected" : ""} ${b.is_current ? "current" : ""}`}
                      onClick={() => handleSelectHead(b.name)}
                    >
                      <span className="branch-option-name">{b.name}</span>
                      {b.is_current && <span className="branch-current-badge">current</span>}
                      {b.has_upstream && <span className="branch-upstream-badge">tracked</span>}
                    </li>
                  ))}
                  {baseBranches.length === 0 && (
                    <li className="branch-option disabled">No branches found</li>
                  )}
                  {recentCommits.length > 0 && (
                    <>
                      <li
                        className="branch-option"
                        onClick={() => setShowHeadCommits((open) => !open)}
                        title="Show or hide recent commits"
                      >
                        <span className="branch-option-name">
                          {showHeadCommits ? "Hide recent commits" : "Show recent commits"}
                        </span>
                      </li>
                      {showHeadCommits && recentCommits.map((commit) => (
                        <li
                          key={`head-${commit.sha}`}
                          className={`branch-option ${commit.sha === headRef ? "selected" : ""}`}
                          onClick={() => handleSelectHead(commit.sha)}
                          title={`${commit.sha} · ${commit.author}`}
                        >
                          <span className="branch-option-name">{commit.short_sha} {commit.summary}</span>
                        </li>
                      ))}
                    </>
                  )}
                </ul>
              )}
            </div>

            <span className="branch-arrow" title="compared against">&#8594;</span>

            {/* Base (target) branch dropdown */}
            <div className="branch-dropdown-wrapper" data-testid="base-branch-dropdown">
              <button
                className="btn branch-dropdown-trigger"
                onClick={() => {
                  setBranchDropdownOpen(!branchDropdownOpen);
                  setHeadBranchDropdownOpen(false);
                }}
                title="Select target branch (what you're comparing against)"
              >
                <span className="branch-label">target</span>
                <span className="branch-icon">&#9741;</span>
                <span className="branch-name">{baseLabel}</span>
                <span className="dropdown-arrow">&#9662;</span>
              </button>
              {branchDropdownOpen && (
                <ul className="branch-dropdown">
                  <li
                    className={`branch-option ${baseRef === COMPARE_TARGET_UNSTAGED ? "selected" : ""}`}
                    onClick={() => handleSelectBase(COMPARE_TARGET_UNSTAGED)}
                  >
                    <span className="branch-option-name">Unstaged changes</span>
                  </li>
                  <li
                    className={`branch-option ${baseRef === COMPARE_TARGET_STAGED ? "selected" : ""}`}
                    onClick={() => handleSelectBase(COMPARE_TARGET_STAGED)}
                  >
                    <span className="branch-option-name">Staged changes</span>
                  </li>
                  {baseBranches.map((b) => (
                    <li
                      key={b.name}
                      className={`branch-option ${b.name === baseRef ? "selected" : ""} ${b.is_current ? "current" : ""}`}
                      onClick={() => handleSelectBase(b.name)}
                    >
                      <span className="branch-option-name">{b.name}</span>
                      {b.is_current && <span className="branch-current-badge">current</span>}
                      {b.has_upstream && <span className="branch-upstream-badge">tracked</span>}
                    </li>
                  ))}
                  {baseBranches.length === 0 && (
                    <li className="branch-option disabled">No branches found</li>
                  )}
                  {recentCommits.length > 0 && (
                    <>
                      <li
                        className="branch-option"
                        onClick={() => setShowBaseCommits((open) => !open)}
                        title="Show or hide recent commits"
                      >
                        <span className="branch-option-name">
                          {showBaseCommits ? "Hide recent commits" : "Show recent commits"}
                        </span>
                      </li>
                      {showBaseCommits && recentCommits.map((commit) => (
                        <li
                          key={`base-${commit.sha}`}
                          className={`branch-option ${commit.sha === baseRef ? "selected" : ""}`}
                          onClick={() => handleSelectBase(commit.sha)}
                          title={`${commit.sha} · ${commit.author}`}
                        >
                          <span className="branch-option-name">{commit.short_sha} {commit.summary}</span>
                        </li>
                      ))}
                    </>
                  )}
                </ul>
              )}
            </div>

            {/* Worktree indicator — shown when NOT a worktree (regular repo) */}
            {repoInfo && !repoInfo.is_worktree && (
              <span className="branch-repo-badge" title="Regular repository (not a worktree)">repo</span>
            )}
            {repoInfo?.is_worktree && (
              <span className="branch-worktree-badge" title="Linked worktree">worktree</span>
            )}
          </div>

          {repoQuickPickOpen && (
            <div className="branch-dropdown" style={{ maxHeight: 220, overflowY: "auto", minWidth: 320 }}>
              {favoriteRepoPaths.length > 0 && (
                <>
                  <li className="branch-option disabled">Favorites</li>
                  {favoriteRepoPaths.map((path) => (
                    <li
                      key={`fav-${path}`}
                      className="branch-option"
                      onClick={() => {
                        setRepoPath(path);
                        setRepoQuickPickOpen(false);
                      }}
                      title={path}
                    >
                      <span className="branch-option-name">★ {shortPath(path)}</span>
                    </li>
                  ))}
                </>
              )}
              {recentRepoPaths.length > 0 && (
                <>
                  <li className="branch-option disabled">Recent</li>
                  {recentRepoPaths.map((path) => (
                    <li
                      key={`recent-${path}`}
                      className="branch-option"
                      onClick={() => {
                        setRepoPath(path);
                        setRepoQuickPickOpen(false);
                      }}
                      title={path}
                    >
                      <span className="branch-option-name">{shortPath(path)}</span>
                    </li>
                  ))}
                </>
              )}
              {favoriteRepoPaths.length === 0 && recentRepoPaths.length === 0 && (
                <li className="branch-option disabled">No recent repositories</li>
              )}
            </div>
          )}

          <button
            className="btn btn-primary"
            onClick={() => {
              const path = repoPathDraft.trim();
              if (!path) return;
              commitRepoPath(path);
              void runAnalysis(path);
            }}
            disabled={loading || !repoPathDraft.trim() || comparisonMode === "invalid"}
          >
            {loading ? "Analyzing..." : "Analyze"}
          </button>
        </div>
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
          {/* Settings gear icon */}
          <button
            className="btn btn-settings"
            onClick={() => setSettingsOpen(!settingsOpen)}
            title="Settings"
          >
            &#9881;
          </button>
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
