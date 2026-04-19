import * as vscode from "vscode";
import * as path from "path";
import * as os from "os";
import * as fs from "fs/promises";
import { execFile } from "child_process";
import { promisify } from "util";
import { runDiffcore } from "./diffcoreRunner";
import { FlowGroupsProvider, InfrastructureProvider, GroupItem, FileItem } from "./treeView";
import { AnnotationsPanel } from "./webviewPanel";
import type { AnalysisOutput, FlowGroup } from "./types";

const execFileAsync = promisify(execFile);

// ── State ───────────────────────────────────────────────────────────

let analysis: AnalysisOutput | null = null;
let repoPath = "";
let selectedGroupIndex = 0;
let selectedFileIndex = 0;

let groupsProvider: FlowGroupsProvider;
let infraProvider: InfrastructureProvider;
let annotationsPanel: AnnotationsPanel;
let groupsView: vscode.TreeView<GroupItem | FileItem>;

// ── Activation ──────────────────────────────────────────────────────

export function activate(context: vscode.ExtensionContext): void {
  // Tree data providers
  groupsProvider = new FlowGroupsProvider();
  infraProvider = new InfrastructureProvider();

  groupsView = vscode.window.createTreeView("diffcore.groups", {
    treeDataProvider: groupsProvider,
    showCollapseAll: true,
  });

  const infraView = vscode.window.createTreeView("diffcore.infrastructure", {
    treeDataProvider: infraProvider,
  });

  // Annotations webview panel
  annotationsPanel = new AnnotationsPanel(context.extensionUri);

  // Register commands
  context.subscriptions.push(
    vscode.commands.registerCommand("diffcore.analyze", cmdAnalyze),
    vscode.commands.registerCommand("diffcore.analyzeRange", cmdAnalyzeRange),
    vscode.commands.registerCommand("diffcore.annotate", cmdAnnotate),
    vscode.commands.registerCommand("diffcore.nextFile", cmdNextFile),
    vscode.commands.registerCommand("diffcore.prevFile", cmdPrevFile),
    vscode.commands.registerCommand("diffcore.nextGroup", cmdNextGroup),
    vscode.commands.registerCommand("diffcore.prevGroup", cmdPrevGroup),
    vscode.commands.registerCommand("diffcore.openDiff", cmdOpenDiff),
    vscode.commands.registerCommand("diffcore.openAnnotations", cmdOpenAnnotations),
    vscode.commands.registerCommand("diffcore.refreshProviderModels", cmdRefreshProviderModels),
    groupsView,
    infraView,
    { dispose: () => annotationsPanel.dispose() }
  );

  // Reset keybinding context when diffcore tree view loses visibility
  groupsView.onDidChangeVisibility((e) => {
    if (!e.visible) {
      vscode.commands.executeCommand("setContext", "diffcore.active", false);
    } else if (analysis) {
      vscode.commands.executeCommand("setContext", "diffcore.active", true);
    }
  });

  // Set context for keybinding activation
  vscode.commands.executeCommand("setContext", "diffcore.active", false);
}

export function deactivate(): void {
  annotationsPanel?.dispose();
}

// ── Commands ────────────────────────────────────────────────────────

async function cmdAnalyze(): Promise<void> {
  const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
  if (!workspaceFolder) {
    vscode.window.showErrorMessage("No workspace folder open.");
    return;
  }

  repoPath = workspaceFolder.uri.fsPath;
  const config = vscode.workspace.getConfiguration("diffcore");
  const defaultBase = config.get<string>("defaultBase", "main");

  const refs = await pickComparisonRefs(repoPath, defaultBase);
  if (!refs) {
    return;
  }

  await runAnalysis({ repoPath, base: refs.base, head: refs.head });
}

async function cmdAnalyzeRange(): Promise<void> {
  const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
  if (!workspaceFolder) {
    vscode.window.showErrorMessage("No workspace folder open.");
    return;
  }

  const range = await vscode.window.showInputBox({
    prompt: "Enter commit range (e.g., HEAD~5..HEAD)",
    placeHolder: "HEAD~5..HEAD",
  });

  if (!range) {
    return;
  }

  repoPath = workspaceFolder.uri.fsPath;
  await runAnalysis({ repoPath, range });
}

async function cmdAnnotate(): Promise<void> {
  if (!analysis) {
    vscode.window.showWarningMessage("Run diffcore.analyze first.");
    return;
  }

  await vscode.window.withProgress(
    {
      location: vscode.ProgressLocation.Notification,
      title: "diffcore: Running LLM annotation...",
      cancellable: false,
    },
    async () => {
      try {
        const result = await runDiffcore({
          repoPath,
          base: analysis!.diff_source.base ?? "main",
          head: analysis!.diff_source.head ?? undefined,
          annotate: true,
        });
        analysis = result.output;
        groupsProvider.setAnalysis(analysis, repoPath);

        if (analysis.annotations) {
          annotationsPanel.setPass1(analysis.annotations as any);
        }

        vscode.window.showInformationMessage("diffcore: LLM annotation complete.");
      } catch (err: unknown) {
        const msg = err instanceof Error ? err.message : String(err);
        vscode.window.showErrorMessage(`diffcore annotation failed: ${msg}`);
      }
    }
  );
}

function cmdNextFile(): void {
  if (!analysis || analysis.groups.length === 0) {
    return;
  }
  const group = sortedGroups()[selectedGroupIndex];
  if (selectedFileIndex < group.files.length - 1) {
    selectedFileIndex++;
    openCurrentFile();
  }
}

function cmdPrevFile(): void {
  if (!analysis || analysis.groups.length === 0) {
    return;
  }
  if (selectedFileIndex > 0) {
    selectedFileIndex--;
    openCurrentFile();
  }
}

function cmdNextGroup(): void {
  if (!analysis || analysis.groups.length === 0) {
    return;
  }
  if (selectedGroupIndex < analysis.groups.length - 1) {
    selectedGroupIndex++;
    selectedFileIndex = 0;
    openCurrentFile();
    showCurrentGroupAnnotations();
  }
}

function cmdPrevGroup(): void {
  if (!analysis || analysis.groups.length === 0) {
    return;
  }
  if (selectedGroupIndex > 0) {
    selectedGroupIndex--;
    selectedFileIndex = 0;
    openCurrentFile();
    showCurrentGroupAnnotations();
  }
}

async function cmdOpenDiff(
  diffRepoPath: string,
  filePath: string,
  groupId: string
): Promise<void> {
  const baseRef = analysis?.diff_source.base ?? "main";
  const headRef = analysis?.diff_source.head ?? "HEAD";

  try {
    const baseUri = vscode.Uri.parse(
      `git-show:${baseRef}:${filePath}`
    ).with({ scheme: "git", query: JSON.stringify({ ref: baseRef, path: filePath }) });

    const headUri = vscode.Uri.file(path.join(diffRepoPath, filePath));

    const renderSideBySide = await resolveRenderSideBySide(groupId, filePath);
    await vscode.workspace
      .getConfiguration("diffEditor")
      .update("renderSideBySide", renderSideBySide, vscode.ConfigurationTarget.Workspace);

    await vscode.commands.executeCommand(
      "vscode.diff",
      baseUri,
      headUri,
      `${filePath} (${baseRef} ↔ ${headRef})`
    );
  } catch {
    // Fallback: open file directly if git scheme fails
    const fileUri = vscode.Uri.file(path.join(diffRepoPath, filePath));
    await vscode.commands.executeCommand("vscode.open", fileUri);
  }
}

function cmdOpenAnnotations(): void {
  if (!analysis || analysis.groups.length === 0) {
    return;
  }
  showCurrentGroupAnnotations();
}

async function cmdRefreshProviderModels(): Promise<void> {
  const provider = await vscode.window.showQuickPick(["anthropic", "openrouter"], {
    title: "Refresh models for provider",
    placeHolder: "Select provider",
  });
  if (!provider) {
    return;
  }

  await vscode.window.withProgress(
    {
      location: vscode.ProgressLocation.Notification,
      title: `diffcore: Refreshing ${provider} model list...`,
      cancellable: false,
    },
    async () => {
      try {
        const models = await fetchProviderModelsFromCli(provider);
        const configPath = await resolveModelsConfigPath();
        const config = await readModelsConfig(configPath);
        config.providers[provider] = models.map((m) => m.id);
        await fs.mkdir(path.dirname(configPath), { recursive: true });
        await fs.writeFile(configPath, `${JSON.stringify(config, null, 2)}\n`, "utf8");

        vscode.window.showInformationMessage(
          `diffcore: refreshed ${models.length} models for ${provider} in ${configPath}`
        );
      } catch (err: unknown) {
        const msg = err instanceof Error ? err.message : String(err);
        vscode.window.showErrorMessage(`diffcore model refresh failed: ${msg}`);
      }
    }
  );
}

// ── Helpers ─────────────────────────────────────────────────────────

function sortedGroups(): FlowGroup[] {
  if (!analysis) {
    return [];
  }
  return analysis.groups.slice().sort((a, b) => a.review_order - b.review_order);
}

async function runAnalysis(options: {
  repoPath: string;
  base?: string;
  head?: string;
  range?: string;
}): Promise<void> {
  await vscode.window.withProgress(
    {
      location: vscode.ProgressLocation.Notification,
      title: "diffcore: Analyzing...",
      cancellable: false,
    },
    async () => {
      try {
        const result = await runDiffcore(options);
        analysis = result.output;
        selectedGroupIndex = 0;
        selectedFileIndex = 0;

        groupsProvider.setAnalysis(analysis, options.repoPath);
        infraProvider.setInfrastructure(analysis.infrastructure_group);

        vscode.commands.executeCommand("setContext", "diffcore.active", true);

        const { summary } = analysis;
        vscode.window.showInformationMessage(
          `diffcore: ${summary.total_files_changed} files in ${summary.total_groups} groups ` +
            `(${summary.languages_detected.join(", ")})`
        );

        // Auto-open first file
        if (analysis.groups.length > 0) {
          openCurrentFile();
        }
      } catch (err: unknown) {
        const msg = err instanceof Error ? err.message : String(err);
        vscode.window.showErrorMessage(`diffcore: ${msg}`);
      }
    }
  );
}

function openCurrentFile(): void {
  const groups = sortedGroups();
  if (groups.length === 0) {
    return;
  }
  const group = groups[selectedGroupIndex];
  const sortedFiles = group.files.slice().sort((a, b) => a.flow_position - b.flow_position);
  if (sortedFiles.length === 0) {
    return;
  }
  const file = sortedFiles[selectedFileIndex];
  vscode.commands.executeCommand("diffcore.openDiff", repoPath, file.path, group.id);
}

function showCurrentGroupAnnotations(): void {
  const groups = sortedGroups();
  if (groups.length === 0 || !analysis) {
    return;
  }
  annotationsPanel.showGroup(groups[selectedGroupIndex], analysis);
}

interface PickedRefs {
  base: string;
  head?: string;
}

interface BranchChoice {
  name: string;
  isCurrent: boolean;
}

interface CommitChoice {
  sha: string;
  shortSha: string;
  summary: string;
}

interface CliModelInfo {
  id: string;
  display_name: string;
  context_length: number | null;
}

interface ModelsConfigFile {
  providers: Record<string, string[]>;
}

async function pickComparisonRefs(repoFsPath: string, defaultBase: string): Promise<PickedRefs | null> {
  const branches = await listLocalBranches(repoFsPath);
  const commits = await listRecentCommits(repoFsPath, 80);

  const base = await pickRefTarget({
    title: "Select target ref",
    placeHolder: "Branches are listed first. Choose recent commits if needed.",
    branches,
    commits,
    defaultRef: defaultBase,
    includeHeadOption: false,
  });
  if (!base) {
    return null;
  }

  const currentBranch = branches.find((b) => b.isCurrent)?.name;
  const head = await pickRefTarget({
    title: "Select source ref",
    placeHolder: "Pick branch/commit to compare from, or use HEAD.",
    branches,
    commits,
    defaultRef: currentBranch,
    includeHeadOption: true,
  });
  if (!head) {
    return null;
  }

  if (head === "HEAD") {
    return { base };
  }

  return { base, head };
}

async function pickRefTarget(options: {
  title: string;
  placeHolder: string;
  branches: BranchChoice[];
  commits: CommitChoice[];
  defaultRef?: string;
  includeHeadOption: boolean;
}): Promise<string | null> {
  type RefQuickPick = vscode.QuickPickItem & { value: string; fromCommits?: boolean };
  const items: RefQuickPick[] = [];

  if (options.includeHeadOption) {
    items.push({ label: "HEAD", description: "current workspace state", value: "HEAD" });
  }

  for (const branch of options.branches) {
    items.push({
      label: branch.name,
      description: branch.isCurrent ? "current branch" : "branch",
      value: branch.name,
    });
  }

  items.push({
    label: "$(git-commit) Pick from recent commits",
    description: `${options.commits.length} recent commits`,
    value: "__RECENT_COMMITS__",
  });

  const picked = await vscode.window.showQuickPick(items, {
    title: options.title,
    placeHolder: options.placeHolder,
  });

  if (!picked) {
    return null;
  }

  if (picked.value !== "__RECENT_COMMITS__") {
    return picked.value;
  }

  if (options.commits.length === 0) {
    vscode.window.showWarningMessage("No recent commits were found.");
    return null;
  }

  const commitItems: RefQuickPick[] = options.commits.map((commit) => ({
    label: commit.shortSha,
    description: commit.summary,
    detail: commit.sha,
    value: commit.sha,
    fromCommits: true,
  }));

  const pickedCommit = await vscode.window.showQuickPick(commitItems, {
    title: `${options.title} (Recent Commits)`,
    placeHolder: "Select commit",
  });

  return pickedCommit?.value ?? null;
}

async function listLocalBranches(repoFsPath: string): Promise<BranchChoice[]> {
  const { stdout } = await execFileAsync(
    "git",
    ["for-each-ref", "--format=%(refname:short)%00%(HEAD)", "refs/heads"],
    { cwd: repoFsPath }
  );

  const branches = stdout
    .split("\n")
    .map((line: string) => line.trim())
    .filter((line: string) => line.length > 0)
    .map((line: string) => {
      const [name, headMarker] = line.split("\u0000");
      return {
        name,
        isCurrent: headMarker === "*",
      } as BranchChoice;
    });

  branches.sort((a: BranchChoice, b: BranchChoice) => Number(b.isCurrent) - Number(a.isCurrent) || a.name.localeCompare(b.name));
  return branches;
}

async function listRecentCommits(repoFsPath: string, limit: number): Promise<CommitChoice[]> {
  const { stdout } = await execFileAsync(
    "git",
    ["log", `--max-count=${limit}`, "--pretty=format:%H%x00%h%x00%s"],
    { cwd: repoFsPath }
  );

  return stdout
    .split("\n")
    .map((line: string) => line.trim())
    .filter((line: string) => line.length > 0)
    .map((line: string) => {
      const [sha, shortSha, summary] = line.split("\u0000");
      return { sha, shortSha, summary: summary || "<no message>" } as CommitChoice;
    });
}

async function resolveRenderSideBySide(groupId: string, filePath: string): Promise<boolean> {
  const config = vscode.workspace.getConfiguration("diffcore");
  const mode = config.get<string>("diffViewMode", "sideBySide");

  if (mode === "sideBySide") {
    return true;
  }
  if (mode === "inline") {
    return false;
  }

  const fileChange = analysis?.groups
    .find((group) => group.id === groupId)
    ?.files.find((file) => file.path === filePath);

  if (!fileChange) {
    return true;
  }

  const totalChanged = fileChange.changes.additions + fileChange.changes.deletions;
  const linesInHead = await safeCountHeadFileLines(filePath);
  const linesInBase = await safeCountBaseFileLines(filePath);
  const maxLines = Math.max(linesInHead, linesInBase, 1);
  const density = totalChanged / maxLines;

  return density > (2/3);
}

async function safeCountHeadFileLines(filePath: string): Promise<number> {
  try {
    const absolutePath = path.join(repoPath, filePath);
    const content = await fs.readFile(absolutePath, "utf8");
    return content.split("\n").length;
  } catch {
    return 0;
  }
}

async function safeCountBaseFileLines(filePath: string): Promise<number> {
  const baseRef = analysis?.diff_source.base;
  if (!baseRef || !repoPath) {
    return 0;
  }

  try {
    const { stdout } = await execFileAsync("git", ["show", `${baseRef}:${filePath}`], {
      cwd: repoPath,
      maxBuffer: 10 * 1024 * 1024,
    });
    return stdout.split("\n").length;
  } catch {
    return 0;
  }
}

async function fetchProviderModelsFromCli(provider: string): Promise<CliModelInfo[]> {
  const binaryPath = vscode.workspace.getConfiguration("diffcore").get<string>("binaryPath", "") || "diffcore";
  const { stdout } = await execFileAsync(binaryPath, ["list-models", "--provider", provider, "--refresh"], {
    maxBuffer: 10 * 1024 * 1024,
  });
  const parsed = JSON.parse(stdout.trim()) as CliModelInfo[];
  if (!Array.isArray(parsed)) {
    throw new Error("Unexpected model list output from diffcore CLI.");
  }
  return parsed;
}

async function resolveModelsConfigPath(): Promise<string> {
  const configured = vscode.workspace.getConfiguration("diffcore").get<string>("modelsConfigPath", "");
  const rawPath = configured?.trim() || "~/.diffcore/provider-models.json";
  if (!rawPath.startsWith("~/")) {
    return rawPath;
  }
  return path.join(os.homedir(), rawPath.slice(2));
}

async function readModelsConfig(configPath: string): Promise<ModelsConfigFile> {
  try {
    const raw = await fs.readFile(configPath, "utf8");
    const parsed = JSON.parse(raw) as Partial<ModelsConfigFile>;
    return {
      providers: parsed.providers ?? {},
    };
  } catch {
    return { providers: {} };
  }
}
