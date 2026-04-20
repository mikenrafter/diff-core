# UI Polish — Tauri Desktop App

Focused round of visual / interaction fixes for the Tauri review UI. Each phase
ships independently (one commit per phase) and is testable on its own.

## Goals

- Make the file row layout visually consistent and free of accidental
  emphasis (Phase 2).
- Stop the right pane from overflowing when long model names are selected,
  and consolidate the diff toolbar so replay controls are always visible
  (Phases 3–4).
- Reuse a single `FileDisplay` component everywhere a file/path is rendered,
  including the edges list, with a 1-line and 2-line variant (Phases 5, 7).
- Add a working "hide unchanged" toggle to the SourceExplorer pane so the
  reviewer only sees touched symbols (Phase 6).
- Rebalance the AI Activity panel so the live stream is never crushed by the
  hero / verdict cards, and let users dismiss the "Direct API mode"
  disclaimer (Phase 8).

## Non-goals

- No backend / Rust changes. All work is in `crates/diffcore-tauri/ui/src/`.
- No new analyses or LLM behavior changes.
- No theme / palette overhaul beyond what each phase requires.

## Phases

### Phase 1 — File row visual fixes (already done)

Recorded for completeness; covered in a previous change set. No additional
work in this spec.

### Phase 2 — File row tag spacing & "moved" highlight

**Files:** `App.tsx` file-list rendering, `styles.css` (`.file-status-token`,
`.file-role`, `.file-ext-token`, `.file-moved`).

- Equalize gaps so the leading badges (`[M]`, `[I]`, `[TSX]`) are spaced with
  one consistent value. Today the gap between `[M]` and `[I]` is visibly
  smaller than `[I]` and `[TSX]` because separate margins / paddings are
  applied per badge. Pick one gap value and apply via the parent `.file-item`
  flex container, removing per-badge margins.
- Replace the purple left-border on `.file-moved` rows with a subtle tinted
  background (e.g. `rgba(180, 142, 173, 0.08)` or whatever lines up with the
  `--moved` accent) and remove the border. No layout shift.

**Acceptance:** visual diff against the second screenshot — even badge gaps,
moved row tinted instead of bordered.

### Phase 3 — Right pane overflow & inline diff toolbar (part 1)

**Files:** `App.tsx` (right pane Info tab), `styles.css` (`.dropdown-trigger`,
`.dropdown-value`, `.right-pane`).

- Long model values like `anthropic/claude-haiku-4-5` are pushing the right
  pane wide enough to introduce a scrollbar. Constrain the dropdown trigger:
  - `.dropdown-trigger` width is already `100%`, but child `.dropdown-value`
    needs `min-width: 0` so the ellipsis kicks in inside flex layouts.
  - Audit the surrounding `settings-row` / Info-tab container for any
    `min-width` or unconstrained child that prevents shrink.
- Confirm the right-pane root has `overflow: auto` only on its inner scroll
  container, not on the panel chrome — the value pill should ellipsize, not
  cause a horizontal scrollbar.

**Acceptance:** selecting the longest model name in OpenRouter's list does
not introduce a horizontal scrollbar in the right pane; the trigger
ellipsizes with `…`.

### Phase 4 — Persistent diff toolbar (replay controls always visible)

**Files:** `App.tsx` (center pane diff toolbar + replay UI), `styles.css`
(`.editor-toolbar`, `.replay-toolbar`, possibly new `.diff-toolbar` rules).

- Today the dot-pager / `+ Hunk Comment` / `◀ Hunk` / `Hunk ▶` / `◀ File` /
  `File ▶` / `×` controls only appear when replay is active, and they sit on
  their own bar above the diff. Move them into the same row as the
  `Open With` button at the top of the diff pane so the toolbar is one line.
- Render the full set of controls at all times. When replay is **not**
  active, hunk-nav (`◀ Hunk` / `Hunk ▶`) remain enabled because hunk
  navigation is meaningful in any diff. The replay-only controls
  (dot-pager, file nav, `×` exit) render disabled (`aria-disabled`,
  reduced opacity, no pointer events) so the layout is stable.
- The `▶ Replay Flow` / `✕ Exit Replay` toggle stays in the right pane Info
  tab where it lives today.

**Acceptance:** the diff pane has a single top bar containing both
`Open With` and the hunk/file/dot/× controls. Entering and exiting replay
mode toggles enabled state but does not change which controls are visible
or shift any layout.

### Phase 5 — `FileDisplay` component + edges list reuse

**Files:** new `components/FileDisplay.tsx`, refactor in `App.tsx`
(left pane, Edges tab), `styles.css` (`.file-display-*` rules).

- Extract the existing `[git-status] [role badge] [ext] name : folder
  +adds -dels` row into a single component:

  ```ts
  interface FileDisplayProps {
    path: string;
    gitStatus?: "M" | "A" | "D" | "R" | "U" | "?";
    roleBadge?: string;        // "I", "TSX", "JSON", etc.
    roleLabel?: string;        // full label for 2-line variant ("Infrastructure")
    additions?: number;
    deletions?: number;
    movedFrom?: string;
    reviewedInReplay?: boolean;
    variant?: "one-line" | "two-line";
    onClick?: () => void;
    onContextMenu?: (e: React.MouseEvent) => void;
    suffix?: React.ReactNode; // extra content (e.g. "→ symbolName" for edges)
  }
  ```

- Replace the inline JSX in the left flow-groups list and the infrastructure
  sub-group list with `<FileDisplay variant="two-line" …>`.
- In the Edges list, render each edge as
  `<FileDisplay variant="one-line" path={file} suffix={<>→ {symbolName}</>} />`
  with the `CALLS` (or other edge type) badge in place of `roleBadge`. The
  function-name suffix is unchanged.

**Acceptance:** edges list rows visually align with file rows in the left
pane (same `[ext] name : folder` structure); the file-row JSX in `App.tsx`
is replaced by `<FileDisplay …>`; no behavior regressions.

### Phase 6 — SourceExplorer "hide unchanged" toggle

**Files:** `components/SourceExplorer.tsx`, `styles.css`.

- Add a small toolbar above the Operations / Interfaces & Types / Classes /
  Constants sections with a single toggle: **"Only changed"**. Default off
  (current behavior — show everything in the file).
- A symbol counts as "changed" when **either** of these is true:
  - Its source line range overlaps any line in the file's
    added/removed hunks; **or**
  - Its name is present in the group's `symbols_changed` list for that file.
- When the toggle is on, every section filters down to the matching
  symbols. Section counts in the headers update to reflect the filtered
  count (e.g. `OPERATIONS 12` → `OPERATIONS 2`). Empty sections render an
  italic "No changes in this file." placeholder, matching the existing
  Classes empty state.
- Toggle state lives in component-local `useState`; not persisted — opens
  fresh each navigation, so the user sees the full file by default.

**Acceptance:** toggling "Only changed" on the screenshotted file leaves
only `extractFilePath` and `deduplicateEdges` (the two highlighted in red)
under Operations, and removes everything from Constants except whatever
overlaps a hunk.

### Phase 7 — Two-line `FileDisplay` variant

**Files:** `components/FileDisplay.tsx`, `styles.css`.

- Per the user's direction: variant is **per-context, not adaptive**.
  - Left flow-groups list and infra sub-groups → always
    `variant="two-line"`.
  - Edges list, file tabs, search results, anywhere compact → always
    `variant="one-line"`.
- Two-line layout:
  ```
  [status/✓] [ext] <name> : … <folder right-aligned>
  [moved-from →]? [full role label] … <+adds  -dels right-aligned>
  ```
  - Line 1 is the existing one-line layout.
  - Line 2 is muted (`var(--text-muted)`), tighter font (10–11px), bottom
    aligned with line 1, sharing the same horizontal padding so the two
    rows feel like one row visually.
  - When `additions === 0 && deletions === 0`, omit the `+0 -0` chip on
    line 2.
  - When `movedFrom` is absent and there is no extra metadata to show, line
    2 collapses to just the role label + diff stats.

**Acceptance:** left pane file rows show two visual lines matching the
seventh screenshot; edges and tabs remain one line; `FileDisplay` has a
clean variant switch with no duplicated layout code.

### Phase 8 — Activity panel rebalance & dismissible disclaimer

**Files:** `App.tsx` (Activity tab JSX), `styles.css` (`.activity-*`
rules), local storage helper for the dismissed flag.

- Make both the hero/verdict region and the live-stream region
  independently scrollable by giving them each their own `overflow: auto`
  container with `min-height: 0`. Today the hero region grows to its
  natural height and pushes the live stream off-screen.
- The `Direct API mode` callout becomes a banner inside the events readout
  (not its own card), and only renders when the active provider is a
  direct-API provider (OpenAI / Anthropic / Gemini). It gets a `×` dismiss
  button that persists the dismissal in `localStorage` keyed by provider
  (`diffcore.directApiNotice.dismissed.<provider>`).
- When the direct-API banner is showing or the active provider is direct-
  API, hide the SEARCH / READS / COMMANDS stat tiles entirely (they are
  always 0 in that mode). Keep only the EVENTS tile.
- Layout: hero card + verdict card share one scroll region; events readout
  + live stream share the other. Use a CSS grid with two flex children and
  a small visible divider so users can see where to drag focus.

**Acceptance:** with a long verdict and many events, both regions scroll
independently and neither steals all the height. The disclaimer can be
dismissed and stays dismissed for that provider across reloads.

## Testing strategy per phase

- After each phase: `npx tsc --noEmit` clean, plus a manual visual check
  in the running Tauri app.
- After Phase 5: spot-check that the existing E2E tests under
  `crates/diffcore-tauri/ui/tests/e2e/` still find their target rows
  (most use `data-testid`s, which `FileDisplay` will preserve).
- After Phase 8: verify dismissal persists across a full app reload.

## Commit cadence

One commit per phase, prefixed `ui:` and titled with the phase name, e.g.
`ui: phase 2 — equalize file-row badge gaps and tint "moved" rows`.
