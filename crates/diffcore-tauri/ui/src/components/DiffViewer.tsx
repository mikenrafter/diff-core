import { useState, useCallback, useRef, useImperativeHandle, forwardRef, useEffect } from "react";
import { DiffEditor } from "@monaco-editor/react";
import type { FileDiffContent, ReviewComment } from "../types";

export interface EditedHunk {
  originalStartLine: number;
  originalEndLine: number;
  modifiedStartLine: number;
  modifiedEndLine: number;
  selectedCode: string;
}

export interface DiffViewerHandle {
  /** Scroll the modified editor to a line range, select it, and briefly highlight it. */
  scrollToLine: (startLine: number, endLine?: number) => void;
  /** Return current Monaco diff hunks from the modified side. */
  getDiffHunks: () => EditedHunk[];
  /** Open Monaco find widget for in-file search. */
  openFindWidget: () => void;
}

interface DiffViewerProps {
  fileDiff: FileDiffContent | null;
  /** Whether modified pane is editable. */
  editable?: boolean;
  /** Whether to render side-by-side (true) or inline/unified (false). Default: true. */
  renderSideBySide?: boolean;
  /** Called when user selects lines and clicks "Comment" in the modified editor. */
  onCommentRequest?: (startLine: number, endLine: number, selectedCode: string) => void;
  /** Comments for the current file (code-level only). */
  codeComments?: ReviewComment[];
  /** Called when user clicks a glyph icon in the gutter — passes the comment ID. */
  onGlyphClick?: (commentId: string) => void;
  /** Called when user triggers "Go To Definition" on a word — receives the word under cursor. */
  onGoToDefinition?: (word: string) => void;
  /** Called when modified content changes so parent can persist edits and sync comments. */
  onEditedContentChange?: (newContent: string, hunks: EditedHunk[]) => void;
  /** Called when Monaco recomputes diff hunks for the current file. */
  onDiffHunksChange?: (hunks: EditedHunk[]) => void;
}

/** Monaco-based side-by-side diff viewer for the center panel. */
const DiffViewer = forwardRef<DiffViewerHandle, DiffViewerProps>(function DiffViewer({ fileDiff, editable = false, onCommentRequest, codeComments, onGlyphClick, onGoToDefinition, renderSideBySide: renderSideBySideProp = true, onEditedContentChange, onDiffHunksChange }, ref) {
  const [selectionRange, setSelectionRange] = useState<{ startLine: number; endLine: number } | null>(null);
  const [commentBtnPos, setCommentBtnPos] = useState<{ top: number; left: number } | null>(null);
  const editorRef = useRef<any>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const goToDefRef = useRef(onGoToDefinition);
  goToDefRef.current = onGoToDefinition;

  const decorationsRef = useRef<any>(null);

  const buildEditedHunks = useCallback((diffEditor: any, modifiedModel: any): EditedHunk[] => {
    if (!diffEditor || !modifiedModel) return [];
    const lineCount = modifiedModel.getLineCount?.() ?? 1;
    const lineChanges = diffEditor.getLineChanges?.() ?? [];
    return lineChanges.map((change: any) => {
      const modifiedStart = change.modifiedStartLineNumber;
      const modifiedEnd = change.modifiedEndLineNumber;
      const isDeletionOnly = modifiedStart === 0 || modifiedEnd === 0;
      const safeStart = isDeletionOnly
        ? Math.max(1, Math.min(change.originalStartLineNumber ?? 1, lineCount))
        : Math.max(1, Math.min(modifiedStart, lineCount));
      const safeEnd = isDeletionOnly
        ? safeStart
        : Math.max(safeStart, Math.min(modifiedEnd, lineCount));
      const selectedCode = isDeletionOnly
        ? ""
        : modifiedModel.getValueInRange({
            startLineNumber: safeStart,
            startColumn: 1,
            endLineNumber: safeEnd,
            endColumn: modifiedModel.getLineMaxColumn(safeEnd),
          });
      return {
        originalStartLine: change.originalStartLineNumber,
        originalEndLine: change.originalEndLineNumber,
        modifiedStartLine: safeStart,
        modifiedEndLine: safeEnd,
        selectedCode,
      };
    });
  }, []);

  const handleEditorMount = useCallback(
    (editor: any) => {
      editorRef.current = editor;

      // Get the modified editor (right side of the diff)
      const modifiedEditor = editor.getModifiedEditor();
      const originalEditor = editor.getOriginalEditor();
      if (!modifiedEditor) return;

      // Apply options to both sub-editors (DiffEditor options don't propagate)
      const subEditorOpts = {
        glyphMargin: true,
        overviewRulerBorder: false,
        overviewRulerLanes: 0,
        hideCursorInOverviewRuler: true,
        scrollbar: { verticalScrollbarSize: 4, horizontalScrollbarSize: 4 },
      };
      modifiedEditor.updateOptions(subEditorOpts);
      if (originalEditor) {
        originalEditor.updateOptions(subEditorOpts);
      }

      // Listen for selection changes
      modifiedEditor.onDidChangeCursorSelection((e: any) => {
        const sel = e.selection;
        // Only show the comment button for multi-line selections
        if (sel && sel.startLineNumber !== sel.endLineNumber) {
          const startLine = Math.min(sel.startLineNumber, sel.endLineNumber);
          const endLine = Math.max(sel.startLineNumber, sel.endLineNumber);
          setSelectionRange({ startLine, endLine });

          // Position the comment button near the selection end
          const coords = modifiedEditor.getScrolledVisiblePosition({
            lineNumber: endLine,
            column: 1,
          });
          if (coords && containerRef.current) {
            const containerRect = containerRef.current.getBoundingClientRect();
            const editorDom = modifiedEditor.getDomNode();
            const editorRect = editorDom?.getBoundingClientRect();
            if (editorRect) {
              setCommentBtnPos({
                top: coords.top + (editorRect.top - containerRect.top) + coords.height + 4,
                left: editorRect.left - containerRect.left + 40,
              });
            }
          }
        } else {
          setSelectionRange(null);
          setCommentBtnPos(null);
        }
      });

      modifiedEditor.onDidChangeModelContent(() => {
        if (!onEditedContentChange) return;
        const modifiedModel = modifiedEditor.getModel();
        if (!modifiedModel) return;
        const newContent = modifiedModel.getValue();
        const hunks = buildEditedHunks(editor, modifiedModel);
        onEditedContentChange(newContent, hunks);
      });

      editor.onDidUpdateDiff?.(() => {
        if (!onDiffHunksChange) return;
        const model = modifiedEditor.getModel();
        if (!model) return;
        onDiffHunksChange(buildEditedHunks(editor, model));
      });

      if (onDiffHunksChange) {
        setTimeout(() => {
          const model = modifiedEditor.getModel();
          if (!model) return;
          onDiffHunksChange(buildEditedHunks(editor, model));
        }, 0);
      }

      // Register "Go To Definition" action on both sub-editors
      const registerGoToDef = (subEditor: any) => {
        subEditor.addAction({
          id: "diffcore.goToDefinition",
          label: "Go To Definition",
          keybindings: [2048 /* CtrlCmd */ | 60 /* F12 */],
          contextMenuGroupId: "navigation",
          contextMenuOrder: 1,
          run: (ed: any) => {
            const pos = ed.getPosition();
            const model = ed.getModel();
            if (!pos || !model) return;
            const wordInfo = model.getWordAtPosition(pos);
            if (wordInfo?.word && goToDefRef.current) {
              goToDefRef.current(wordInfo.word);
            }
          },
        });
      };
      registerGoToDef(modifiedEditor);
      if (originalEditor) registerGoToDef(originalEditor);

      // Apply initial decorations
      if (codeComments && codeComments.length > 0) {
        decorationsRef.current = applyCommentDecorations(modifiedEditor, codeComments, onGlyphClick);
      }
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [],
  );

  // Re-apply decorations when codeComments changes (e.g., after adding/deleting a comment)
  useEffect(() => {
    const editor = editorRef.current?.getModifiedEditor?.();
    if (!editor) return;
    // Clear old decorations
    if (decorationsRef.current) {
      decorationsRef.current.clear();
    }
    if (codeComments && codeComments.length > 0) {
      decorationsRef.current = applyCommentDecorations(editor, codeComments, onGlyphClick);
    }
  }, [codeComments, onGlyphClick]);

  const handleCommentClick = useCallback(() => {
    if (!selectionRange || !editorRef.current || !onCommentRequest) return;

    const modifiedEditor = editorRef.current.getModifiedEditor();
    if (!modifiedEditor) return;

    const model = modifiedEditor.getModel();
    if (!model) return;

    // Extract the selected code text
    const { startLine, endLine } = selectionRange;
    const lines: string[] = [];
    for (let i = startLine; i <= endLine; i++) {
      lines.push(model.getLineContent(i));
    }
    const selectedCode = lines.join("\n");

    onCommentRequest(startLine, endLine, selectedCode);
    setSelectionRange(null);
    setCommentBtnPos(null);
  }, [selectionRange, onCommentRequest]);

  // Expose scrollToLine to parent via ref
  useImperativeHandle(ref, () => ({
    scrollToLine(startLine: number, endLine?: number) {
      const editor = editorRef.current?.getModifiedEditor?.();
      if (!editor) return;
      const model = editor.getModel?.();
      const lineCount = model?.getLineCount?.() ?? 1;
      const safeStart = Math.max(1, Math.min(startLine, lineCount));
      const rawEnd = endLine ?? safeStart;
      const safeEnd = Math.max(safeStart, Math.min(rawEnd, lineCount));
      // Scroll to the range center
      editor.revealLineInCenter(safeStart);
      // Select the entire range so the code block is visually obvious
      editor.setSelection({
        startLineNumber: safeStart,
        startColumn: 1,
        endLineNumber: safeEnd,
        endColumn: model?.getLineMaxColumn(safeEnd) ?? 1,
      });
      // Add a temporary highlight decoration on top
      const decs = editor.createDecorationsCollection([{
        range: { startLineNumber: safeStart, startColumn: 1, endLineNumber: safeEnd, endColumn: 1 },
        options: { isWholeLine: true, className: "comment-scroll-highlight" },
      }]);
      setTimeout(() => decs.clear(), 2500);
    },
    getDiffHunks() {
      const diffEditor = editorRef.current;
      const modifiedEditor = diffEditor?.getModifiedEditor?.();
      const modifiedModel = modifiedEditor?.getModel?.();
      if (!diffEditor || !modifiedModel) return [];

      const lineCount = modifiedModel.getLineCount?.() ?? 1;
      const lineChanges = diffEditor.getLineChanges?.() ?? [];
      return lineChanges.map((change: any) => {
        const modifiedStart = change.modifiedStartLineNumber;
        const modifiedEnd = change.modifiedEndLineNumber;
        const isDeletionOnly = modifiedStart === 0 || modifiedEnd === 0;
        const safeStart = isDeletionOnly
          ? Math.max(1, Math.min(change.originalStartLineNumber ?? 1, lineCount))
          : Math.max(1, Math.min(modifiedStart, lineCount));
        const safeEnd = isDeletionOnly
          ? safeStart
          : Math.max(safeStart, Math.min(modifiedEnd, lineCount));
        const selectedCode = isDeletionOnly
          ? ""
          : modifiedModel.getValueInRange({
              startLineNumber: safeStart,
              startColumn: 1,
              endLineNumber: safeEnd,
              endColumn: modifiedModel.getLineMaxColumn(safeEnd),
            });
        return {
          originalStartLine: change.originalStartLineNumber,
          originalEndLine: change.originalEndLineNumber,
          modifiedStartLine: safeStart,
          modifiedEndLine: safeEnd,
          selectedCode,
        };
      });
    },
    openFindWidget() {
      const editor = editorRef.current?.getModifiedEditor?.();
      if (!editor) return;
      const findAction = editor.getAction?.("actions.find");
      void findAction?.run?.();
    },
  }), [buildEditedHunks]);

  if (!fileDiff) {
    return (
      <div className="empty-state">Select a file to view its diff.</div>
    );
  }

  // Map our language strings to Monaco language IDs
  const monacoLang = mapLanguage(fileDiff.language);

  return (
    <div ref={containerRef} style={{ position: "relative", width: "100%", height: "100%" }}>
      <DiffEditor
        original={fileDiff.old_content || ""}
        modified={fileDiff.new_content || ""}
        language={monacoLang}
        theme="diffcore-dark"
        options={{
          readOnly: !editable,
          originalEditable: false,
          readOnlyMessage: { value: "" },
          renderSideBySide: renderSideBySideProp,
          enableSplitViewResizing: true,
          automaticLayout: true,
          scrollBeyondLastLine: false,
          minimap: { enabled: false },
          fontSize: 12,
          fontFamily:
            "'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace",
          lineNumbers: "on",
          glyphMargin: true,
          folding: true,
          renderWhitespace: "selection",
          scrollbar: {
            verticalScrollbarSize: 4,
            horizontalScrollbarSize: 4,
          },
          overviewRulerBorder: false,
          overviewRulerLanes: 0,
          hideCursorInOverviewRuler: true,
          renderOverviewRuler: false,
        }}
        beforeMount={(monaco) => {
          // Define custom dark theme matching the app's Catppuccin palette
          monaco.editor.defineTheme("diffcore-dark", {
            base: "vs-dark",
            inherit: true,
            rules: [],
            colors: {
              "editor.background": "#1e1e2e",
              "editor.foreground": "#cdd6f4",
              "editorLineNumber.foreground": "#6c7086",
              "editorLineNumber.activeForeground": "#a6adc8",
              "editor.selectionBackground": "#45475a",
              "editor.inactiveSelectionBackground": "#31324480",
              "editorIndentGuide.background1": "#31324480",
              "editorIndentGuide.activeBackground1": "#45475a",
              "diffEditor.insertedTextBackground": "#a6e3a118",
              "diffEditor.removedTextBackground": "#f38ba818",
              "diffEditor.insertedLineBackground": "#a6e3a110",
              "diffEditor.removedLineBackground": "#f38ba810",
              "scrollbar.shadow": "#00000000",
              "scrollbarSlider.background": "#45475a80",
              "scrollbarSlider.hoverBackground": "#6c7086",
              "scrollbarSlider.activeBackground": "#a6adc8",
            },
          });
        }}
        onMount={handleEditorMount}
      />

      {/* Inline "Comment" button appearing below the selected lines */}
      {selectionRange && commentBtnPos && onCommentRequest && (
        <button
          className="inline-comment-btn"
          style={{ top: commentBtnPos.top, left: commentBtnPos.left }}
          onClick={handleCommentClick}
          title={`Comment on lines ${selectionRange.startLine}-${selectionRange.endLine}`}
        >
          &#128172; Comment
        </button>
      )}
    </div>
  );
});

export default DiffViewer;

/** Apply gutter decorations for code-level comments. Returns the collection for cleanup. */
function applyCommentDecorations(
  editor: any,
  comments: ReviewComment[],
  onGlyphClick?: (commentId: string) => void,
): any {
  const decorations: any[] = [];

  for (const c of comments) {
    if (c.start_line == null || c.end_line == null) continue;

    // Glyph icon on the FIRST line only
    decorations.push({
      range: {
        startLineNumber: c.start_line,
        startColumn: 1,
        endLineNumber: c.start_line,
        endColumn: 1,
      },
      options: {
        glyphMarginClassName: `comment-glyph-icon comment-glyph-${c.id}`,
        glyphMarginHoverMessage: { value: `**Comment:** ${c.text}` },
      },
    });

    // Line highlight on the full range
    decorations.push({
      range: {
        startLineNumber: c.start_line,
        startColumn: 1,
        endLineNumber: c.end_line,
        endColumn: 1,
      },
      options: {
        isWholeLine: true,
        className: "comment-line-highlight",
      },
    });
  }

  if (decorations.length === 0) return null;

  const collection = editor.createDecorationsCollection(decorations);

  // Set up click handler on glyph margin to select the comment
  if (onGlyphClick) {
    editor.onMouseDown?.((e: any) => {
      if (e.target?.type === 2 /* GLYPH_MARGIN */ && e.target?.element?.classList?.contains("comment-glyph-icon")) {
        // Find which comment this glyph belongs to by matching the line number
        const line = e.target.position?.lineNumber;
        if (line != null) {
          const comment = comments.find((c) => c.start_line === line);
          if (comment) {
            onGlyphClick(comment.id);
          }
        }
      }
    });
  }

  return collection;
}

function mapLanguage(lang: string): string {
  const map: Record<string, string> = {
    typescript: "typescript",
    javascript: "javascript",
    python: "python",
    rust: "rust",
    json: "json",
    toml: "toml",
    yaml: "yaml",
    markdown: "markdown",
    css: "css",
    html: "html",
    sql: "sql",
    shell: "shell",
    go: "go",
    java: "java",
    ruby: "ruby",
    prisma: "graphql", // Closest Monaco match for Prisma
    plaintext: "plaintext",
  };
  return map[lang] || "plaintext";
}
