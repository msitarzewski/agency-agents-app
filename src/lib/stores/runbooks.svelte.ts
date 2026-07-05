/**
 * Runbooks store — the NEXUS scenario runbooks from the catalog's
 * `strategy/runbooks.json` (catalog PR #664). Each runbook names its roster BY
 * SLUG; the UI resolves those against the loaded corpus to deploy the set.
 *
 * `strategy/` only ships in a synced catalog (not the bundled snapshot), so an
 * empty list is the normal "not synced yet" state — the UI shows a
 * "sync to unlock" nudge rather than an error. Backend-not-ready posture matches
 * the corpus/install stores: a failed invoke degrades to empty.
 *
 * Singleton: import `runbooks` everywhere.
 */
import { invoke } from "@tauri-apps/api/core";

import type { Runbook } from "$lib/types";

class RunbooksStore {
  /** The scenario runbooks, in manifest order. Empty until loaded / when unsynced. */
  list: Runbook[] = $state([]);
  /** True once the first load resolves (so "empty" ≠ "not fetched yet"). */
  loaded: boolean = $state(false);
  /** True while a load is in flight. */
  loading: boolean = $state(false);

  /** Load the manifest from the active catalog. Safe to call on mount. */
  async load(): Promise<void> {
    if (this.loading) return;
    this.loading = true;
    try {
      this.list = await invoke<Runbook[]>("runbooks_list");
    } catch {
      this.list = []; // backend not ready / no manifest → empty
    } finally {
      this.loaded = true;
      this.loading = false;
    }
  }
}

export const runbooks = new RunbooksStore();
