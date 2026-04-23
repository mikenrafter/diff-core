/**
 * ESLint v9 flat config for diffcore-tauri UI.
 *
 * Rules enforced:
 *  - max-lines: 2000 on all .ts/.tsx files (test files exempt)
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
      // 2 000 lines per non-test TS/TSX file. Blank lines and comments count
      // (they are part of the cognitive load too).
      "max-lines": [
        "error",
        { max: 2000, skipBlankLines: false, skipComments: false },
      ],

      // ── React hooks ─────────────────────────────────────────────────────
      "react-hooks/rules-of-hooks": "error",
      "react-hooks/exhaustive-deps": "warn",

      // ── Tauri command alignment ──────────────────────────────────────────
      // Every tauriInvoke("cmd") must reference a real Rust backend command.
      "diffcore-local/no-orphan-tauri-commands": "error",
    },
  },
];
