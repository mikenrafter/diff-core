import React, { useState } from "react";

/** Single-letter git status used by the file-row layout. */
export type FileDisplayGitStatus = "A" | "M" | "D" | "R" | "C";

export interface FileDisplayProps {
  /** Repo-relative path, used for both layout (`baseName : dirPrefix`) and tooltip. */
  path: string;
  /** Status badge (`A`/`M`/`D`/...). Hidden when omitted. */
  gitStatus?: FileDisplayGitStatus;
  /** Category badge (`Infrastructure`/`Utility`/...) */
  roleBadge?: string;
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
  prefix?: React.ReactNode;
}

/**
 * Compact file-row layout shared by the flow-groups list, infrastructure
 * sub-groups, and the edges list. Renders a fragment of spans so callers
 * can wrap it in whatever element they need (`<li>` for lists, `<div>` for
 * the edges section) and own the click/context handlers.
 *
 * The visual contract for one-line is `[role?] [ext | +N/-N] name : dir [prefix]`;
 * for two-line, line 1 is `[prefix status] [role?] [+N/-N]` and line 2 is
 * `[ext] [dirname ............... baseName]`. Padding, flex layout, and
 * hover/selection states live on the parent `.file-item` (or `.edge-item-row`).
 */
export default function FileDisplay({
  path,
  gitStatus,
  roleBadge,
  additions,
  deletions,
  hideChanges,
  variant = "one-line",
  movedFrom,
  reviewedInReplay,
  prefix,
}: FileDisplayProps) {
  const [hovered, setHovered] = useState(false);
  const compact = compactFileLabel(path);
  const showChanges = !hideChanges && (additions != null || deletions != null);
  // Hover only reveals the badge when actual counts exist; avoids showing
  // "+0/-0" for infra files that have no additions/deletions data at all.
  const hasChangesData = additions != null || deletions != null;

  // Combined +N/-N badge. For one-line, also activates on hover so the ext
  // token swaps out even when changes are normally hidden (hideChanges=true).
  const changesBadge = (showChanges || (hovered && hasChangesData)) && (
    <span className="file-display-changes-badge">
      <span className="file-display-additions">+{additions ?? 0}</span>
      <span className="file-display-changes-sep">/</span>
      <span className="file-display-deletions">-{deletions ?? 0}</span>
    </span>
  );

  // Two-line variant: primary line has three sections — left (prefix + status
  // token), centre (role badge), right (combined changes badge). Secondary
  // line sets dirname flush-left and baseName flush-right inside file-path.
  if (variant === "two-line") {
    return (
      <div className="file-display file-display-two-line">
        <div className="file-display-line file-display-line-primary">
          <div className="file-display-primary-left">
            {prefix}
            {gitStatus && (
              <span className={`file-status-token file-status-${gitStatus}`}>{reviewedInReplay ? "✓" : gitStatus}</span>
            )}
            <div className="file-display-primary-left">
              {changesBadge}
            </div>
          </div>
          <div className="file-display-primary-right">
            {roleBadge && movedFrom && (
              <span className="file-display-moved-from" title={`Moved from ${movedFrom}`}>
                &rarr;&nbsp;{roleBadge}
              </span>
            )}
            {roleBadge && !movedFrom && (
              <span className="file-role">{roleBadge}</span>
            )}
          </div>
          <div className="file-display-primary-right">
            <span className="file-ext-token">{compact.extension || "-"}</span>
          </div>
        </div>
        <div className="file-display-line file-display-line-secondary">
          <span className="file-path" title={path}>
            <span className="file-dir-prefix">{compact.dirPrefix}</span>
            <span className="file-base-name">{compact.baseName}</span>
          </span>
        </div>
      </div>
    );
  }

  // One-line variant: span wrapper enables hover detection so the ext token
  // swaps to the changes badge on mouse-enter, reverting on mouse-leave only
  // when changes were not already shown. Mirrors .file-item's flex gap so the
  // parent <li>/<div> layout is unchanged.
  return (
    <span
      className="file-display file-display-one-line"
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
    >
      {roleBadge && (
        <span className="file-role" title={roleBadge}>
          {roleBadge}
        </span>
      )}
      {gitStatus && (
        <span className={`file-status-token file-status-${gitStatus}`}>{reviewedInReplay ? "✓" : gitStatus}</span>
      )}
      {(showChanges || (hovered && hasChangesData))
        ? changesBadge
        : <span className="file-ext-token">{compact.extension || "-"}</span>
      }
      <span className="file-path" title={path}>
        {compact.dirPrefix && (
          <span className="file-dir-prefix">{compact.dirPrefix}</span>
        )}
        <span className="file-dir-sep">/</span>
        <span className="file-base-name">{compact.baseName}</span>
      </span>
      {prefix}
    </span>
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
    i < dirs.length - 1 ? abbreviateSegment(dir) : dir,
  );
  const dirPrefix = abbreviatedDirs.join("/");

  return { dirPrefix, baseName, extension };
}

/**
 * Abbreviate a single directory segment according to its casing style.
 *
 * Separator characters `[-_+.,]` are preserved verbatim and used to split
 * the segment into parts. Each part is then abbreviated independently and
 * the separators are re-inserted at their original positions.
 *
 * Within a part, PascalCase / camelCase boundaries produce further subwords.
 * Multi-subword parts join abbreviated subwords with the segment's primary
 * separator; single-word parts use `initial + next_consonant` when the word
 * starts uppercase or `initial only` when it starts lowercase.
 *
 * Consonant set (y treated as vowel): bcdfghjklmnpqrstvwxz.
 *
 * Examples:
 *   LongPathPascalCase  → LnPtPsCs
 *   kebab-first         → k-f
 *   Kebab-AndPascal-Too → Kb-An-Ps-T
 *   camelCaseNow        → cmCsNw
 *   Oh_Handle_Snake     → Oh_Hn_Sn
 *   and_standard_snake  → a_s_s
 */
function abbreviateSegment(segment: string): string {
  // Split on separator chars, capturing them so we can restore their positions.
  const tokens = segment.split(/([-_+.,]+)/);
  if (tokens.length === 1) {
    // No separator — pure PascalCase / camelCase.
    return abbreviatePart(segment, "");
  }
  // Use the first separator found as the join character for intra-part
  // PascalCase subwords (e.g. AndPascal → An-Ps when separator is "-").
  const primarySep = tokens.find((t) => /^[-_+.,]+$/.test(t)) ?? "";
  return tokens
    .map((token) =>
      /^[-_+.,]+$/.test(token) || token === ""
        ? token
        : abbreviatePart(token, primarySep),
    )
    .join("");
}

/**
 * Abbreviate one segment part, splitting on internal PascalCase boundaries.
 * Multiple subwords join with `sep`; a single word delegates to
 * `abbreviateStandaloneWord`.
 */
function abbreviatePart(part: string, sep: string): string {
  const subwords = splitOnCamelBoundaries(part);
  if (subwords.length > 1) {
    // Multi-subword context: ALL subwords get initial + next consonant.
    return subwords.map(abbreviateSubword).join(sep);
  }
  return abbreviateStandaloneWord(part);
}

/** Split a string at every uppercase-letter boundary after the first char. */
function splitOnCamelBoundaries(s: string): string[] {
  const words: string[] = [];
  let start = 0;
  for (let i = 1; i < s.length; i++) {
    if (/[A-Z]/.test(s[i])) {
      words.push(s.slice(start, i));
      start = i;
    }
  }
  words.push(s.slice(start));
  return words.filter(Boolean);
}

/**
 * Abbreviate one subword that is part of a multi-subword expression.
 * Always returns `initial + next_consonant` regardless of case.
 */
function abbreviateSubword(word: string): string {
  if (!word) return "";
  const nextConsonant = word.slice(1).match(/[bcdfghjklmnpqrstvwxz]/i)?.[0] ?? "";
  return word[0] + nextConsonant;
}

/**
 * Abbreviate a standalone single word (separator-split but no internal
 * PascalCase). Uppercase-starting words get `initial + next_consonant`;
 * lowercase-starting words get `initial` only.
 */
function abbreviateStandaloneWord(word: string): string {
  if (!word) return "";
  if (/[A-Z]/.test(word[0])) {
    const nextConsonant = word.slice(1).match(/[bcdfghjklmnpqrstvwxz]/i)?.[0] ?? "";
    return word[0] + nextConsonant;
  }
  return word[0];
}
