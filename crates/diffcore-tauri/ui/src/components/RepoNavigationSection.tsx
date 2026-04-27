import { useAppContext } from "../hooks/AppContext";
import { COMPARE_TARGET_STAGED, COMPARE_TARGET_UNSTAGED } from "../utils/gitUtils";
import { RepoPathCombobox } from "./RepoPathCombobox";

/**
 * RepoNavigationSection — repo + branch selectors + Analyze, rendered inside panels.
 * Kept as a component so HeaderBar can stay minimal while Info tab hosts navigation.
 */
export function RepoNavigationSection({ title }: { title?: string }) {
  const {
    repoPath, setRepoPath, repoInputRef, browseForRepository, loading,
    recentRepoPaths,
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
  } = useAppContext();

  const commitRepoPath = (nextRaw: string) => {
    const next = nextRaw.trim();
    if (next === repoPath) return;
    setRepoPath(next);
  };

  return (
    <div className="annotation-section" data-testid="repo-navigation-section">
      <h3>{title ?? "Repository"}</h3>

      <div className="repo-nav-grid">
        {/* Row 1: ([path selector] [pin/unpin star]) */}
        <div className="repo-nav-row repo-nav-row-1">
          <RepoPathCombobox
            inputRef={repoInputRef}
            value={repoPath}
            favorites={favoriteRepoPaths}
            recents={recentRepoPaths}
            disabled={loading}
            actions="none"
            triggerMaxWidth="100%"
            onCommit={(p) => {
              if (p.length === 0) setRepoPath("");
              else commitRepoPath(p);
            }}
            onToggleFavorite={(path) => {
              commitRepoPath(path);
              setFavoriteRepoPaths((prev) => (
                prev.includes(path) ? prev.filter((p) => p !== path) : [path, ...prev]
              ));
            }}
            onBrowse={() => { void browseForRepository(); }}
            onAnalyze={(path) => { void runAnalysis(path); }}
          />
          <button
            type="button"
            className="btn repo-nav-star"
            onClick={() => {
              const path = repoPath.trim();
              if (!path) return;
              commitRepoPath(path);
              setFavoriteRepoPaths((prev) => (
                prev.includes(path) ? prev.filter((p) => p !== path) : [path, ...prev]
              ));
            }}
            disabled={loading || !repoPath.trim()}
            title={favoriteRepoPaths.includes(repoPath.trim()) ? "Unpin repository" : "Pin repository"}
            aria-label={favoriteRepoPaths.includes(repoPath.trim()) ? "Unpin repository" : "Pin repository"}
          >
            {favoriteRepoPaths.includes(repoPath.trim()) ? "★" : "☆"}
          </button>
        </div>

        {/* Row 2: ([source]) */}
        <div className="repo-nav-row repo-nav-row-2">
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
        </div>

        {/* Row 3: ([target]) */}
        <div className="repo-nav-row repo-nav-row-3">
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
        </div>
        {/* Row 4: ([browse] [analyze]) */}
        <div className="repo-nav-row repo-nav-row-4">
          <button
            type="button"
            className="btn repo-nav-browse"
            onClick={() => { void browseForRepository(); }}
            disabled={loading}
            title="Browse for a repository folder"
          >
            Browse
          </button>

          <button
            type="button"
            className="btn btn-primary repo-nav-analyze"
            onClick={() => {
              const path = repoPath.trim();
              if (!path) return;
              commitRepoPath(path);
              void runAnalysis(path);
            }}
            disabled={loading || !repoPath.trim() || comparisonMode === "invalid"}
          >
            {loading ? "Analyzing..." : "Analyze"}
          </button>
        </div>
      </div>

      <div className="repo-nav-meta">
        {repoInfo && !repoInfo.is_worktree && (
          <span className="branch-repo-badge" title="Regular repository (not a worktree)">repo</span>
        )}
        {repoInfo?.is_worktree && (
          <span className="branch-worktree-badge" title="Linked worktree">worktree</span>
        )}
        {comparisonMode === "invalid" && (
          <span className="settings-hint" style={{ margin: 0 }}>
            Invalid compare selection.
          </span>
        )}
      </div>
    </div>
  );
}

