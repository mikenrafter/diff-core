<!-- 67269847-7a4e-433c-8212-55692a61bf92 -->
---
todos:
  - id: "spec-draft"
    content: "Draft/update a spec covering repo path combobox, pane layout, settings tab, flow collapse/scroll, and staged→committed compare mode."
    status: pending
  - id: "repo-path-combobox-design"
    content: "Design repo path combobox data model (favorites+recents+typeahead), backend suggestion command shape, and debounce responsibilities (UI vs backend)."
    status: pending
  - id: "pane-layout-design"
    content: "Design new left+two-right layout with two resize handles and tab placement."
    status: pending
  - id: "compare-mode-design"
    content: "Design staged→commit diff semantics, UI selection UX for commit target, and required backend diff extraction."
    status: pending
  - id: "flow-collapse-design"
    content: "Specify collapse state rules, defaults (Ungrouped expanded), and scroll/max-height behavior for huge groups."
    status: pending
isProject: false
---
## Scope
- Fix repo path selection UX (browse, pin/favorites, dropdown+textbox, typeahead).
- Re-layout the review UI to: **Diff viewer on the left**, **two resizable panes on the right** (inner-right: existing tabs like activity/comments/annotations/source; rightmost: flow groups + settings tab).
- Add diff pairing: **Staged → Committed (selected commit SHA)**.
- Make flow groups **collapsible independent of selection**, header click toggles expansion, always works.
- Cap huge unrefined group lists: **max height 100vh + scrollbar**; keep **Ungrouped expanded by default**.
- Update specs first; implementation comes after plan acceptance.

## What exists today (key findings)
- Repo path is a plain `<input>` in `crates/diffcore-tauri/ui/src/components/panels/HeaderBar.tsx` with local debounce (~450ms) before committing to `repoPath`.
- Browse uses `@tauri-apps/plugin-dialog` in `crates/diffcore-tauri/ui/src/App.tsx` (`browseForRepository`). If it’s non-functional in-app, the likely cause is missing Tauri permissions: `crates/diffcore-tauri/capabilities/default.json` currently does **not** include a dialog permission.
- Favorites (“Pin”) and Recents are stored in `localStorage` (`diffcore.favoriteRepos`, `diffcore.recentRepos`) and shown via a separate “Recent” quick-pick popover in `HeaderBar.tsx`.
- `Dropdown.tsx` already supports **inline filtering** (immediate) for long option lists and keyboard navigation; it is not currently a “creatable” combobox.
- Flow group expansion is effectively “selected group shows its files” in `crates/diffcore-tauri/ui/src/components/panels/RightmostPane.tsx` (so collapse/expand isn’t independent or persistent).
- Layout is currently 3 columns (`RightmostPane` groups, `CenterPane` diff, `RightPane` tabs) in `crates/diffcore-tauri/ui/src/App.tsx`.
- Compare modes currently supported in UI state are `branch` and `unstaged_to_staged` in `App.tsx` (`CompareMode`).

## Spec updates (before code)
- Update/add a UI spec describing:
  - Repo path combobox behavior and data model (favorites + recents + typeahead results).
  - Pane layout + resize behavior + new Settings tab placement.
  - Flow group collapse semantics (independent expansion state, defaults).
  - New compare mode: staged→selected commit.

## Implementation plan (after spec approval)
### A) Repo path selection revamp
- Replace repo path `<input>` + separate “Recent” popover with a **single combobox** control:
  - A dropdown-style closed state (like the model selection dropdowns): shows the current repo (or a placeholder) and a caret.
  - When opened, the dropdown contains a searchable text field (typeahead) plus the option list.
  - A dropdown of options containing:
    - Favorites (pinned) entries (same rendering/behavior as other selectable entries). **Favorites are always displayed**, even when the user types a query (i.e. they are not filtered away).
    - Recent entries.
    - Typeahead suggestions.
  - Selecting an option commits `repoPath` immediately.
  - Freeform typing resolves on blur/Enter:
    - Accept any existing directory path and let `git2::Repository::discover` determine repo validity (same semantics as `get_repo_info` / `analyze` today).
    - If invalid/unreadable, preserve the draft and surface a non-blocking validation error.
- Build this by extending `crates/diffcore-tauri/ui/src/components/Dropdown.tsx` (or extracting shared internals) into a new component (e.g. `RepoPathCombobox`) that supports:
  - Controlled `inputValue` + controlled `selectedValue`.
  - “Creatable” commit (string not in options).
  - Immediate client-side filtering of currently-loaded options.
- Backend-debounced typeahead:
  - Add a new Tauri command in `crates/diffcore-tauri/src/commands/workspace.rs` (and export via `commands/mod.rs`) to return repo-path suggestions.
  - Debounce **backend** calls by ~200ms in UI (so backend work is throttled) while keeping the **UI filter immediate** for already-fetched results.
  - Suggested algorithm:
    - Never search global home.
    - Suggestions must be **real existing directories** (never synthesize “prefix” paths that don’t exist).
    - If the query looks like a path (contains `/` or `\`), treat it as **path completion**:
      - Find the nearest existing parent directory and list its direct child directories whose names start with the final fragment.
      - This should avoid junk suggestions like `.../abcd`, `.../abcde` when only `.../abcdef` exists.
    - Otherwise, search forward from the current working directory (PWD) only, breadth-first, **max depth 2**, **max 15 folders searched**.
    - When scanning, if a directory contains a `.git` subdirectory, treat it as a repository candidate **and do not descend into it** (don’t retrieve its subfolders).
    - Rank candidates by fuzzy pertinence (substring match against the full path; then score by closeness/position).
    - Example: query `/home/tester/source/repos/pro` can suggest sibling/descendant dirs within lookahead: `.../projectA`, `.../projectB`, `.../example/professor`, `.../example/rocket-propellant`.
- Fix Browse:
  - Add required dialog permission in `crates/diffcore-tauri/capabilities/default.json` for Tauri v2 plugin dialog.

### B) Pane layout restructure (diff left, two right panes)
- Update `App.tsx` layout from `RightmostPane | CenterPane | RightPane` to:
  - Left: `CenterPane` (diff viewer).
  - Right split: two panes:
    - Inner-right: existing `RightPane` tabbed inspector (activity/comments/annotations/source).
    - Rightmost: new `GroupsPane` containing Flow Groups + Settings as tabs.
- Add two independent resize handles:
  - One between left diff pane and right split.
  - One between inner-right and rightmost.
  - Persist widths in `localStorage`.

### C) Settings moves from overlay to a tab
- Replace `SettingsPanel` overlay usage with a “Settings” tab inside the **rightmost** pane.
- Keep `AISetupModal` as an overlay workflow (unchanged).
- Keep keyboard accessibility (Esc to close dropdowns, focus management) consistent.

### D) Compare mode: Staged → Committed (selected commit)
- Extend compare-mode state machine in `App.tsx`:
  - Add a new `CompareMode` variant representing “staged_to_commit”.
  - Extend the “source/target” selectors to allow selecting:
    - Source: Staged changes
    - Target: a commit SHA chosen from **recent commits only** (no raw SHA entry for now), and only after analysis populates recent commits (don’t prefetch commits for arbitrary paths yet).
- Add backend support in `crates/diffcore-tauri/src/commands/mod.rs` / `diffcore-core` git layer if missing:
  - Compute diff of index vs selected commit (likely `git diff <sha> --cached` equivalent).
  - Ensure file-diff fetching (`get_file_diff`) and analysis (`analyze`) use the same diff source args.

### E) Flow group collapse + massive list handling
- Introduce explicit expansion state keyed by group id (and infra/ungrouped) in UI state:
  - Clicking a group header toggles expanded/collapsed.
  - Selection highlights a group but does not force expansion.
  - Works in refined and unrefined views.
  - Defaults after analysis:
    - Preserve existing behavior: **first flow group expanded**.
    - “Ungrouped” is handled separately and should be **expanded by default** to aid discovery.
- Cap list heights:
  - For the flow group pane(s), ensure max height is 100vh and overflow scrolls (no page scroll coupling).
  - Apply especially when unrefined groups are massive.

## Files likely to change (implementation phase)
- UI:
  - `crates/diffcore-tauri/ui/src/components/panels/HeaderBar.tsx`
  - `crates/diffcore-tauri/ui/src/components/Dropdown.tsx` (extend/extract for combobox)
  - `crates/diffcore-tauri/ui/src/App.tsx` (layout, state, compare modes, debounces)
  - `crates/diffcore-tauri/ui/src/components/panels/RightmostPane.tsx` (move/replace groups UI into rightmost pane; implement collapsible groups)
  - `crates/diffcore-tauri/ui/src/components/panels/RightPane.tsx` (may become inner-right pane)
  - `crates/diffcore-tauri/ui/src/components/modals/SettingsPanel.tsx` (convert to in-pane tab component)
  - `crates/diffcore-tauri/ui/src/styles.css` (new split layout + resize handles + scroll constraints)
- Backend:
  - `crates/diffcore-tauri/src/commands/workspace.rs` (repo path suggestions command)
  - `crates/diffcore-tauri/src/commands/mod.rs` (diff extraction support for staged→commit if needed)
  - `crates/diffcore-tauri/capabilities/default.json` (dialog permission)

## Follow-up questions (asked after spec draft is written)
- Repo path combobox: confirm BFS forward search constraints (depth=2, maxFolders=15) are correct for all OSes we support.
- Pane layout: confirm inner-right tabs remain as-is for now.
- Settings: confirm only `AISetupModal` remains an overlay; settings content moves into rightmost Settings tab.
- Staged→Committed: confirm target commit picker is recent commits only for now.
- Collapse defaults: confirm preserve first flow group expanded, plus Ungrouped expanded by default.