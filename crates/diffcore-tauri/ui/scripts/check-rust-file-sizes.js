#!/usr/bin/env node
/**
 * check-rust-file-sizes.js
 *
 * Enforces the 3 000-line cap on Rust source files in diffcore-core and
 * diffcore-tauri. Test files (any file whose name contains "test") are
 * explicitly exempt — they may grow as the test suite grows.
 *
 * Usage:
 *   node scripts/check-rust-file-sizes.js
 *
 * Exit codes:
 *   0 — all files are within the limit
 *   1 — one or more files exceed the limit (details printed to stdout)
 */

import { readdirSync, readFileSync, statSync } from "node:fs";
import { resolve, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = new URL(".", import.meta.url).pathname;
const REPO_ROOT = resolve(__dirname, "../../../..");
const MAX_LINES = 3000;

// Directories to scan (relative to repo root).
const SCAN_DIRS = [
  "crates/diffcore-core/src",
  "crates/diffcore-tauri/src",
];

/** Return true if this file should be exempt from the size limit. */
function isTestFile(filename) {
  const lower = filename.toLowerCase();
  return lower.includes("test") || lower.includes("spec");
}

/** Recursively list all .rs files under a directory. */
function findRustFiles(dir) {
  const results = [];
  let entries;
  try {
    entries = readdirSync(dir, { withFileTypes: true });
  } catch {
    return results;
  }
  for (const entry of entries) {
    const fullPath = join(dir, entry.name);
    if (entry.isDirectory()) {
      results.push(...findRustFiles(fullPath));
    } else if (entry.isFile() && entry.name.endsWith(".rs")) {
      results.push(fullPath);
    }
  }
  return results;
}

let violations = 0;

for (const scanDir of SCAN_DIRS) {
  const absDir = join(REPO_ROOT, scanDir);
  const files = findRustFiles(absDir);

  for (const file of files) {
    if (isTestFile(file)) continue;

    let content;
    try {
      content = readFileSync(file, "utf8");
    } catch {
      continue;
    }

    const lineCount = content.split("\n").length;
    if (lineCount > MAX_LINES) {
      console.log(
        `OVER LIMIT: ${relative(REPO_ROOT, file)} — ${lineCount} lines (limit: ${MAX_LINES})`,
      );
      violations += 1;
    }
  }
}

if (violations === 0) {
  console.log(`All Rust source files are within the ${MAX_LINES}-line limit.`);
  process.exit(0);
} else {
  console.log(`\n${violations} file(s) exceed the ${MAX_LINES}-line limit.`);
  process.exit(1);
}
