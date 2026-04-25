# Specs Index

| Spec | Description | Status |
|------|-------------|--------|
| [component-architecture](./component-architecture.md) | React UI component structure: panel/tab/modal split, utility extraction, 2000-line cap, AppContext data flow | Active (utility extraction done; panel/tab/modal split pending) |
| [diff-analyzer](./diff-analyzer.md) | diffcore — semantic diff review tool with ranked data-flow grouping, Tauri app + VS Code extension | Active (Phase 12 complete — 7/18 acceptance tests passed, 11 require GUI/VS Code/human review; Section 15 refinement robustness Steps 1-4 executed, 5-6 pending) |
| [improved-clustering](./improved-clustering.md) | Reduce infrastructure bloat: path-based entrypoints for all languages, bidirectional BFS, infrastructure redefinition + sub-grouping | Complete (Phases 1-6 done — core types, bidirectional BFS, path-based entrypoints, sub-clustering, consumer updates, spec updates) |
| [language-expansion](./language-expansion.md) | Expand tree-sitter coverage to ~25+ languages, gate every grammar behind a Cargo feature, parameterize flake.nix `withLanguages` | Active (Phase 1 — inventory complete; Nim + SQL deferred pending build.rs vendoring) |
| [navigation-revamp](./navigation-revamp.md) | Revamp navigation: repo path combobox w/ typeahead + browse/pin fixes, diff-left + two-right resizable panes, settings-in-tab, staged→recent-commit compare, collapsible flow groups | Draft |
| [ui-polish](./ui-polish.md) | Tauri UI polish — file-row spacing, persistent diff toolbar, FileDisplay component, SourceExplorer hide-unchanged toggle, two-line file rows, activity panel rebalance | Active (8 phases) |

## Completed Tasks

| Archive | Description |
|---------|-------------|
| [diff-analyzer-completed](./tasks/diff-analyzer-completed.md) | Granular completed tasks from Phases 1-8 |
| [security-remediation](./security-remediation.md) | Exploitability-ranked security findings and fixes: key_cmd trust boundary, git2 upgrade, CSP hardening, CORS restriction, bincode migration, comment-code drift |
