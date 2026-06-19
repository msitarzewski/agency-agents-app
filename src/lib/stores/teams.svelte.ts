/**
 * Teams store — user-saved "teams": named, reusable sets of agents you can
 * redeploy or share. Distinct from the live install ledger (that's "your team",
 * reconciled in the install store) and from the app-bundled PRESET_TEAMS.
 *
 * Persistence: localStorage. A saved team is just a name + a list of corpus
 * slugs; deploying it runs the same DeployModal tri-state picker as a division
 * or preset. On any persist failure we console-warn (not silently swallow) so a
 * regression is visible during dev/testing.
 */

const STORAGE_KEY = "agency-agents:teams:v1";

export interface SavedTeam {
  /** Stable id (crypto.randomUUID). */
  id: string;
  name: string;
  /** Corpus agent slugs. */
  agents: string[];
  /** ISO timestamp. */
  createdAt: string;
}

interface PersistedShape {
  v: 1;
  teams: SavedTeam[];
}

class TeamsStore {
  saved: SavedTeam[] = $state([]);
  private hydrated = false;

  hydrate(): void {
    if (this.hydrated || typeof window === "undefined") return;
    this.hydrated = true;
    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      if (!raw) return;
      const parsed = JSON.parse(raw) as PersistedShape;
      if (parsed && parsed.v === 1 && Array.isArray(parsed.teams)) this.saved = parsed.teams;
    } catch (e) {
      console.warn(`[teams] hydrate failed: ${e instanceof Error ? e.message : String(e)}`);
    }
  }

  private persist(): void {
    if (typeof window === "undefined") return;
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify({ v: 1, teams: this.saved } satisfies PersistedShape));
    } catch (e) {
      console.warn(`[teams] persist failed: ${e instanceof Error ? e.message : String(e)}`);
    }
  }

  /** Create a saved team from a set of slugs. Newest first. */
  save(name: string, agents: string[]): SavedTeam {
    const team: SavedTeam = {
      id: crypto.randomUUID(),
      name: name.trim() || "Untitled team",
      agents: [...new Set(agents)],
      createdAt: new Date().toISOString(),
    };
    this.saved = [team, ...this.saved];
    this.persist();
    return team;
  }

  rename(id: string, name: string): void {
    this.saved = this.saved.map((t) => (t.id === id ? { ...t, name: name.trim() || t.name } : t));
    this.persist();
  }

  remove(id: string): void {
    this.saved = this.saved.filter((t) => t.id !== id);
    this.persist();
  }
}

export const teams = new TeamsStore();
