/**
 * Tauri runtime detection and invoke wrapper.
 *
 * WHY: Several components need to know whether they're running inside the
 * Tauri shell (vs. a plain browser for demo/test) and invoke Tauri commands.
 * Centralising these here avoids duplicating the detection logic and lets
 * component files import clean constants rather than re-deriving them.
 */

/** True when the app is running inside the Tauri shell. */
export const IS_TAURI =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

import { logger } from "./logger";

/**
 * Session save/restore is currently disabled. This flag gates the
 * "Restore Session" button in the HeaderBar so it never appears until
 * the feature is re-enabled.
 */
export const STATE_SAVE_RESTORE_ENABLED = false;

/**
 * Lazy-loads the Tauri `invoke` function and calls it.
 * Only use this inside code paths that are already guarded by `IS_TAURI`.
 */
export async function tauriInvoke<T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return logger.time({
    category: "ipc",
    event: cmd,
    data: logger.summarizeArgs(args),
    fn: () => invoke<T>(cmd, args),
  });
}
