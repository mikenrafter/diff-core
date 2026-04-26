import type React from "react";
import { createContext, useContext } from "react";
import type { MutableRefObject } from "react";
import type { FlowGroup, FileDiffContent, ReviewComment } from "../types";
import type { DiffViewerHandle, EditedHunk } from "../components/DiffViewer";
import type { EditorId } from "./AppContext";

export interface DiffContextValue {
  fileDiff: FileDiffContent | null;
  selectedFile: string | null;
  selectedGroup: FlowGroup | null;

  openTabs: Array<{ path: string; groupId: string }>;
  handleSelectFile: (path: string) => Promise<void>;
  closeTab: (tabPath: string) => void;
  setTabContextMenu: React.Dispatch<React.SetStateAction<{ x: number; y: number; tabPath: string } | null>>;

  openWithRef: MutableRefObject<HTMLDivElement | null>;
  openWithDropdown: boolean;
  setOpenWithDropdown: React.Dispatch<React.SetStateAction<boolean>>;
  lastEditor: EditorId;
  editorOptions: Array<{ id: EditorId; label: string }>;
  openInEditor: (editor: EditorId) => Promise<void>;

  replayActive: boolean;
  replayStep: number;
  replayVisited: Set<string>;
  replayHunks: Array<{ id: string }>;
  replayHunkIndex: number;
  replayViewedHunkIds: Set<string>;
  hasNextReplayHunk: boolean;
  hasPrevReplayHunk: boolean;
  navigateReplayHunk: (direction: 1 | -1) => void;
  goToReplayStep: (step: number) => void;
  exitReplay: () => void;

  diffViewerRef: MutableRefObject<DiffViewerHandle | null>;
  editsEnabled: boolean;
  shouldRenderSideBySide: boolean;
  /** Persisted Monaco split-view ratio when side-by-side is enabled. */
  diffSplitRatio: number;
  setDiffSplitRatio: React.Dispatch<React.SetStateAction<number>>;
  codeCommentsForSelectedFile: ReviewComment[];
  setActiveCommentId: React.Dispatch<React.SetStateAction<string | null>>;
  setRightPanelTab: React.Dispatch<React.SetStateAction<"activity" | "annotations" | "source" | "comments">>;
  rightPanelCollapsed: boolean;
  setRightPanelCollapsed: React.Dispatch<React.SetStateAction<boolean>>;
  handleGoToDefinition: (word: string) => void;
  handleDiffCommentRequest: (startLine: number, endLine: number, selectedCode: string) => void;
  handleDiffEditorContentChange: (newContent: string, hunks: EditedHunk[]) => void;
  handleDiffHunksChanged: (hunks: EditedHunk[]) => void;
}

export const DiffContext = createContext<DiffContextValue | null>(null);

export function useDiffContext(): DiffContextValue {
  const ctx = useContext(DiffContext);
  if (!ctx) throw new Error("useDiffContext must be used within a DiffContext.Provider");
  return ctx;
}

