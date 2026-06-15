/**
 * Install store — drives the Phase 2 install/reconcile backend
 * (install_agent / uninstall_agent / installs_reconcile / tools_list).
 *
 * Singleton: import `install` everywhere. `reconcile()` refreshes the
 * cross-tool installed view (the 5-state Library model); `install()` /
 * `uninstall()` mutate then re-reconcile so the UI reflects truth.
 *
 * Backend-not-ready posture (matches corpus store): every invoke is wrapped
 * so a missing command degrades to empty state rather than throwing.
 */
import { invoke } from "@tauri-apps/api/core";

import { activity } from "$lib/stores/activity.svelte";
import { corpus } from "$lib/stores/corpus.svelte";
import type { AgentDiff, InstalledAgent, InstallRecord, InstallState, Tool, ToolInfo, ToolVersion } from "$lib/types";

/** The tools Phase 2 can install to, with display + scope. Mirrors the Rust
    `SUPPORTED` set in `install/mod.rs`. Order = install-menu order. */
export interface ToolDef {
  id: Tool;
  label: string;
  scope: "user" | "project";
}

// Module-level in-flight guard (NOT a class #private field — those can trip up
// Svelte 5's class-$state transform). Coalesces the many on-mount reconcile()
// callers into one heavy scan.
let reconcileInflight: Promise<void> | null = null;

/** Persisted "Install into…" tool selection — remembered across agents/launches. */
const INSTALL_SELECTION_KEY = "agency-agents:install-selection";

export const SUPPORTED_TOOLS: ToolDef[] = [
  { id: "claudeCode", label: "Claude Code", scope: "user" },
  { id: "codex", label: "Codex", scope: "user" },
  { id: "geminiCli", label: "Gemini CLI", scope: "user" },
  { id: "copilot", label: "Copilot", scope: "user" },
  { id: "qwen", label: "Qwen", scope: "user" },
  { id: "cursor", label: "Cursor", scope: "project" },
  { id: "opencode", label: "opencode", scope: "project" },
];

class InstallStore {
  /** Reconciled cross-tool installs (the Library model). */
  installed: InstalledAgent[] = $state([]);
  /** Detected tools + counts (the Tools section). */
  tools: ToolInfo[] = $state([]);
  /** `${slug}:${tool}` currently mid-install/uninstall (for spinners). */
  busy: string | null = $state(null);
  /** True while a reconcile is in flight (drives loading states). */
  reconciling: boolean = $state(false);
  /** True once the first reconcile has completed (so we can tell "empty"
      apart from "not scanned yet"). */
  reconciled: boolean = $state(false);
  /** Tools currently checked in the "Install into…" menu. Persisted so the
      choice is remembered for the next agent and the next launch. */
  selectedTools: Tool[] = $state([]);

  /** Load the remembered tool selection; defaults to Claude Code on first run. */
  loadSelection(): void {
    let parsed: Tool[] = [];
    try {
      const raw = localStorage.getItem(INSTALL_SELECTION_KEY);
      if (raw) {
        const arr = JSON.parse(raw) as unknown;
        if (Array.isArray(arr)) {
          parsed = arr.filter((id): id is Tool => SUPPORTED_TOOLS.some((t) => t.id === id));
        }
      }
    } catch {
      /* ignore */
    }
    this.selectedTools = parsed.length > 0 ? parsed : ["claudeCode"];
  }

  /** Is `tool` checked in the Install-into menu? */
  isSelected(tool: Tool): boolean {
    return this.selectedTools.includes(tool);
  }

  /** Toggle a tool's checked state and persist the selection. */
  toggleSelected(tool: Tool): void {
    const nowSelected = !this.isSelected(tool);
    this.selectedTools = nowSelected
      ? [...this.selectedTools, tool]
      : this.selectedTools.filter((t) => t !== tool);
    try {
      localStorage.setItem(INSTALL_SELECTION_KEY, JSON.stringify(this.selectedTools));
    } catch {
      /* ignore */
    }
    // Journal the default-target switch (purely local; no backend call).
    activity.log({
      action: "switch",
      tool,
      scope: this.scopeOf(tool),
      outcome: "ok",
      detail: nowSelected ? "added as default target" : "removed as default target",
    });
  }

  /**
   * Reconcile installs against disk + corpus. Called from many views on mount,
   * so it COALESCES via a module-level in-flight promise: concurrent callers
   * share one scan (the command reads every installed file + sweeps each tool
   * dir). On error we KEEP the previous result rather than blanking the UI.
   */
  async reconcile(): Promise<void> {
    if (reconcileInflight) return reconcileInflight;
    this.reconciling = true;
    reconcileInflight = (async () => {
      try {
        const result = await invoke<InstalledAgent[]>("installs_reconcile");
        this.installed = result;
        this.reconciled = true;
      } catch {
        // keep prior `installed`; just stop the spinner
      } finally {
        this.reconciling = false;
        reconcileInflight = null;
      }
    })();
    return reconcileInflight;
  }

  async loadTools(): Promise<void> {
    try {
      this.tools = await invoke<ToolInfo[]>("tools_list");
    } catch {
      this.tools = [];
    }
  }

  /** Best-effort detected tool versions (`<bin> --version`), keyed by tool id.
      Populated by `loadVersions()`; absent/unknown tools just don't appear. */
  versions: Record<string, string | null> = $state({});

  /** Probe tool versions in the background (slow-ish; spawns processes). */
  async loadVersions(): Promise<void> {
    try {
      const list = await invoke<ToolVersion[]>("tool_versions");
      const m: Record<string, string | null> = {};
      for (const v of list) m[v.tool] = v.version;
      this.versions = m;
    } catch {
      /* leave prior versions */
    }
  }

  /** Detected version string for a tool, or null if unknown. */
  versionOf(tool: Tool): string | null {
    return this.versions[tool] ?? null;
  }

  /** Reveal a path in the OS file manager (Finder / Explorer / xdg-open). */
  async revealPath(path: string): Promise<void> {
    await invoke("reveal_path", { path });
  }

  /** All installed rows for an agent across tools/projects. */
  forSlug(slug: string): InstalledAgent[] {
    return this.installed.filter((i) => i.slug === slug);
  }

  /** Whether `slug` is installed in `tool` (matching project for project tools). */
  isInstalled(slug: string, tool: Tool, projectPath: string | null = null): boolean {
    return this.installed.some(
      (i) =>
        i.slug === slug &&
        i.tool === tool &&
        (i.projectPath ?? null) === (projectPath ?? null),
    );
  }

  /** The reconciled state for `slug` in `tool` (current/outdated/modified/
      removed/foreign), or null if there's no install on disk. Lets the UI show
      the SAME truth everywhere instead of a flat "installed". */
  stateFor(slug: string, tool: Tool, projectPath: string | null = null): InstallState | null {
    const row = this.installed.find(
      (i) =>
        i.slug === slug &&
        i.tool === tool &&
        (i.projectPath ?? null) === (projectPath ?? null),
    );
    return row?.state ?? null;
  }

  /** Resolve an agent's friendly name from the loaded corpus, if available.
      Returns undefined when the corpus list hasn't loaded the slug — the
      journal then falls back to the slug alone. */
  private agentName(slug: string): string | undefined {
    return corpus.agents.find((a) => a.slug === slug)?.name;
  }

  /** Deployment scope for a tool (user-global vs project-scoped). */
  private scopeOf(tool: Tool): "user" | "project" {
    return SUPPORTED_TOOLS.find((t) => t.id === tool)?.scope ?? "user";
  }

  async install(slug: string, tool: Tool, projectPath: string | null = null): Promise<InstallRecord> {
    this.busy = `${slug}:${tool}`;
    try {
      const rec = await invoke<InstallRecord>("install_agent", { slug, tool, projectPath });
      await this.reconcile();
      void this.loadTools();
      activity.log({
        action: "install",
        agentSlug: slug,
        agentName: this.agentName(slug),
        tool,
        scope: this.scopeOf(tool),
        projectPath: projectPath ?? undefined,
        outcome: "ok",
      });
      return rec;
    } catch (e) {
      activity.log({
        action: "install",
        agentSlug: slug,
        agentName: this.agentName(slug),
        tool,
        scope: this.scopeOf(tool),
        projectPath: projectPath ?? undefined,
        outcome: "error",
        detail: e instanceof Error ? e.message : String(e),
      });
      throw e;
    } finally {
      this.busy = null;
    }
  }

  async uninstall(slug: string, tool: Tool, projectPath: string | null = null): Promise<void> {
    this.busy = `${slug}:${tool}`;
    try {
      await invoke("uninstall_agent", { slug, tool, projectPath });
      await this.reconcile();
      void this.loadTools();
      activity.log({
        action: "uninstall",
        agentSlug: slug,
        agentName: this.agentName(slug),
        tool,
        scope: this.scopeOf(tool),
        projectPath: projectPath ?? undefined,
        outcome: "ok",
      });
    } catch (e) {
      activity.log({
        action: "uninstall",
        agentSlug: slug,
        agentName: this.agentName(slug),
        tool,
        scope: this.scopeOf(tool),
        projectPath: projectPath ?? undefined,
        outcome: "error",
        detail: e instanceof Error ? e.message : String(e),
      });
      throw e;
    } finally {
      this.busy = null;
    }
  }

  /** Update an Outdated install to the current corpus version. */
  async update(slug: string, tool: Tool, projectPath: string | null = null): Promise<void> {
    this.busy = `${slug}:${tool}`;
    try {
      await invoke("update_agent", { slug, tool, projectPath });
      await this.reconcile();
      activity.log({
        action: "update",
        agentSlug: slug,
        agentName: this.agentName(slug),
        tool,
        scope: this.scopeOf(tool),
        projectPath: projectPath ?? undefined,
        outcome: "ok",
      });
    } catch (e) {
      activity.log({
        action: "update",
        agentSlug: slug,
        agentName: this.agentName(slug),
        tool,
        scope: this.scopeOf(tool),
        projectPath: projectPath ?? undefined,
        outcome: "error",
        detail: e instanceof Error ? e.message : String(e),
      });
      throw e;
    } finally {
      this.busy = null;
    }
  }

  /**
   * Track a recognized Foreign install into the ledger NON-DESTRUCTIVELY — the
   * backend records provenance but never writes to the user's file. After this,
   * reconcile shows Current (file already matches the catalog) or Modified (it
   * differs; an explicit Update reconciles it, backing up first).
   */
  async track(slug: string, tool: Tool, projectPath: string | null = null): Promise<void> {
    this.busy = `${slug}:${tool}`;
    try {
      await invoke("track_agent", { slug, tool, projectPath });
      await this.reconcile();
      activity.log({
        action: "track",
        agentSlug: slug,
        agentName: this.agentName(slug),
        tool,
        scope: this.scopeOf(tool),
        projectPath: projectPath ?? undefined,
        outcome: "ok",
      });
    } catch (e) {
      activity.log({
        action: "track",
        agentSlug: slug,
        agentName: this.agentName(slug),
        tool,
        scope: this.scopeOf(tool),
        projectPath: projectPath ?? undefined,
        outcome: "error",
        detail: e instanceof Error ? e.message : String(e),
      });
      throw e;
    } finally {
      this.busy = null;
    }
  }

  /** Diff the on-disk file against the canonical render (review before Update). */
  async diff(slug: string, tool: Tool, projectPath: string | null = null): Promise<AgentDiff> {
    return invoke<AgentDiff>("agent_diff", { slug, tool, projectPath });
  }

  /**
   * Run one action across many installs with a SINGLE reconcile at the end
   * (calling install()/update()/etc. in a loop would reconcile per item). Each
   * target is an existing install row, so project tools already know their dest
   * — no folder prompts. Returns {ok, fail} counts.
   */
  async bulk(
    action: "update" | "track" | "uninstall",
    targets: { slug: string; tool: Tool; projectPath: string | null }[],
  ): Promise<{ ok: number; fail: number }> {
    const cmd =
      action === "uninstall" ? "uninstall_agent" : action === "track" ? "track_agent" : "update_agent";
    let ok = 0;
    let fail = 0;
    for (const t of targets) {
      try {
        await invoke(cmd, { slug: t.slug, tool: t.tool, projectPath: t.projectPath });
        ok++;
      } catch {
        fail++;
      }
    }
    await this.reconcile();
    void this.loadTools();
    // ONE summarizing journal entry for the whole batch (not one per item). An
    // "update" sweep is a Sync; track/uninstall sweeps are generic Bulk ops.
    // `detail` is a self-contained verb phrase so the row reads naturally; no
    // single `tool` since a batch can span tools.
    const plural = (n: number) => `${n} agent${n === 1 ? "" : "s"}`;
    const verb = action === "update" ? "Updated" : action === "track" ? "Tracked" : "Removed";
    activity.log({
      action: action === "update" ? "sync" : "bulk",
      outcome: fail > 0 ? "error" : "ok",
      detail: fail > 0 ? `${verb} ${plural(ok)}, ${fail} failed` : `${verb} ${plural(ok)}`,
    });
    return { ok, fail };
  }

  /** Label for a tool id (for view-models that only have the wire value). */
  toolLabel(tool: Tool): string {
    return SUPPORTED_TOOLS.find((t) => t.id === tool)?.label ?? tool;
  }

  /** Export the current install set to an Agentfile at `path`. Returns count. */
  async exportLoadout(path: string): Promise<number> {
    return invoke<number>("loadout_export", { path });
  }

  /** Restore an Agentfile from `path`; installs each entry. Returns records. */
  async importLoadout(path: string): Promise<InstallRecord[]> {
    const recs = await invoke<InstallRecord[]>("loadout_import", { path });
    await this.reconcile();
    void this.loadTools();
    return recs;
  }
}

export const install = new InstallStore();
