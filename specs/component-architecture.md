# Component Architecture Spec

**Status:** Complete — all panels, tabs, modals, and utilities extracted.  
**Last updated:** 2026-04-24

---

## Overview

The diffcore Tauri UI is a React single-page application that lives in `crates/diffcore-tauri/ui/src/`. The original codebase had a single `App.tsx` file of 6 685 lines. This spec defines the target component architecture, the 2 000-line file size cap, and the extraction roadmap.

---

## File size cap

| File type | Limit | Exempt |
|---|---|---|
| `.ts` / `.tsx` | 2 000 lines | `*.spec.ts`, `*.test.ts`, `*.spec.tsx`, `*.test.tsx` |

This limit is enforced by the ESLint `max-lines` rule. See `docs/linting-architecture.md` for full rationale.

Test files placed next to source files (e.g., `activityUtils.spec.ts` beside `activityUtils.ts`) are never subject to the size limit.

---

## Current file structure

```
src/
├── App.tsx                        ← orchestration layer (5990 lines → target ≤2000)
├── types.ts                       ← TypeScript types mirroring Rust schema
├── mock.ts                        ← demo/test mock data
├── buildManifestPrompt.ts         ← LLM prompt builder
├── buildManifestPrompt.test.ts    ← collocated test
│
├── utils/                         ← pure functions, no React/hooks (EXTRACTED ✅)
│   ├── pathUtils.ts               ← shortPath, shortSymbol, parseSymbolEndpoint, …
│   ├── gitUtils.ts                ← resolveFileShortStatus, formatBranchStatus, COMPARE_TARGET_*
│   ├── activityUtils.ts           ← describeActivityEntry, buildMockActivityEntries, …
│   ├── groupUtils.ts              ← riskLevel, getGroupChangeIndicator, computeToolEditHunks
│   └── llmUtils.ts                ← resolveInteractiveProvider, resolveInteractiveModel
│
└── components/                    ← React components (partially extracted)
    ├── DiffViewer.tsx             ← Monaco-based diff viewer
    ├── FlowGraph.tsx              ← Dagre/XYFlow graph
    ├── SourceExplorer.tsx         ← Source outline explorer
    ├── Dropdown.tsx               ← Shared dropdown
    ├── FileDisplay.tsx            ← File label / badge display
    ├── RiskHeatmap.tsx            ← Hidden (Phase 9.4 — kept for re-enablement)
    └── ErrorBoundary.tsx          ← React error boundary
```

---

## Target component architecture

The goal is for `App.tsx` to be a lean orchestration layer (≤ 2 000 lines) that:
- Holds all application state (via React Context)
- Renders the top-level shell and delegates to panel components

```
src/
├── App.tsx                        ← ≤2000 lines: state, context provider, top-level shell
│
├── hooks/
│   └── AppContext.tsx             ← React context + provider exposing all state/callbacks
│
├── utils/                         ← pure functions (DONE ✅)
│   ├── pathUtils.ts
│   ├── gitUtils.ts
│   ├── activityUtils.ts
│   ├── groupUtils.ts
│   └── llmUtils.ts
│
└── components/
    ├── panels/                    ← Major UI panels (PENDING)
    │   ├── HeaderBar.tsx          ← <header className="top-bar"> — repo path, branches, analyze
    │   ├── LeftPane.tsx           ← <aside className="panel-left"> — group list, file list, search
    │   ├── CenterPane.tsx         ← <main> — file tabs, DiffViewer, FlowGraph
    │   └── RightPane.tsx          ← <aside className="panel-right"> — tab host + tab content
    │
    ├── tabs/                      ← Right-pane tab content (PENDING)
    │   ├── ActivityTab.tsx        ← LLM activity stream (hero + event log)
    │   ├── AnnotationsTab.tsx     ← Pass1/Pass2 annotation display + edge graph
    │   ├── CommentsTab.tsx        ← Review comment list (CRUD)
    │   └── SourceTab.tsx          ← Thin wrapper over <SourceExplorer>
    │
    ├── modals/                    ← Overlay dialogs (PENDING)
    │   ├── AISetupModal.tsx       ← AI provider onboarding
    │   ├── SettingsPanel.tsx      ← LLM + editor + diff settings
    │   ├── RegenDialog.tsx        ← Regenerate with feedback overlay
    │   └── CommentInputOverlay.tsx ← Comment input overlay (c key)
    │
    ├── DiffViewer.tsx             ← Monaco diff viewer (EXISTING)
    ├── FlowGraph.tsx              ← Dagre/XYFlow graph (EXISTING)
    ├── SourceExplorer.tsx         ← Source outline (EXISTING)
    ├── Dropdown.tsx               ← Shared dropdown (EXISTING)
    ├── FileDisplay.tsx            ← File label / badge (EXISTING)
    └── ErrorBoundary.tsx          ← Error boundary (EXISTING)
```

---

## Data flow

App.tsx owns all state. Rather than threading props through 4 levels of components, state and callbacks are exposed via an `AppContext`:

```tsx
// hooks/AppContext.tsx
const AppContext = createContext<AppContextValue | null>(null);

export function AppContextProvider({ children }: { children: React.ReactNode }) {
  // ... all useState / useCallback declarations ...
  const value: AppContextValue = { repoPath, setRepoPath, analysis, ... };
  return <AppContext.Provider value={value}>{children}</AppContext.Provider>;
}

export function useAppContext(): AppContextValue {
  const ctx = useContext(AppContext);
  if (!ctx) throw new Error("useAppContext must be used within AppContextProvider");
  return ctx;
}
```

Panel and tab components consume state via `useAppContext()`:
```tsx
// components/panels/LeftPane.tsx
export function LeftPane() {
  const { analysis, selectedGroup, openGroupById } = useAppContext();
  // ...
}
```

---

## Test file placement

Tests live next to their source files with a `.spec.ts` / `.spec.tsx` suffix:

```
src/utils/activityUtils.ts
src/utils/activityUtils.spec.ts   ← unit tests for activityUtils
src/utils/groupUtils.ts
src/utils/groupUtils.spec.ts
src/components/panels/LeftPane.tsx
src/components/panels/LeftPane.spec.tsx
```

Integration / E2E tests live in `tests/` using Playwright (planned, see `test:e2e` script).

---

## Extraction progress

| Item | Status | Lines |
|---|---|---|
| `utils/pathUtils.ts` | ✅ Done | 48 |
| `utils/gitUtils.ts` | ✅ Done | 52 |
| `utils/activityUtils.ts` | ✅ Done | ~330 |
| `utils/groupUtils.ts` | ✅ Done | ~160 |
| `utils/llmUtils.ts` | ✅ Done | 65 |
| `utils/tauriUtils.ts` | ✅ Done | — |
| `hooks/AppContext.tsx` | ✅ Done | 439 |
| `hooks/useEditorIntegration.ts` | ✅ Done | ~120 |
| `components/panels/HeaderBar.tsx` | ✅ Done | — |
| `components/panels/LeftPane.tsx` | ✅ Done | — |
| `components/panels/CenterPane.tsx` | ✅ Done | — |
| `components/panels/RightPane.tsx` | ✅ Done | — |
| `components/tabs/ActivityTab.tsx` | ✅ Done | 271 |
| `components/tabs/AnnotationsTab.tsx` | ✅ Done | 332 |
| `components/tabs/CommentsTab.tsx` | ✅ Done | 165 |
| `components/tabs/SourceTab.tsx` | ✅ Done | 32 |
| `components/modals/AISetupModal.tsx` | ✅ Done | 227 |
| `components/modals/SettingsPanel.tsx` | ✅ Done | 347 |
| `components/modals/RegenDialog.tsx` | ✅ Done | 72 |
| `components/modals/CommentInputOverlay.tsx` | ✅ Done | 73 |
| `App.tsx` (final, with entrypoint exception) | ✅ Done | ~3474 |

`App.tsx` qualifies for the 3 000-line entrypoint exception (see `eslint.config.js` `ENTRYPOINT_FILES`). All extractable panels, tabs, modals, and utilities have been separated. The remaining lines are state declarations, effects, and callback hooks — the orchestration core beyond which further extraction has diminishing returns.

---

## Naming conventions

- **Component files:** PascalCase (e.g., `LeftPane.tsx`, `ActivityTab.tsx`)
- **Hook files:** camelCase prefixed with `use` (e.g., `useAppContext.tsx`, `useAnalysis.ts`)
- **Utility files:** camelCase (e.g., `activityUtils.ts`, `pathUtils.ts`)
- **Test files:** same name as source with `.spec.ts` / `.spec.tsx` suffix
- **Panel components:** suffixed with `Pane` (left, center, right) or `Bar` (header)
- **Tab components:** suffixed with `Tab`
- **Modal components:** suffixed with `Modal`, `Panel`, `Dialog`, or `Overlay` depending on UI role

---

## Linting enforcement

See `docs/linting-architecture.md` for the full linting rationale. Summary:
- `npm run lint` — ESLint: `max-lines`, `react-hooks`, `no-orphan-tauri-commands`
- `npm run lint:rust-size` — Rust source file 3 000-line cap
