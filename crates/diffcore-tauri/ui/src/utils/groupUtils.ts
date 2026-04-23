import type { FlowGroup, RefinementResponse } from "../types";

export function riskLevel(score: number): string {
  if (score >= 0.7) return "high";
  if (score >= 0.4) return "medium";
  return "low";
}

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
