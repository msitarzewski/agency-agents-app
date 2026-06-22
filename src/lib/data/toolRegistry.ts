/**
 * Tool registry — the SINGLE source of truth for every supported tool.
 *
 * Loaded from `src-tauri/data/tools.json` — a bundled baseline of the canonical
 * catalog the upstream `agency-agents` repo owns (alongside `divisions.json`),
 * read identically by the Rust backend. It carries *upstream truth* (what the
 * CLI converts + installs) and nothing app-specific. Whether THIS app can
 * install a tool is **derived**, not stored: a tool is installable iff we ship a
 * native renderer for its `format` (`IMPLEMENTED_FORMATS`). Brand icons live as
 * SVGs under `assets/tools/`, resolved by the `icon` field.
 */

import catalog from "../../../src-tauri/data/tools.json";

export interface ToolMeta {
  /** camelCase id — the wire value used by the backend + install ledger. */
  id: string;
  /** Full display name, e.g. "Claude Code". */
  label: string;
  /** Compact name for dense UIs (the coverage matrix), e.g. "Claude". */
  short: string;
  /** kebab id matching the CLI install scripts. */
  kebab: string;
  /** Badge accent color (hex). */
  accent: string;
  /** Brand-mark filename stem under assets/tools/ (`claudecode` → claudecode.svg),
      or null to fall back to the letter mark. */
  icon: string | null;
  /** Install-menu position; unset sorts after, by label. */
  order?: number;
  scope?: { user: boolean; project: boolean };
  detect?: { dirs: string[]; agentsDir: string | null };
  version?: { bin: string; args: string[] };
  /** Renderer contract: same `format` ⇒ byte-identical output. */
  format?: string;
  slugFrom?: string | null;
  slugPrefix?: string;
  dest?: { user: string[]; project: string[] };
}

/** The render formats this app implements natively. A tool is installable iff
    its `format` is one of these — derived from the catalog, never stored there. */
const IMPLEMENTED_FORMATS = new Set([
  "identity",
  "codex-toml",
  "gemini-md",
  "qwen-md",
  "cursor-mdc",
  "opencode-md",
  "skill-md",
]);

// Brand marks (Lobe Icons, MIT) — monochrome SVG keyed by filename stem.
const iconMods = import.meta.glob("../assets/tools/*.svg", {
  eager: true,
  query: "?raw",
  import: "default",
}) as Record<string, string>;

const ICONS: Record<string, string> = {};
for (const [path, svg] of Object.entries(iconMods)) {
  const stem = path.split("/").pop()!.replace(/\.svg$/, "");
  ICONS[stem] = svg;
}

const TOOLS_MAP = (catalog as unknown as { tools: Record<string, ToolMeta> }).tools;

/** All tools — by explicit install-menu `order` first, then label. */
export const TOOLS: ToolMeta[] = Object.values(TOOLS_MAP).sort(
  (a, b) => (a.order ?? 999) - (b.order ?? 999) || a.label.localeCompare(b.label),
);

const BY_ID = new Map(TOOLS.map((t) => [t.id, t]));

export function toolMeta(id: string): ToolMeta | null {
  return BY_ID.get(id) ?? null;
}

/** Whether this app ships a native renderer for the tool's format. */
export function isInstallable(t: ToolMeta): boolean {
  return t.format != null && IMPLEMENTED_FORMATS.has(t.format);
}

/** Accent color for a tool's badge (neutral grey fallback). */
export function toolAccent(id: string): string {
  return BY_ID.get(id)?.accent ?? "#8A8F98";
}

/** Full display label (falls back to the raw id). */
export function toolLabel(id: string): string {
  return BY_ID.get(id)?.label ?? id;
}

/** Compact display name for dense UIs (falls back to the label). */
export function toolShort(id: string): string {
  return BY_ID.get(id)?.short ?? toolLabel(id);
}

/** Inline brand-mark SVG (monochrome, currentColor) for a tool, or null. */
export function toolIcon(id: string): string | null {
  const name = BY_ID.get(id)?.icon;
  return name ? ICONS[name] ?? null : null;
}

/** Single-character fallback mark — the label's first letter, uppercased. */
export function toolMark(label: string): string {
  return (label.trim()[0] ?? "?").toUpperCase();
}

/** The tools this app can install (we render their format). */
export function wiredTools(): ToolMeta[] {
  return TOOLS.filter(isInstallable);
}
