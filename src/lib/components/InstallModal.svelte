<script lang="ts">
  /**
   * InstallModal — the ONE place agents get deployed. A destinations × tools
   * grid: rows are destinations (Global + each registered project), columns are
   * the detected tools, and every cell is a toggle that installs/removes the
   * agent set into that (scope, tool).
   *
   * Driven by an agent SET (`agentSlugs`) so it serves a single agent (from the
   * detail pane), a whole division, or a team — same component. For a single
   * agent a cell is on/off; for a set it's tri-state (all / some / none), and
   * toggling fills the missing ones or removes the whole set.
   *
   * "Global" only offers user-capable tools (Cursor's global cell is blank — its
   * global rules are UI-only). Removal of `foreign` files asks first.
   */
  import { onMount } from "svelte";
  import FolderPlus from "@lucide/svelte/icons/folder-plus";
  import FolderIcon from "@lucide/svelte/icons/folder";
  import GlobeIcon from "@lucide/svelte/icons/globe";
  import Modal from "./Modal.svelte";
  import Button from "./Button.svelte";
  import DestructiveConfirm from "./DestructiveConfirm.svelte";
  import { corpus } from "$lib/stores/corpus.svelte";
  import { install, SUPPORTED_TOOLS, type ToolDef } from "$lib/stores/install.svelte";
  import { projects } from "$lib/stores/projects.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import { i18n } from "$lib/stores/i18n.svelte";
  import type { Tool, Agent, InstalledAgent } from "$lib/types";

  interface Props {
    title: string;
    agentSlugs: string[];
    onClose: () => void;
  }
  let { title, agentSlugs, onClose }: Props = $props();

  onMount(() => {
    projects.refresh();
    // Refresh detection so the columns reflect tools ACTUALLY on this device.
    void install.loadTools();
  });

  // The agents in this set that exist in the corpus (stale slugs skipped).
  const slugSet = $derived(new Set(agentSlugs));
  const agents = $derived<Agent[]>(corpus.agents.filter((a) => slugSet.has(a.slug)));
  const total = $derived(agents.length);

  // Columns = tools present on this device (detected, or already holding an
  // install of this set), that can take an agent in SOME scope.
  function detected(t: ToolDef): boolean {
    return (
      install.tools.length === 0 ||
      install.tools.some((ti) => ti.tool === t.id && ti.detected) ||
      install.installed.some((r) => r.tool === t.id && r.state !== "removed" && slugSet.has(r.slug))
    );
  }
  const cols = $derived(SUPPORTED_TOOLS.filter((t) => (t.supportsUser || t.supportsProject) && detected(t)));

  // Rows = Global + each registered/used project.
  type Row = { kind: "global" } | { kind: "project"; path: string; label: string };
  const rows = $derived<Row[]>([
    { kind: "global" },
    ...projects.list.map((p) => ({ kind: "project" as const, path: p.path, label: p.label })),
  ]);

  function targetOf(row: Row): string | null {
    return row.kind === "global" ? null : row.path;
  }
  function applicable(row: Row, t: ToolDef): boolean {
    return row.kind === "global" ? t.supportsUser : t.supportsProject;
  }

  // Coverage of the set in one (tool, target) cell.
  function cover(tool: Tool, target: string | null) {
    const rs = install.installed.filter(
      (r) => r.state !== "removed" && slugSet.has(r.slug) && r.tool === tool && (r.projectPath ?? null) === target,
    );
    const present = new Set(rs.map((r) => r.slug));
    return {
      rows: rs,
      count: present.size,
      all: total > 0 && present.size === total,
      some: present.size > 0 && present.size < total,
      hasForeign: rs.some((r) => r.state === "foreign"),
    };
  }

  let busy = $state<string | null>(null);
  const cellKey = (tool: Tool, target: string | null) => `${tool}:${target ?? ""}`;
  let confirm = $state<{ tool: Tool; target: string | null; rows: InstalledAgent[] } | null>(null);

  async function toggle(tool: Tool, target: string | null) {
    if (busy) return;
    const cov = cover(tool, target);
    if (cov.all) {
      if (cov.hasForeign) {
        confirm = { tool, target, rows: cov.rows };
        return;
      }
      await remove(tool, target, cov.rows);
      return;
    }
    const present = new Set(cov.rows.map((r) => r.slug));
    const missing = agents.filter((a) => !present.has(a.slug));
    if (missing.length === 0) return;
    busy = cellKey(tool, target);
    try {
      const { ok, fail } = await install.bulk(
        "install",
        missing.map((a) => ({ slug: a.slug, tool, projectPath: target })),
      );
      const where = target ? labelOf(target) : i18n.t("common.global");
      if (fail === 0) toast.success(i18n.t("install.installedToast", { count: ok, tool: install.toolLabel(tool), where }));
      else toast.error(i18n.t("install.installFailedToast", { tool: install.toolLabel(tool), ok, fail }));
    } finally {
      busy = null;
    }
  }

  async function remove(tool: Tool, target: string | null, rs: InstalledAgent[]) {
    busy = cellKey(tool, target);
    try {
      const { ok, fail } = await install.bulk(
        "uninstall",
        rs.map((r) => ({ slug: r.slug, tool: r.tool, projectPath: r.projectPath })),
      );
      if (fail === 0) toast.success(i18n.t("install.removedToast", { count: ok, tool: install.toolLabel(tool) }));
      else toast.error(i18n.t("install.removeFailedToast", { tool: install.toolLabel(tool), ok, fail }));
    } finally {
      busy = null;
    }
  }

  async function confirmRemove() {
    if (!confirm) return;
    const { tool, target, rows: rs } = confirm;
    confirm = null;
    await remove(tool, target, rs);
  }

  function labelOf(path: string): string {
    return path.replace(/\/+$/, "").split("/").pop() || path;
  }

  // ── "Add project…" popover ──
  // Instead of firing a native folder dialog on click (which feels broken in the
  // dev shim and is easy to spam), open a list of the projects you already manage
  // — clicking one jumps to its grid row — with an explicit "New Project…" that
  // opens the picker only when you mean it.
  let addOpen = $state(false);
  let flashPath = $state<string | null>(null);
  const destEls: Record<string, HTMLElement> = {};

  function regDest(node: HTMLElement, path: string | null) {
    if (path) destEls[path] = node;
    return { destroy() { if (path) delete destEls[path]; } };
  }

  function flash(path: string) {
    flashPath = path;
    setTimeout(() => { if (flashPath === path) flashPath = null; }, 1200);
  }

  function jumpTo(path: string) {
    addOpen = false;
    destEls[path]?.scrollIntoView({ block: "nearest" });
    flash(path);
  }

  async function newProject() {
    addOpen = false;
    const p = await projects.addViaPicker();
    if (p) {
      await projects.refresh();
      flash(p);
    }
  }
</script>

<Modal open {title} size="wide" onClose={onClose}>
  <p class="sub">{i18n.t("install.sub", { count: total })}</p>

  {#if cols.length === 0}
    <p class="no-tools">{i18n.t("install.noTools")}</p>
  {:else}
  <div class="grid-wrap">
  <div class="grid" style="--cols: {cols.length}">
    <!-- header row -->
    <div class="cell head corner"></div>
    {#each cols as t (t.id)}
      <div class="cell head tool" title={t.label}>{t.label}</div>
    {/each}

    {#each rows as row (row.kind === "global" ? "global" : row.path)}
      <div class="cell dest" class:flash={flashPath !== null && targetOf(row) === flashPath} use:regDest={targetOf(row)}>
        {#if row.kind === "global"}
          <span class="d-ic"><GlobeIcon size={15} /></span>
          <span class="d-body"><span class="d-label">{i18n.t("common.global")}</span><span class="d-path">{i18n.t("common.everyMachine")}</span></span>
        {:else}
          <span class="d-ic"><FolderIcon size={15} /></span>
          <span class="d-body"><span class="d-label">{row.label}</span><span class="d-path" title={row.path}>{row.path}</span></span>
        {/if}
      </div>
      {#each cols as t (t.id)}
        {#if applicable(row, t)}
          {@const cov = cover(t.id, targetOf(row))}
          {@const isBusy = busy === cellKey(t.id, targetOf(row))}
          <button
            class="cell toggle"
            class:on={cov.all}
            class:partial={cov.some}
            disabled={isBusy || total === 0}
            title={i18n.t("install.cellTitle", { tool: t.label, target: row.kind === "global" ? i18n.t("common.global") : row.label })}
            aria-label={i18n.t(cov.all ? "install.removeFromAria" : "install.installIntoAria", { tool: t.label, target: row.kind === "global" ? i18n.t("install.globally") : i18n.t("install.inProject", { project: row.label }) })}
            onclick={() => toggle(t.id, targetOf(row))}
          >
            {#if isBusy}<span class="dot busy"></span>
            {:else if cov.all}<span class="dot full"></span>
            {:else if cov.some}<span class="dot half"></span>
            {:else}<span class="dot"></span>{/if}
          </button>
        {:else}
          <div class="cell na">—</div>
        {/if}
      {/each}
    {/each}
  </div>
  </div>
  {/if}

  <div class="add-wrap">
    <button class="addrow" onclick={() => (addOpen = !addOpen)} aria-haspopup="menu" aria-expanded={addOpen}>
      <FolderPlus size={14} /> {i18n.t("install.addProject")}
    </button>
    {#if addOpen}
      <button class="add-scrim" aria-label={i18n.t("common.close")} onclick={() => (addOpen = false)}></button>
      <div class="add-menu" role="menu">
        {#if projects.list.length > 0}
          <p class="add-head">{i18n.t("install.yourProjects")}</p>
          {#each projects.list as p (p.path)}
            <button class="add-opt" role="menuitem" onclick={() => jumpTo(p.path)}>
              <FolderIcon size={14} />
              <span class="add-body">
                <span class="add-label">{p.label}</span>
                <span class="add-path" title={p.path}>{p.path}</span>
              </span>
            </button>
          {/each}
          <div class="add-div"></div>
        {/if}
        <button class="add-opt new" role="menuitem" onclick={newProject}>
          <FolderPlus size={14} /> <span>{i18n.t("install.newProject")}</span>
        </button>
      </div>
    {/if}
  </div>

  {#snippet actions()}
    <span class="legend"><span class="dot full"></span> {i18n.t("common.installed")} <span class="dot half"></span> {i18n.t("common.some")} <span class="dot"></span> {i18n.t("common.none")}</span>
    <Button variant="primary" onclick={onClose}>{i18n.t("common.done")}</Button>
  {/snippet}
</Modal>

{#if confirm}
  {@const n = confirm.rows.length}
  {@const label = install.toolLabel(confirm.tool)}
  <DestructiveConfirm
    open
    title={i18n.t("install.deleteTitle", { count: n, label })}
    confirmLabel={i18n.t("install.deleteConfirm", { count: n })}
    cancelLabel={i18n.t("common.cancel")}
    onConfirm={confirmRemove}
    onCancel={() => (confirm = null)}
  >
    <p>
      {i18n.t("install.deleteBody", { count: n })}
    </p>
  </DestructiveConfirm>
{/if}

<style>
  .sub { font-size: var(--text-body-sm); color: var(--color-text-muted); margin-bottom: var(--space-3); }
  .no-tools { font-size: var(--text-body-sm); color: var(--color-text-muted); }

  /* Horizontal scroll guards against a very wide tool set (≫ the modal width). */
  .grid-wrap { overflow-x: auto; border: 1px solid var(--color-border); border-radius: var(--radius-md); }
  .grid {
    display: grid;
    grid-template-columns: minmax(180px, 1fr) repeat(var(--cols), 68px);
    width: max-content; min-width: 100%;
    align-items: stretch;
  }
  .cell { display: flex; align-items: center; justify-content: center; padding: var(--space-2); border-bottom: 1px solid var(--color-border); }
  .head { background: var(--color-surface-sunken); font-size: var(--text-caption); color: var(--color-text-muted); font-weight: var(--fw-semibold); min-height: 34px; }
  .head.tool { writing-mode: horizontal-tb; text-align: center; padding: var(--space-2) 8px; line-height: 1.15; }
  .corner { background: var(--color-surface-sunken); }

  .dest { justify-content: flex-start; gap: var(--space-2); min-width: 0; }
  .d-ic { flex: none; display: inline-flex; color: var(--color-text-secondary); }
  .d-body { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 0; }
  .d-label { font-size: var(--text-body-sm); font-weight: var(--fw-medium); color: var(--color-text-primary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .d-path { font-size: var(--text-caption); color: var(--color-text-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

  .toggle { background: transparent; cursor: pointer; }
  .toggle:hover:not(:disabled) { background: var(--color-surface-sunken); }
  .toggle:disabled { cursor: default; }
  .na { color: var(--color-text-muted); opacity: 0.4; }

  .dot { width: 16px; height: 16px; border-radius: 999px; border: 1.5px solid var(--color-border-strong, var(--color-text-muted)); box-sizing: border-box; }
  .dot.full { background: var(--color-brand); border-color: var(--color-brand); }
  .dot.half { border-color: var(--color-brand); background: linear-gradient(90deg, var(--color-brand) 50%, transparent 50%); }
  .dot.busy { border-color: var(--color-text-muted); border-top-color: transparent; animation: spin 0.6s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }

  .add-wrap { position: relative; display: inline-block; margin-top: var(--space-2); }
  .addrow {
    display: inline-flex; align-items: center; gap: 6px;
    padding: var(--space-2);
    background: transparent; color: var(--color-brand); font-size: var(--text-body-sm); cursor: pointer;
  }
  .addrow:hover { text-decoration: underline; }

  /* Backdrop closes the popover on any outside click. */
  .add-scrim { position: fixed; inset: 0; z-index: 1; background: transparent; border: 0; cursor: default; }
  .add-menu {
    position: absolute; bottom: calc(100% + 4px); left: 0; z-index: 2;
    min-width: 260px; max-width: 360px; max-height: 280px; overflow-y: auto; padding: 4px;
    background: var(--color-surface-raised); border: 1px solid var(--color-border);
    border-radius: var(--radius-md); box-shadow: var(--shadow-lg);
    display: flex; flex-direction: column; gap: 1px;
  }
  .add-head {
    padding: 6px 8px 2px; font-size: var(--text-caption); font-weight: var(--fw-semibold);
    color: var(--color-text-muted); text-transform: uppercase; letter-spacing: 0.04em;
  }
  .add-opt {
    display: flex; align-items: center; gap: var(--space-2);
    padding: 6px 8px; border-radius: var(--radius-sm);
    background: transparent; color: var(--color-text-primary);
    font-size: var(--text-body-sm); text-align: left; cursor: pointer; min-width: 0;
  }
  .add-opt:hover { background: var(--color-surface-sunken); }
  .add-opt.new { color: var(--color-brand); font-weight: var(--fw-medium); }
  .add-body { display: flex; flex-direction: column; gap: 0; min-width: 0; }
  .add-label { font-weight: var(--fw-medium); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .add-path { font-size: var(--text-caption); color: var(--color-text-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .add-div { height: 1px; margin: 4px 0; background: var(--color-border); }

  .dest.flash { animation: destFlash 1.2s var(--motion-ease-out, ease-out); }
  @keyframes destFlash {
    0%, 100% { background: transparent; }
    25% { background: var(--color-brand-subtle, color-mix(in srgb, var(--color-brand) 16%, transparent)); }
  }

  .legend { display: inline-flex; align-items: center; gap: 6px; margin-right: auto; font-size: var(--text-caption); color: var(--color-text-muted); }
  .legend .dot { width: 13px; height: 13px; }
</style>
