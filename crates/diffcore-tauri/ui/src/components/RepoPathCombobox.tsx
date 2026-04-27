import { useEffect, useMemo, useRef, useState } from "react";
import { useDebouncedValue } from "../hooks/useDebouncedValue";
import { tauriInvoke } from "../utils/tauriUtils";
import { shortPath } from "../utils/pathUtils";

type RepoPathComboboxProps = {
  inputRef?: React.RefObject<HTMLInputElement | null>;
  value: string;
  favorites: string[];
  recents: string[];
  disabled?: boolean;
  onCommit: (path: string) => void;
  onToggleFavorite: (path: string) => void;
  onBrowse: () => void;
  onAnalyze: (path: string) => void;
  /** Control whether Browse/Pin actions render inside the combobox row. */
  actions?: "inline" | "none";
  /** Optional max width for the trigger button. */
  triggerMaxWidth?: number | string;
};

type RepoPathOption = {
  key: string;
  path: string;
  kind: "favorite" | "recent" | "suggestion";
};

export function RepoPathCombobox({
  inputRef,
  value,
  favorites,
  recents,
  disabled,
  onCommit,
  onToggleFavorite,
  onBrowse,
  onAnalyze,
  actions = "inline",
  triggerMaxWidth = 400,
}: RepoPathComboboxProps) {
  const wrapperRef = useRef<HTMLDivElement | null>(null);
  const localInputRef = useRef<HTMLInputElement | null>(null);
  const [open, setOpen] = useState(false);
  const [draft, setDraft] = useState(value);
  const [activeIndex, setActiveIndex] = useState(0);
  const [backendSuggestions, setBackendSuggestions] = useState<string[]>([]);

  // Keep draft in sync while closed; while open, draft is user-owned.
  useEffect(() => {
    if (!open) setDraft(value);
  }, [value, open]);

  // Close on outside click / Escape.
  useEffect(() => {
    if (!open) return;
    function onMouseDown(e: MouseEvent) {
      const t = e.target as Node | null;
      if (t && wrapperRef.current && !wrapperRef.current.contains(t)) {
        setOpen(false);
        commit(draft);
      }
    }
    function onKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") {
        setOpen(false);
        // Revert uncommitted edits on Escape.
        setDraft(value);
      }
    }
    window.addEventListener("mousedown", onMouseDown);
    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("mousedown", onMouseDown);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [open, draft, value]);

  const debouncedDraft = useDebouncedValue(draft, 200);

  useEffect(() => {
    const q = debouncedDraft.trim();
    if (!open || q.length < 2) {
      setBackendSuggestions([]);
      return;
    }

    let cancelled = false;
    void (async () => {
      try {
        const results = await tauriInvoke<string[]>("list_repo_path_suggestions", { query: q });
        if (!cancelled) setBackendSuggestions(results ?? []);
      } catch {
        if (!cancelled) setBackendSuggestions([]);
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [debouncedDraft, open]);

  const allOptions = useMemo<RepoPathOption[]>(() => {
    const seen = new Set<string>();
    const out: RepoPathOption[] = [];

    for (const path of favorites) {
      if (!path) continue;
      if (seen.has(path)) continue;
      seen.add(path);
      out.push({ key: `fav:${path}`, path, kind: "favorite" });
    }
    for (const path of recents) {
      if (!path) continue;
      if (seen.has(path)) continue;
      seen.add(path);
      out.push({ key: `recent:${path}`, path, kind: "recent" });
    }
    for (const path of backendSuggestions) {
      if (!path) continue;
      if (seen.has(path)) continue;
      seen.add(path);
      out.push({ key: `sugg:${path}`, path, kind: "suggestion" });
    }
    return out;
  }, [favorites, recents, backendSuggestions]);

  // Favorites should always display, even while typing; other options are filtered.
  const filteredOptions = useMemo(() => {
    const q = draft.trim().toLowerCase();
    const favs = allOptions.filter((opt) => opt.kind === "favorite");
    const others = allOptions.filter((opt) => opt.kind !== "favorite");
    if (!q) return [...favs, ...others];
    const filteredOthers = others.filter((opt) => opt.path.toLowerCase().includes(q));
    return [...favs, ...filteredOthers];
  }, [allOptions, draft]);

  useEffect(() => {
    if (!open) return;
    const initial = Math.max(0, filteredOptions.findIndex((o) => o.path === value));
    setActiveIndex(initial === -1 ? 0 : initial);
  }, [open, filteredOptions, value]);

  const commit = (raw: string) => {
    const next = raw.trim();
    onCommit(next);
  };

  const pickAt = (index: number) => {
    const opt = filteredOptions[index] ?? filteredOptions[0];
    if (!opt) return;
    setOpen(false);
    setDraft(opt.path);
    commit(opt.path);
  };

  return (
    <div ref={wrapperRef} className="repo-combobox">
      <button
        type="button"
        className="btn branch-dropdown-trigger repo-combobox-trigger"
        style={{ flex: 1, maxWidth: triggerMaxWidth, justifyContent: "space-between" }}
        onClick={() => {
          if (disabled) return;
          setOpen((v) => {
            const next = !v;
            if (next) setDraft(value);
            return next;
          });
        }}
        onKeyDown={(e) => {
          if (disabled) return;
          if (!open) {
            if (e.key === "ArrowDown" || e.key === "ArrowUp" || e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              setDraft(value);
              setOpen(true);
            }
            return;
          }

          if (e.key === "ArrowDown") {
            e.preventDefault();
            if (filteredOptions.length === 0) return;
            setActiveIndex((i) => (i + 1) % filteredOptions.length);
          } else if (e.key === "ArrowUp") {
            e.preventDefault();
            if (filteredOptions.length === 0) return;
            setActiveIndex((i) => (i - 1 + filteredOptions.length) % filteredOptions.length);
          } else if (e.key === "Enter") {
            e.preventDefault();
            if (filteredOptions.length > 0) {
              pickAt(activeIndex);
            } else {
              commit(draft);
              setOpen(false);
            }
          } else if (e.key === "Tab") {
            setOpen(false);
            commit(draft);
          }
        }}
        disabled={disabled}
        title={value ? value : "Select repository"}
        aria-haspopup="listbox"
        aria-expanded={open}
      >
        <span className="branch-name" style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
          {value ? shortPath(value) : "Select repository…"}
        </span>
        <span className="dropdown-arrow">&#9662;</span>
      </button>

      {actions === "inline" && (
        <>
          <button className="btn" onClick={onBrowse} disabled={disabled} title="Browse for a repository folder">
            Browse
          </button>
          <button
            className="btn"
            onClick={() => {
              const path = draft.trim();
              if (!path) return;
              onToggleFavorite(path);
            }}
            disabled={disabled || !draft.trim()}
            title="Pin or unpin current repository"
          >
            {favorites.includes(draft.trim()) ? "Unpin" : "Pin"}
          </button>
        </>
      )}

      {open && (
        <div className="branch-dropdown repo-combobox-menu" style={{ width: "100%", paddingTop: 6 }}>
          <div style={{ padding: "0 8px 8px 8px" }}>
            <input
              ref={(node) => {
                localInputRef.current = node;
                if (inputRef) inputRef.current = node;
              }}
              className="input"
              type="text"
              placeholder="Type to search or paste a repo path…"
              value={draft}
              onChange={(e) => {
                setDraft(e.target.value);
                setActiveIndex(0);
              }}
              onKeyDown={(e) => {
                if (e.key === "ArrowDown") {
                  e.preventDefault();
                  if (filteredOptions.length === 0) return;
                  setActiveIndex((i) => (i + 1) % filteredOptions.length);
                } else if (e.key === "ArrowUp") {
                  e.preventDefault();
                  if (filteredOptions.length === 0) return;
                  setActiveIndex((i) => (i - 1 + filteredOptions.length) % filteredOptions.length);
                } else if (e.key === "Enter") {
                  e.preventDefault();
                  if (filteredOptions.length > 0) {
                    pickAt(activeIndex);
                    return;
                  }
                  commit(draft);
                  if (draft.trim()) onAnalyze(draft.trim());
                  setOpen(false);
                } else if (e.key === "Escape") {
                  e.preventDefault();
                  setOpen(false);
                  setDraft(value);
                }
              }}
              onBlur={() => {
                // If focus moved fully outside the combobox, close and commit.
                window.setTimeout(() => {
                  if (!wrapperRef.current?.contains(document.activeElement)) {
                    setOpen(false);
                    commit(draft);
                  }
                }, 0);
              }}
              style={{ width: "100%" }}
              autoFocus
            />
          </div>
          <div style={{ maxHeight: 220, overflowY: "auto" }}>
            {filteredOptions.length === 0 ? (
              <li className="branch-option disabled">No matches</li>
            ) : (
              filteredOptions.map((opt, idx) => {
                const prefix = opt.kind === "favorite" ? "★ " : opt.kind === "recent" ? "" : "↳ ";
                const isActive = idx === activeIndex;
                return (
                  <li
                    key={opt.key}
                    className={`branch-option ${isActive ? "selected" : ""}`}
                    onMouseEnter={() => setActiveIndex(idx)}
                    onMouseDown={(e) => e.preventDefault()}
                    onClick={() => pickAt(idx)}
                    title={opt.path}
                  >
                    <span className="branch-option-name">{prefix}{shortPath(opt.path)}</span>
                  </li>
                );
              })
            )}
          </div>
        </div>
      )}
    </div>
  );
}

