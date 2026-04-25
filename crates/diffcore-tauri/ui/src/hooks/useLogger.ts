import { useSyncExternalStore } from "react";
import { logger, type LoggerSnapshot } from "../utils/logger";

export function useLogger(): LoggerSnapshot {
  return useSyncExternalStore(
    (cb) => logger.subscribe(cb),
    () => logger.getState(),
    () => logger.getState(),
  );
}

