/** TypeScript types matching the Rust AnalysisOutput schema. */

export type DiffType =
  | "BranchComparison"
  | "CommitRange"
  | "Staged"
  | "Unstaged"
  | "Worktree"
  | "BranchWithWorktree";

export type EdgeType = "Imports" | "Calls" | "Extends" | "Instantiates" | "Reads" | "Writes" | "Emits" | "Handles";

export type FileRole = "Entrypoint" | "Handler" | "Service" | "Repository" | "Model" | "Utility" | "Config" | "Test" | "Infrastructure";

export type EntrypointType = "HttpRoute" | "CliCommand" | "QueueConsumer" | "CronJob" | "ReactPage" | "TestFile" | "EventHandler" | "EffectService";

export interface DiffSource {
  diff_type: DiffType;
  base: string | null;
  head: string | null;
  base_sha: string | null;
  head_sha: string | null;
}

export interface ChangeStats {
  additions: number;
  deletions: number;
}

export interface FlowEdge {
  from: string;
  to: string;
  edge_type: EdgeType;
}

export interface Entrypoint {
  file: string;
  symbol: string;
  entrypoint_type: EntrypointType;
}

export interface FileChange {
  path: string;
  flow_position: number;
  role: FileRole;
  changes: ChangeStats;
  symbols_changed: string[];
}

export interface FlowGroup {
  id: string;
  name: string;
  entrypoint: Entrypoint | null;
  files: FileChange[];
  edges: FlowEdge[];
  risk_score: number;
  review_order: number;
}

export type InfraCategory =
  | "Infrastructure"
  | "Schema"
  | "Script"
  | "Migration"
  | "Deployment"
  | "Documentation"
  | "Lint"
  | "TestUtil"
  | "Generated"
  | "DirectoryGroup"
  | "Unclassified";

export interface InfraSubGroup {
  name: string;
  category: InfraCategory;
  /** Bare paths (backward-compatible). Prefer `file_changes` for change stats. */
  files: string[];
  /** Per-file change stats, populated by the output layer. */
  file_changes?: FileChange[];
}

export interface InfrastructureGroup {
  /** Bare paths (backward-compatible). Prefer `file_changes` for change stats. */
  files: string[];
  sub_groups?: InfraSubGroup[];
  /** Per-file change stats, parallel to `files`. */
  file_changes?: FileChange[];
  reason: string;
}

export interface AnalysisSummary {
  total_files_changed: number;
  total_groups: number;
  languages_detected: string[];
  frameworks_detected: string[];
}

export interface AnalysisOutput {
  version: string;
  diff_source: DiffSource;
  summary: AnalysisSummary;
  groups: FlowGroup[];
  infrastructure_group: InfrastructureGroup | null;
  annotations: Pass1Response | null;
}

export interface FileDiffContent {
  path: string;
  old_content: string;
  new_content: string;
  language: string;
}

// ── Parsed-file IR (mirrors diffcore_core::ast::ParsedFile) ──
//
// Returned by the `parse_file_content` Tauri command. The source-explorer
// outline panel reads this directly instead of running its own per-language
// regex parsers, so any language the Rust query engine supports is also
// supported by the outline (Dart, Elixir, Haskell, Lua, Nix, Svelte, Vue, …).

/** SymbolKind values mirror the Rust enum `diffcore_core::types::SymbolKind`.
 *  Serde uses the variant name as the JSON value (default representation),
 *  so the strings here must match exactly. */
export type ParsedSymbolKind =
  | "Function"
  | "Class"
  | "Struct"
  | "Interface"
  | "TypeAlias"
  | "Constant"
  | "Module";

export interface ParsedDefinition {
  name: string;
  kind: ParsedSymbolKind;
  start_line: number;
  end_line: number;
}

export interface ParsedImportedName {
  name: string;
  alias: string | null;
}

export interface ParsedImport {
  source: string;
  names: ParsedImportedName[];
  is_default: boolean;
  is_namespace: boolean;
  line: number;
}

export interface ParsedExport {
  name: string;
  is_default: boolean;
  is_reexport: boolean;
  source: string | null;
  line: number;
}

export interface ParsedCallSite {
  callee: string;
  line: number;
  containing_function: string | null;
}

export interface ParsedFile {
  path: string;
  /** Serde-default tag of `diffcore_core::ast::Language` (CamelCase variant
   *  name, e.g. "TypeScript", "Python", "Dart", "CSharp"); "Unknown" when
   *  the file extension is unrecognised. Note this differs from
   *  `FileDiffContent.language`, which is lowercase ("typescript", …) —
   *  callers that need to compare the two must normalise. */
  language: string;
  definitions: ParsedDefinition[];
  imports: ParsedImport[];
  exports: ParsedExport[];
  call_sites: ParsedCallSite[];
}

/** Parameters for the analyze command. */
export interface AnalyzeParams {
  repo_path: string;
  base?: string;
  head?: string;
  range?: string;
  staged: boolean;
  unstaged: boolean;
  pr_preview?: boolean;
  include_uncommitted?: boolean;
}

// ── Git Auto-Discovery Types ──

/** Information about a git branch. */
export interface BranchInfo {
  name: string;
  is_current: boolean;
  has_upstream: boolean;
}

/** Information about a recent commit shown in ref pickers. */
export interface CommitInfo {
  sha: string;
  short_sha: string;
  summary: string;
  author: string;
  timestamp: number;
}

/** Information about a git worktree. */
export interface WorktreeInfo {
  path: string;
  branch: string | null;
  is_main: boolean;
}

/** Branch tracking status (ahead/behind remote). */
export interface BranchStatus {
  branch: string;
  upstream: string | null;
  ahead: number;
  behind: number;
}

/** Summary of repository state for the UI. */
export interface RepoInfo {
  current_branch: string | null;
  default_branch: string;
  branches: BranchInfo[];
  worktrees: WorktreeInfo[];
  status: BranchStatus | null;
  /** Whether the opened path is a linked worktree (not the main worktree). */
  is_worktree: boolean;
}

// ── LLM Annotation Types ──

/** Pass 1 overview response — per-group summaries + overall summary. */
export interface Pass1Response {
  groups: Pass1GroupAnnotation[];
  overall_summary: string;
  suggested_review_order: string[];
}

/** Per-group annotation from Pass 1 overview. */
export interface Pass1GroupAnnotation {
  id: string;
  name: string;
  summary: string;
  review_order_rationale: string;
  risk_flags: string[];
}

/** Pass 2 deep analysis response for a single group. */
export interface Pass2Response {
  group_id: string;
  flow_narrative: string;
  file_annotations: Pass2FileAnnotation[];
  cross_cutting_concerns: string[];
}

/** Per-file annotation from Pass 2 deep analysis. */
export interface Pass2FileAnnotation {
  file: string;
  role_in_flow: string;
  changes_summary: string;
  risks: string[];
  suggestions: string[];
}

/** Combined annotations container. */
export interface Annotations {
  overview: Pass1Response | null;
  deep_analyses: Pass2Response[];
}

// ── LLM Settings Types ──

/** LLM settings for the settings panel. */
export interface LlmSettings {
  annotations_enabled: boolean;
  refinement_enabled: boolean;
  provider: string;
  model: string;
  api_key_source: string;
  has_api_key: boolean;
  refinement_provider: string;
  refinement_model: string;
  refinement_max_iterations: number;
  global_config_path: string;
  codex_available: boolean;
  codex_authenticated: boolean;
  claude_available: boolean;
  claude_authenticated: boolean;
  include_uncommitted: boolean;
}

export interface AsyncLlmJobStart {
  job_id: string;
  stream_url: string;
  operation: string;
  provider: string;
  model: string;
  title: string;
}

export interface LlmActivityEntry {
  source: string;
  level: string;
  message: string;
  event_type?: string | null;
  payload?: unknown;
  timestamp_ms: number;
}

export interface LlmActivityJob {
  job_id: string;
  operation: string;
  provider: string;
  model: string;
  title: string;
}

// ── LLM Refinement Types ──

/** Result of an LLM refinement pass. */
export interface RefinementResult {
  refined_groups: FlowGroup[];
  infrastructure_group: InfrastructureGroup | null;
  refinement_response: RefinementResponse;
  provider: string;
  model: string;
  had_changes: boolean;
  warnings: RefinementWarning[];
  stop_reason: RefinementStopReason;
  attempts_used: number;
  parse_failures: number;
}

export type RefinementStopReason =
  | "no_op"
  | "no_score_gain"
  | "max_iterations_reached"
  | "parse_retries_exhausted"
  | "provider_failure";

export interface RefinementWarning {
  op: "pipeline" | "split" | "merge" | "re_rank" | "reclassify";
  action:
    | { repaired: { field: string; original: string; resolved: string } }
    | { dropped: { reason: string } };
  message: string;
}

/** Raw refinement response with structural operations. */
export interface RefinementResponse {
  splits: RefinementSplit[];
  merges: RefinementMerge[];
  re_ranks: RefinementReRank[];
  reclassifications: RefinementReclassify[];
  reasoning: string;
}

/** Split operation: one group becomes multiple. */
export interface RefinementSplit {
  source_group_id: string;
  new_groups: { name: string; files: string[] }[];
  reason: string;
}

/** Merge operation: multiple groups become one. */
export interface RefinementMerge {
  group_ids: string[];
  merged_name: string;
  reason: string;
}

/** Re-rank operation: change a group's review position. */
export interface RefinementReRank {
  group_id: string;
  new_position: number;
  reason: string;
}

/** Reclassify operation: move a file between groups. */
export interface RefinementReclassify {
  file: string;
  from_group_id: string;
  to_group_id: string;
  reason: string;
}

// ── Review Comment Types ──

/** A single review comment — scoped to a group, file, or code range. */
export interface ReviewComment {
  /** Unique identifier. */
  id: string;
  /** Comment scope: "code", "file", or "group". */
  type: "code" | "file" | "group";
  /** The flow group this comment belongs to. */
  group_id: string;
  /** File path (null for group-level comments). */
  file_path: string | null;
  /** Start line (null for file/group-level). */
  start_line: number | null;
  /** End line (null for file/group-level). */
  end_line: number | null;
  /** Selected code snippet (for code-level comments). */
  selected_code: string | null;
  /** The comment text. */
  text: string;
  /** ISO 8601 timestamp. */
  created_at: string;
}

/** Comment input state — tracks what the user is currently commenting on. */
export interface CommentInput {
  /** Comment scope. */
  type: "code" | "file" | "group";
  /** Group ID. */
  group_id: string;
  /** File path (for file/code-level). */
  file_path?: string;
  /** Start line (for code-level). */
  start_line?: number;
  /** End line (for code-level). */
  end_line?: number;
  /** Selected code text (for code-level). */
  selected_code?: string;
}

/** Model descriptor returned by the fetch_provider_models command. */
export interface ModelInfo {
  /** Model identifier for API calls. */
  id: string;
  /** Human-readable display name. */
  display_name: string;
  /** Context window size in tokens, if known. */
  context_length: number | null;
}

/** Diff viewer layout mode. */
export type DiffViewMode = "side-by-side" | "inline" | "dynamic";

/** Available LLM providers. */
export const LLM_PROVIDERS = ["codex", "claude", "anthropic", "openai", "gemini", "openrouter", "github_copilot"] as const;
export type LlmProvider = (typeof LLM_PROVIDERS)[number];

/** Static fallback models per provider — used when API listing is unavailable. */
export const DEFAULT_MODELS_BY_PROVIDER: Record<LlmProvider, string[]> = {
  codex: ["default", "gpt-5.4", "gpt-5.4-mini", "gpt-4.1", "o4-mini", "o3"],
  claude: ["default", "claude-opus-4-6", "claude-sonnet-4-6", "claude-haiku-4-5"],
  anthropic: ["claude-opus-4-6", "claude-sonnet-4-6", "claude-haiku-4-5"],
  openai: ["gpt-5.4", "gpt-5.4-mini", "gpt-4.1", "o4-mini", "o3"],
  gemini: [
    "gemini-3.1-pro-preview",
    "gemini-3-flash-preview",
    "gemini-2.5-flash",
  ],
  openrouter: [
    "anthropic/claude-sonnet-4-6",
    "anthropic/claude-opus-4-6",
    "openai/gpt-4.1",
    "google/gemini-2.5-flash",
    "meta-llama/llama-4-maverick",
    "deepseek/deepseek-r1",
    "qwen/qwen3-235b-a22b",
    "mistralai/mistral-large-latest",
  ],
  github_copilot: [
    "gpt-4.1",
    "gpt-5",
    "o4-mini",
    "claude-sonnet-4-6",
    "claude-opus-4-6",
    "claude-opus-4-7",
    "gemini-2.5-pro",
  ],
};

/**
 * @deprecated Use DEFAULT_MODELS_BY_PROVIDER instead. Kept for backward compat.
 */
export const MODELS_BY_PROVIDER = DEFAULT_MODELS_BY_PROVIDER;
