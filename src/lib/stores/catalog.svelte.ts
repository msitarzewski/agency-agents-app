/**
 * Catalog store — drives the catalog-source layer (#1 clone-as-source-of-truth):
 * which on-disk catalog the app reads from, detection of existing clones, and
 * provisioning/pulling. Wraps the `catalog_*` Tauri commands.
 *
 * Posture: the app is a respectful frontend. We default to the bundled snapshot,
 * we never pull a user clone without `manage` permission, and every switch is an
 * explicit user action. After any source change we `corpus.reload()` so the
 * Agents grid reflects the newly-active catalog.
 *
 * Backend-not-ready posture (matches corpus/install stores): every invoke is
 * wrapped so a missing command degrades gracefully rather than throwing.
 */
import { invoke } from "@tauri-apps/api/core";

import { i18n } from "$lib/i18n.svelte";
import type {
  CatalogDetection,
  CatalogSource,
  CatalogStatus,
  CatalogUpdateCheck,
  CorpusMeta,
} from "$lib/types";
import { corpus } from "$lib/stores/corpus.svelte";

class CatalogStore {
  /** The active catalog source (default bundled until loaded). */
  source: CatalogSource = $state({ kind: "bundled" });
  /** Whether the user has made an explicit choice yet (drives first-run). */
  configured: boolean = $state(true);
  /** Live status of the active catalog (git commit/branch/freshness). */
  status: CatalogStatus | null = $state(null);
  /** Last upstream update-check result (behind/ahead + diffstat). */
  updateCheck: CatalogUpdateCheck | null = $state(null);
  /** True while a network update-check (`git fetch`) is in flight. */
  checking: boolean = $state(false);
  /** Last detection result (null until `detect` runs). */
  detection: CatalogDetection | null = $state(null);
  /** True while a provision/pull/switch is in flight. */
  busy: boolean = $state(false);
  /** True specifically while scanning the disk for clones. */
  scanning: boolean = $state(false);
  /** Last error surfaced to the UI (cleared on the next action). */
  error: string | null = $state(null);

  /** Load the persisted source + configured flag. Safe to call on mount. */
  async load(): Promise<void> {
    try {
      const [source, configured] = await Promise.all([
        invoke<CatalogSource>("catalog_source_get"),
        invoke<boolean>("catalog_configured"),
      ]);
      this.source = source;
      this.configured = configured;
    } catch {
      // leave defaults (bundled / configured) so the app still runs
    }
  }

  /** Load live catalog status (source, git provenance, freshness). Local-only. */
  async loadStatus(): Promise<void> {
    try {
      this.status = await invoke<CatalogStatus>("catalog_status");
    } catch (e) {
      this.error = String(e);
    }
  }

  /** Check upstream for updates (git fetch + diff stats). Network. */
  async checkUpdates(): Promise<void> {
    this.checking = true;
    this.error = null;
    try {
      this.updateCheck = await invoke<CatalogUpdateCheck>("catalog_check_updates");
    } catch (e) {
      this.error = String(e);
    } finally {
      this.checking = false;
    }
  }

  /** Detect candidate catalogs. `scan` walks common dev roots ("Find" button). */
  async detect(scan = false): Promise<void> {
    this.error = null;
    if (scan) this.scanning = true;
    try {
      this.detection = await invoke<CatalogDetection>("catalog_detect", { scan });
    } catch (e) {
      this.error = String(e);
    } finally {
      this.scanning = false;
    }
  }

  /** Switch to an explicit source, persist it, and reload the corpus. */
  async setSource(source: CatalogSource): Promise<void> {
    this.busy = true;
    this.error = null;
    try {
      await invoke<CorpusMeta>("catalog_source_set", { source });
      this.source = source;
      this.configured = true;
      this.updateCheck = null;
      await corpus.reload();
      await this.loadStatus();
    } catch (e) {
      this.error = String(e);
      throw e;
    } finally {
      this.busy = false;
    }
  }

  /** Use the bundled snapshot (the always-works default). */
  async useBundled(): Promise<void> {
    return this.setSource({ kind: "bundled" });
  }

  /** Adopt the user's own clone. `manage` = permission to pull it later. */
  async useClone(path: string, manage: boolean): Promise<void> {
    return this.setSource({ kind: "userClone", path, manage });
  }

  /** Provision `~/.agency-agents` (clone or snapshot), then make it active. */
  async provisionManaged(): Promise<void> {
    this.busy = true;
    this.error = null;
    try {
      await invoke<CorpusMeta>("catalog_provision_managed");
      this.configured = true;
      this.updateCheck = null;
      await this.load();
      await corpus.reload();
      await this.loadStatus();
    } catch (e) {
      this.error = String(e);
      throw e;
    } finally {
      this.busy = false;
    }
  }

  /** Pull the active catalog up to date (git pull or tarball refresh). */
  async pull(): Promise<void> {
    this.busy = true;
    this.error = null;
    try {
      await invoke<CorpusMeta>("catalog_pull");
      this.updateCheck = null;
      await corpus.reload();
      await this.loadStatus();
    } catch (e) {
      this.error = String(e);
      throw e;
    } finally {
      this.busy = false;
    }
  }

  /** Human label for the active source (for headers/badges). */
  get sourceLabel(): string {
    switch (this.source.kind) {
      case "bundled":
        return i18n.t("catalog.source.bundled");
      case "managed":
        return i18n.t("catalog.source.managed");
      case "userClone":
        return this.source.manage ? i18n.t("catalog.source.userManaged") : i18n.t("catalog.source.userReadOnly");
    }
  }

  /** Active source path, or null for the bundled snapshot. */
  get sourcePath(): string | null {
    return this.source.kind === "bundled" ? null : this.source.path;
  }
}

export const catalog = new CatalogStore();
