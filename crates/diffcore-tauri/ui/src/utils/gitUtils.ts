import type { RepoInfo, CommitInfo } from "../types";

// ---------------------------------------------------------------------------
// Constants (shared between App.tsx and consumers)
// ---------------------------------------------------------------------------

export const COMPARE_TARGET_UNSTAGED = "__DIFFCORE_UNSTAGED__";
export const COMPARE_TARGET_STAGED = "__DIFFCORE_STAGED__";

export function deriveGitShortStatus(additions: number, deletions: number): "A" | "D" | "M" {
  if (additions > 0 && deletions === 0) return "A";
  if (deletions > 0 && additions === 0) return "D";
  return "M";
}

export function resolveFileShortStatus(
  path: string,
  statusMap: Record<string, string>,
  additions: number,
  deletions: number,
): "A" | "M" | "D" | "R" | "C" {
  const fromMap = statusMap[path];
  if (fromMap === "A" || fromMap === "M" || fromMap === "D" || fromMap === "R" || fromMap === "C") {
    return fromMap;
  }
  return deriveGitShortStatus(additions, deletions);
}

/** Format the branch tracking status into a readable string. */
export function formatBranchStatus(repoInfo: RepoInfo | null): string | null {
  if (!repoInfo?.status) return null;
  const { ahead, behind, upstream } = repoInfo.status;
  if (!upstream) return null;
  if (ahead === 0 && behind === 0) return "up to date";
  const parts: string[] = [];
  if (ahead > 0) parts.push(`${ahead} ahead`);
  if (behind > 0) parts.push(`${behind} behind`);
  return parts.join(", ");
}

export function formatCompareTargetLabel(refName: string | null, recentCommits: CommitInfo[]): string | null {
  if (!refName) return null;
  if (refName === COMPARE_TARGET_UNSTAGED) return "Unstaged changes";
  if (refName === COMPARE_TARGET_STAGED) return "Staged changes";
  const commit = recentCommits.find((item) => item.sha === refName);
  if (!commit) return refName;
  return `${commit.short_sha} (${commit.summary})`;
}
