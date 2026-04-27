import { useEffect, useRef, useState, useCallback, useMemo } from "react";

/**
 * A single selectable option in a `Dropdown`.
 *
 * `label` is what the user sees; `value` is what the consumer receives in
 * `onChange`. `description` renders as a secondary muted line under the label
 * inside the popover (handy for "preview"/"recommended" hints) and is ignored
 * in the closed trigger.
 */
export interface DropdownOption<T extends string> {
  value: T;
  label: string;
  description?: string;
}

interface DropdownProps<T extends string> {
  /** Currently-selected value. Must match one of `options[].value`. */
  value: T;
  /** Available options, rendered in the order provided. */
  options: ReadonlyArray<DropdownOption<T>>;
  /** Fired when the user picks a new option. */
  onChange: (value: T) => void;
  /** Optional `data-testid` forwarded to the trigger button. */
  testId?: string;
  /** Native `title` tooltip on the trigger. */
  title?: string;
  /** Inline width override; defaults to filling the parent. */
  maxWidth?: number | string;
  /** Disables interaction and dims the trigger. */
  disabled?: boolean;
  /**
   * Shown when `value` does not match any option (e.g. uninitialised state),
   * instead of an empty trigger.
   */
  placeholder?: string;
  /**
   * Show a search input above the option list once `options.length` exceeds
   * this threshold. Defaults to 8 — small lists (providers, view modes) stay
   * filter-free, while long ones (OpenRouter models) get a filter box.
   */
  searchThreshold?: number;
  /** Placeholder text inside the search input when shown. */
  searchPlaceholder?: string;
}

/**
 * Custom popover-style dropdown that visually matches the top-bar branch
 * dropdowns (`branch-dropdown-trigger` + `branch-dropdown` list). We use this
 * instead of native `<select>` because native chrome on Linux/Tauri renders
 * with low-contrast text and a chunky platform arrow, which made the model
 * pickers look broken next to the polished branch buttons.
 *
 * Closes on outside click and Escape; opens on click. Generic over the option
 * value type so callers like provider/model pickers stay strongly typed.
 *
 * When the option count exceeds `searchThreshold` (default 8), a fuzzy
 * substring search input is rendered at the top of the popover and auto-
 * focused. Matching is case-insensitive against `label`, `value`, and
 * `description`.
 */
export default function Dropdown<T extends string>({
  value,
  options,
  onChange,
  testId,
  title,
  maxWidth,
  disabled,
  placeholder,
  searchThreshold = 8,
  searchPlaceholder = "Filter…",
}: DropdownProps<T>) {
  const [open, setOpen] = useState(false);
  const [openUpwards, setOpenUpwards] = useState(false);
  const [query, setQuery] = useState("");
  const [activeIndex, setActiveIndex] = useState(0);
  const wrapperRef = useRef<HTMLDivElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const optionRefs = useRef<Array<HTMLLIElement | null>>([]);

  const showSearch = options.length > searchThreshold;

  // Close on outside click. Scoped per-instance so multiple dropdowns on the
  // same screen don't fight each other.
  useEffect(() => {
    if (!open) return;
    function handleClick(event: MouseEvent) {
      const target = event.target as Node | null;
      if (target && wrapperRef.current && !wrapperRef.current.contains(target)) {
        setOpen(false);
      }
    }
    function handleKey(event: KeyboardEvent) {
      if (event.key === "Escape") setOpen(false);
    }
    window.addEventListener("mousedown", handleClick);
    window.addEventListener("keydown", handleKey);
    return () => {
      window.removeEventListener("mousedown", handleClick);
      window.removeEventListener("keydown", handleKey);
    };
  }, [open]);

  const selected = options.find((option) => option.value === value);
  const triggerLabel = selected?.label ?? placeholder ?? String(value);

  const filteredOptions = useMemo(() => {
    const trimmed = query.trim().toLowerCase();
    if (!trimmed) return options;
    return options.filter((option) => {
      const haystack = `${option.label} ${option.value} ${option.description ?? ""}`.toLowerCase();
      return haystack.includes(trimmed);
    });
  }, [options, query]);

  // When the popover opens, point the active highlight at the currently
  // selected value (or the top of the filtered list if nothing matches), and
  // focus the search box if present. Resets the query so the next open shows
  // the full list again.
  useEffect(() => {
    if (open) {
      setQuery("");
      const initialIndex = Math.max(
        0,
        options.findIndex((option) => option.value === value),
      );
      setActiveIndex(initialIndex);
      if (showSearch) {
        requestAnimationFrame(() => searchRef.current?.focus());
      }
    } else {
      setQuery("");
    }
  }, [open, options, value, showSearch]);

  // Keep the active index inside the filtered range whenever the user types,
  // so highlighting doesn't point at a hidden item after filtering.
  useEffect(() => {
    if (!open) return;
    if (activeIndex >= filteredOptions.length) {
      setActiveIndex(filteredOptions.length === 0 ? 0 : filteredOptions.length - 1);
    }
  }, [open, filteredOptions.length, activeIndex]);

  // Scroll the highlighted option into view as the user moves up/down.
  useEffect(() => {
    if (!open) return;
    const node = optionRefs.current[activeIndex];
    node?.scrollIntoView({ block: "nearest" });
  }, [open, activeIndex]);

  // Choose menu direction dynamically so long option lists remain visible near
  // the bottom edge of the viewport (including inside scrollable panes).
  useEffect(() => {
    if (!open) {
      setOpenUpwards(false);
      return;
    }

    const updateMenuDirection = () => {
      const wrapper = wrapperRef.current;
      const menu = menuRef.current;
      if (!wrapper || !menu) return;

      const wrapperRect = wrapper.getBoundingClientRect();
      const menuHeight = menu.getBoundingClientRect().height;
      const spaceBelow = window.innerHeight - wrapperRect.bottom;
      const spaceAbove = wrapperRect.top;
      const shouldOpenUp = menuHeight > spaceBelow && spaceAbove > spaceBelow;
      setOpenUpwards(shouldOpenUp);
    };

    const rafId = requestAnimationFrame(updateMenuDirection);
    window.addEventListener("resize", updateMenuDirection);
    window.addEventListener("scroll", updateMenuDirection, true);

    return () => {
      cancelAnimationFrame(rafId);
      window.removeEventListener("resize", updateMenuDirection);
      window.removeEventListener("scroll", updateMenuDirection, true);
    };
  }, [open, filteredOptions.length, showSearch]);

  const handlePick = useCallback(
    (next: T) => {
      setOpen(false);
      if (next !== value) onChange(next);
    },
    [onChange, value],
  );

  // Shared keyboard handler used by both the search input and the trigger
  // (so users without a filter input still get arrow navigation when the
  // popover is open).
  const handleListKey = useCallback(
    (event: React.KeyboardEvent) => {
      if (!open) return;
      if (event.key === "ArrowDown") {
        event.preventDefault();
        setActiveIndex((prev) =>
          filteredOptions.length === 0 ? 0 : (prev + 1) % filteredOptions.length,
        );
      } else if (event.key === "ArrowUp") {
        event.preventDefault();
        setActiveIndex((prev) =>
          filteredOptions.length === 0
            ? 0
            : (prev - 1 + filteredOptions.length) % filteredOptions.length,
        );
      } else if (event.key === "Home") {
        event.preventDefault();
        setActiveIndex(0);
      } else if (event.key === "End") {
        event.preventDefault();
        setActiveIndex(Math.max(0, filteredOptions.length - 1));
      } else if (event.key === "Enter") {
        if (filteredOptions.length === 0) return;
        event.preventDefault();
        const target = filteredOptions[activeIndex] ?? filteredOptions[0];
        handlePick(target.value);
      }
    },
    [open, filteredOptions, activeIndex, handlePick],
  );

  // Trigger-level keys: open with arrows/Enter/Space, then forward to list
  // navigation while the popover is open.
  const handleTriggerKey = useCallback(
    (event: React.KeyboardEvent<HTMLButtonElement>) => {
      if (!open) {
        if (
          event.key === "ArrowDown" ||
          event.key === "ArrowUp" ||
          event.key === "Enter" ||
          event.key === " "
        ) {
          event.preventDefault();
          if (!disabled) setOpen(true);
        }
        return;
      }
      handleListKey(event);
    },
    [open, disabled, handleListKey],
  );

  return (
    <div
      ref={wrapperRef}
      className="dropdown-wrapper"
      style={maxWidth !== undefined ? { maxWidth } : undefined}
    >
      <button
        type="button"
        className="dropdown-trigger"
        onClick={() => !disabled && setOpen((prev) => !prev)}
        onKeyDown={handleTriggerKey}
        disabled={disabled}
        data-testid={testId}
        title={title}
        aria-haspopup="listbox"
        aria-expanded={open}
      >
        <span className="dropdown-value">{triggerLabel}</span>
        <span className="dropdown-arrow">▾</span>
      </button>
      {open && (
        <div
          ref={menuRef}
          className={`dropdown-menu ${openUpwards ? "dropdown-menu-up" : ""}`}
        >
          {showSearch && (
            <input
              ref={searchRef}
              type="text"
              className="dropdown-search"
              placeholder={searchPlaceholder}
              value={query}
              onChange={(event) => {
                setQuery(event.target.value);
                setActiveIndex(0);
              }}
              onKeyDown={handleListKey}
            />
          )}
          <ul className="dropdown-options" role="listbox">
            {filteredOptions.map((option, index) => (
              <li
                key={option.value}
                ref={(node) => {
                  optionRefs.current[index] = node;
                }}
                role="option"
                aria-selected={option.value === value}
                className={`dropdown-option ${option.value === value ? "selected" : ""} ${
                  index === activeIndex ? "active" : ""
                }`}
                onMouseEnter={() => setActiveIndex(index)}
                onClick={() => handlePick(option.value)}
              >
                <span className="dropdown-option-label">{option.label}</span>
                {option.description && (
                  <span className="dropdown-option-description">{option.description}</span>
                )}
              </li>
            ))}
            {filteredOptions.length === 0 && (
              <li className="dropdown-option disabled">
                {options.length === 0 ? "No options" : "No matches"}
              </li>
            )}
          </ul>
        </div>
      )}
    </div>
  );
}

