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
      const where = target ? labelOf(target) : "Global";
      if (fail === 0) toast.success(`Installed ${ok} → ${install.toolLabel(tool)} · ${where}`);
      else toast.error(`${install.toolLabel(tool)}: ${ok} installed, ${fail} failed`);
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
      if (fail === 0) toast.success(`Removed ${ok} from ${install.toolLabel(tool)}`);
      else toast.error(`${install.toolLabel(tool)}: ${ok} removed, ${fail} failed`);
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

  async function addProject() {
    const p = await projects.addViaPicker();
    if (p) await projects.refresh();
  }
</script>

<Modal open {title} size="wide" onClose={onClose}>
  <p class="sub">{total} agent{total === 1 ? "" : "s"} · toggle a cell to install into that tool, globally or per project.</p>

  {#if cols.length === 0}
    <p class="no-tools">No supported tools detected on this device. Open <strong>Tools</strong> to check what's installed.</p>
  {:else}
  <div class="grid-wrap">
  <div class="grid" style="--cols: {cols.length}">
    <!-- header row -->
    <div class="cell head corner"></div>
    {#each cols as t (t.id)}
      <div class="cell head tool" title={t.label}>{t.label}</div>
    {/each}

    {#each rows as row (row.kind === "global" ? "global" : row.path)}
      <div class="cell dest">
        {#if row.kind === "global"}
          <span class="d-ic"><GlobeIcon size={15} /></span>
          <span class="d-body"><span class="d-label">Global</span><span class="d-path">Every machine</span></span>
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
            title={`${t.label} · ${row.kind === "global" ? "Global" : row.label}`}
            aria-label={`${cov.all ? "Remove from" : "Install into"} ${t.label} ${row.kind === "global" ? "globally" : "in " + row.label}`}
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

  <button class="addrow" onclick={addProject}><FolderPlus size={14} /> Add project…</button>

  {#snippet actions()}
    <span class="legend"><span class="dot full"></span> installed <span class="dot half"></span> some <span class="dot"></span> none</span>
    <Button variant="primary" onclick={onClose}>Done</Button>
  {/snippet}
</Modal>

{#if confirm}
  {@const n = confirm.rows.length}
  {@const label = install.toolLabel(confirm.tool)}
  <DestructiveConfirm
    open
    title="Delete {n} file{n === 1 ? '' : 's'} from {label}?"
    confirmLabel="Delete {n}"
    onConfirm={confirmRemove}
    onCancel={() => (confirm = null)}
  >
    <p>
      This <strong>permanently removes {n} file{n === 1 ? "" : "s"} from disk</strong>,
      <strong>including files installed outside this app</strong>. Any edits you made are backed up first.
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

  .addrow {
    display: inline-flex; align-items: center; gap: 6px;
    margin-top: var(--space-2); padding: var(--space-2);
    background: transparent; color: var(--color-brand); font-size: var(--text-body-sm); cursor: pointer;
  }
  .addrow:hover { text-decoration: underline; }

  .legend { display: inline-flex; align-items: center; gap: 6px; margin-right: auto; font-size: var(--text-caption); color: var(--color-text-muted); }
  .legend .dot { width: 13px; height: 13px; }
</style>
