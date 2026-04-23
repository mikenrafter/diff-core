/**
 * Group / refinement display utilities.
 *
 * Pure functions for risk scoring, refinement change indicators, and
 * tool-edit hunk computation. No React or hooks.
 */
import type { FlowGroup, RefinementResponse } from "../types";
import type { EditedHunk } from "../components/DiffViewer";

// ---------------------------------------------------------------------------
// Risk helpers
// ---------------------------------------------------------------------------

export function riskLevel(score: number): string {
  if (score >= 0.7) return "high";
  if (score >= 0.4) return "medium";
  return "low";
}

// ---------------------------------------------------------------------------
// Refinement change indicators
// ---------------------------------------------------------------------------

/** Get a change indicator for a group based on the refinement response. */
export function getGroupChangeIndicator(
  group: FlowGroup,
  response: RefinementResponse | null,
): { type: string; label: string; reason: string } | null {
  if (!response) return null;

  for (const split of response.splits) {
    for (const newGroup of split.new_groups) {
      if (group.name === newGroup.name || group.id.startsWith("group_refined_")) {
        return {
          type: "split",
          label: `split from ${split.source_group_id}`,
          reason: split.reason,
        };
      }
    }
  }

  for (const merge of response.merges) {
    if (group.name === merge.merged_name) {
      return {
        type: "merge",
        label: `merged from ${merge.group_ids.join(" + ")}`,
        reason: merge.reason,
      };
    }
  }

  for (const reRank of response.re_ranks) {
    if (group.id === reRank.group_id) {
      return {
        type: "rerank",
        label: `re-ranked to #${reRank.new_position}`,
        reason: reRank.reason,
      };
    }
  }

  return null;
}

/** Check if a file was moved by a reclassification. */
export function getFileMovedIndicator(
  filePath: string,
  response: RefinementResponse | null,
): { from: string; reason: string } | null {
  if (!response) return null;

  for (const reclass of response.reclassifications) {
    if (reclass.file === filePath) {
      return {
        from: reclass.from_group_id,
        reason: reclass.reason,
      };
    }
  }
  return null;
}

// ---------------------------------------------------------------------------
// Tool-edit hunk computation
// ---------------------------------------------------------------------------

export function computeToolEditHunks(baselineContent: string, currentContent: string): EditedHunk[] {
  if (baselineContent === currentContent) return [];

  const baselineLines = baselineContent.split("\n");
  const currentLines = currentContent.split("\n");
  const hunks: EditedHunk[] = [];

  let i = 0;
  let j = 0;
  const LOOKAHEAD = 80;

  while (i < baselineLines.length || j < currentLines.length) {
    if (i < baselineLines.length && j < currentLines.length && baselineLines[i] === currentLines[j]) {
      i += 1;
      j += 1;
      continue;
    }

    const startOld = i;
    const startNew = j;
    let aligned = false;

    for (let offset = 1; offset <= LOOKAHEAD; offset += 1) {
      const oldIdx = i + offset;
      const newIdx = j + offset;

      if (j + offset < currentLines.length && i < baselineLines.length && baselineLines[i] === currentLines[j + offset]) {
        j += offset;
        aligned = true;
        break;
      }
      if (i + offset < baselineLines.length && j < currentLines.length && baselineLines[i + offset] === currentLines[j]) {
        i += offset;
        aligned = true;
        break;
      }
      if (oldIdx < baselineLines.length && newIdx < currentLines.length && baselineLines[oldIdx] === currentLines[newIdx]) {
        i = oldIdx;
        j = newIdx;
        aligned = true;
        break;
      }
    }

    if (!aligned) {
      i = baselineLines.length;
      j = currentLines.length;
    }

    const oldStartLine = startOld + 1;
    const oldEndLine = Math.max(oldStartLine, i);
    const newStartLine = startNew + 1;
    const rawNewEndLine = j;
    const isDeletionOnly = rawNewEndLine < newStartLine;
    const safeNewLine = Math.max(1, Math.min(newStartLine, currentLines.length || 1));
    const newEndLine = isDeletionOnly ? safeNewLine : Math.max(newStartLine, rawNewEndLine);
    const selectedCode = isDeletionOnly
      ? ""
      : currentLines.slice(startNew, j).join("\n");

    hunks.push({
      originalStartLine: oldStartLine,
      originalEndLine: oldEndLine,
      modifiedStartLine: isDeletionOnly ? 0 : newStartLine,
      modifiedEndLine: isDeletionOnly ? 0 : newEndLine,
      selectedCode,
    });
  }

  return hunks;
}
