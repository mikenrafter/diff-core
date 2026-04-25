## Navigation Revamp — Repo Path + Pane Layout

**Status:** Draft (plan-first; implementation follows)
**Last updated:** 2026-04-25

### Goals

- **Repo path selection is first-class**
  - Single control: dropdown + text box (combobox).
  - Favorites (pinned) appear as normal selections.
  - Freeform typed paths resolve.
  - Typeahead suggestions with **backend debounce (~200ms)**; **UI filtering immediate**.
  - Browse button works.

- **Reduce mouse travel**
  - Move to layout: **Diff viewer on the left**, **two panes on the right**.
  - The **rightmost pane** holds Flow Groups and a **Settings tab**.
  - Both right panes are **resizable**.

- **More compare options**
  - Add compare pairing: **Staged → Committed (selected recent commit)**.

- **Flow groups behave predictably**
  - Collapsible under all conditions.
  - Clicking the group header toggles expansion.
  - Very large unrefined groups are constrained: **max height 100vh + internal scroll**.
  - Defaults: **first flow group expanded**, and **Ungrouped expanded by default** (handled separately).

### Non-goals

- No new analysis semantics in diffcore-core beyond what’s required to support staged→commit diff extraction.
- No reorganization of the inner-right pane tabs (keep as-is for now).
- No global filesystem search for repo-path suggestions.

### Current implementation touchpoints

- **Repo input & controls**: `crates/diffcore-tauri/ui/src/components/panels/HeaderBar.tsx`
  - Current: plain `<input>` + separate “Recent” menu + Pin button.
- **Browse**: `crates/diffcore-tauri/ui/src/App.tsx` uses `@tauri-apps/plugin-dialog`.
  - Likely broken today due to missing dialog permission in `crates/diffcore-tauri/capabilities/default.json`.
- **Layout**: `crates/diffcore-tauri/ui/src/App.tsx` renders `LeftPane | CenterPane | RightPane`.
- **Flow groups list**: `crates/diffcore-tauri/ui/src/components/panels/LeftPane.tsx`
  - Expansion is effectively tied to selection (`selectedGroup`).
- **Dropdown base component**: `crates/diffcore-tauri/ui/src/components/Dropdown.tsx`
  - Already supports immediate client-side filtering and keyboard navigation.

### Design

#### 1) Repo path combobox (dropdown + text box)

**Behavior**

- The repo selector is a **combobox** with:
  - **Text input** (draft value).
  - **Dropdown list** showing:
    - Favorites (pinned)
    - Recents
    - Typeahead suggestions (filesystem lookahead)
- **Selecting** an option commits `repoPath` immediately.
- **Typing freeform**:
  - Resolves on blur/Enter by committing trimmed text to `repoPath`.
  - We accept any directory path and rely on existing backend behavior (`git2::Repository::discover`) to decide if it’s a repo.
  - On invalid paths, keep the draft visible and show a non-blocking error (and keep focus affordances consistent with current behavior).

**Typeahead**

- UI behavior:
  - Filtering of the currently-visible dropdown options is **immediate**.
  - Backend suggestion requests are throttled by debouncing ~200ms on the UI side.
- Backend behavior (suggestion generation constraints):
  - Search **forward from the current working directory (PWD) only**.
  - Breadth-first traversal.
  - **Max depth: 2**
  - **Max folders visited: 15**
  - Candidate ranking is fuzzy and query-pertinent (must include query substring; then score by closeness/position).
  - Example: input `/home/tester/source/repos/pro` yields candidates like `.../projectA`, `.../projectB`, `.../example/professor`, `.../example/rocket-propellant` within traversal limits.

**Browse**

- Browse button opens a directory picker via Tauri dialog plugin.
- Ensure Tauri v2 capabilities include the required dialog permission.

#### 2) Pane layout: diff left, two right panes

Target structure:

```mermaid
flowchart LR
  DiffPane[DiffViewerPane_left] --> InnerRight[InspectorPane_right]
  InnerRight --> Rightmost[GroupsAndSettingsPane_rightmost]
```

- **Left**: diff viewer (`CenterPane` content).
- **Inner-right**: existing `RightPane` tabs unchanged (activity/comments/annotations/source).
- **Rightmost**: new pane with tabs:
  - Flow Groups
  - Settings
- Resizing:
  - Handle A: between left diff and inner-right.
  - Handle B: between inner-right and rightmost.
  - Persist widths in `localStorage`.

#### 3) Settings tab (no overlay)

- Move current `SettingsPanel` content into the rightmost pane “Settings” tab.
- Keep **`AISetupModal`** as the only overlay workflow.

#### 4) Compare mode: Staged → Committed (recent commit)

- Add a compare pairing where:
  - Source: **Staged changes**
  - Target: **a selected recent commit**
- UX constraints:
  - Commit target selection is **recent commits only** (no raw SHA entry for now).
  - Do not prefetch commits for arbitrary paths yet; rely on the existing “after repoPath resolves + analysis runs” flow to populate recent commits.
- Backend semantics:
  - Diff should represent index vs chosen commit (equivalent to `git diff <sha> --cached`).
  - Both analysis (`analyze`) and file diff fetch (`get_file_diff`) must use identical semantics for correctness.

#### 5) Flow group collapse & huge lists

- Flow group expansion is driven by explicit expansion state keyed by group id.
- Clicking a flow group header toggles expansion under all conditions.
- Selection and expansion are independent (selection does not force expansion).
- Defaults after an analysis completes:
  - First flow group expanded (preserve today’s “first group visible” affordance).
  - Ungrouped expanded by default (separate accordion behavior).
- Huge lists:
  - Flow group pane body uses **max height 100vh** and an internal scrollbar to prevent runaway page scrolling.

### Acceptance criteria (first pass)

- Repo path picker:
  - Browse works on all platforms.
  - Favorites appear in the dropdown and are selectable like any other option.
  - Freeform typing can set repoPath and successfully triggers repo info loading + analysis.
  - Typeahead suggestions respect BFS constraints (depth 2, 15 folders max) and feel responsive (backend calls debounced; UI filtering immediate).
- Layout:
  - Diff viewer is on the left.
  - Two right panes exist, both resizable, and sizes persist across reload.
  - Rightmost pane contains Flow Groups + Settings tabs.
- Compare mode:
  - New pairing Staged→Committed works and is selectable (commit chosen from recent list).
- Flow groups:
  - Group headers toggle expansion reliably.
  - Ungrouped is expanded by default.
  - Massive unrefined lists scroll within 100vh without breaking overall layout.

