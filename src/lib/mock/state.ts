/**
 * Persisted state for the browser IPC mock.
 *
 * The native app keeps `settings.json` and the install ledger in the OS app-data
 * directory; the browser has no such directory, so the mock keeps the same two
 * documents in `localStorage`. Shapes are unchanged — they are exactly what the
 * Rust `Settings` struct and `installs.json` serialise to — so stores and
 * components read them with their normal code paths.
 *
 * Everything here is dev-only: nothing imports this module outside the mock.
 */

import type { InstallRecord, Settings } from "$lib/types";
import { SETTINGS_DEFAULTS } from "$lib/types";

const SETTINGS_KEY = "agency-agents:mock:settings:v1";
const LEDGER_KEY = "agency-agents:mock:installs:v1";

/**
 * `Settings` plus `skippedUpdateVersions`.
 *
 * The backend always sends `skippedUpdateVersions` (used by `update_skip`), but
 * the renderer's `Settings` interface never declared it — a pre-existing drift
 * noted in `$lib/types`. The mock holds the field so skipping an update round-
 * trips instead of being dropped on the next read.
 */
export type MockSettings = Settings & { skippedUpdateVersions: string[] };

/** Backend defaults, plus the undeclared skipped-versions list. */
export const MOCK_SETTINGS_DEFAULTS: MockSettings = {
  ...SETTINGS_DEFAULTS,
  skippedUpdateVersions: [],
};

function readJson<T>(key: string): T | null {
  try {
    const raw = localStorage.getItem(key);
    return raw ? (JSON.parse(raw) as T) : null;
  } catch {
    // Private mode or a corrupt entry: fall back to defaults.
    return null;
  }
}

function writeJson(key: string, value: unknown): void {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    /* private mode / quota — the preview just forgets on reload */
  }
}

/**
 * Clamp the numeric bounds the backend enforces in `settings.rs` so a value
 * typed into Settings behaves the same way here as natively.
 */
function clampSettings(settings: MockSettings): MockSettings {
  const stale = Number.isFinite(settings.catalogStaleBannerDays) ? settings.catalogStaleBannerDays : 14;
  const ttl = Number.isFinite(settings.trendingTtlMinutes) ? settings.trendingTtlMinutes : 60;
  return {
    ...MOCK_SETTINGS_DEFAULTS,
    ...settings,
    catalogStaleBannerDays: Math.min(365, Math.max(1, Math.round(stale))),
    trendingTtlMinutes: Math.min(1440, Math.max(5, Math.round(ttl))),
    // Backend caps the skip-list at 10 entries (FIFO).
    skippedUpdateVersions: (settings.skippedUpdateVersions ?? []).slice(-10),
  };
}

/** Current settings (defaults on first run / after a reset). */
export function loadSettings(): MockSettings {
  return clampSettings(readJson<MockSettings>(SETTINGS_KEY) ?? MOCK_SETTINGS_DEFAULTS);
}

/** Persist settings and return the canonicalised copy, like `settings_set`. */
export function saveSettings(settings: MockSettings): MockSettings {
  const next = clampSettings(settings);
  writeJson(SETTINGS_KEY, next);
  return next;
}

/** Overwrite with defaults, like `settings_reset`. */
export function resetSettings(): MockSettings {
  writeJson(SETTINGS_KEY, MOCK_SETTINGS_DEFAULTS);
  return MOCK_SETTINGS_DEFAULTS;
}

/** The in-memory stand-in for `installs.json`. Empty until something is installed. */
export function loadLedger(): InstallRecord[] {
  return readJson<InstallRecord[]>(LEDGER_KEY) ?? [];
}

/** Persist the ledger. */
export function saveLedger(rows: InstallRecord[]): void {
  writeJson(LEDGER_KEY, rows);
}

/**
 * Where the mock's file dialogs "pick" a folder.
 *
 * A browser cannot open a native folder picker, so returning a fixed, obviously
 * synthetic path keeps the Projects flow walkable instead of dead-ending on
 * "no selection".
 */
export const MOCK_PROJECT_PATH = "browser-mock/projects/demo-site";
