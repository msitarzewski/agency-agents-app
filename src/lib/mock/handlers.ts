/**
 * Command handlers for the browser IPC mock.
 *
 * One entry per Tauri command the frontend actually calls (plus the plugin
 * commands the dialog/opener/event plugins go through). Every handler returns
 * the same camelCase JSON the Rust command serialises, and failures reject with
 * the same `{ code, … }` object a Rust `AppError` becomes — so the stores'
 * existing `try/catch`, `isAppError()` and `errorText()` paths are exercised
 * verbatim instead of being bypassed.
 *
 * What the mock deliberately cannot do, and says so instead of pretending:
 * reading/writing real files, probing installed tool binaries, git, the macOS
 * Keychain, GitHub OAuth, and the signed update channel.
 */

import packageJson from "../../../package.json";
import { isInstallable, TOOLS, toolMeta } from "$lib/data/toolRegistry";
import type {
  Agent,
  AgentDiff,
  CatalogDetection,
  CatalogSource,
  CatalogStatus,
  InstallRecord,
  InstalledAgent,
  ProjectInfo,
  Scope,
  ToolInfo,
  ToolVersion,
} from "$lib/types";

import { MOCK_AGENTS, MOCK_CATEGORIES, MOCK_CORPUS_META, mockAgent } from "./corpus";
import {
  MOCK_PROJECT_PATH,
  loadLedger,
  loadSettings,
  resetSettings,
  saveLedger,
  saveSettings,
  type MockSettings,
} from "./state";

/** `app_version` reads `Cargo.toml` natively; keep the mock in step with package.json. */
const MOCK_APP_VERSION = (packageJson as { version?: string }).version ?? "0.0.0";

/** The bundled snapshot is the only catalog a browser can see. */
const MOCK_CATALOG_SOURCE: CatalogSource = { kind: "bundled" };

type Args = Record<string, unknown>;
type Handler = (args: Args) => unknown;

// =========================================================
// Errors — same wire shape as src-tauri/src/error.rs
// =========================================================

function appError(code: string, fields: Record<string, unknown> = {}): Record<string, unknown> {
  return { code, ...fields };
}

function invalidArgument(message: string): Record<string, unknown> {
  return appError("invalid_argument", { message });
}

/** Rejection for anything that needs the native shell. */
function needsNativeApp(feature: string): Record<string, unknown> {
  return appError("io", {
    message: `the browser IPC mock has no filesystem or network — ${feature} needs the native app`,
  });
}

// =========================================================
// Argument helpers (JS callers send camelCase keys)
// =========================================================

function asString(args: Args, key: string): string {
  const value = args[key];
  if (typeof value !== "string") throw invalidArgument(`expected '${key}' to be a string`);
  return value;
}

function asStringOrNull(args: Args, key: string): string | null {
  const value = args[key];
  return typeof value === "string" ? value : null;
}

// =========================================================
// Rendering / ledger
// =========================================================

async function sha256Hex(text: string): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(text));
  return Array.from(new Uint8Array(digest))
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("");
}

/** Destination path for an install, derived from the tool's `dest` templates. */
function destFor(tool: string, slug: string, scope: Scope, projectPath: string | null): string {
  const meta = toolMeta(tool);
  const template = meta?.dest?.[scope === "project" ? "project" : "user"]?.[0];
  if (!template) throw invalidArgument(`tool '${tool}' has no ${scope} destination`);
  const file = template.replace("{slug}", `${meta?.slugPrefix ?? ""}${slug}`);
  // No home directory in a browser; `~` reads as "that tool's config dir".
  return projectPath ? `${projectPath}/${file}` : `~/${file}`;
}

/**
 * Canonical render for the tool's `format`.
 *
 * Byte parity with the Rust renderer is out of scope for a UI preview — the
 * mock only needs stable bytes so ledger hashes and `agent_diff` agree with
 * each other.
 */
function renderForTool(agent: Agent, tool: string): string {
  const format = toolMeta(tool)?.format ?? "identity";
  const body = agent.body.trimStart();

  if (format === "codex-toml") {
    return [
      `name = ${JSON.stringify(agent.name)}`,
      `description = ${JSON.stringify(agent.description)}`,
      "",
      "instructions = '''",
      body,
      "'''",
      "",
    ].join("\n");
  }

  if (format === "cursor-mdc") {
    return [
      "---",
      `description: ${agent.description}`,
      "globs:",
      "alwaysApply: false",
      "---",
      "",
      body,
    ].join("\n");
  }

  return ["---", `name: ${agent.name}`, `description: ${agent.description}`, "---", "", body].join("\n");
}

/** Write one ledger row (replace an existing row for the same slug/tool/target). */
async function writeInstall(args: Args): Promise<InstallRecord> {
  const slug = asString(args, "slug");
  const tool = asString(args, "tool");
  const projectPath = asStringOrNull(args, "projectPath");

  const agent = mockAgent(slug);
  if (!agent) throw invalidArgument(`unknown agent '${slug}'`);
  if (!toolMeta(tool)) throw invalidArgument(`unknown tool '${tool}'`);

  const scope: Scope = projectPath ? "project" : "user";
  const rendered = renderForTool(agent, tool);
  const record: InstallRecord = {
    slug,
    tool,
    scope,
    projectPath,
    dest: destFor(tool, slug, scope, projectPath),
    sourceHash: await sha256Hex(`${agent.name}\n${agent.description}\n${agent.body}`),
    bodyHash: await sha256Hex(agent.body),
    renderedHash: await sha256Hex(rendered),
    installedAt: new Date().toISOString(),
    corpusVersion: MOCK_CORPUS_META.version,
  };

  const ledger = loadLedger().filter(
    (row) => !(row.slug === slug && row.tool === tool && (row.projectPath ?? null) === projectPath),
  );
  ledger.push(record);
  saveLedger(ledger);
  return record;
}

function removeInstall(args: Args): null {
  const slug = asString(args, "slug");
  const tool = asString(args, "tool");
  const projectPath = asStringOrNull(args, "projectPath");

  saveLedger(
    loadLedger().filter(
      (row) => !(row.slug === slug && row.tool === tool && (row.projectPath ?? null) === projectPath),
    ),
  );
  return null;
}

/**
 * Reconcile the ledger into the Library view-model.
 *
 * The real command also sweeps each tool's directory for files it did not write
 * ("foreign" installs). A browser has no disk to sweep, so every ledger row is
 * by definition `current` and `tracked`; foreign/modified/outdated states cannot
 * occur here and therefore never show up as fake rows.
 */
function reconcileInstalls(): InstalledAgent[] {
  return loadLedger().map((row) => ({
    slug: row.slug,
    name: mockAgent(row.slug)?.name ?? row.slug,
    tool: row.tool,
    scope: row.scope,
    projectPath: row.projectPath,
    dest: row.dest,
    state: "current",
    updateKind: null,
    tracked: true,
  }));
}

/** Tools panel view-model: one row per registry tool, counts from the ledger. */
function toolsList(): ToolInfo[] {
  const ledger = loadLedger();
  return TOOLS.map((meta) => {
    const rows = ledger.filter((row) => row.tool === meta.id);
    const userTemplate = meta.dest?.user?.[0];
    return {
      tool: meta.id,
      label: meta.label,
      // Natively `detected` means "the tool's config dir exists on disk", which a
      // browser cannot probe. The mock therefore claims every tool this app can
      // render, which is what the install matrix gates its columns on — without
      // it the Agents → Install dialog is empty and nothing can ever be
      // installed. Recognized-only tools (no native renderer) stay undetected,
      // matching how they render natively as dimmed rows.
      detected: rows.length > 0 || isInstallable(meta),
      scope: (meta.scope?.user ? "user" : "project") as Scope,
      // Dest template shown as a hint, with `{slug}` rendered as the tool's
      // prefix plus a wildcard (e.g. `~/claude/agents/*.md`).
      userDest: userTemplate
        ? `~/${userTemplate.replace("{slug}", `${meta.slugPrefix ?? ""}*`)}`
        : null,
      installedCount: rows.length,
      customPath: null,
    };
  });
}

/** Live catalog status — the bundled snapshot, with no git provenance. */
function catalogStatus(): CatalogStatus {
  return {
    source: MOCK_CATALOG_SOURCE,
    root: null,
    isGit: false,
    branch: null,
    commit: null,
    lastCommitSubject: null,
    lastCommitDate: null,
    dirtyCount: 0,
    remoteUrl: null,
    repoSlug: null,
    version: MOCK_CORPUS_META.version,
    fetchedAt: MOCK_CORPUS_META.fetchedAt,
    agentCount: MOCK_AGENTS.length,
  };
}

/** Projects derived from the ledger, like the native `projects_list`. */
function projectsList(): ProjectInfo[] {
  const byPath = new Map<string, Set<string>>();
  for (const row of loadLedger()) {
    if (!row.projectPath) continue;
    const slugs = byPath.get(row.projectPath) ?? new Set<string>();
    slugs.add(row.slug);
    byPath.set(row.projectPath, slugs);
  }
  return [...byPath.entries()].map(([path, slugs]) => ({
    path,
    label: path.replace(/[\\/]+$/, "").split(/[\\/]/).pop() || path,
    installedCount: slugs.size,
  }));
}

// =========================================================
// Command handlers
// =========================================================

const HANDLERS: Record<string, Handler> = {
  // ── App / settings ───────────────────────────────────────
  app_version: () => MOCK_APP_VERSION,
  settings_get: () => loadSettings(),
  settings_set: (args) => saveSettings(args.settings as MockSettings),
  settings_reset: () => resetSettings(),

  // ── Corpus ───────────────────────────────────────────────
  // `corpus_list` omits bodies to keep the payload small (contracts.md §A).
  corpus_list: () => MOCK_AGENTS.map((agent) => ({ ...agent, body: "" })),
  corpus_get: (args) => {
    const slug = asString(args, "slug");
    const agent = mockAgent(slug);
    if (!agent) throw invalidArgument(`unknown agent '${slug}'`);
    return agent;
  },
  corpus_categories: () => MOCK_CATEGORIES,
  corpus_status: () => MOCK_CORPUS_META,
  corpus_refresh: () => MOCK_CORPUS_META,

  // ── Catalog source ───────────────────────────────────────
  catalog_source_get: () => MOCK_CATALOG_SOURCE,
  // Always "configured" so the first-run picker doesn't cover the preview: a
  // browser can only ever use the bundled snapshot.
  catalog_configured: () => true,
  catalog_status: () => catalogStatus(),
  catalog_check_updates: () => ({
    isGit: false,
    behind: 0,
    ahead: 0,
    changedFiles: 0,
    diffstat: "",
    upToDate: true,
  }),
  catalog_detect: (args): CatalogDetection => ({
    gitAvailable: false,
    scanned: args.scan === true,
    candidates: [],
  }),
  catalog_source_set: (args) => {
    const source = args.source as CatalogSource | undefined;
    if (source?.kind === "bundled") return MOCK_CORPUS_META;
    throw needsNativeApp("switching to an on-disk catalog");
  },
  catalog_provision_managed: () => {
    throw needsNativeApp("cloning the catalog");
  },
  catalog_pull: () => {
    throw needsNativeApp("pulling the catalog");
  },

  // ── Runbooks ─────────────────────────────────────────────
  // `strategy/` only ships in a synced clone, never in the bundled snapshot —
  // so the bundled-backed mock reports the same empty state the real app would.
  runbooks_list: () => [],

  // ── Installs ─────────────────────────────────────────────
  installs_reconcile: () => reconcileInstalls(),
  installs_for_agent: (args) => loadLedger().filter((row) => row.slug === asString(args, "slug")),
  install_agent: (args) => writeInstall(args),
  update_agent: (args) => writeInstall(args),
  track_agent: (args) => writeInstall(args),
  uninstall_agent: (args) => removeInstall(args),
  agent_diff: (args): AgentDiff => {
    const slug = asString(args, "slug");
    const tool = asString(args, "tool");
    const projectPath = asStringOrNull(args, "projectPath");

    const agent = mockAgent(slug);
    const row = loadLedger().find(
      (entry) =>
        entry.slug === slug && entry.tool === tool && (entry.projectPath ?? null) === projectPath,
    );
    if (!agent || !row) throw invalidArgument(`'${slug}' is not installed in '${tool}'`);

    const proposed = renderForTool(agent, tool);
    // Nothing can edit the file behind the mock's back, so disk == render.
    return { slug, tool, projectPath, dest: row.dest, onDisk: proposed, proposed, differs: false };
  },
  project_forget: (args) => {
    const projectPath = asString(args, "projectPath");
    saveLedger(loadLedger().filter((row) => (row.projectPath ?? null) !== projectPath));
    return null;
  },
  projects_list: () => projectsList(),

  // ── Tools ────────────────────────────────────────────────
  tools_list: () => toolsList(),
  // No binaries to probe from a browser.
  tool_versions: (): ToolVersion[] => TOOLS.map((meta) => ({ tool: meta.id, version: null })),
  reveal_path: (args) => {
    console.info(`[agency-agents mock] reveal_path ${asString(args, "path")} (no file manager in a browser)`);
    return null;
  },

  // ── Agentfiles ───────────────────────────────────────────
  loadout_export: () => {
    throw needsNativeApp("exporting an Agentfile");
  },
  loadout_import: () => {
    throw needsNativeApp("importing an Agentfile");
  },

  // ── GitHub ───────────────────────────────────────────────
  // Anonymous stats are off by default natively; the mock has no network at all.
  github_repo_stats: () => null,
  github_status: () => ({ signedIn: false, username: null, scopes: [] }),
  github_signin_start: () => {
    throw appError("keychain_unavailable", {
      message: "browser preview has no macOS Keychain, so OAuth device flow is unavailable",
    });
  },
  github_signin_poll: () => {
    throw appError("auth_required");
  },
  github_signout: () => null,
  github_star: () => {
    throw appError("auth_required");
  },
  github_unstar: () => {
    throw appError("auth_required");
  },
  github_watch: () => {
    throw appError("auth_required");
  },
  github_unwatch: () => {
    throw appError("auth_required");
  },
  github_is_starred: () => false,
  github_create_issue: () => {
    throw appError("auth_required");
  },

  // ── Updater ──────────────────────────────────────────────
  // There is no signed manifest to compare against in the preview.
  update_check_now: () => ({ kind: "upToDate" }),
  update_skip: (args) => {
    const version = asString(args, "version");
    const settings = loadSettings();
    saveSettings({ ...settings, skippedUpdateVersions: [...settings.skippedUpdateVersions, version] });
    return null;
  },
  update_install: () => {
    throw needsNativeApp("installing an app update");
  },
  update_relaunch: () => null,

  // ── Plugin: dialog ───────────────────────────────────────
  "plugin:dialog|open": (args) => {
    const options = args.options as { multiple?: boolean } | undefined;
    return options?.multiple ? [MOCK_PROJECT_PATH] : MOCK_PROJECT_PATH;
  },
  "plugin:dialog|save": () => `${MOCK_PROJECT_PATH}/agentfile.json`,
  "plugin:dialog|message": () => null,

  // ── Plugin: opener ───────────────────────────────────────
  "plugin:opener|open_url": (args) => {
    const url = asString(args, "url");
    window.open(url, "_blank", "noopener");
    return null;
  },
  "plugin:opener|open_path": (args) => {
    console.info(`[agency-agents mock] open_path ${asString(args, "path")}`);
    return null;
  },
  "plugin:opener|reveal_item_in_dir": (args) => {
    console.info("[agency-agents mock] reveal_item_in_dir", args.paths);
    return null;
  },
};

/**
 * Build the `invoke()` handler `mockIPC` forwards to.
 *
 * Unknown commands reject loudly instead of resolving `undefined`: a silent
 * `undefined` would look like a UI bug in the preview, while a clear message
 * names the command the shim is missing.
 */
export function createMockInvokeHandler(): (command: string, args?: Args) => unknown {
  return (command, args = {}) => {
    const handler = HANDLERS[command];
    if (!handler) throw invalidArgument(`browser IPC mock has no handler for '${command}'`);
    return handler(args);
  };
}
