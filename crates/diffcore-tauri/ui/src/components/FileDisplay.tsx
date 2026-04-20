import React from "react";

/** Single-letter git status used by the file-row layout. */
export type FileDisplayGitStatus = "A" | "M" | "D" | "R" | "C";

export interface FileDisplayProps {
  /** Repo-relative path, used for both layout (`baseName : dirPrefix`) and tooltip. */
  path: string;
  /** Status badge (`A`/`M`/`D`/...). Hidden when omitted. */
  gitStatus?: FileDisplayGitStatus;
  /** Single-token category badge, e.g. `I`, `TSX`, `JSON`, `CALLS`. */
  roleBadge?: string;
  /** Full label for the role badge tooltip / two-line layout, e.g. `Infrastructure`. */
  roleLabel?: string;
  /** Additions count for the green `+N` chip. */
  additions?: number;
  /** Deletions count for the red `-N` chip. */
  deletions?: number;
  /** Suppresses the `+adds -dels` chip even if numbers are provided. */
  hideChanges?: boolean;
  /** Visual variant. One-line is the existing compact row; two-line adds a
   * muted secondary line under the path with the role label and diff stats.
   * Phase 7 implements the two-line layout. */
  variant?: "one-line" | "two-line";
  /** When present, renders a `moved from <path>` chip on line 2 of the
   * two-line variant (and after the path on the one-line variant). */
  movedFrom?: string;
  /** Renders a leading ✓ glyph when this file has been visited in replay. */
  reviewedInReplay?: boolean;
  /** Trailing slot for context-specific content like an edge `→ symbol`,
   * a comment-count button, or the legacy `file-moved-tag`. Rendered after
   * the `+adds -dels` chip on the first line. */
  suffix?: React.ReactNode;
}

/**
 * Compact file-row layout shared by the flow-groups list, infrastructure
 * sub-groups, and the edges list. Renders a fragment of spans so callers
 * can wrap it in whatever element they need (`<li>` for lists, `<div>` for
 * the edges section) and own the click/context handlers.
 *
 * The visual contract is `[✓?] [status?] [role?] [ext] name : dir +adds -dels [suffix]`.
 * Padding, flex layout, and hover/selection states live on the parent
 * `.file-item` (or `.edge-item-row`) class.
 */
export default function FileDisplay({
  path,
  gitStatus,
  roleBadge,
  roleLabel,
  additions,
  deletions,
  hideChanges,
  variant = "one-line",
  movedFrom,
  reviewedInReplay,
  suffix,
}: FileDisplayProps) {
  const compact = compactFileLabel(path);
  const showChanges = !hideChanges && (additions != null || deletions != null);

  // Two-line variant: line 1 shows status + ext + path, line 2 shows the
  // full role label, optional moved-from chip, and the +adds/-dels stats
  // right-aligned. The compact role *badge* (single letter / abbreviation)
  // is intentionally omitted on line 1 because the full label on line 2
  // already conveys the same information without adding noise next to the
  // ext token.
  if (variant === "two-line") {
    return (
      <div className="file-display file-display-two-line">
        <div className="file-display-line file-display-line-primary">
          {reviewedInReplay && (
            <span className="replay-visited-check" title="Visited">&#10003;</span>
          )}
          {gitStatus && (
            <span className={`file-status-token file-status-${gitStatus}`}>{gitStatus}</span>
          )}
          <span className="file-ext-token">{compact.extension || "-"}</span>
          <span className="file-path" title={path}>
            <span className="file-base-name">{compact.baseName}</span>
            <span className="file-folder-sep">:</span>
            {compact.dirPrefix && (
              <span className="file-dir-prefix">{compact.dirPrefix}</span>
            )}
          </span>
          {suffix}
        </div>
        {(movedFrom || roleLabel || showChanges) && (
          <div className="file-display-line file-display-line-secondary">
            {movedFrom && (
              <span className="file-display-moved-from" title={`Moved from ${movedFrom}`}>
                {movedFrom}&nbsp;&rarr;
              </span>
            )}
            {roleLabel && (
              <span className="file-display-role-label">{roleLabel}</span>
            )}
            {showChanges && (
              <span className="file-display-changes">
                <span className="file-display-additions">+{additions ?? 0}</span>
                <span className="file-display-deletions">-{deletions ?? 0}</span>
              </span>
            )}
          </div>
        )}
      </div>
    );
  }

  return (
    <>
      {reviewedInReplay && (
        <span className="replay-visited-check" title="Visited">&#10003;</span>
      )}
      {gitStatus && (
        <span className={`file-status-token file-status-${gitStatus}`}>{gitStatus}</span>
      )}
      {roleBadge && (
        <span className="file-role" title={roleLabel ?? roleBadge}>
          {roleBadge}
        </span>
      )}
      <span className="file-ext-token">{compact.extension || "-"}</span>
      <span className="file-path" title={path}>
        <span className="file-base-name">{compact.baseName}</span>
        <span className="file-folder-sep">:</span>
        {compact.dirPrefix && (
          <span className="file-dir-prefix">{compact.dirPrefix}</span>
        )}
      </span>
      {showChanges && (
        <span className="file-changes">
          +{additions ?? 0} -{deletions ?? 0}
        </span>
      )}
      {suffix}
    </>
  );
}

/**
 * Split a repo-relative path into the parts the file-row layout uses:
 * `extension` (uppercase, badge), `baseName` (filename without extension),
 * and `dirPrefix` (the rest, with the leading folder optionally
 * abbreviated to its initials so deep paths still fit on one line).
 *
 * Mirrors the inline helper that lived in `App.tsx` so callers can stop
 * computing this themselves; kept here next to the only consumer.
 */
function compactFileLabel(path: string): {
  dirPrefix: string;
  baseName: string;
  extension: string;
} {
  const normalized = path.replace(/\\/g, "/");
  const parts = normalized.split("/").filter(Boolean);
  const filename = parts.pop() ?? normalized;
  const dirs = parts;

  const dotIndex = filename.lastIndexOf(".");
  const baseName = dotIndex > 0 ? filename.slice(0, dotIndex) : filename;
  const extension = dotIndex > 0 ? filename.slice(dotIndex + 1).toUpperCase() : "";

  const abbreviatedDirs = dirs.map((dir, i) =>
    i === 0 ? abbreviateLeadingSegment(dir) : dir,
  );
  const dirPrefix = abbreviatedDirs.join("/");

  return { dirPrefix, baseName, extension };
}

function abbreviateLeadingSegment(segment: string): string {
  if (segment.includes("-")) {
    return segment
      .split("-")
      .filter(Boolean)
      .map((part) => part.charAt(0).toLowerCase())
      .join("-");
  }

  const uppercase = segment.slice(1).match(/[A-Z]/g) ?? [];
  if (uppercase.length > 0) {
    return `~${segment.charAt(0)}${uppercase.join("")}`;
  }

  return segment;
}
