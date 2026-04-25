/**
 * ESLint v9 flat config for diffcore-tauri UI.
 *
 * Rules enforced:
 *  - max-lines: 2000 on all .ts/.tsx files (test files exempt)
 *    Exception: "entrypoint files" (see ENTRYPOINT_FILES below) are allowed up
 *    to 3000 lines because they own a lot of application state, effects, and
 *    the composing return JSX. Extracting panels/tabs/modals is preferred,
 *    but the root orchestration layer has a naturally higher floor.
 *  - react-hooks/rules-of-hooks: error
 *  - react-hooks/exhaustive-deps: warn
 *  - no-orphan-tauri-commands: error — every tauriInvoke() call must
 *    reference a real #[tauri::command] in the Rust backend
 *
 * See docs/linting-architecture.md for full rationale.
 */
import tsParser from "@typescript-eslint/parser";
import tsPlugin from "@typescript-eslint/eslint-plugin";
import reactPlugin from "eslint-plugin-react";
import reactHooksPlugin from "eslint-plugin-react-hooks";
import noOrphanTauriCommands from "./eslint-rules/no-orphan-tauri-commands.js";

/**
 * Files that legitimately exceed the 2 000-line cap.
 *
 * These are root orchestration / entrypoint files that own all React state,
 * effects, and the composing return JSX for an entire application. They are
 * exempt from the standard 2 000-line cap and use a 3 000-line cap instead.
 * Each entry must have a comment explaining why it qualifies.
 *
 * Rules for adding to this list:
 *  1. The file must be the single root component of a Tauri/web app.
 *  2. All extractable panels, tabs, modals, and utilities must already be
 *     extracted (see component-architecture.md).
 *  3. The remaining lines must be irreducible React state + callbacks.
 */
const ENTRYPOINT_FILES = [
  // App.tsx — root component owning all application state and the context
  // provider. Panels (HeaderBar, LeftPane, CenterPane, RightPane), tabs
  // (ActivityTab, AnnotationsTab, SourceTab, CommentsTab), and modals
  // (AISetupModal, SettingsPanel, RegenDialog, CommentInputOverlay) are all
  // extracted. The remaining lines are state + callback definitions.
  "src/App.tsx",
];

export default [
  // ── Global ignores ────────────────────────────────────────────────────────
  {
    ignores: [
      "dist/**",
      "node_modules/**",
      // Test files are exempt from the size limit (they can grow as needed).
      "**/*.spec.ts",
      "**/*.spec.tsx",
      "**/*.test.ts",
      "**/*.test.tsx",
    ],
  },

  // ── Source files ──────────────────────────────────────────────────────────
  {
    files: ["src/**/*.ts", "src/**/*.tsx"],

    languageOptions: {
      parser: tsParser,
      parserOptions: {
        ecmaVersion: "latest",
        sourceType: "module",
        ecmaFeatures: { jsx: true },
      },
    },

    plugins: {
      "@typescript-eslint": tsPlugin,
      react: reactPlugin,
      "react-hooks": reactHooksPlugin,
      "diffcore-local": {
        rules: {
          "no-orphan-tauri-commands": noOrphanTauriCommands,
        },
      },
    },

    settings: {
      react: { version: "detect" },
    },

    rules: {
      // ── File size cap ───────────────────────────────────────────────────
      // 2 000 lines per non-test TS/TSX file. Blank lines count,
      // but comments do not. Even though blank lines do contribute in a small
      // way to cognitive load, comments typically provide clarity and should
      // not be avoided merely for a max line count. Keep comments free.
      "max-lines": [
        "error",
        { max: 2000, skipBlankLines: false, skipComments: true },
      ],

      // ── React hooks ─────────────────────────────────────────────────────
      "react-hooks/rules-of-hooks": "error",
      "react-hooks/exhaustive-deps": "warn",

      // ── Tauri command alignment ──────────────────────────────────────────
      // Every tauriInvoke("cmd") must reference a real Rust backend command.
      "diffcore-local/no-orphan-tauri-commands": "error",
    },
  },

  // ── Entrypoint file overrides ─────────────────────────────────────────────
  // Root orchestration files listed in ENTRYPOINT_FILES are allowed up to
  // 3 000 lines. All other rules are identical to the standard block above.
  {
    files: ENTRYPOINT_FILES,
    rules: {
      "max-lines": [
        "error",
        { max: 3000, skipBlankLines: false, skipComments: true },
      ],
    },
  },
];
