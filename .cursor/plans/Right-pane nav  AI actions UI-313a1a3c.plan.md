<!-- 313a1a3c-77b8-4663-8f52-9fc46e0e8d56 -->
---
todos:
  - id: "reviewed-hunks-display"
    content: "Add reviewed/total hunk rendering to `FileDisplay` and pass reviewed counts from `App.tsx` → `RightmostPane`."
    status: pending
  - id: "move-nav-controls"
    content: "Remove replay step tracker and +hunk-comment from `CenterPane`; add hunk/file nav toolbar to `GroupsAndSettingsPane` header with file nav working outside replay."
    status: pending
  - id: "fix-activity-tab-lock"
    content: "Adjust `useActivityStream` so Activity tab is selected only on job start (not on every entry)."
    status: pending
  - id: "info-tab-restructure"
    content: "Move repo navigation controls into `AnnotationsTab` Info subtab; restructure AI actions into 2x2 grid with 4 blocks and feedback rerun flow."
    status: pending
  - id: "ai-overlay-tab"
    content: "Add a new AI overlay tab to `RightPane` with provider-type dropdown and CLI-vs-API disclaimer; remove header gear and iconize Settings tab."
    status: pending
  - id: "scrollbar-sticky-banners"
    content: "Fix rightmost pane double-scrollbar and make top banners (Export & Watch / refinement) sticky via CSS + layout tweaks."
    status: pending
  - id: "backend-feedback-hooks"
    content: "Extend Rust/Tauri commands (and core schema if needed) to support feedback reruns for summary, flow analysis, and refinement to match the new UI controls."
    status: pending
isProject: false
---
## Goal
Implement the 9 UI changes requested:
- File rows show **hunks reviewed/total** as `(reviewed/total)` with reviewed in white and total in gray.
- Move **hunk/file navigation** out of the top diff toolbar into the **rightmost pane header** (`GroupsAndSettingsPane`). Layout: `(<hunk) (hunk>) (<file) (file>)`. File buttons must be enabled even when replay is inactive.
- Remove the **step tracker** entirely.
- Move/redo the **regenerate + model selectors** into a 2x2 grid in the **Info** subtab of `AnnotationsTab` with 4 action blocks (one disabled).
- Fix **Activity tab repeatedly forcing navigation**.
- Move **repo navigation controls** into the **Info** subtab.
- Remove the **settings gear** from the top header; make the **Settings tab use an icon** instead of the text label.
- Move the **AI overlay** into the inner-right inspector (`RightPane`) as its own tab; provider-type dropdown should include CLI + API options; disclaimer should be generalized to CLI-vs-API.
- Fix the **double-scrollbar** in the rightmost pane and make the top banners (e.g. “Edit groups via CLI / Export & Watch”) **sticky**.

## Key findings (current code locations)
- File hunk display is in `crates/diffcore-tauri/ui/src/components/FileDisplay.tsx` (`file-display-hunks` currently renders `({hunks})`).
- Hunk/file nav + replay step tracker is in `crates/diffcore-tauri/ui/src/components/panels/CenterPane.tsx` (see the block rendering `Step {replayStep + 1}...`, the progress dots `.replay-progress`, and the `+ Hunk Comment` button).
- Rightmost pane tabs are in `crates/diffcore-tauri/ui/src/components/panels/GroupsAndSettingsPane.tsx`.
- Repo navigation UI is in `crates/diffcore-tauri/ui/src/components/panels/HeaderBar.tsx` today.
- “Regenerate with feedback/question” is currently in `crates/diffcore-tauri/ui/src/components/tabs/AnnotationsTab.tsx`.
- Activity-tab lock bug is caused by `crates/diffcore-tauri/ui/src/hooks/useActivityStream.ts`:

```71:76:crates/diffcore-tauri/ui/src/hooks/useActivityStream.ts
  // Switch to the activity tab whenever a job fires or entries arrive
  useEffect(() => {
    if (activityJob || activityEntries.length > 0 || activityError) {
      onJobActive();
    }
  }, [activityEntries.length, activityError, activityJob, onJobActive]);
```

## Plan (implementation steps)

### 1) Hunk count: show reviewed/total in `FileDisplay`
- Update `FileDisplay.tsx` props to accept `reviewedHunks?: number` (or `reviewedHunksInReplay?: number`).
- Render `(reviewed/total)` when both reviewed+total exist; keep `(total)` fallback when reviewed not provided.
- Add CSS tokens (e.g. `.file-display-hunks-reviewed` white, `.file-display-hunks-total` muted/gray) in `crates/diffcore-tauri/ui/src/styles.css`.

### 2) Compute per-file reviewed hunk counts (TS state)
- In `crates/diffcore-tauri/ui/src/App.tsx`, derive a `Map<string, number>` of `reviewedHunksByFile` from `replayViewedHunkIds`.
  - The hunk IDs are currently generated as `monaco_hunk_${filePath}_...` (see `mapEditedHunksToReplayHunks`), so we can count by prefix match.
  - Provide this map via context (`AppContextValue`) for `RightmostPane` (file list) to pass into `FileDisplay`.
- Update `crates/diffcore-tauri/ui/src/hooks/AppContext.tsx` types accordingly.
- Update `crates/diffcore-tauri/ui/src/components/panels/RightmostPane.tsx` to pass `reviewedHunks={reviewedHunksByFile.get(file.path)}` to `FileDisplay`.

### 3) Move hunk/file navigation to the rightmost pane header
- Remove navigation UI from `CenterPane.tsx`:
  - Delete the replay “Step X of Y” label.
  - Delete the progress-dot tracker `.replay-progress`.
  - Delete the `+ Hunk Comment` button.
  - Keep (or slim) the “REPLAY” badge + Exit replay button if still useful, but ensure step tracker is gone.
- Add a new compact nav control row into `GroupsAndSettingsPane.tsx` header area (shown when a file diff is open):
  - Buttons: `<hunk`, `hunk>`, `<file`, `file>`.
  - Hunk buttons wire to existing context (`navigateReplayHunk(-1|+1)`, `hasPrevReplayHunk`, `hasNextReplayHunk`). These already work outside replay because `currentReplayHunks` is populated from Monaco.
  - File buttons: implement new context callbacks that navigate **selectedGroup files** regardless of replay mode.
    - Add `hasPrevFileInGroup`, `hasNextFileInGroup`, `goToPrevFileInGroup`, `goToNextFileInGroup` to context.
    - Implementation in `App.tsx` should use `selectedGroup` + `selectedFile` to compute index and call `openFileInTab(nextPath, selectedGroup.id)`.
    - Buttons must be enabled whenever `selectedGroup && selectedFile` and an adjacent file exists (independent of `replayActive`).
- Theme these controls like the existing “Cross-file search” header block (same button classes, spacing, background, border) so it visually matches.

### 4) Remove step tracker everywhere
- Verify no other components render the tracker (search already suggests it’s primarily in `CenterPane.tsx`).
- Also remove any CSS related to `.replay-progress`, `.replay-dot`, `.replay-step-label` if no longer used.

### 5) Fix Activity tab auto-focus bug
- Change `useActivityStream.ts` so it **does not call** `onJobActive()` on every entry.
  - New rule: only switch to Activity when a job transitions from null→non-null (job start), not when entries arrive.
  - Optionally: still switch on `activityError` if a job is active and the user hasn’t manually navigated away.
- This prevents the observed behavior: after any LLM run, the tab re-select effect fires repeatedly and blocks navigation away.

### 6) Info tab: move repo navigation controls here
- In `AnnotationsTab.tsx` Info subtab, add a new “Repository” section at the top that contains:
  - `[repo dropdown] [pin star]` using `recentRepoPaths`/`favoriteRepoPaths` (reuse `RepoPathCombobox` behavior from `HeaderBar.tsx`, but embedded in the info section).
  - `[browse] [analyze]` calling `browseForRepository` and `runAnalysis`.
  - `[source dropdown] [target dropdown]` using `handleSelectHead`/`handleSelectBase` and the same options (branches + special compare targets) currently rendered in `HeaderBar.tsx`.
- Then simplify `HeaderBar.tsx`:
  - Remove repo/branch/analyze controls.
  - Remove the settings gear button entirely.
  - Keep only brand + lightweight status (e.g. current branch/status text) and “Restore Session”/“Setup AI” if desired.

### 7) Settings icon: remove from header, add icon tab label
- Remove the gear button in `HeaderBar.tsx`.
- In `GroupsAndSettingsPane.tsx`, change the “Settings” tab button content from text to an icon (while keeping an accessible label/title).
  - Keep “Groups” as text.

### 8) Regenerate/model controls: 4-block grid in Info tab (TS + Rust)
- Replace the current single “Model (…)” dropdown + “Regenerate with feedback/question” button in `AnnotationsTab.tsx` with **four action blocks** arranged as a 2x2 grid:
  - **Summary**: `[summary model dropdown] [Summarize PR button] [redo-with-feedback icon-only]`
  - **Flow analysis**: `[flow analysis model dropdown] [Analyze This Flow button] [redo-with-feedback icon-only]`
  - **Flow group refinement**: `[refinement model dropdown] [Refine flow groups button] [redo-with-feedback icon-only]`
  - **Disabled PR storyline**: disabled dropdown + disabled button + disabled icon.
- “Redo with feedback” icon-only buttons should open the existing `RegenDialog`, but with a new “operation” field to indicate which job is being rerun.
- TS changes:
  - Extend regen dialog state in `App.tsx` / context to include `regenOperation: "summary" | "flow_analysis" | "refine_groups"`.
  - Route dialog submit to the correct runner:
    - summary → `runAnnotateOverview({ feedback, includePreviousOutput })`
    - flow analysis → new `runDeepAnalysis({ feedback, includePreviousOutput })` variant
    - refine groups → new `runRefinement({ feedback, includePreviousOutput })` variant
- Rust changes (required):
  - Add optional `user_feedback` / `include_previous_output` / `previous_output` fields to the Tauri commands for Pass2 and refinement (Pass1 already has these).
  - Update `crates/diffcore-tauri/src/commands/llm.rs` to thread these into the core `diffcore_core::llm` request builders.
  - If `diffcore_core` schemas don’t currently support feedback for Pass2/refinement, extend the relevant request types in `diffcore-core` and update prompt construction accordingly.

### 9) Move AI overlay into `RightPane` as a new tab
- Add a new `RightPanelTab` value (e.g. `"ai"`) in:
  - `crates/diffcore-tauri/ui/src/hooks/AppContext.tsx` type
  - `crates/diffcore-tauri/ui/src/App.tsx` state initialization and persisted state
  - `crates/diffcore-tauri/ui/src/components/panels/RightPane.tsx` tab bar + tab switch
- Implement `components/tabs/AiOverlayTab.tsx`:
  - Dropdown for provider type containing:
    - cursor_cli, cursor_api, claude_cli, anthropic_api, codex_cli, openai_api, qwen_cli, alibaba_api, gemini_cli, gemini_api, copilot_cli, copilot_api, ollama_api
  - Conditional fields for CLI vs API (key input, key source, install/login hints, etc.).
  - Replace provider-specific tool-fidelity disclaimer with a generic statement based on **CLI vs API**.
- Decide how onboarding behaves:
  - Keep `AISetupModal` as a lightweight “first run” nudge, but primary configuration lives in the new tab; or
  - Replace modal open action with “open AI tab” (preferred for consistency).

### 10) Rightmost pane double scrollbar + sticky banners
- The rightmost pane renders `RightmostPane embedded` inside a `.panel-body` with `overflow: hidden` while `RightmostPane` itself uses its own `.panel-body` scroll; this can create nested scroll areas.
- Make the rightmost pane have **one** scrolling container:
  - Ensure only one element in the rightmost pane controls `overflow-y: auto`.
  - Remove/avoid nested `overflow: auto` wrappers where possible.
- Make the top banners in `RightmostPane.tsx` sticky:
  - Target the refinement banner and manifest-watch banner (“Edit groups via CLI / Export & Watch”) by wrapping them in a container that is `position: sticky; top: 0; z-index: ...` within the scrollable area.
  - Ensure sticky works by not placing them inside an element with `overflow: hidden` that breaks sticky positioning.
- Update `styles.css` accordingly (likely `.panel-rightmost`, `.panel-body`, `.refinement-banner`, `.manifest-watch-banner`).

## Files likely to change
- **TS/React**
  - `crates/diffcore-tauri/ui/src/components/FileDisplay.tsx`
  - `crates/diffcore-tauri/ui/src/components/panels/CenterPane.tsx`
  - `crates/diffcore-tauri/ui/src/components/panels/HeaderBar.tsx`
  - `crates/diffcore-tauri/ui/src/components/panels/GroupsAndSettingsPane.tsx`
  - `crates/diffcore-tauri/ui/src/components/panels/RightPane.tsx`
  - `crates/diffcore-tauri/ui/src/components/tabs/AnnotationsTab.tsx`
  - `crates/diffcore-tauri/ui/src/components/tabs/ActivityTab.tsx` (copy updates to disclaimer text, if needed)
  - `crates/diffcore-tauri/ui/src/components/tabs/AiOverlayTab.tsx` (new)
  - `crates/diffcore-tauri/ui/src/hooks/useActivityStream.ts`
  - `crates/diffcore-tauri/ui/src/hooks/AppContext.tsx` (types)
  - `crates/diffcore-tauri/ui/src/App.tsx` (state, callbacks, regen routing, new tab)
  - `crates/diffcore-tauri/ui/src/styles.css`

- **Rust/Tauri**
  - `crates/diffcore-tauri/src/commands/llm.rs`
  - Possibly `crates/diffcore-tauri/src/commands/mod.rs` for command signatures
  - Potentially `crates/diffcore-core/src/llm/schema/...` and prompt builders if Pass2/refinement need new feedback fields

## Acceptance checks (manual)
- File list rows show `(reviewed/total)` with correct coloring and correct counts.
- Hunk/file nav appears in the **rightmost pane header** and works with a diff open; file nav works even when replay is off.
- No step tracker remains; replay progress dots are gone.
- Activity tab no longer “locks” the UI; user can switch away during/after runs.
- Info subtab contains repo controls and the 2x2 AI action grid.
- Settings gear removed from header; settings tab shows icon.
- AI overlay is a dedicated `RightPane` tab with provider-type dropdown and generalized CLI/API disclaimer.
- Rightmost pane has a single scrollbar; top banners are sticky.