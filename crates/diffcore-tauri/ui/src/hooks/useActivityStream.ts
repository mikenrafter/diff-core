/**
 * useActivityStream — owns the LLM activity streaming state and all SSE
 * connection management.
 *
 * Keeps the EventSource lifecycle (open, append, close) co-located with the
 * job state (job metadata, raw entries, error). App.tsx receives the returned
 * values and threads them into the context value.
 *
 * WHY a separate hook: the streaming state and its callbacks form a cohesive
 * unit. closeActivityStream must close the ref; appendActivityEntry updates
 * the entries list; runStreamingJob orchestrates both. None of these belong
 * on App.tsx alongside routing, diff loading, or comment logic.
 */
import { useState, useRef, useCallback, useEffect } from "react";
import type { LlmActivityEntry, LlmActivityJob, AsyncLlmJobStart } from "../types";
import { tauriInvoke } from "../utils/tauriUtils";

type ActivityViewMode = "stream" | "all";

interface UseActivityStreamParams {
  /** Called when a job becomes active — used to switch the right panel to the activity tab. */
  onJobActive: () => void;
  /** Setter for the inspected activity card — reset to null when a new job starts. */
  setInspectedActivityId: (id: string | null) => void;
}

export interface UseActivityStreamResult {
  activityJob: LlmActivityJob | null;
  setActivityJob: React.Dispatch<React.SetStateAction<LlmActivityJob | null>>;
  activityEntries: LlmActivityEntry[];
  setActivityEntries: React.Dispatch<React.SetStateAction<LlmActivityEntry[]>>;
  activityError: string | null;
  setActivityError: React.Dispatch<React.SetStateAction<string | null>>;
  activityViewMode: ActivityViewMode;
  setActivityViewMode: React.Dispatch<React.SetStateAction<ActivityViewMode>>;
  closeActivityStream: () => void;
  runMockActivityJob: <T>(
    job: LlmActivityJob,
    entries: Array<Omit<LlmActivityEntry, "timestamp_ms">>,
    result: T,
    onComplete: (value: T) => void,
  ) => Promise<void>;
  runStreamingJob: <T>(
    command: string,
    args: Record<string, unknown>,
    onComplete: (value: T) => void,
  ) => Promise<void>;
}

/** Manages the SSE activity stream for LLM annotation and refinement jobs. */
export function useActivityStream({
  onJobActive,
  setInspectedActivityId,
}: UseActivityStreamParams): UseActivityStreamResult {
  const [activityJob, setActivityJob] = useState<LlmActivityJob | null>(null);
  const [activityEntries, setActivityEntries] = useState<LlmActivityEntry[]>([]);
  const [activityError, setActivityError] = useState<string | null>(null);
  const [activityViewMode, setActivityViewMode] = useState<ActivityViewMode>("stream");
  const activitySourceRef = useRef<EventSource | null>(null);

  const closeActivityStream = useCallback(() => {
    if (activitySourceRef.current) {
      activitySourceRef.current.close();
      activitySourceRef.current = null;
    }
  }, []);

  // Clean up the SSE connection on unmount
  useEffect(() => () => closeActivityStream(), [closeActivityStream]);

  // Switch to the activity tab only when a job becomes active (or first error).
  // Avoid re-selecting the Activity tab on every streamed entry; that prevents
  // users from navigating away while a run is in progress or after it completes.
  const lastJobIdRef = useRef<string | null>(null);
  const lastErrorRef = useRef<string | null>(null);
  useEffect(() => {
    if (activityJob?.job_id && activityJob.job_id !== lastJobIdRef.current) {
      lastJobIdRef.current = activityJob.job_id;
      onJobActive();
      return;
    }
    if (activityError && activityError !== lastErrorRef.current) {
      lastErrorRef.current = activityError;
      onJobActive();
    }
  }, [activityError, activityJob?.job_id, onJobActive]);

  const appendActivityEntry = useCallback((entry: LlmActivityEntry) => {
    setActivityEntries((prev) => [...prev, entry]);
  }, []);

  const runMockActivityJob = useCallback(
    async <T,>(
      job: LlmActivityJob,
      entries: Array<Omit<LlmActivityEntry, "timestamp_ms">>,
      result: T,
      onComplete: (value: T) => void,
    ) => {
      closeActivityStream();
      setActivityViewMode("stream");
      setInspectedActivityId(null);
      setActivityJob(job);
      setActivityEntries([]);
      setActivityError(null);
      for (const [index, entry] of entries.entries()) {
        await new Promise((resolve) => setTimeout(resolve, index === 0 ? 120 : 220));
        appendActivityEntry({ ...entry, timestamp_ms: Date.now() });
      }
      onComplete(result);
      setActivityJob(null);
    },
    [appendActivityEntry, closeActivityStream, setInspectedActivityId],
  );

  const runStreamingJob = useCallback(
    async <T,>(
      command: string,
      args: Record<string, unknown>,
      onComplete: (value: T) => void,
    ) => {
      closeActivityStream();
      setActivityViewMode("stream");
      setInspectedActivityId(null);
      setActivityEntries([]);
      setActivityError(null);

      const start = await tauriInvoke<AsyncLlmJobStart>(command, args);
      setActivityJob({
        job_id: start.job_id,
        operation: start.operation,
        provider: start.provider,
        model: start.model,
        title: start.title,
      });

      await new Promise<void>((resolve, reject) => {
        const source = new EventSource(start.stream_url);
        activitySourceRef.current = source;

        source.addEventListener("job_started", (event) => {
          try {
            const payload = JSON.parse((event as MessageEvent).data) as {
              title: string; provider: string; model: string; job_id: string; operation: string;
            };
            setActivityJob({
              job_id: payload.job_id,
              operation: payload.operation,
              provider: payload.provider,
              model: payload.model,
              title: payload.title,
            });
          } catch {
            // Ignore malformed status events
          }
        });

        source.addEventListener("activity", (event) => {
          try {
            const payload = JSON.parse((event as MessageEvent).data) as { entry: LlmActivityEntry };
            appendActivityEntry(payload.entry);
          } catch {
            // Ignore malformed activity events
          }
        });

        source.addEventListener("completed", (event) => {
          closeActivityStream();
          try {
            const payload = JSON.parse((event as MessageEvent).data) as { result: T };
            onComplete(payload.result);
            setActivityJob(null);
            resolve();
          } catch (error) {
            const message = error instanceof Error ? error.message : String(error);
            setActivityError(message);
            reject(new Error(message));
          }
        });

        source.addEventListener("failed", (event) => {
          closeActivityStream();
          try {
            const payload = JSON.parse((event as MessageEvent).data) as { error: string };
            setActivityError(payload.error);
            appendActivityEntry({
              source: "diffcore",
              level: "error",
              message: payload.error,
              event_type: "job.failed",
              timestamp_ms: Date.now(),
            });
            setActivityJob(null);
            reject(new Error(payload.error));
          } catch {
            setActivityError("Activity stream failed");
            setActivityJob(null);
            reject(new Error("Activity stream failed"));
          }
        });

        source.onerror = () => {
          closeActivityStream();
          setActivityError("Activity stream disconnected");
          setActivityJob(null);
          reject(new Error("Activity stream disconnected"));
        };
      });
    },
    [appendActivityEntry, closeActivityStream, setInspectedActivityId],
  );

  return {
    activityJob,
    setActivityJob,
    activityEntries,
    setActivityEntries,
    activityError,
    setActivityError,
    activityViewMode,
    setActivityViewMode,
    closeActivityStream,
    runMockActivityJob,
    runStreamingJob,
  };
}
