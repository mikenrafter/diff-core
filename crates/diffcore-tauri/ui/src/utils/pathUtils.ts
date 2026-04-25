/** Path and symbol utility functions. */

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

export function truncateSearchResultLine(line: string): string {
  if (line.length <= 200) return line;
  return `${line.slice(0, 200)}...`;
}
