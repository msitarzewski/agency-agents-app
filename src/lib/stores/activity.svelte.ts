/**
 * Activity store — a JOURNAL of discrete agent actions (install, remove,
 * update, track, tool default-target switch, sync, bulk ops).
 *
 * Drives the "Activity" view. This is NOT a live stream: each entry is a
 * single, already-resolved record appended after a backend action returns.
 * Entries are clearable.
 *
 * Persistence: the journal is mirrored to localStorage so the Activity view
 * survives app restarts. The cap keeps the mirror bounded; older entries drop
 * off the tail. On any persist failure we console-warn (NOT a silent swallow)
 * so a regression — quota exhaustion, serialization failure, a webview-side
 * storage policy quirk — is visible during dev/testing instead of presenting
 * as "the activity log silently empties."
 */

import type { Tool } from "$lib/types";

/** Bumped v1 -> v2: the persisted shape changed from streaming jobs to journal
 *  entries. The old v1 store was never populated (no backend emitted stream
 *  events), so there's nothing to migrate. */
const STORAGE_KEY = "agency-agents:activity:v2";
/** Cap how many entries we persist. Older drop off the tail. */
const MAX_ENTRIES = 500;
/** How long to wait after a change before writing to localStorage. */
const PERSIST_DEBOUNCE_MS = 400;

/** A discrete, already-resolved agent action recorded in the journal. */
export interface JournalEntry {
  /** Stable id (crypto.randomUUID). */
  id: string;
  /** ISO timestamp the action resolved. */
  ts: string;
  action: "install" | "uninstall" | "update" | "track" | "switch" | "sync" | "bulk";
  agentSlug?: string;
  agentName?: string;
  tool?: Tool;
  scope?: "user" | "project";
  projectPath?: string;
  outcome: "ok" | "error";
  /** Free-form detail — error message, bulk summary ("3 agents"), etc. */
  detail?: string;
}

interface PersistedShape {
  v: 2;
  entries: JournalEntry[];
}

class ActivityStore {
  /** The journal, newest-first. */
  entries: JournalEntry[] = $state([]);

  private persistTimer: ReturnType<typeof setTimeout> | null = null;
  private hydrated = false;

  /**
   * Restore persisted entries from localStorage. Safe to call multiple times —
   * only the first call hydrates. Should be invoked once during app bootstrap
   * (e.g. from `+layout.svelte`).
   */
  hydrate(): void {
    if (this.hydrated || typeof window === "undefined") return;
    this.hydrated = true;
    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      if (!raw) {
        console.info("[activity] hydrate: no persisted entry (first launch or storage cleared)");
        return;
      }
      const parsed = JSON.parse(raw) as PersistedShape;
      if (!parsed || parsed.v !== 2 || !Array.isArray(parsed.entries)) {
        console.warn("[activity] hydrate: persisted entry has unexpected shape; ignoring");
        return;
      }
      this.entries = parsed.entries;
      console.info(`[activity] hydrate: restored ${parsed.entries.length} entry(ies) from localStorage`);
    } catch (e) {
      console.warn(
        `[activity] hydrate failed (corrupt entry): ${
          e instanceof Error ? e.message : String(e)
        }`,
      );
      try { localStorage.removeItem(STORAGE_KEY); } catch { /* ignore */ }
    }
  }

  /**
   * Schedule a debounced write to localStorage. Coalesces rapid bursts (e.g. a
   * bulk loop that logs once per item) into a single write at most every
   * PERSIST_DEBOUNCE_MS milliseconds.
   */
  private schedulePersist(): void {
    if (typeof window === "undefined") return;
    if (this.persistTimer) clearTimeout(this.persistTimer);
    this.persistTimer = setTimeout(() => {
      this.persistTimer = null;
      this.persistNow();
    }, PERSIST_DEBOUNCE_MS);
  }

  /**
   * Write current entries to localStorage immediately. Caps entry count to keep
   * storage bounded. On failure, logs a warning to the console.
   */
  private persistNow(): void {
    if (typeof window === "undefined") return;
    try {
      const trimmed = this.entries.slice(0, MAX_ENTRIES);
      const payload: PersistedShape = { v: 2, entries: trimmed };
      localStorage.setItem(STORAGE_KEY, JSON.stringify(payload));
    } catch (e) {
      console.warn(
        `[activity] persistNow failed (entries=${this.entries.length}): ${
          e instanceof Error ? e.message : String(e)
        }`,
      );
    }
  }

  /**
   * Record a resolved action. Generates id + ts, prepends (newest-first), and
   * persists. Callers pass everything but `id`/`ts`.
   */
  log(entry: Omit<JournalEntry, "id" | "ts">): void {
    const full: JournalEntry = {
      ...entry,
      id: crypto.randomUUID(),
      ts: new Date().toISOString(),
    };
    this.entries = [full, ...this.entries].slice(0, MAX_ENTRIES);
    // Debounced so a bulk loop's per-item logs coalesce into one write.
    this.schedulePersist();
  }

  /** Wipe the journal including the localStorage mirror. */
  clear(): void {
    this.entries = [];
    this.persistNow();
  }
}

export const activity = new ActivityStore();
