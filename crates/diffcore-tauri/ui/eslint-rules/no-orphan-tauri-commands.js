/**
 * Custom ESLint rule: no-orphan-tauri-commands
 *
 * Multi-file comprehension rule: reads all #[tauri::command] function names
 * from the Rust backend (crates/diffcore-tauri/src/commands/) and verifies
 * that every tauriInvoke("cmd_name", ...) call in TypeScript references a
 * command that actually exists.
 *
 * This keeps the TS UI layer and the Rust command layer in sync — a
 * tauriInvoke call for a non-existent command will fail silently at runtime,
 * so catching it at lint time is valuable.
 */
import { readFileSync, readdirSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const COMMANDS_DIR = resolve(__dirname, "../../../src/commands");

/** Extract all #[tauri::command] pub fn names from a Rust source directory. */
function extractRustCommandNames(dir) {
  const commands = new Set();
  let files;
  try {
    files = readdirSync(dir).filter((f) => f.endsWith(".rs"));
  } catch {
    // Commands directory not found — skip silently (e.g. running in CI without
    // the Rust source present).
    return commands;
  }

  for (const file of files) {
    let content;
    try {
      content = readFileSync(resolve(dir, file), "utf8");
    } catch {
      continue;
    }

    // Match  #[tauri::command]  (with optional whitespace / newlines)
    // followed by  pub [async] fn <name>
    const pattern = /#\[tauri::command\][\s\S]*?pub\s+(?:async\s+)?fn\s+(\w+)/g;
    let match;
    while ((match = pattern.exec(content)) !== null) {
      commands.add(match[1]);
    }
  }

  return commands;
}

// Extract once at rule-init time (happens once per ESLint run, not per file).
const rustCommands = extractRustCommandNames(COMMANDS_DIR);

export default {
  meta: {
    type: "problem",
    docs: {
      description:
        "Ensure every tauriInvoke() call references a #[tauri::command] that exists in the Rust backend.",
      recommended: false,
    },
    schema: [],
    messages: {
      unknownCommand:
        "tauriInvoke('{{cmd}}') has no matching #[tauri::command] fn in the Rust backend (crates/diffcore-tauri/src/commands/).",
    },
  },

  create(context) {
    // If the commands directory couldn't be read, BLOW UP. No circumvention here!
    if (!rustCommands?.size) {
      throw new Error(
        `ESLint rule 'no-orphan-tauri-commands' failed to read Rust commands from ${COMMANDS_DIR}. Ensure this path is correct and the Rust source files are present.`
      );
    }

    return {
      CallExpression(node) {
        // Match both  tauriInvoke("cmd")  and  tauriInvoke<T>("cmd")
        const callee = node.callee;
        const isDirectCall =
          callee.type === "Identifier" && callee.name === "tauriInvoke";
        const isGenericCall =
          callee.type === "TSInstantiationExpression" &&
          callee.expression?.type === "Identifier" &&
          callee.expression.name === "tauriInvoke";

        if (!isDirectCall && !isGenericCall) return;

        const firstArg = node.arguments[0];
        if (
          !firstArg
          || firstArg.type !== "Literal"
          || typeof firstArg.value !== "string"
        ) {
          return;
        }

        const cmd = firstArg.value;
        if (!rustCommands.has(cmd)) {
          context.report({
            node: firstArg,
            messageId: "unknownCommand",
            data: { cmd },
          });
        }
      },
    };
  },
};
