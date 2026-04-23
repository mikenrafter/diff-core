import type { EditedHunk } from "../components/DiffViewer";

export function shortPath(path: string): string {
  const parts = path.split("/");
  if (parts.length <= 4) return path;
  return parts.slice(-4).join("/");
}

export function shortSymbol(symbol: string): string {
  const parts = symbol.split("::");
  if (parts.length <= 1) return symbol;
  return parts[parts.length - 1];
}

/** Inverse of `shortSymbol` — returns just the file/module portion of a
 *  qualified symbol like `crates/foo/src/bar.ts::myFn`. Falls back to the
 *  raw symbol when no `::` separator is present (e.g. external symbols). */
export function symbolFilePath(symbol: string): string {
  const idx = symbol.indexOf("::");
  return idx >= 0 ? symbol.slice(0, idx) : symbol;
}

export function truncateSearchResultLine(line: string): string {
  if (line.length <= 200) return line;
  return `${line.slice(0, 200)}...`;
}

export function parseSymbolEndpoint(endpoint: string): { filePath: string; symbol: string | null } {
  const [filePath, ...symbolParts] = endpoint.split("::");
  return {
    filePath,
    symbol: symbolParts.length > 0 ? symbolParts.join("::") : null,
  };
}

export function findLineContainingSymbol(content: string, symbol: string): number | null {
  const cleaned = symbol.split(".").pop()?.split("::").pop()?.trim();
  if (!cleaned) return null;
  const lines = content.split("\n");
  for (let index = 0; index < lines.length; index += 1) {
    if (lines[index].includes(cleaned)) {
      return index + 1;
    }
  }
  return null;
}

export function extractPathLikeToken(text: string): string | null {
  const matches = text.match(/(?:\/|\.{1,2}\/)?[A-Za-z0-9._@-]+(?:\/[A-Za-z0-9._@-]+)+/g);
  return matches && matches.length > 0 ? matches[matches.length - 1] : null;
}

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
