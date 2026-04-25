/**
 * useCrossFileSearch — owns all cross-file search state, the search callback,
 * the open-result callback, and the input focus effect.
 *
 * WHY a separate hook: cross-file search is a fully self-contained sub-feature
 * with 5 state vars plus two async callbacks and one effect. Moving it here
 * removes ~90 lines from App.tsx without touching any other domain.
 */
import { useState, useRef, useCallback, useEffect } from "react";
import type { FileDiffContent, FlowGroup } from "../types";
import { IS_TAURI, tauriInvoke } from "../utils/tauriUtils";
import { MOCK_DIFFS } from "../mock";

export interface CrossFileSearchResult {
  file_path: string;
  matches: CrossFileSearchMatch[];
}

export interface CrossFileSearchMatch {
  line_number: number;
  line_text: string;
}

interface UseCrossFileSearchParams {
  changedFilePathSet: Set<string>;
  selectedGroupRef: React.MutableRefObject<FlowGroup | null>;
  openFileInTab: (path: string, groupId: string) => void;
  repoPath: string | null;
  showToast: (message: string) => void;
  showUnchangedFiles: boolean;
  diffViewerRef: React.MutableRefObject<{ scrollToLine: (start: number, end: number) => void } | null>;
  setSelectedFile: (path: string) => void;
  setFileDiff: (diff: FileDiffContent) => void;
  setOpenTabs: React.Dispatch<React.SetStateAction<Array<{ path: string; groupId: string }>>>;
}

export interface UseCrossFileSearchResult {
  crossFileSearchOpen: boolean;
  setCrossFileSearchOpen: React.Dispatch<React.SetStateAction<boolean>>;
  crossFileSearchQuery: string;
  setCrossFileSearchQuery: React.Dispatch<React.SetStateAction<string>>;
  crossFileSearchLoading: boolean;
  crossFileSearchResults: CrossFileSearchResult[];
  crossFileSearchError: string | null;
  crossFileSearchInputRef: React.RefObject<HTMLInputElement | null>;
  runCrossFileSearch: () => Promise<void>;
  openCrossFileSearchResult: (filePath: string, lineNumber: number) => Promise<void>;
}

/** Manages the cross-file keyword search panel in the left pane. */
export function useCrossFileSearch({
  changedFilePathSet,
  selectedGroupRef,
  openFileInTab,
  repoPath,
  showToast,
  showUnchangedFiles,
  diffViewerRef,
  setSelectedFile,
  setFileDiff,
  setOpenTabs,
}: UseCrossFileSearchParams): UseCrossFileSearchResult {
  const [crossFileSearchOpen, setCrossFileSearchOpen] = useState(false);
  const [crossFileSearchQuery, setCrossFileSearchQuery] = useState("");
  const [crossFileSearchLoading, setCrossFileSearchLoading] = useState(false);
  const [crossFileSearchResults, setCrossFileSearchResults] = useState<CrossFileSearchResult[]>([]);
  const [crossFileSearchError, setCrossFileSearchError] = useState<string | null>(null);
  const crossFileSearchInputRef = useRef<HTMLInputElement | null>(null);

  const openCrossFileSearchResult = useCallback(async (filePath: string, lineNumber: number) => {
    const group = selectedGroupRef.current;
    const changed = changedFilePathSet.has(filePath);

    if (changed && group) {
      openFileInTab(filePath, group.id);
      setTimeout(() => {
        diffViewerRef.current?.scrollToLine(lineNumber, lineNumber);
      }, 120);
      return;
    }

    if (IS_TAURI && repoPath) {
      try {
        const content = await tauriInvoke<FileDiffContent>("get_workspace_file_content", {
          repoPath,
          filePath,
        });
        setSelectedFile(filePath);
        setFileDiff(content);
        setOpenTabs((prev) => {
          if (prev.some((t) => t.path === filePath)) return prev;
          return [...prev, { path: filePath, groupId: group?.id ?? "__workspace_search__" }];
        });
        setTimeout(() => {
          diffViewerRef.current?.scrollToLine(lineNumber, lineNumber);
        }, 120);
        return;
      } catch (e) {
        showToast(`Unable to open ${filePath}: ${String(e)}`);
      }
    }
  }, [changedFilePathSet, diffViewerRef, openFileInTab, repoPath, selectedGroupRef, setFileDiff, setOpenTabs, setSelectedFile, showToast]);

  const runCrossFileSearch = useCallback(async () => {
    const query = crossFileSearchQuery.trim();
    if (!query) {
      setCrossFileSearchResults([]);
      setCrossFileSearchError(null);
      return;
    }

    setCrossFileSearchLoading(true);
    setCrossFileSearchError(null);
    try {
      if (IS_TAURI && repoPath) {
        const results = await tauriInvoke<CrossFileSearchResult[]>("cross_file_search", {
          repoPath,
          query,
          showUnchangedFiles,
          maxResults: 200,
        });
        setCrossFileSearchResults(results);
      } else {
        // Demo fallback: search known mock diff files.
        const needle = query.toLowerCase();
        const results: CrossFileSearchResult[] = Object.entries(MOCK_DIFFS)
          .map(([path, diff]) => {
            const lines = (diff?.new_content || "").split("\n");
            const matches: CrossFileSearchMatch[] = [];
            lines.forEach((line, idx) => {
              if (line.toLowerCase().includes(needle)) {
                matches.push({ line_number: idx + 1, line_text: line });
              }
            });
            return { file_path: path, matches };
          })
          .filter((r) => r.matches.length > 0);
        setCrossFileSearchResults(results);
      }
    } catch (e) {
      setCrossFileSearchResults([]);
      setCrossFileSearchError(String(e));
    } finally {
      setCrossFileSearchLoading(false);
    }
  }, [crossFileSearchQuery, repoPath, showUnchangedFiles]);

  // Auto-focus and select the search input when the panel opens
  useEffect(() => {
    if (!crossFileSearchOpen) return;
    const timer = setTimeout(() => {
      crossFileSearchInputRef.current?.focus();
      crossFileSearchInputRef.current?.select();
    }, 0);
    return () => clearTimeout(timer);
  }, [crossFileSearchOpen]);

  return {
    crossFileSearchOpen,
    setCrossFileSearchOpen,
    crossFileSearchQuery,
    setCrossFileSearchQuery,
    crossFileSearchLoading,
    crossFileSearchResults,
    crossFileSearchError,
    crossFileSearchInputRef,
    runCrossFileSearch,
    openCrossFileSearchResult,
  };
}
