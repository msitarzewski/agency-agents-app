/**
 * Dev-only corpus fixture for the browser IPC mock.
 *
 * The Rust backend owns the real catalog: it reads the bundled baseline out of
 * `src-tauri/resources/corpus-baseline`, parses each `.md`, and serves the
 * result over IPC. In a plain browser there is no backend, so this module does
 * the same read at build time: Vite inlines the baseline `.md` files as raw
 * strings, and we parse them with the SAME rules as
 * `src-tauri/src/corpus/parse.rs` (fenced frontmatter, `name` required, body =
 * everything after the closing fence) so the browser preview shows the real
 * catalog instead of a hand-written fixture.
 *
 * Division labels/icons/colors come straight from
 * `src-tauri/data/agency-categories.json` — the same file the backend reads, so
 * nothing here duplicates catalog truth.
 *
 * Not shipped: only reachable from `src/hooks.client.ts` behind
 * `import.meta.env.DEV`, and Rollup drops the branch in production builds.
 */

import categoriesJson from "../../../src-tauri/data/agency-categories.json";
import type { Agent, Category, CorpusMeta } from "$lib/types";

/** Marks where the baseline root ends in a glob key, e.g. `…/corpus-baseline/engineering/x.md`. */
const BASELINE_MARKER = "/corpus-baseline/";

/**
 * Every agent `.md` in the bundled baseline. `eager` because the catalog is
 * needed on first paint; 280-odd small files is a few hundred KB in dev only.
 */
const RAW_AGENT_FILES = import.meta.glob(
  "../../../src-tauri/resources/corpus-baseline/**/*.md",
  { eager: true, query: "?raw", import: "default" },
) as Record<string, string>;

/** The three canonical slices of an agent file (contracts.md §E). */
interface SplitSource {
  /** Frontmatter YAML, between the fences (no `---` lines). */
  frontmatter: string;
  /** Persona body, after the closing fence. */
  body: string;
}

/**
 * Split a raw `.md` into frontmatter + body, exactly like
 * `split_frontmatter` in `corpus/parse.rs`. Returns `null` when there is no
 * well-formed `---` block at the head of the file (not an agent — e.g. a
 * README), tolerating a leading BOM and trailing whitespace on fence lines.
 */
function splitFrontmatter(source: string): SplitSource | null {
  const text = source.startsWith("\uFEFF") ? source.slice(1) : source;

  const firstLineEnd = text.indexOf("\n");
  if (firstLineEnd === -1) return null;
  if (text.slice(0, firstLineEnd).trimEnd() !== "---") return null;

  let searchFrom = firstLineEnd + 1;
  while (searchFrom <= text.length) {
    const lineEndAt = text.indexOf("\n", searchFrom);
    const lineEnd = lineEndAt === -1 ? text.length : lineEndAt;
    if (text.slice(searchFrom, lineEnd).trimEnd() === "---") {
      return {
        frontmatter: text.slice(firstLineEnd + 1, searchFrom),
        body: lineEndAt === -1 ? "" : text.slice(lineEndAt + 1),
      };
    }
    // EOF without a closing fence — malformed, not an agent.
    if (lineEndAt === -1) return null;
    searchFrom = lineEndAt + 1;
  }
  return null;
}

/**
 * Parse the flat `key: value` frontmatter the catalog actually uses.
 *
 * The backend runs `serde_yaml`; every agent file in the baseline carries only
 * single-line scalars, so a tolerant line parser covers the real corpus without
 * pulling a YAML dependency into the app bundle. Unknown keys are ignored and a
 * blank value is treated as absent, matching the Rust `Option<String>` fields.
 */
function parseFrontmatter(frontmatter: string): Record<string, string> {
  const fields: Record<string, string> = {};
  for (const rawLine of frontmatter.split("\n")) {
    const line = rawLine.trim();
    if (!line || line.startsWith("#")) continue;
    const colon = line.indexOf(":");
    if (colon <= 0) continue;
    const key = line.slice(0, colon).trim();
    let value = line.slice(colon + 1).trim();
    const quoted =
      value.length > 1 &&
      ((value.startsWith('"') && value.endsWith('"')) || (value.startsWith("'") && value.endsWith("'")));
    if (quoted) value = value.slice(1, -1);
    if (!key || !value) continue;
    fields[key] = value;
  }
  return fields;
}

/** Parse the bundled baseline into agents, skipping non-agent files. */
function buildAgents(): Agent[] {
  const agents: Agent[] = [];

  for (const [path, source] of Object.entries(RAW_AGENT_FILES)) {
    const markerAt = path.indexOf(BASELINE_MARKER);
    if (markerAt === -1) continue;

    // Agent files live exactly one directory deep: `<division>/<slug>.md`.
    const segments = path.slice(markerAt + BASELINE_MARKER.length).split("/");
    if (segments.length !== 2) continue;
    const [category, file] = segments;
    if (!file.endsWith(".md")) continue;

    const split = splitFrontmatter(source);
    if (!split) continue;

    const fields = parseFrontmatter(split.frontmatter);
    const name = (fields.name ?? "").trim();
    // `name` is the only required field — without it the file is not an agent.
    if (!name) continue;

    agents.push({
      slug: file.slice(0, -".md".length),
      name,
      description: fields.description ?? "",
      category,
      emoji: fields.emoji ?? null,
      color: fields.color ?? null,
      vibe: fields.vibe ?? null,
      body: split.body,
    });
  }

  // Slug order mirrors the backend's BTreeMap index, so the list is stable.
  agents.sort((a, b) => a.slug.localeCompare(b.slug));
  return agents;
}

/** Every agent in the bundled baseline, bodies included. */
export const MOCK_AGENTS: Agent[] = buildAgents();

const AGENTS_BY_SLUG = new Map(MOCK_AGENTS.map((agent) => [agent.slug, agent]));

/** Full agent (incl. `body`), or `null` when the slug isn't in the baseline. */
export function mockAgent(slug: string): Agent | null {
  return AGENTS_BY_SLUG.get(slug) ?? null;
}

/** Division metadata the backend ships as the bundled floor. */
const DIVISIONS = (
  categoriesJson as { categories: Record<string, { label: string; icon: string; color: string }> }
).categories;

const AGENTS_PER_DIVISION = new Map<string, number>();
for (const agent of MOCK_AGENTS) {
  AGENTS_PER_DIVISION.set(agent.category, (AGENTS_PER_DIVISION.get(agent.category) ?? 0) + 1);
}

/** The division taxonomy, in the metadata file's (alphabetical) order. */
export const MOCK_CATEGORIES: Category[] = Object.entries(DIVISIONS).map(([slug, meta]) => ({
  slug,
  label: meta.label,
  icon: meta.icon,
  color: meta.color,
  count: AGENTS_PER_DIVISION.get(slug) ?? 0,
}));

/** Provenance for the bundled snapshot — the browser has no git checkout. */
export const MOCK_CORPUS_META: CorpusMeta = {
  version: "baseline",
  commit: null,
  fetchedAt: new Date().toISOString(),
  count: MOCK_AGENTS.length,
};
