/**
 * Stable Monaco hunk IDs for replay / reviewed-hunk counting.
 *
 * Paths may contain underscores, so we must not join path + numbers with bare
 * underscores only. New format: `monaco_hunk|<encodeURIComponent(path)>|<start>|<end>|<index>`.
 */
export function makeMonacoHunkId(
  filePath: string,
  modifiedStartLine: number,
  modifiedEndLine: number,
  index: number,
): string {
  return `monaco_hunk|${encodeURIComponent(filePath)}|${modifiedStartLine}|${modifiedEndLine}|${index}`;
}

/** Returns repo-relative file path for a stored hunk id, or null if unknown. */
export function filePathFromMonacoHunkId(id: string): string | null {
  if (id.startsWith("monaco_hunk|")) {
    const parts = id.split("|");
    if (parts.length !== 5 || parts[0] !== "monaco_hunk") return null;
    try {
      return decodeURIComponent(parts[1]!);
    } catch {
      return null;
    }
  }

  // Legacy: monaco_hunk_<path with underscores>_<start>_<end>_<index> — parse from the right.
  if (!id.startsWith("monaco_hunk_")) return null;
  const rest = id.slice("monaco_hunk_".length);
  const segments = rest.split("_");
  if (segments.length < 4) return null;
  const indexStr = segments[segments.length - 1]!;
  const endStr = segments[segments.length - 2]!;
  const startStr = segments[segments.length - 3]!;
  if (!/^\d+$/.test(indexStr) || !/^\d+$/.test(endStr) || !/^\d+$/.test(startStr)) {
    return null;
  }
  return segments.slice(0, -3).join("_");
}
