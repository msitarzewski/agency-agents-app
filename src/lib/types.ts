/**
 * TypeScript equivalents of all Rust DTOs from `memory-bank/backendApi.md`.
 *
 * Camel-case JSON shape on the wire — these types match exactly what
 * `invoke()` returns for each Tauri command.
 */

// =========================================================
// 2.7 Streaming events (Phase 3 & 4)
// =========================================================

export type AppStreamEvent =
  | { kind: "started";  jobId: string; command: string; startedAt: string }
  | { kind: "stdout";   jobId: string; line: string; ts: string }
  | { kind: "stderr";   jobId: string; line: string; ts: string }
  | { kind: "progress"; jobId: string; message: string; percent: number | null }
  | { kind: "exit";     jobId: string; exitCode: number; success: boolean; durationMs: number }
  | { kind: "canceled"; jobId: string }
  | { kind: "error";    jobId: string; error: BrewErrorPayload };

// =========================================================
// 2.10 Settings (Phase 12d)
// =========================================================

/**
 * Catalog auto-refresh cadence. Wire-format mirrors the Rust enum
 * `CatalogAutoRefresh` (kebab-case).
 */
export type CatalogAutoRefresh = "off" | "weekly" | "daily";

/**
 * Legacy icon-fetching mode inherited from the source app. Kept in the
 * settings schema for compatibility until the settings model is pruned.
 */
export type CaskIconMode = "off" | "installed-only" | "all";

/**
 * Persisted user settings (Phase 12d). Lives at
 * `~/Library/Application Support/com.zerologic.agency-agents-app/settings.json` and is
 * round-tripped via `settingsGet` / `settingsSet`.
 *
 * Bounds (enforced server-side, also re-checked client-side for snappier
 * UX): `catalogStaleBannerDays` ∈ [1, 365]; `trendingTtlMinutes` ∈ [5, 1440].
 */
export interface Settings {
  /** Master switch — when true, every outbound command fails with
      `paranoid_mode_blocked`. */
  paranoidMode: boolean;
  catalogAutoRefresh: CatalogAutoRefresh;
  catalogStaleBannerDays: number;
  caskIconMode: CaskIconMode;
  trendingTtlMinutes: number;
  /** Phase 12c — when true, PackageDetail probes `api.github.com` for
      repo stats whenever the package's homepage is a GitHub URL. Off
      by default; the user opts in via Settings → GitHub. Independent
      of sign-in (anonymous probes still get the 60/hr public limit). */
  githubEnabled: boolean;
  /** Phase 13 — master AI Features toggle. When false, ALL AI-derived
      data is hidden in the UI: categories (Phase 9), enrichment
      (Phase 13), donut chart, category pills, friendly names,
      summaries, use cases, similar packages, tags. Default true. */
  aiFeaturesEnabled: boolean;
  /** Phase 15 — when true, the backend's auto-check scheduler wakes
      every 24h and calls `update_check_now`. Default off; the user
      opts in via Settings → Network → Updates. Suppressed (no fetch)
      while Offline Mode is on, regardless of this flag. */
  updateAutoCheck: boolean;
  /** Legacy enhanced-trending toggle inherited from the source app.
      Retained for settings-file compatibility. */
  enhancedTrendingEnabled: boolean;
  /** Legacy vulnerability-scanning toggle inherited from the source app.
      Agency Agents does not currently run a vulnerability scanner. */
  vulnerabilityScanningEnabled: boolean;
  /** Legacy live-enrichment toggle inherited from the source app. Agency
      Agents currently reads metadata from the active AA catalog. */
  liveEnrichmentEnabled: boolean;
}

/** Defaults matching the Rust `Settings::default()`. Used when seeding
    the settings store before the first `settingsGet` resolves so the UI
    doesn't have to render an empty state. */
export const SETTINGS_DEFAULTS: Settings = {
  paranoidMode: false,
  catalogAutoRefresh: "off",
  catalogStaleBannerDays: 14,
  caskIconMode: "all",
  trendingTtlMinutes: 60,
  // Phase 12c — anonymous GitHub stats opt-in. Off by default per the
  // "zero outbound unless user consented" posture.
  githubEnabled: false,
  // Phase 13 — AI-enriched rendering. ON by default so users get the
  // friendly names, summaries, and categories out of the box.
  aiFeaturesEnabled: true,
  // Auto-check for new Agency Agents releases. Off by
  // default per the "zero outbound unless user consented" posture.
  updateAutoCheck: false,
  // Legacy retained field. Off by default.
  enhancedTrendingEnabled: false,
  // Legacy retained field. Off by default.
  vulnerabilityScanningEnabled: false,
  // Opt-in live refresh of categories + descriptions. Off by default; same
  // legacy live enrichment path.
  liveEnrichmentEnabled: false,
};

// =========================================================
// 2.11 GitHub (Phase 12c + 12e)
// =========================================================

/**
 * Anonymous (or token-authenticated) repo metadata fetched from
 * `api.github.com/repos/{owner}/{repo}`. The backend caches the
 * response on disk for 24h, keyed by the validated owner/repo pair.
 *
 * `null`-able fields are absent on real-world repos: a repo with no
 * GitHub release will have `lastReleaseTag === null`, a live repo
 * will have `archivedAt === null`, etc.
 */
export interface RepoStats {
  owner: string;
  repo: string;
  stars: number;
  forks: number;
  openIssues: number;
  lastReleaseTag: string | null;
  lastReleaseDate: string | null;
  archived: boolean;
  archivedAt: string | null;
  licenseSpdx: string | null;
  defaultBranch: string;
  primaryLanguage: string | null;
}

/**
 * Sign-in status surface returned by `githubStatus`.
 *
 * **Token is never on the wire** — only the derived "what can the
 * session do?" view is. See `github::auth::GithubStatusDto` in the
 * backend for the matching Rust struct and the regression test that
 * pins the wire shape.
 */
export interface GithubStatus {
  signedIn: boolean;
  username: string | null;
  scopes: string[];
}

/**
 * Result of `githubSigninStart` — payload the frontend uses to show
 * the user code and drive the polling loop.
 */
export interface DeviceFlowStart {
  /** Short human-readable code (e.g. `WDJB-MJHT`) to type at
      `verificationUri`. */
  userCode: string;
  /** URL to open in the browser (usually `github.com/login/device`). */
  verificationUri: string;
  /** Seconds until `deviceCode` expires. After this, polling will
      return `expired`. */
  expiresIn: number;
  /** Server-recommended polling cadence in seconds. Must be honoured. */
  interval: number;
  /** Opaque code passed to `githubSigninPoll`. Never shown to the user. */
  deviceCode: string;
}

/**
 * Discriminated union returned by each `githubSigninPoll` call.
 *
 * The `slowDown` variant means GitHub asked us to back off — the
 * frontend should double its polling interval before the next call,
 * per RFC 8628 §3.5.
 */
export type DeviceFlowPoll =
  | { kind: "pending" }
  | { kind: "slowDown" }
  | { kind: "approved"; username: string | null; scopes: string[] }
  | { kind: "denied" }
  | { kind: "expired" };

/**
 * Result of `githubCreateIssue` — the freshly-minted issue's number
 * and canonical `html_url`. Returned by the create-issue backend
 * command (Phase 12f). The frontend opens `htmlUrl` via `safeOpenUrl`
 * after a successful submission.
 */
export interface CreatedIssue {
  number: number;
  htmlUrl: string;
}

// =========================================================
// 2.12 Updater (Phase 15)
// =========================================================

/**
 * A newer Agency Agents version surfaced by the manifest at
 * `agency-agents-app.zerologic.com/updater.json`. Held by the updater store
 * once a check returns `available`. Matches the camelCase wire shape
 * the backend's `UpdateCheckOutcome::Available` flattens onto when
 * serde-tagged with `kind`.
 *
 * `notes` is the raw `notes` field from the manifest — free-form text
 * (we publish a "See release notes at <url>" sentence by default), so
 * UI renders it as-is.
 *
 * No `sha256` here: the manifest sha256 is verified inside the plugin
 * before signature verification; never exposed to the renderer.
 */
export interface UpdateInfo {
  version: string;
  currentVersion: string;
  notes: string | null;
  pubDate: string | null;
  skipped: boolean;
}

/**
 * Tagged union returned by `update_check_now`. Matches the backend's
 * `UpdateCheckOutcome` serde shape exactly: `{ kind, ...fields }` with
 * the `Available` fields flattened next to the discriminator (not
 * nested under `.info`). The store narrows on `kind` and lifts the
 * flat fields into the strongly-typed `UpdateInfo` for downstream
 * consumers.
 *
 *   - `upToDate` — manifest version ≤ running version, nothing to show.
 *   - `available` — newer version exists (and isn't on the user's
 *     skip-list); UI surfaces the indicator + the install action.
 *
 * `blocked` is **not** a wire variant — Offline Mode surfaces as
 * `BrewError::ParanoidModeBlocked` instead, so the toast routes
 * through the same channel as every other gated call.
 */
export type UpdateCheckOutcome =
  | { kind: "upToDate" }
  | {
      kind: "available";
      version: string;
      currentVersion: string;
      notes: string | null;
      pubDate: string | null;
      skipped: boolean;
    };

// =========================================================
// 3.3 Error model
// =========================================================

export type BrewErrorPayload =
  | { code: "brew_not_found" }
  | { code: "brew_exit_non_zero"; command: string; exitCode: number; stderrExcerpt: string; friendlyMessage?: string }
  | { code: "json_parse";         command: string; message: string; rawExcerpt: string }
  | { code: "io";                 message: string }
  | { code: "network";            url: string; message: string }
  | { code: "http_status";        url: string; status: number }
  | { code: "invalid_argument";   message: string }
  | { code: "job_not_found";      jobId: string }
  | { code: "canceled" }
  | { code: "brewfile_not_found"; id: string }
  | { code: "internal";           message: string }
  | { code: "paranoid_mode_blocked"; feature: string }
  | { code: "feature_disabled";    feature: string }
  | { code: "github_rate_limited"; resetAt: number }
  | { code: "keychain_unavailable"; message: string }
  | { code: "auth_required" }
  | { code: "scope_required"; scope: string }
  | { code: "hash_mismatch"; expected: string; actual: string }
  | { code: "signature_verification_failed"; message: string }
  | { code: "downgrade_rejected"; current: string; target: string }
  | { code: "vulns_not_installed"; installCommand: string };

/** Type-narrowing helper: is the thrown value a BrewErrorPayload? */
export function isBrewError(e: unknown): e is BrewErrorPayload {
  return (
    typeof e === "object" &&
    e !== null &&
    "code" in e &&
    typeof (e as { code: unknown }).code === "string"
  );
}

/** Human-readable message for a BrewError. */
export function brewErrorMessage(e: BrewErrorPayload): string {
  switch (e.code) {
    case "brew_not_found":      return "A required tool was not found on PATH.";
    case "brew_exit_non_zero":  return e.friendlyMessage ?? `Command exited ${e.exitCode}: ${e.stderrExcerpt}`;
    case "json_parse":          return `Failed to parse output: ${e.message}`;
    case "io":                  return `I/O error: ${e.message}`;
    case "network":             return `Network error: ${e.message}`;
    case "http_status":         return `HTTP ${e.status} from ${e.url}`;
    case "invalid_argument":    return `Invalid argument: ${e.message}`;
    case "job_not_found":       return `Job ${e.jobId} not found.`;
    case "canceled":            return "Operation canceled.";
    case "brewfile_not_found":  return `"${e.id}" not found.`;
    case "internal":            return `Internal error: ${e.message}`;
    case "paranoid_mode_blocked":
      return `Offline Mode is on — ${e.feature} is blocked. Disable it in Settings → Network.`;
    case "feature_disabled":
      return `${e.feature} is disabled. Enable it in Settings → Network.`;
    case "github_rate_limited": {
      const reset = e.resetAt > 0 ? new Date(e.resetAt * 1000).toLocaleTimeString() : "soon";
      return `GitHub API rate limit reached. Resets at ${reset}. Sign in to lift the limit.`;
    }
    case "keychain_unavailable":
      return `macOS Keychain unavailable: ${e.message}`;
    case "auth_required":
      return "Sign in to GitHub to use this feature.";
    case "scope_required":
      return `GitHub permission "${e.scope}" required. Sign in again to grant it.`;
    case "hash_mismatch":
      return `Update aborted: downloaded artifact hash didn't match the manifest (expected ${e.expected.slice(0, 12)}…, got ${e.actual.slice(0, 12)}…).`;
    case "signature_verification_failed":
      return `Update aborted: signature verification failed (${e.message}).`;
    case "downgrade_rejected":
      return `Update refused: ${e.target} is not newer than the installed version (${e.current}).`;
    case "vulns_not_installed":
      return `A required subcommand is not installed. Run \`${e.installCommand}\` to enable it.`;
  }
}

// =========================================================
// Agency Agents — corpus subsystem (contracts.md §A)
// =========================================================
//
// Mirrors the Rust DTOs in `src-tauri/src/types.rs`. Wire shape is
// camelCase. These are ADDITIVE alongside the inherited brew types
// while the corpus subsystem is built out phase by phase.

/**
 * An AI coding tool we can deploy an agent into. The 11 members are the
 * authoritative install-target set from agency-agents' `scripts/install.sh`
 * (contracts.md §B). Matches the Rust `Tool` enum (`rename_all = "camelCase"`),
 * so multi-word variants are `claudeCode` / `geminiCli` (not snake/kebab).
 */
export type Tool =
  | "claudeCode"
  | "copilot"
  | "cursor"
  | "geminiCli"
  | "codex"
  | "opencode"
  | "windsurf"
  | "aider"
  | "qwen"
  | "openclaw"
  | "antigravity";

/** Deployment scope. User-global tools write to fixed `~/…` dests;
    project-scoped tools install into a tracked `projectPath`. */
export type Scope = "user" | "project";

/**
 * An agent as parsed from a single corpus `.md` file. `body` is the
 * markdown persona — empty in list views (`corpusList`) to keep payloads
 * small, populated by `corpusGet`.
 */
export interface Agent {
  /** Filename without `.md`, e.g. `"frontend-developer"`. */
  slug: string;
  /** Frontmatter `name`. */
  name: string;
  /** Frontmatter `description`. */
  description: string;
  /** Parent directory, e.g. `"engineering"`. */
  category: string;
  /** Frontmatter `emoji`. */
  emoji: string | null;
  /** Frontmatter `color` (named or hex). */
  color: string | null;
  /** Frontmatter `vibe`. */
  vibe: string | null;
  /** Markdown body (persona) — empty in list views. */
  body: string;
}

/**
 * One row of `corpus-index.json`. The three split hashes let update
 * classification distinguish cosmetic (frontmatter-only) from substantive
 * (body) changes. Each hash is SHA-256 lowercase hex of UTF-8 bytes.
 */
export interface CorpusEntry {
  slug: string;
  name: string;
  category: string;
  emoji: string | null;
  color: string | null;
  vibe: string | null;
  description: string;
  /** SHA-256 of the full canonical `.md`. */
  sourceHash: string;
  /** SHA-256 of the frontmatter block. */
  frontmatterHash: string;
  /** SHA-256 of the body. */
  bodyHash: string;
}

/** Top-level metadata for the maintained corpus copy. */
export interface CorpusMeta {
  version: string;
  commit: string | null;
  fetchedAt: string;
  count: number;
}

/**
 * Where the active agent catalog lives. Discriminated on `kind`:
 * - `bundled` — app-managed copy from the bundled baseline (default).
 * - `managed` — a clone the app provisioned/owns (default `~/.agency-agents`).
 * - `userClone` — the user's own clone; `manage` = permission to pull it.
 */
export type CatalogSource =
  | { kind: "bundled" }
  | { kind: "managed"; path: string }
  | { kind: "userClone"; path: string; manage: boolean };

/** Live status of the active catalog (source + git provenance + freshness). */
export interface CatalogStatus {
  source: CatalogSource;
  root: string | null;
  isGit: boolean;
  branch: string | null;
  commit: string | null;
  lastCommitSubject: string | null;
  lastCommitDate: string | null;
  dirtyCount: number;
  remoteUrl: string | null;
  /** "owner/repo" parsed from the remote, for GitHub repo stats. */
  repoSlug: string | null;
  version: string;
  fetchedAt: string;
  agentCount: number;
}

/** Result of checking the active catalog for upstream updates (diff stats). */
export interface CatalogUpdateCheck {
  isGit: boolean;
  behind: number;
  ahead: number;
  changedFiles: number;
  diffstat: string;
  upToDate: boolean;
}

/** A catalog directory discovered on disk (first-run / Settings picker). */
export interface CatalogCandidate {
  path: string;
  /** "managed" for ~/.agency-agents, else "userClone". */
  kind: "managed" | "userClone";
  hasGit: boolean;
  agentCount: number;
}

/** Result of `catalog_detect`. */
export interface CatalogDetection {
  gitAvailable: boolean;
  scanned: boolean;
  candidates: CatalogCandidate[];
}

/**
 * One row of `installs.json` — the ledger of local install actions.
 * `sourceHash` records the corpus version installed from; `renderedHash`
 * is the SHA-256 of the exact bytes written after per-tool conversion.
 */
export interface InstallRecord {
  slug: string;
  tool: Tool;
  scope: Scope;
  projectPath: string | null;
  /** Absolute path written. */
  dest: string;
  sourceHash: string;
  /** SHA-256 of the agent body at install time (cosmetic vs substantive updates). */
  bodyHash: string;
  renderedHash: string;
  installedAt: string;
  corpusVersion: string;
}

/**
 * The five reconciliation states (our `brew list`/`brew outdated`). See
 * systemPatterns.md §4 for the disk ↔ ledger ↔ corpus classification.
 */
export type InstallState =
  | "current"
  | "outdated"
  | "modified"
  | "removed"
  | "foreign";

/** Whether an available update is cosmetic (frontmatter/metadata only,
    `bodyHash` unchanged) or substantive (prompt body changed). */
export type UpdateKind = "cosmetic" | "substantive";

/**
 * Reconciled view-model for the Library — one on-disk agent file resolved
 * against the ledger and corpus-index. `updateKind` is set only when
 * `state === "outdated"`.
 */
export interface InstalledAgent {
  slug: string;
  name: string;
  tool: Tool;
  scope: Scope;
  projectPath: string | null;
  dest: string;
  state: InstallState;
  updateKind: UpdateKind | null;
}

/** Result of `agent_diff` — current on-disk contents vs the canonical render
    the app would write. Powers "review before Update" with zero file writes. */
export interface AgentDiff {
  slug: string;
  tool: Tool;
  projectPath: string | null;
  dest: string;
  /** Current on-disk contents (null if the file is missing). */
  onDisk: string | null;
  /** The canonical render the app would write. */
  proposed: string;
  /** Whether the two differ (false ⇒ Update is a no-op). */
  differs: boolean;
}

/** View-model for the Tools section — a detected AI tool plus its
    deployment surface. */
export interface ToolInfo {
  tool: Tool;
  label: string;
  detected: boolean;
  scope: Scope;
  userDest: string | null;
  installedCount: number;
}

/** Best-effort detected version for a tool (`<bin> --version`); null when the
    binary isn't on PATH, the probe timed out, or there's no known command. */
export interface ToolVersion {
  tool: Tool;
  version: string | null;
}

/**
 * One category for the Discover grid. `slug` is the corpus parent dir
 * (e.g. `"engineering"`); `icon` is a PascalCase Lucide icon name the
 * frontend resolves via its static icon map (same convention as
 * `CategoryMeta.icon`).
 */
export interface Category {
  slug: string;
  label: string;
  icon: string;
  count: number;
}

/**
 * A registered project directory for project-scoped installs. The app
 * keeps a Projects list so Library/Tools can show per-project deployment;
 * one agent in five projects = five tracked rows.
 */
export interface ProjectInfo {
  /** Absolute project root path. */
  path: string;
  /** Display label (defaults to the directory name). */
  label: string;
  /** Count of agents installed into this project across all
      project-scoped tools. */
  installedCount: number;
}

// =========================================================
// UI-only types (frontend stores, command palette, etc.)
// =========================================================

export type SidebarSection =
  | "dashboard"
  | "personas"
  | "tools"
  | "loadouts"
  | "activity";

/**
 * Filter lens for the unified Agents workspace. The catalog browse and the
 * old Library view are one surface now; the install-state split becomes a
 * filter over a single agent list rather than a separate destination.
 *   - `all`          — every agent in the catalog (browse mode).
 *   - `installed`    — agents deployed in at least one tool (any state on disk).
 *   - `notInstalled` — agents not deployed anywhere (the complement of installed).
 *   - `attention`    — agents with a drifted install (outdated/modified/removed).
 *   - `untracked`    — agents recognized on disk but not yet tracked (foreign).
 */
export type AgentsFilter = "all" | "installed" | "notInstalled" | "attention" | "untracked";

export type ThemePreference = "light" | "dark" | "system";

/** Settings modal subsection. Kept in sync with Settings.svelte's
    internal section list — use this when deep-linking via
    `ui.openSettings(section)`. */
export type SettingsSection =
  | "appearance"
  | "catalog"
  | "network"
  | "github"
  | "activity"
  | "about";

/** A job tracked locally on the frontend (status + accumulated lines). */
export interface ActivityJob {
  jobId: string;
  label: string;             // human-friendly: "Installing wget"
  command: string;
  startedAt: string;
  status: "running" | "succeeded" | "failed" | "canceled";
  lines: ActivityLine[];
  exitCode?: number;
  durationMs?: number;
}

export interface ActivityLine {
  stream: "stdout" | "stderr";
  text: string;
  ts: string;
}

/** Command-palette item — a verb (action). */
export type PaletteItem =
  | { kind: "command"; id: string; label: string; shortcut?: string; section?: string; run: () => void | Promise<void> };
