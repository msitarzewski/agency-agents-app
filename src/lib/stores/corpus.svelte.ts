/**
 * Corpus store — the in-memory view of the agent catalog and its category
 * taxonomy. Lazy-loads the agent corpus and its category taxonomy
 * via the `corpus_list` / `corpus_get` / `corpus_categories` Tauri commands and
 * exposes `$derived` helpers for the Discover tile grid, search, and the
 * per-agent slide-over.
 *
 * Singleton: import `corpus` from this module everywhere; the store fetches the
 * list + categories once per process and caches the result. Per-agent bodies
 * (`corpus_get`) are fetched on demand and memoised in `bodyCache`.
 *
 * Backend-not-ready posture (contracts.md / api.ts convention): every `invoke`
 * is wrapped in try/catch. On any failure the store lands in an empty-but-valid
 * state (`agents: []`, `categories: []`, `error` set) so the UI builds and
 * renders an empty state instead of crashing — exactly mirroring the
 * `categories` store's soft-fail.
 */

import { invoke } from "@tauri-apps/api/core";

import type { Agent, Category } from "$lib/types";

class CorpusStore {
  /** List-view agents (body omitted by the backend to keep the payload small). */
  agents: Agent[] = $state([]);
  /** The category taxonomy for the 18-tile grid. */
  categories: Category[] = $state([]);
  loading: boolean = $state(false);
  error: string | null = $state(null);

  /** Per-slug full-agent (incl. `body`) cache for the slide-over. */
  private bodyCache = new Map<string, Agent>();
  private loadPromise: Promise<void> | null = null;

  /**
   * Lazy-load the agent list + category taxonomy on first access. Safe to call
   * repeatedly — only fetches once. Soft-fails to an empty state so the UI
   * builds before the backend corpus module is wired (contracts.md §C).
   */
  async ensureLoaded(): Promise<void> {
    if ((this.agents.length > 0 || this.categories.length > 0) && !this.error) {
      return;
    }
    if (this.loadPromise) return this.loadPromise;

    this.loading = true;
    this.error = null;
    this.loadPromise = (async () => {
      try {
        // Parallel: list + categories are independent reads.
        const [agents, categories] = await Promise.all([
          invoke<Agent[]>("corpus_list"),
          invoke<Category[]>("corpus_categories"),
        ]);
        this.agents = agents ?? [];
        this.categories = categories ?? [];
      } catch (e) {
        // Backend may not be implemented yet — degrade to an empty state
        // rather than throwing into the component tree.
        this.error = `Corpus unavailable: ${String(e)}`;
        this.agents = [];
        this.categories = [];
      } finally {
        this.loading = false;
        this.loadPromise = null;
      }
    })();
    return this.loadPromise;
  }

  /**
   * Force a re-fetch of the agent list + taxonomy — used after the catalog
   * SOURCE changes (clone switch / provision / pull), where the cached agents
   * are now stale. Clears caches so `ensureLoaded` actually re-runs.
   */
  async reload(): Promise<void> {
    this.loadPromise = null;
    this.agents = [];
    this.categories = [];
    this.error = null;
    this.bodyCache.clear();
    return this.ensureLoaded();
  }

  /**
   * Full agent (incl. markdown `body`) for the slide-over. Memoised per slug.
   * Soft-fails to `null` so a backend hiccup shows the list-view agent without
   * a body rather than crashing the panel.
   */
  async get(slug: string): Promise<Agent | null> {
    const cached = this.bodyCache.get(slug);
    if (cached) return cached;
    try {
      const agent = await invoke<Agent>("corpus_get", { slug });
      if (agent) this.bodyCache.set(slug, agent);
      return agent ?? null;
    } catch {
      // Fall back to the list-view record (no body) if we have it.
      return this.agents.find((a) => a.slug === slug) ?? null;
    }
  }

  /**
   * Category-keyed agent counts, derived from the loaded list so the tile grid
   * never disagrees with the agents actually present. Keyed by category slug.
   */
  private countsByCategory = $derived.by<Map<string, number>>(() => {
    const counts = new Map<string, number>();
    for (const a of this.agents) {
      counts.set(a.category, (counts.get(a.category) ?? 0) + 1);
    }
    return counts;
  });

  /**
   * Sorted tile list for the Discover grid. Backend-provided `count` wins when
   * present (> 0); otherwise we fall back to the derived list count so tiles
   * stay accurate even before the backend stamps counts. Alphabetical by label
   * for a stable, deterministic scan order.
   */
  tiles = $derived.by<Category[]>(() => {
    const out = this.categories.map((c) => ({
      ...c,
      count: c.count > 0 ? c.count : (this.countsByCategory.get(c.slug) ?? 0),
    }));
    out.sort((a, b) => a.label.localeCompare(b.label));
    return out;
  });

  /** Pretty label for a category slug. Falls back to the slug. */
  labelOf(slug: string): string {
    return this.categories.find((c) => c.slug === slug)?.label ?? slug;
  }

  /** Brand color (hex) for a division slug, from the catalog metadata. Falls
      back to a neutral grey for an unknown slug. */
  colorOf(slug: string): string {
    return this.categories.find((c) => c.slug === slug)?.color ?? "#94A3B8";
  }

  /** Lucide icon NAME for a division slug, from the catalog metadata. Falls
      back to "HelpCircle" (resolveCategoryIcon's own fallback) for unknowns. */
  iconOf(slug: string): string {
    return this.categories.find((c) => c.slug === slug)?.icon ?? "HelpCircle";
  }

  /**
   * Filter the agent list by an optional category slug and a free-text query
   * (matched case-insensitively against name + description + vibe). Returns a
   * deterministic alphabetical-by-name slice so the dense grid is stable.
   */
  filtered(categorySlug: string | null, query: string): Agent[] {
    const q = query.trim().toLowerCase();
    const out = this.agents.filter((a) => {
      if (categorySlug && a.category !== categorySlug) return false;
      if (!q) return true;
      const hay = `${a.name} ${a.description} ${a.vibe ?? ""}`.toLowerCase();
      return hay.includes(q);
    });
    out.sort((a, b) => a.name.localeCompare(b.name));
    return out;
  }
}

export const corpus = new CorpusStore();
