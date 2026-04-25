/**
 * useEditorIntegration — manages external-editor launching from the diff viewer.
 *
 * Owns the editor selection state, detects which editors are installed on the
 * machine, and provides the `openInEditor` callback that CenterPane uses for
 * the "Open With" toolbar button.
 *
 * WHY a separate hook: this is a fully self-contained feature (3 state vars,
 * 2 effects, 1 callback) with no cross-cutting dependencies on other domains.
 * Extracting it reduces App.tsx by ~75 lines and makes the editor integration
 * easy to test in isolation.
 */
import { useState, useEffect, useCallback, useRef } from "react";
import { IS_TAURI, tauriInvoke } from "../utils/tauriUtils";
import type { EditorId } from "./AppContext";

/** Supported external editors — mirrors the EditorId type in AppContext. */
const ALL_EDITOR_OPTIONS: Array<{ id: EditorId; label: string }> = [
  { id: "vscode", label: "VS Code" },
  { id: "cursor", label: "Cursor" },
  { id: "zed", label: "Zed" },
  { id: "vim", label: "Vim" },
  { id: "terminal", label: "Terminal" },
];

interface UseEditorIntegrationParams {
  selectedFile: string | null;
  buildAbsolutePath: (path: string) => string;
  showToast: (message: string) => void;
}

/** Return value of useEditorIntegration. */
export interface UseEditorIntegrationResult {
  openWithDropdown: boolean;
  setOpenWithDropdown: React.Dispatch<React.SetStateAction<boolean>>;
  lastEditor: EditorId;
  availableEditors: Set<EditorId> | null;
  openWithRef: React.MutableRefObject<HTMLDivElement | null>;
  /** Filtered editor list — only shows editors confirmed installed. Falls back to all editors on detection error. */
  editorOptions: Array<{ id: EditorId; label: string }>;
  openInEditor: (editor: EditorId) => Promise<void>;
}

/**
 * Detects installed editors on mount, provides the `openInEditor` action, and
 * manages the "Open With" dropdown state.
 */
export function useEditorIntegration({
  selectedFile,
  buildAbsolutePath,
  showToast,
}: UseEditorIntegrationParams): UseEditorIntegrationResult {
  const [openWithDropdown, setOpenWithDropdown] = useState(false);
  const [lastEditor, setLastEditor] = useState<EditorId>("vscode");
  const [availableEditors, setAvailableEditors] = useState<Set<EditorId> | null>(null);
  const openWithRef = useRef<HTMLDivElement | null>(null);

  const editorOptions = availableEditors
    ? ALL_EDITOR_OPTIONS.filter((opt) => availableEditors.has(opt.id))
    : ALL_EDITOR_OPTIONS;

  // Detect which editors are installed on first render
  useEffect(() => {
    if (!IS_TAURI) return;
    tauriInvoke<Record<string, boolean>>("check_editors_available").then(
      (result) => {
        const available = new Set<EditorId>();
        for (const [id, isAvailable] of Object.entries(result)) {
          if (isAvailable) available.add(id as EditorId);
        }
        setAvailableEditors(available);
        // If the currently selected editor is no longer available, fall back to the first one
        if (!available.has(lastEditor)) {
          const first = ALL_EDITOR_OPTIONS.find((opt) => available.has(opt.id));
          if (first) setLastEditor(first.id);
        }
      },
      () => {
        // Detection failed — leave availableEditors null so all editors show
      },
    );
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const openInEditor = useCallback(
    async (editor: EditorId) => {
      if (!selectedFile) {
        showToast("No file selected");
        return;
      }
      const absPath = buildAbsolutePath(selectedFile);
      setLastEditor(editor);
      setOpenWithDropdown(false);
      if (!IS_TAURI) {
        showToast(`Would open ${absPath} in ${editor}`);
        return;
      }
      try {
        await tauriInvoke("open_in_editor", { editor, filePath: absPath });
      } catch (e: unknown) {
        const msg = e instanceof Error ? e.message : String(e);
        showToast(msg);
      }
    },
    [selectedFile, buildAbsolutePath, showToast],
  );

  // Close the "Open With" dropdown when the user clicks anywhere outside it
  useEffect(() => {
    if (!openWithDropdown) return;
    function handleClick(e: MouseEvent) {
      if (openWithRef.current && !openWithRef.current.contains(e.target as Node)) {
        setOpenWithDropdown(false);
      }
    }
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, [openWithDropdown]);

  return {
    openWithDropdown,
    setOpenWithDropdown,
    lastEditor,
    availableEditors,
    openWithRef,
    editorOptions,
    openInEditor,
  };
}
