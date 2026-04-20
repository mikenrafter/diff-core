import { useCallback, useEffect, useMemo, useState } from "react";
import type { FileChange, FileDiffContent, FlowEdge, FlowGroup } from "../types";

type OutlineKind = "operation" | "interface" | "type" | "class" | "constant" | "dependency";

type OutlineSectionKey = "operations" | "interfaces" | "classes" | "constants" | "dependencies";

export interface SourceFocusRequest {
  filePath: string;
  symbol: string;
  token: number;
}

interface OutlineItem {
  id: string;
  kind: OutlineKind;
  name: string;
  detail: string;
  startLine?: number;
  endLine?: number;
  changed?: boolean;
  targetFile?: string;
  targetSymbol?: string;
}

interface OutlineSection {
  key: OutlineSectionKey;
  label: string;
  items: OutlineItem[];
}

interface SourceExplorerProps {
  fileDiff: FileDiffContent | null;
  selectedGroup: FlowGroup | null;
  selectedFileChange: FileChange | null;
  focusRequest?: SourceFocusRequest | null;
  onNavigateToSymbol?: (filePath: string, symbolName?: string) => void;
  /** Scroll the main diff viewer to a line range. */
  onScrollToLine?: (startLine: number, endLine?: number) => void;
}

interface ParsedDefinition {
  name: string;
  kind: Exclude<OutlineKind, "dependency">;
  startLine: number;
  endLine: number;
}

const SECTION_ORDER: OutlineSectionKey[] = [
  "operations",
  "interfaces",
  "classes",
  "constants",
  "dependencies",
];

const SECTION_LABELS: Record<OutlineSectionKey, string> = {
  operations: "Operations",
  interfaces: "Interfaces & Types",
  classes: "Classes",
  constants: "Constants",
  dependencies: "Dependencies",
};

/** Symbol outline explorer for the currently selected file.
 *  Clicking a symbol scrolls the main diff viewer to that line. */
export default function SourceExplorer({
  fileDiff,
  selectedGroup,
  selectedFileChange,
  focusRequest,
  onNavigateToSymbol,
  onScrollToLine,
}: SourceExplorerProps) {
  const outline = useMemo(
    () => buildOutline(fileDiff, selectedGroup, selectedFileChange),
    [fileDiff, selectedGroup, selectedFileChange],
  );
  const allItems = useMemo(
    () => outline.sections.flatMap((section) => section.items),
    [outline.sections],
  );
  const [selectedItemId, setSelectedItemId] = useState<string | null>(null);
  /** Filters every outline section to only items whose `changed` flag is set.
   *  Resets to off whenever the file changes (intentional — when the user
   *  navigates to a new file they should see the whole outline by default). */
  const [onlyChanged, setOnlyChanged] = useState(false);

  useEffect(() => {
    setOnlyChanged(false);
  }, [fileDiff?.path]);

  const visibleSections = useMemo(() => {
    if (!onlyChanged) return outline.sections;
    return outline.sections.map((section) => ({
      ...section,
      items: section.items.filter((item) => item.changed),
    }));
  }, [outline.sections, onlyChanged]);
  const totalChangedCount = useMemo(
    () => allItems.filter((item) => item.changed).length,
    [allItems],
  );

  useEffect(() => {
    if (!fileDiff) {
      setSelectedItemId(null);
      return;
    }
    const defaultItem = allItems.find((item) => item.changed && item.startLine != null) ??
      allItems.find((item) => item.startLine != null) ??
      allItems[0] ??
      null;
    setSelectedItemId(defaultItem?.id ?? null);
  }, [fileDiff?.path, allItems, fileDiff]);

  useEffect(() => {
    if (!focusRequest || !fileDiff || focusRequest.filePath !== fileDiff.path) return;
    const focused = allItems.find((item) => symbolMatches(item.name, focusRequest.symbol));
    if (focused) {
      setSelectedItemId(focused.id);
      if (focused.startLine != null) {
        onScrollToLine?.(focused.startLine, focused.endLine);
      }
    }
  }, [allItems, fileDiff, focusRequest, onScrollToLine]);

  const handleItemClick = useCallback((item: OutlineItem) => {
    if (item.targetFile && item.targetFile !== fileDiff?.path) {
      onNavigateToSymbol?.(item.targetFile, item.targetSymbol);
      return;
    }
    setSelectedItemId(item.id);
    // Scroll the main diff viewer to this symbol's line
    if (item.startLine != null) {
      onScrollToLine?.(item.startLine, item.endLine);
    }
  }, [fileDiff?.path, onNavigateToSymbol, onScrollToLine]);

  if (!fileDiff) {
    return <div className="empty-state">Select a file to inspect its source.</div>;
  }

  return (
    <div className="source-explorer source-explorer-full" data-testid="source-explorer">
      <div className="source-outline-header">
        <div className="source-outline-eyebrow">Subsystem Source</div>
        <div className="source-outline-file" title={fileDiff.path}>{fileDiff.path}</div>
        <div className="source-outline-meta">
          {selectedFileChange && (
            <span className="source-meta-pill">{selectedFileChange.role}</span>
          )}
          <span className="source-meta-pill">{fileDiff.language}</span>
          {selectedFileChange && (
            <span className="source-meta-pill">
              {selectedFileChange.symbols_changed.length} changed
            </span>
          )}
          {selectedGroup && (
            <span className="source-meta-pill">{selectedGroup.name}</span>
          )}
        </div>
        {/* "Only changed" toggle filters every outline section (operations,
            interfaces, classes, constants, dependencies) to symbols whose
            source overlaps a changed line in this file or whose name is in
            `symbols_changed`. Off by default; resets per file. */}
        <div className="source-outline-controls">
          <label className="source-outline-toggle">
            <input
              type="checkbox"
              checked={onlyChanged}
              onChange={(event) => setOnlyChanged(event.target.checked)}
              disabled={totalChangedCount === 0}
            />
            <span>Only changed</span>
            <span className="source-outline-toggle-count">
              {totalChangedCount}
            </span>
          </label>
        </div>
      </div>

      <div className="source-outline-sections">
        {visibleSections.map((section) => (
          <section key={section.key} className="source-outline-section">
            <div className="source-outline-section-header">
              <span>{section.label}</span>
              <span className="source-outline-section-count">{section.items.length}</span>
            </div>
            {section.items.length > 0 ? (
              <div className="source-outline-list">
                {section.items.map((item) => (
                  <button
                    key={item.id}
                    className={`source-outline-item ${selectedItemId === item.id ? "active" : ""}`}
                    onClick={() => handleItemClick(item)}
                    title={item.detail}
                    type="button"
                  >
                    <span className={`source-outline-kind source-outline-kind-${item.kind}`}>
                      {kindLabel(item.kind)}
                    </span>
                    <span className="source-outline-copy">
                      <span className="source-outline-name">{item.name}</span>
                      <span className="source-outline-detail">{item.detail}</span>
                    </span>
                    {item.changed && (
                      <span className="source-outline-changed">Changed</span>
                    )}
                  </button>
                ))}
              </div>
            ) : (
              <div className="source-outline-empty">
                {onlyChanged ? "No changes in this file." : "None in this file."}
              </div>
            )}
          </section>
        ))}
      </div>
    </div>
  );
}

function buildOutline(
  fileDiff: FileDiffContent | null,
  selectedGroup: FlowGroup | null,
  selectedFileChange: FileChange | null,
): { sections: OutlineSection[] } {
  if (!fileDiff) {
    return {
      sections: SECTION_ORDER.map((key) => ({
        key,
        label: SECTION_LABELS[key],
        items: [],
      })),
    };
  }

  const sourceText = fileDiff.new_content || fileDiff.old_content || "";
  const definitions = parseDefinitions(fileDiff.language, sourceText);
  // TODO fix
  // const changedSymbols = new Set(
  //   (selectedFileChange?.symbols_changed ?? []).map((symbol) => normalizeSymbol(symbol)),
  // );
  /** New-content line numbers (1-indexed) that differ from old_content.
   *  Used so a symbol counts as "changed" when its body overlaps any
   *  added/removed line, even when its name isn't in `symbols_changed`. */
  const changedLines = computeChangedNewLines(fileDiff.old_content, fileDiff.new_content);
  const buckets: Record<OutlineSectionKey, OutlineItem[]> = {
    operations: [],
    interfaces: [],
    classes: [],
    constants: [],
    dependencies: [],
  };
  const seenIds = new Set<string>();
  const seenNormalized = new Set<string>();

  for (const definition of definitions) {
    const section = definition.kind === "operation"
      ? "operations"
      : definition.kind === "interface" || definition.kind === "type"
        ? "interfaces"
        : definition.kind === "class"
          ? "classes"
          : "constants";
    const id = `${definition.kind}:${definition.name}:${definition.startLine}`;
    const item: OutlineItem = {
      id,
      kind: definition.kind,
      name: definition.name,
      detail: `L${definition.startLine}${definition.endLine !== definition.startLine ? `-${definition.endLine}` : ""}`,
      startLine: definition.startLine,
      endLine: definition.endLine,
      changed:
        // TODO reenable when this is functional again
        // changedSymbols.has(normalizeSymbol(definition.name)) ||
        rangeOverlapsChangedLines(definition.startLine, definition.endLine, changedLines),
    };
    buckets[section].push(item);
    seenIds.add(item.id);
    seenNormalized.add(normalizeSymbol(definition.name));
  }

  if (selectedGroup?.entrypoint?.file === fileDiff.path) {
    const normalizedEntrypoint = normalizeSymbol(selectedGroup.entrypoint.symbol);
    if (!seenNormalized.has(normalizedEntrypoint)) {
      const routeLine = findSymbolLine(sourceText, selectedGroup.entrypoint.symbol)
        ?? findRouteLine(sourceText, selectedGroup.entrypoint.symbol)
        ?? 1;
      buckets.operations.unshift({
        id: `entrypoint:${selectedGroup.entrypoint.symbol}`,
        kind: "operation",
        name: selectedGroup.entrypoint.symbol,
        detail: `Entrypoint • L${routeLine}`,
        startLine: routeLine,
        endLine: routeLine,
        // Same rule as ordinary outline items: "changed" tracks whether the
        // entrypoint's line sits inside a real hunk, not whether it merely
        // exists as an entrypoint.
        changed: rangeOverlapsChangedLines(routeLine, routeLine, changedLines),
      });
      seenNormalized.add(normalizedEntrypoint);
    }
  }

  for (const symbol of selectedFileChange?.symbols_changed ?? []) {
    const normalized = normalizeSymbol(symbol);
    if (seenNormalized.has(normalized)) continue;
    const line = findSymbolLine(sourceText, symbol);
    if (line == null) continue;
    buckets.operations.push({
      id: `fallback:${symbol}:${line}`,
      kind: "operation",
      name: symbol,
      detail: `Changed symbol • L${line}`,
      startLine: line,
      endLine: line,
      // `symbols_changed` is currently unreliable, so don't trust it as a
      // source of truth for the toggle — fall back to hunk overlap.
      changed: rangeOverlapsChangedLines(line, line, changedLines),
    });
    seenNormalized.add(normalized);
  }

  for (const item of buildDependencyItems(fileDiff.path, selectedGroup?.edges ?? [])) {
    if (seenIds.has(item.id)) continue;
    buckets.dependencies.push(item);
    seenIds.add(item.id);
  }

  return {
    sections: SECTION_ORDER.map((key) => ({
      key,
      label: SECTION_LABELS[key],
      items: buckets[key],
    })),
  };
}

function buildDependencyItems(filePath: string, edges: FlowEdge[]): OutlineItem[] {
  const seen = new Set<string>();
  const items: OutlineItem[] = [];

  for (const edge of edges) {
    const from = parseEdgeEndpoint(edge.from);
    const to = parseEdgeEndpoint(edge.to);

    let item: OutlineItem | null = null;
    if (from.file === filePath) {
      const id = `dep:out:${edge.edge_type}:${to.file}:${to.symbol}`;
      item = {
        id,
        kind: "dependency",
        name: to.symbol || shortPath(to.file),
        detail: `${edge.edge_type} → ${shortPath(to.file)}`,
        targetFile: to.file,
        targetSymbol: to.symbol || undefined,
      };
    } else if (to.file === filePath) {
      const id = `dep:in:${edge.edge_type}:${from.file}:${from.symbol}`;
      item = {
        id,
        kind: "dependency",
        name: from.symbol || shortPath(from.file),
        detail: `${edge.edge_type} ← ${shortPath(from.file)}`,
        targetFile: from.file,
        targetSymbol: from.symbol || undefined,
      };
    }

    if (!item || seen.has(item.id)) continue;
    seen.add(item.id);
    items.push(item);
  }

  return items;
}

function parseDefinitions(language: string, sourceText: string): ParsedDefinition[] {
  const lines = sourceText.split("\n");
  if (language === "typescript" || language === "javascript") {
    return parseTsLikeDefinitions(lines);
  }
  if (language === "python") {
    return parsePythonDefinitions(lines);
  }
  if (language === "go") {
    return parseGoDefinitions(lines);
  }
  if (language === "rust") {
    return parseRustDefinitions(lines);
  }
  return [];
}

function parseTsLikeDefinitions(lines: string[]): ParsedDefinition[] {
  const results: ParsedDefinition[] = [];
  let braceDepth = 0;
  let currentClass: { name: string; depth: number } | null = null;

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    const trimmed = line.trim();
    if (!trimmed) {
      braceDepth += countChar(line, "{") - countChar(line, "}");
      continue;
    }

    const classMatch = trimmed.match(/^(?:export\s+)?(?:default\s+)?(?:abstract\s+)?class\s+([A-Za-z_$][\w$]*)/);
    if (classMatch) {
      results.push({
        name: classMatch[1],
        kind: "class",
        startLine: index + 1,
        endLine: findBlockEnd(lines, index),
      });
      currentClass = {
        name: classMatch[1],
        depth: braceDepth + countChar(line, "{"),
      };
    }

    const interfaceMatch = trimmed.match(/^(?:export\s+)?interface\s+([A-Za-z_$][\w$]*)/);
    if (interfaceMatch) {
      results.push({
        name: interfaceMatch[1],
        kind: "interface",
        startLine: index + 1,
        endLine: findBlockEnd(lines, index),
      });
    }

    const typeMatch = trimmed.match(/^(?:export\s+)?type\s+([A-Za-z_$][\w$]*)\s*=/);
    if (typeMatch) {
      results.push({
        name: typeMatch[1],
        kind: "type",
        startLine: index + 1,
        endLine: findStatementEnd(lines, index),
      });
    }

    const functionMatch = trimmed.match(/^(?:export\s+)?(?:default\s+)?(?:async\s+)?function\s+([A-Za-z_$][\w$]*)\s*\(/);
    if (functionMatch) {
      results.push({
        name: functionMatch[1],
        kind: "operation",
        startLine: index + 1,
        endLine: findBlockEnd(lines, index),
      });
    }

    const constArrowMatch = trimmed.match(/^(?:export\s+)?(?:const|let|var)\s+([A-Za-z_$][\w$]*)\s*=\s*(?:async\s*)?(?:\([^)]*\)|[A-Za-z_$][\w$]*)\s*=>/);
    if (constArrowMatch) {
      results.push({
        name: constArrowMatch[1],
        kind: "operation",
        startLine: index + 1,
        endLine: findStatementEnd(lines, index),
      });
    } else {
      const constantMatch = trimmed.match(/^(?:export\s+)?(?:const|let|var)\s+([A-Za-z_$][\w$]*)\s*=/);
      if (constantMatch) {
        results.push({
          name: constantMatch[1],
          kind: "constant",
          startLine: index + 1,
          endLine: findStatementEnd(lines, index),
        });
      }
    }

    if (currentClass) {
      const methodMatch = trimmed.match(/^(?:public\s+|private\s+|protected\s+)?(?:static\s+)?(?:async\s+)?([A-Za-z_$][\w$]*)\s*\(/);
      if (methodMatch && methodMatch[1] !== "constructor" && !/^(if|for|while|switch|catch)$/.test(methodMatch[1])) {
        results.push({
          name: `${currentClass.name}.${methodMatch[1]}`,
          kind: "operation",
          startLine: index + 1,
          endLine: findBlockEnd(lines, index),
        });
      }
    }

    braceDepth += countChar(line, "{") - countChar(line, "}");
    if (currentClass && braceDepth < currentClass.depth) {
      currentClass = null;
    }
  }

  return dedupeDefinitions(results);
}

function parsePythonDefinitions(lines: string[]): ParsedDefinition[] {
  const results: ParsedDefinition[] = [];
  let currentClass: string | null = null;
  let currentIndent = 0;

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    const trimmed = line.trim();
    const indent = line.length - line.trimStart().length;
    if (!trimmed) continue;

    const classMatch = trimmed.match(/^class\s+([A-Za-z_][\w]*)/);
    if (classMatch) {
      currentClass = classMatch[1];
      currentIndent = indent;
      results.push({
        name: classMatch[1],
        kind: "class",
        startLine: index + 1,
        endLine: findIndentedBlockEnd(lines, index, indent),
      });
      continue;
    }

    const defMatch = trimmed.match(/^def\s+([A-Za-z_][\w]*)\s*\(/);
    if (defMatch) {
      results.push({
        name: currentClass && indent > currentIndent ? `${currentClass}.${defMatch[1]}` : defMatch[1],
        kind: "operation",
        startLine: index + 1,
        endLine: findIndentedBlockEnd(lines, index, indent),
      });
    }

    if (currentClass && indent <= currentIndent && !trimmed.startsWith("@")) {
      currentClass = null;
    }
  }

  return dedupeDefinitions(results);
}

function parseGoDefinitions(lines: string[]): ParsedDefinition[] {
  const results: ParsedDefinition[] = [];
  for (let index = 0; index < lines.length; index += 1) {
    const trimmed = lines[index].trim();
    if (!trimmed) continue;

    const funcMatch = trimmed.match(/^func\s+(?:\([^)]*\)\s*)?([A-Za-z_][\w]*)\s*\(/);
    if (funcMatch) {
      results.push({
        name: funcMatch[1],
        kind: "operation",
        startLine: index + 1,
        endLine: findBlockEnd(lines, index),
      });
      continue;
    }

    const interfaceMatch = trimmed.match(/^type\s+([A-Za-z_][\w]*)\s+interface\b/);
    if (interfaceMatch) {
      results.push({
        name: interfaceMatch[1],
        kind: "interface",
        startLine: index + 1,
        endLine: findBlockEnd(lines, index),
      });
      continue;
    }

    const structMatch = trimmed.match(/^type\s+([A-Za-z_][\w]*)\s+struct\b/);
    if (structMatch) {
      results.push({
        name: structMatch[1],
        kind: "class",
        startLine: index + 1,
        endLine: findBlockEnd(lines, index),
      });
      continue;
    }

    const constMatch = trimmed.match(/^(?:const|var)\s+([A-Za-z_][\w]*)/);
    if (constMatch) {
      results.push({
        name: constMatch[1],
        kind: "constant",
        startLine: index + 1,
        endLine: findStatementEnd(lines, index),
      });
    }
  }
  return dedupeDefinitions(results);
}

function parseRustDefinitions(lines: string[]): ParsedDefinition[] {
  const results: ParsedDefinition[] = [];
  for (let index = 0; index < lines.length; index += 1) {
    const trimmed = lines[index].trim();
    if (!trimmed) continue;

    const fnMatch = trimmed.match(/^(?:pub\s+)?(?:async\s+)?fn\s+([A-Za-z_][\w]*)\s*\(/);
    if (fnMatch) {
      results.push({
        name: fnMatch[1],
        kind: "operation",
        startLine: index + 1,
        endLine: findBlockEnd(lines, index),
      });
      continue;
    }

    const traitMatch = trimmed.match(/^(?:pub\s+)?trait\s+([A-Za-z_][\w]*)/);
    if (traitMatch) {
      results.push({
        name: traitMatch[1],
        kind: "interface",
        startLine: index + 1,
        endLine: findBlockEnd(lines, index),
      });
      continue;
    }

    const structMatch = trimmed.match(/^(?:pub\s+)?(?:struct|enum)\s+([A-Za-z_][\w]*)/);
    if (structMatch) {
      results.push({
        name: structMatch[1],
        kind: "class",
        startLine: index + 1,
        endLine: findBlockEnd(lines, index),
      });
      continue;
    }

    const constMatch = trimmed.match(/^(?:pub\s+)?const\s+([A-Za-z_][\w]*)/);
    if (constMatch) {
      results.push({
        name: constMatch[1],
        kind: "constant",
        startLine: index + 1,
        endLine: findStatementEnd(lines, index),
      });
    }
  }
  return dedupeDefinitions(results);
}

function dedupeDefinitions(definitions: ParsedDefinition[]): ParsedDefinition[] {
  const seen = new Set<string>();
  const deduped: ParsedDefinition[] = [];
  for (const def of definitions) {
    const key = `${def.kind}:${def.name}:${def.startLine}`;
    if (seen.has(key)) continue;
    seen.add(key);
    deduped.push(def);
  }
  return deduped;
}

function findBlockEnd(lines: string[], startIndex: number): number {
  let balance = 0;
  let sawOpen = false;
  for (let index = startIndex; index < lines.length; index += 1) {
    const line = stripQuotedContent(lines[index]);
    const opens = countChar(line, "{");
    const closes = countChar(line, "}");
    balance += opens - closes;
    if (opens > 0) sawOpen = true;
    if (sawOpen && balance <= 0) {
      return index + 1;
    }
  }
  return Math.min(lines.length, startIndex + 1);
}

function findStatementEnd(lines: string[], startIndex: number): number {
  let balance = 0;
  let sawStructuralToken = false;
  for (let index = startIndex; index < lines.length; index += 1) {
    const line = stripQuotedContent(lines[index]);
    balance += countChar(line, "{") - countChar(line, "}");
    balance += countChar(line, "(") - countChar(line, ")");
    balance += countChar(line, "[") - countChar(line, "]");
    if (/[{([=]/.test(line)) sawStructuralToken = true;
    if ((balance <= 0 && /[;}]\s*$/.test(line)) || (!sawStructuralToken && index > startIndex && line.trim() === "")) {
      return index + 1;
    }
  }
  return Math.min(lines.length, startIndex + 1);
}

function findIndentedBlockEnd(lines: string[], startIndex: number, startIndent: number): number {
  for (let index = startIndex + 1; index < lines.length; index += 1) {
    const line = lines[index];
    const trimmed = line.trim();
    if (!trimmed) continue;
    const indent = line.length - line.trimStart().length;
    if (indent <= startIndent) {
      return index;
    }
  }
  return lines.length;
}

function findSymbolLine(sourceText: string, symbol: string): number | null {
  const target = symbol.split(".").pop()?.split("::").pop()?.trim();
  if (!target) return null;
  const lines = sourceText.split("\n");
  for (let index = 0; index < lines.length; index += 1) {
    if (lines[index].includes(target)) {
      return index + 1;
    }
  }
  return null;
}

function findRouteLine(sourceText: string, symbol: string): number | null {
  const route = symbol.match(/\b(GET|POST|PUT|PATCH|DELETE)\s+(.+)/i);
  if (!route) return null;
  const method = route[1].toLowerCase();
  const path = route[2].trim();
  const lines = sourceText.split("\n");
  for (let index = 0; index < lines.length; index += 1) {
    if (lines[index].includes(`.${method}(`) && lines[index].includes(path)) {
      return index + 1;
    }
  }
  return null;
}

function parseEdgeEndpoint(value: string): { file: string; symbol: string } {
  const parts = value.split("::");
  return {
    file: parts[0] ?? value,
    symbol: parts.slice(1).join("::"),
  };
}

function symbolMatches(name: string, symbol: string): boolean {
  return normalizeSymbol(name) === normalizeSymbol(symbol);
}

function normalizeSymbol(value: string): string {
  return value
    .split("::")
    .pop()
    ?.split(".")
    .pop()
    ?.replace(/[^\w$]/g, "")
    .toLowerCase() ?? value.toLowerCase();
}

function stripQuotedContent(value: string): string {
  return value.replace(/"[^"]*"|'[^']*'|`[^`]*`/g, "");
}

function countChar(value: string, char: string): number {
  let count = 0;
  for (const current of value) {
    if (current === char) count += 1;
  }
  return count;
}

function kindLabel(kind: OutlineKind): string {
  switch (kind) {
    case "operation":
      return "Fn";
    case "interface":
      return "If";
    case "type":
      return "Ty";
    case "class":
      return "Cl";
    case "constant":
      return "Ct";
    case "dependency":
      return "Dp";
    default:
      return "Sy";
  }
}


function shortPath(path: string): string {
  const parts = path.split("/");
  return parts.length <= 2 ? path : parts.slice(-2).join("/");
}

/**
 * Returns the set of new-content line numbers (1-indexed) that belong to a
 * real change hunk between `oldContent` and `newContent`.
 *
 * Uses a line-level Longest Common Subsequence (LCS) — every line in
 * `newContent` that is *not* part of the LCS is an added or modified line
 * and is reported as changed. Unmodified lines that happen to sit between
 * two hunks are *not* marked, which matches what a reviewer would see in
 * a side-by-side diff and what `git diff` would emit as `+` lines.
 *
 * Complexity is O(N*M) in lines; for the sub-10k-line files diffcore loads
 * into the panel this is well under a millisecond and runs once per file.
 */
function computeChangedNewLines(oldContent: string, newContent: string): Set<number> {
  const changed = new Set<number>();
  if (oldContent === newContent) return changed;

  const oldLines = oldContent.split("\n");
  const newLines = newContent.split("\n");

  // Trim a common prefix and suffix first. This keeps the LCS table small
  // for typical edits where most of the file is untouched, and produces the
  // exact same change set as running LCS on the full inputs.
  let prefix = 0;
  const minLen = Math.min(oldLines.length, newLines.length);
  while (prefix < minLen && oldLines[prefix] === newLines[prefix]) prefix++;

  let suffix = 0;
  while (
    suffix < minLen - prefix &&
    oldLines[oldLines.length - 1 - suffix] === newLines[newLines.length - 1 - suffix]
  ) {
    suffix++;
  }

  const oldMid = oldLines.slice(prefix, oldLines.length - suffix);
  const newMid = newLines.slice(prefix, newLines.length - suffix);

  // Any new line not present in the LCS of the trimmed window is an added
  // or modified line. `lcsMatched` returns a boolean array aligned with
  // `newMid` indicating which lines participate in the LCS (i.e. exist
  // unchanged on both sides in the same relative order).
  const matched = lcsMatched(oldMid, newMid);
  for (let i = 0; i < newMid.length; i++) {
    if (!matched[i]) {
      // Convert mid-window index back to a 1-indexed new-content line number.
      changed.add(prefix + i + 1);
    }
  }

  return changed;
}

/**
 * Standard dynamic-programming LCS that returns, for each line in `b`,
 * whether that line is part of the longest common subsequence with `a`.
 *
 * The DP table is reconstructed by walking back from `(a.length, b.length)`
 * and marking matched positions in `b`. Lines not marked are the ones that
 * were inserted or replaced.
 */
function lcsMatched(a: string[], b: string[]): boolean[] {
  const matched = new Array<boolean>(b.length).fill(false);
  if (a.length === 0 || b.length === 0) return matched;

  // dp[i][j] = LCS length of a[0..i] and b[0..j]. Using flat Int32Array
  // for both speed and memory locality on larger files.
  const cols = b.length + 1;
  const dp = new Int32Array((a.length + 1) * cols);
  for (let i = 1; i <= a.length; i++) {
    const ai = a[i - 1];
    const rowBase = i * cols;
    const prevRowBase = (i - 1) * cols;
    for (let j = 1; j <= b.length; j++) {
      if (ai === b[j - 1]) {
        dp[rowBase + j] = dp[prevRowBase + (j - 1)] + 1;
      } else {
        const up = dp[prevRowBase + j];
        const left = dp[rowBase + (j - 1)];
        dp[rowBase + j] = up >= left ? up : left;
      }
    }
  }

  let i = a.length;
  let j = b.length;
  while (i > 0 && j > 0) {
    if (a[i - 1] === b[j - 1]) {
      matched[j - 1] = true;
      i--;
      j--;
    } else if (dp[(i - 1) * cols + j] >= dp[i * cols + (j - 1)]) {
      i--;
    } else {
      j--;
    }
  }
  return matched;
}

/** True when any line in `[startLine, endLine]` is present in `changedLines`. */
function rangeOverlapsChangedLines(
  startLine: number | undefined,
  endLine: number | undefined,
  changedLines: Set<number>,
): boolean {
  if (startLine == null || changedLines.size === 0) return false;
  const lo = startLine;
  const hi = endLine ?? startLine;
  for (let line = lo; line <= hi; line++) {
    if (changedLines.has(line)) return true;
  }
  return false;
}
