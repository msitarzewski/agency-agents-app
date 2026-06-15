/**
 * Settings store (Phase 12d).
 *
 * Mirrors the backend `settings.json` into the renderer. Holds the
 * three-state load result (loading → loaded | corrupt) so the Settings
 * → Network UI can show a "Reset to defaults" affordance when the file
 * is unreadable on disk.
 *
 * **Single source of truth.** Everywhere the frontend needs to react to
 * a settings value (catalog stale banner threshold, cask icon mode,
 * etc.) it should read from `settings.data` and `$effect`-subscribe
 * rather than re-deriving from localStorage. The on-disk JSON is
 * authoritative; localStorage was only ever a 12b-era stopgap.
 */

import { isAppError, SETTINGS_DEFAULTS, type Settings } from "$lib/types";
import { settingsGet, settingsReset, settingsSet } from "$lib/api";

class SettingsStore {
  /** Authoritative current settings, or `null` until first load. */
  data: Settings | null = $state(null);

  /** True while a load / save / reset is in flight. UI should disable
      mutating controls. */
  loading: boolean = $state(false);

  /** Human-readable error from the most recent operation, if any.
      Cleared by the next successful op. */
  error: string | null = $state(null);

  /** True when the most recent load surfaced a "settings file is
      unreadable" error — the Network section uses this to swap the
      normal controls for a Reset-to-defaults UI. */
  corruptOnDisk: boolean = $state(false);

  /** Load from the backend. Idempotent; safe to call from multiple
      mount points (the Settings modal and the Dashboard catalog
      banner both read these values). */
  async load(): Promise<void> {
    this.loading = true;
    this.error = null;
    try {
      const result = await settingsGet();
      this.data = result;
      this.corruptOnDisk = false;
    } catch (e) {
      if (isAppError(e) && e.code === "internal") {
        // Backend signals "unreadable settings file" via Internal with
        // a message containing "unreadable" — surface the corrupt-on-disk
        // flag so the UI can show the Reset affordance. Falls back to
        // defaults for any reader that needs a value to render.
        this.corruptOnDisk = true;
        this.data = { ...SETTINGS_DEFAULTS };
        this.error = e.message;
      } else if (isAppError(e)) {
        this.error = e.code;
      } else {
        this.error = String(e);
      }
    } finally {
      this.loading = false;
    }
  }

  /** Merge `partial` into the current settings and save. The backend
      returns the canonicalized (clamped) struct, which we then mirror
      back into `data` so subsequent reads see the authoritative values
      — important when the user types e.g. `9999` into a number input
      that gets clamped to `365`. */
  async save(partial: Partial<Settings>): Promise<void> {
    const base: Settings = this.data ?? { ...SETTINGS_DEFAULTS };
    const next: Settings = { ...base, ...partial };

    // Optimistic update so the radio/toggle reflects the click instantly.
    this.data = next;
    this.loading = true;
    this.error = null;
    try {
      const written = await settingsSet(next);
      this.data = written;
      this.corruptOnDisk = false;
    } catch (e) {
      // Revert optimistic update on failure.
      this.data = base;
      if (isAppError(e)) {
        this.error = e.code === "invalid_argument" ? e.message : e.code;
      } else {
        this.error = String(e);
      }
    } finally {
      this.loading = false;
    }
  }

  /** Overwrite settings.json with defaults. Used by the corrupt-file
      recovery path in Settings → Network. */
  async reset(): Promise<void> {
    this.loading = true;
    this.error = null;
    try {
      const fresh = await settingsReset();
      this.data = fresh;
      this.corruptOnDisk = false;
    } catch (e) {
      if (isAppError(e)) {
        this.error = e.code;
      } else {
        this.error = String(e);
      }
    } finally {
      this.loading = false;
    }
  }

  /** Convenience accessor used by other stores that want a defaulted
      view ("use loaded values if available, fall back to defaults"). */
  get effective(): Settings {
    return this.data ?? SETTINGS_DEFAULTS;
  }
}

export const settings = new SettingsStore();
