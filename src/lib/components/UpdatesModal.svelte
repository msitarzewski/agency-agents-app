<script lang="ts">
  /**
   * UpdatesModal — installed agents with a newer version in the catalog
   * (reconcile state "outdated"), as an agents × tools grid (the app's standard
   * install surface): one row per agent, a column per tool that has any outdated
   * install, and a checkable cell only where that (agent, tool) install is stale.
   *
   * Select whole rows/cells, then bulk-update via install.bulk("update", …),
   * which re-reconciles so updated cells drop out of the grid live.
   */
  import Modal from "./Modal.svelte";
  import Button from "./Button.svelte";
  import { install, SUPPORTED_TOOLS } from "$lib/stores/install.svelte";
  import { corpus } from "$lib/stores/corpus.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import { i18n } from "$lib/stores/i18n.svelte";
  import type { Tool } from "$lib/types";

  interface Props {
    onClose: () => void;
  }
  let { onClose }: Props = $props();

  const outdated = $derived(install.installed.filter((r) => r.state === "outdated"));
  const bySlug = $derived(new Map(corpus.agents.map((a) => [a.slug, a])));

  // Rows: distinct outdated agents, by name.
  const rows = $derived.by(() => {
    const seen = new Map<string, string>(); // slug -> name
    for (const r of outdated) if (!seen.has(r.slug)) seen.set(r.slug, r.name);
    return [...seen.entries()]
      .map(([slug, name]) => ({ slug, name }))
      .sort((a, b) => a.name.localeCompare(b.name));
  });
  // Columns: only the tools that actually have an outdated install, in menu order.
  const cols = $derived.by(() => {
    const present = new Set(outdated.map((r) => r.tool));
    return SUPPORTED_TOOLS.filter((t) => present.has(t.id));
  });

  // The install rows behind one (agent, tool) cell — usually one, but an agent
  // installed to the same tool in multiple projects collapses into a cell that
  // updates them all.
  function cellRows(slug: string, tool: Tool) {
    return outdated.filter((r) => r.slug === slug && r.tool === tool);
  }
  const cellKey = (slug: string, tool: Tool) => `${slug}:${tool}`;

  // Track deselected cells, so the default (empty) means "all checked".
  let deselected = $state<Set<string>>(new Set());
  const allCellKeys = $derived.by(() => {
    const out: string[] = [];
    for (const row of rows) for (const t of cols) if (cellRows(row.slug, t.id).length) out.push(cellKey(row.slug, t.id));
    return out;
  });
  const isSel = (slug: string, tool: Tool) =>
    cellRows(slug, tool).length > 0 && !deselected.has(cellKey(slug, tool));

  // Every selected cell's underlying install(s) → the update targets.
  const targets = $derived.by(() => {
    const t: { slug: string; tool: Tool; projectPath: string | null }[] = [];
    for (const key of allCellKeys) {
      if (deselected.has(key)) continue;
      const [slug, tool] = key.split(":") as [string, Tool];
      for (const r of cellRows(slug, tool)) t.push({ slug: r.slug, tool: r.tool, projectPath: r.projectPath });
    }
    return t;
  });

  const allSel = $derived(deselected.size === 0);
  const noneSel = $derived(allCellKeys.length > 0 && allCellKeys.every((k) => deselected.has(k)));

  function toggleCell(slug: string, tool: Tool) {
    if (!cellRows(slug, tool).length) return;
    const k = cellKey(slug, tool);
    const next = new Set(deselected);
    if (next.has(k)) next.delete(k);
    else next.add(k);
    deselected = next;
  }
  function toggleAll() {
    deselected = allSel ? new Set(allCellKeys) : new Set();
  }

  let busy = $state(false);
  async function updateChosen() {
    if (targets.length === 0 || busy) return;
    busy = true;
    try {
      const { ok, fail } = await install.bulk("update", targets);
      if (fail === 0) toast.success(i18n.t("agentUpdates.done", { count: ok }));
      else toast.error(i18n.t("agentUpdates.someFailed", { ok, fail }));
    } finally {
      busy = false;
      if (install.installed.filter((r) => r.state === "outdated").length === 0) onClose();
    }
  }
</script>

<Modal open size="wide" title={i18n.t("agentUpdates.title", { count: outdated.length })} onClose={onClose}>
  <p class="sub">{i18n.t("agentUpdates.sub")}</p>

  {#if outdated.length === 0}
    <p class="empty">{i18n.t("agentUpdates.empty")}</p>
  {:else}
    <div class="head">
      <label class="all">
        <input
          type="checkbox"
          checked={allSel}
          indeterminate={!allSel && !noneSel}
          onchange={toggleAll}
        />
        {i18n.t("agentUpdates.selectAll")}
      </label>
      <span class="n">{i18n.t("common.selected", { count: targets.length })}</span>
    </div>

    <div class="grid-wrap">
      <div class="grid" style="--cols: {cols.length}">
        <div class="cell head corner"></div>
        {#each cols as t (t.id)}
          <div class="cell head tool" title={t.label}>{t.label}</div>
        {/each}

        {#each rows as row (row.slug)}
          <div class="cell agent">
            <span class="emoji">{bySlug.get(row.slug)?.emoji ?? "○"}</span>
            <span class="aname">{row.name}</span>
          </div>
          {#each cols as t (t.id)}
            {#if cellRows(row.slug, t.id).length}
              <button
                class="cell toggle"
                onclick={() => toggleCell(row.slug, t.id)}
                aria-label={i18n.t("agentUpdates.cellAria", { agent: row.name, tool: t.label })}
                aria-pressed={isSel(row.slug, t.id)}
              >
                <span class="dot" class:full={isSel(row.slug, t.id)}></span>
              </button>
            {:else}
              <div class="cell na">—</div>
            {/if}
          {/each}
        {/each}
      </div>
    </div>
  {/if}

  {#snippet actions()}
    <span class="legend"><span class="dot full"></span> {i18n.t("agentUpdates.willUpdate")} <span class="dot"></span> {i18n.t("agentUpdates.skip")}</span>
    <Button variant="secondary" modalAction="cancel" onclick={onClose}>{i18n.t("common.close")}</Button>
    <Button variant="primary" modalAction="confirm" disabled={busy || targets.length === 0} onclick={updateChosen}>
      {busy ? i18n.t("common.working") : i18n.t("agentUpdates.updateN", { count: targets.length })}
    </Button>
  {/snippet}
</Modal>

<style>
  .sub { font-size: var(--text-body-sm); color: var(--color-text-muted); margin-bottom: var(--space-3); }
  .empty { font-size: var(--text-body-sm); color: var(--color-text-muted); }

  .head { display: flex; align-items: center; gap: var(--space-3); margin-bottom: var(--space-2); }
  .all { display: inline-flex; align-items: center; gap: 7px; font-size: var(--text-body-sm); color: var(--color-text-secondary); cursor: pointer; }
  .all input { width: 15px; height: 15px; accent-color: var(--color-brand); cursor: pointer; }
  .n { margin-left: auto; font-size: var(--text-caption); color: var(--color-text-muted); font-variant-numeric: tabular-nums; }

  /* Same grid language as InstallModal: agent rows × tool columns. */
  .grid-wrap { max-height: 52vh; overflow: auto; border: 1px solid var(--color-border); border-radius: var(--radius-md); }
  .grid {
    display: grid;
    grid-template-columns: minmax(190px, 1fr) repeat(var(--cols), 96px);
    width: max-content; min-width: 100%; align-items: stretch;
  }
  .cell { display: flex; align-items: center; justify-content: center; padding: var(--space-2); border-bottom: 1px solid var(--color-border); }
  .head { position: sticky; top: 0; z-index: 1; background: var(--color-surface-sunken); font-size: var(--text-caption); color: var(--color-text-muted); font-weight: var(--fw-semibold); min-height: 34px; padding: var(--space-2) 8px; line-height: 1.15; text-align: center; }
  .corner { background: var(--color-surface-sunken); }

  .agent { justify-content: flex-start; gap: 8px; min-width: 0; }
  .emoji { flex: none; }
  .aname { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: var(--text-body-sm); color: var(--color-text-primary); }

  .toggle { background: transparent; cursor: pointer; }
  .toggle:hover:not(:disabled) { background: var(--color-surface-sunken); }
  /* Same dot affordance as InstallModal: filled = selected to update. */
  .dot {
    width: 16px; height: 16px; border-radius: 999px; box-sizing: border-box;
    border: 1.5px solid var(--color-border-strong, var(--color-text-muted));
  }
  .dot.full { background: var(--color-brand); border-color: var(--color-brand); }
  .na { color: var(--color-text-muted); opacity: 0.4; }

  .legend { display: inline-flex; align-items: center; gap: 6px; margin-right: auto; font-size: var(--text-caption); color: var(--color-text-muted); }
  .legend .dot { width: 12px; height: 12px; }
</style>
