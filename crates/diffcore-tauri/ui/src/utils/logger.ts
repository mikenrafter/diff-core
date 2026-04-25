export type LogLevel = "debug" | "info" | "warn" | "error";

export type LogEntry = {
  id: string;
  ts: number;
  level: LogLevel;
  category: string;
  event: string;
  message?: string;
  duration_ms?: number;
  data?: unknown;
};

const STORAGE_KEY_ENABLED = "diffcore.debugLogs.enabled";
const STORAGE_KEY_LEVEL = "diffcore.debugLogs.level";
const STORAGE_KEY_SLOW_MS = "diffcore.debugLogs.slowMs";

type Listener = () => void;

function nowMs(): number {
  return (typeof performance !== "undefined" && performance.now)
    ? performance.now()
    : Date.now();
}

function wallTs(): number {
  return Date.now();
}

function safeJsonSize(x: unknown): number {
  try {
    return JSON.stringify(x)?.length ?? 0;
  } catch {
    return -1;
  }
}

function readBool(key: string, fallback: boolean): boolean {
  try {
    const v = window.localStorage.getItem(key);
    if (v == null) return fallback;
    return v === "1" || v === "true";
  } catch {
    return fallback;
  }
}

function readString(key: string, fallback: string): string {
  try {
    return window.localStorage.getItem(key) ?? fallback;
  } catch {
    return fallback;
  }
}

function readNumber(key: string, fallback: number): number {
  try {
    const v = window.localStorage.getItem(key);
    if (!v) return fallback;
    const n = Number(v);
    return Number.isFinite(n) ? n : fallback;
  } catch {
    return fallback;
  }
}

function write(key: string, value: string) {
  try {
    window.localStorage.setItem(key, value);
  } catch {
    // ignore storage failures
  }
}

function shouldInclude(level: LogLevel, minLevel: LogLevel): boolean {
  const order: Record<LogLevel, number> = { debug: 0, info: 1, warn: 2, error: 3 };
  return order[level] >= order[minLevel];
}

export type LoggerSnapshot = Readonly<{
  enabled: boolean;
  minLevel: LogLevel;
  slowMs: number;
  entries: LogEntry[];
  maxEntries: number;
}>;

class LoggerStore {
  private listeners = new Set<Listener>();
  private entries: LogEntry[] = [];
  private maxEntries = 800;
  private enabled = readBool(STORAGE_KEY_ENABLED, false);
  private minLevel = (readString(STORAGE_KEY_LEVEL, "info") as LogLevel) ?? "info";
  private slowMs = readNumber(STORAGE_KEY_SLOW_MS, 120);
  private snapshot: LoggerSnapshot | null = null;

  subscribe(listener: Listener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  private invalidate() {
    this.snapshot = null;
    for (const l of this.listeners) l();
  }

  getState(): LoggerSnapshot {
    if (!this.snapshot) {
      this.snapshot = Object.freeze({
        enabled: this.enabled,
        minLevel: this.minLevel,
        slowMs: this.slowMs,
        entries: this.entries,
        maxEntries: this.maxEntries,
      });
    }
    return this.snapshot;
  }

  setEnabled(enabled: boolean) {
    this.enabled = enabled;
    write(STORAGE_KEY_ENABLED, enabled ? "1" : "0");
    this.invalidate();
  }

  setMinLevel(level: LogLevel) {
    this.minLevel = level;
    write(STORAGE_KEY_LEVEL, level);
    this.invalidate();
  }

  setSlowMs(ms: number) {
    this.slowMs = Math.max(0, Math.floor(ms));
    write(STORAGE_KEY_SLOW_MS, String(this.slowMs));
    this.invalidate();
  }

  clear() {
    this.entries = [];
    this.invalidate();
  }

  log(entry: Omit<LogEntry, "id" | "ts">) {
    if (!this.enabled) return;
    if (!shouldInclude(entry.level, this.minLevel)) return;
    const id = `${wallTs()}_${Math.random().toString(16).slice(2)}`;
    const full: LogEntry = { id, ts: wallTs(), ...entry };
    this.entries = [full, ...this.entries].slice(0, this.maxEntries);
    this.invalidate();
  }

  time<T>(opts: {
    category: string;
    event: string;
    level?: LogLevel;
    message?: string;
    data?: unknown;
    slowOverrideMs?: number;
    fn: () => Promise<T>;
  }): Promise<T> {
    const start = nowMs();
    return opts.fn().then(
      (res) => {
        const duration_ms = Math.max(0, nowMs() - start);
        const slowThreshold = opts.slowOverrideMs ?? this.slowMs;
        this.log({
          level: (opts.level ?? (duration_ms >= slowThreshold ? "warn" : "info")),
          category: opts.category,
          event: opts.event,
          message: opts.message,
          duration_ms,
          data: opts.data,
        });
        return res;
      },
      (err) => {
        const duration_ms = Math.max(0, nowMs() - start);
        this.log({
          level: "error",
          category: opts.category,
          event: `${opts.event}:error`,
          message: String(err),
          duration_ms,
          data: opts.data,
        });
        throw err;
      },
    );
  }

  summarizeArgs(args: unknown): { arg_bytes: number } {
    return { arg_bytes: safeJsonSize(args) };
  }
}

export const logger = new LoggerStore();

