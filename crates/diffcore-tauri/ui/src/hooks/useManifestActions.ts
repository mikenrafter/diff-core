/**
 * useManifestActions — groups manifest import, export, agent-prompt building,
 * and the live-reload file-watcher effect.
 *
 * WHY a separate hook: the manifest workflow (export → watch → auto-import on
 * change) is a cohesive sub-feature used only by RightmostPane and the context
 * value. Keeping it co-located makes the watch/unlisten lifecycle impossible
 * to get wrong and shrinks App.tsx by ~70 lines.
 */
import { useState, useCallback, useEffect } from "react";
import type { AnalysisOutput } from "../types";
import { IS_TAURI, tauriInvoke } from "../utils/tauriUtils";
import { buildManifestPrompt } from "../buildManifestPrompt";

interface UseManifestActionsParams {
  analysis: AnalysisOutput | null;
  repoPath: string | null;
  setAnalysis: (a: AnalysisOutput) => void;
  setRefinedGroups: (g: AnalysisOutput["groups"] | null) => void;
  setOriginalGroups: (g: AnalysisOutput["groups"] | null) => void;
  setShowRefined: (v: boolean) => void;
  setReviewedGroupIds: (ids: Set<string>) => void;
  handleSelectGroup: (group: AnalysisOutput["groups"][0]) => void;
  showToast: (message: string) => void;
}

export interface UseManifestActionsResult {
  watchedManifestPath: string | null;
  setWatchedManifestPath: React.Dispatch<React.SetStateAction<string | null>>;
  importGroupsManifest: (manifestPath: string) => Promise<void>;
  exportGroupsManifest: () => Promise<string | null>;
  buildManifestAgentPrompt: (manifestPath: string) => string;
}

/** Manages groups manifest import/export, agent prompt building, and live file-watching. */
export function useManifestActions({
  analysis,
  repoPath,
  setAnalysis,
  setRefinedGroups,
  setOriginalGroups,
  setShowRefined,
  setReviewedGroupIds,
  handleSelectGroup,
  showToast,
}: UseManifestActionsParams): UseManifestActionsResult {
  const [watchedManifestPath, setWatchedManifestPath] = useState<string | null>(null);

  /** Import a groups manifest JSON and apply it to the current analysis. */
  const importGroupsManifest = useCallback(async (manifestPath: string) => {
    if (!IS_TAURI) return;
    try {
      const updated = await tauriInvoke<AnalysisOutput>("import_groups_manifest", { manifestPath });
      setAnalysis(updated);
      // Reset state for new groupings
      setRefinedGroups(null);
      setOriginalGroups(null);
      setShowRefined(false);
      setReviewedGroupIds(new Set());
      if (updated.groups.length > 0) {
        const sorted = [...updated.groups].sort((a, b) => a.review_order - b.review_order);
        handleSelectGroup(sorted[0]);
      }
      showToast(`Loaded ${updated.groups.length} groups from manifest`);
    } catch (e) {
      showToast(`Failed to import manifest: ${String(e)}`);
    }
  }, [handleSelectGroup, setAnalysis, setOriginalGroups, setRefinedGroups, setReviewedGroupIds, setShowRefined, showToast]);

  /** Export current groups as an editable manifest JSON. */
  const exportGroupsManifest = useCallback(async () => {
    if (!IS_TAURI || !analysis) return null;
    try {
      // Default path: .diffcore/groups.json in repo
      const outputPath = repoPath
        ? `${repoPath}/.diffcore/groups.json`
        : "groups.json";
      await tauriInvoke("export_groups_manifest", { outputPath });
      showToast(`Groups manifest exported to ${outputPath}`);
      return outputPath;
    } catch (e) {
      showToast(`Failed to export manifest: ${String(e)}`);
      return null;
    }
  }, [analysis, repoPath, showToast]);

  /** Build an agent prompt for iterative manifest refinement. */
  const buildManifestAgentPrompt = useCallback((manifestPath: string): string => {
    return buildManifestPrompt({
      manifestPath,
      repoPath: repoPath || ".",
      groupCount: analysis?.groups.length ?? 0,
      fileCount: analysis?.summary.total_files_changed ?? 0,
      infraCount: analysis?.infrastructure_group?.files.length ?? 0,
    });
  }, [analysis, repoPath]);

  // Listen for manifest-changed events from the file watcher and auto-reload groups
  useEffect(() => {
    if (!IS_TAURI || !watchedManifestPath) return;
    let cancelled = false;

    (async () => {
      const { listen } = await import("@tauri-apps/api/event");
      const unlisten = await listen<string>("manifest-changed", (event) => {
        if (!cancelled) {
          importGroupsManifest(event.payload);
        }
      });
      return unlisten;
    })().then((unlisten) => {
      if (cancelled && unlisten) unlisten();
      return unlisten;
    });

    return () => { cancelled = true; };
  }, [watchedManifestPath, importGroupsManifest]);

  return {
    watchedManifestPath,
    setWatchedManifestPath,
    importGroupsManifest,
    exportGroupsManifest,
    buildManifestAgentPrompt,
  };
}
