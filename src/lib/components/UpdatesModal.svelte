<script lang="ts">
  /**
   * UpdatesModal — the installed agents that have a newer version in the catalog
   * (reconcile state "outdated"), with per-row selection and a bulk update.
   *
   * One row per INSTALL, not per agent: an agent installed to two tools shows
   * twice, because each install updates independently. Reuses
   * `install.bulk("update", …)`, which re-reconciles afterward — so updated rows
   * flip to "current" and drop out of the list live. Default: everything checked.
   */
  import ArrowUpCircle from "@lucide/svelte/icons/arrow-up-circle";
  import Modal from "./Modal.svelte";
  import Button from "./Button.svelte";
  import { install } from "$lib/stores/install.svelte";
  import { corpus } from "$lib/stores/corpus.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import { i18n } from "$lib/stores/i18n.svelte";
  import type { InstalledAgent } from "$lib/types";

  interface Props {
    onClose: () => void;
  }
  let { onClose }: Props = $props();

  const key = (r: InstalledAgent) => `${r.slug}:${r.tool}:${r.projectPath ?? ""}`;
  const projLabel = (p: string) => p.replace(/\/+$/, "").split("/").pop() || p;
  // Sync slug→agent lookup for the emoji (corpus.get() is async — fetches body).
  const bySlug = $derived(new Map(corpus.agents.map((a) => [a.slug, a])));

  // Every install with a newer catalog version, sorted for a stable, scannable
  // list (agent name, then tool).
  const outdated = $derived(
    install.installed
      .filter((r) => r.state === "outdated")
      .sort((a, b) => a.name.localeCompare(b.name) || a.tool.localeCompare(b.tool)),
  );

  // Track DESELECTED keys, so the default (empty set) means "all checked" and we
  // never need an effect to seed the selection.
  let deselected = $state<Set<string>>(new Set());
  const isChecked = (r: InstalledAgent) => !deselected.has(key(r));
  const chosen = $derived(outdated.filter((r) => !deselected.has(key(r))));
  const allChecked = $derived(deselected.size === 0);
  const noneChecked = $derived(outdated.length > 0 && chosen.length === 0);

  function toggle(r: InstalledAgent) {
    const k = key(r);
    const next = new Set(deselected);
    if (next.has(k)) next.delete(k);
    else next.add(k);
    deselected = next;
  }
  function toggleAll() {
    deselected = allChecked ? new Set(outdated.map(key)) : new Set();
  }

  let busy = $state(false);
  async function updateChosen() {
    if (chosen.length === 0 || busy) return;
    busy = true;
    try {
      const { ok, fail } = await install.bulk(
        "update",
        chosen.map((r) => ({ slug: r.slug, tool: r.tool, projectPath: r.projectPath })),
      );
      if (fail === 0) toast.success(i18n.t("agentUpdates.done", { count: ok }));
      else toast.error(i18n.t("agentUpdates.someFailed", { ok, fail }));
    } finally {
      busy = false;
      // bulk() re-reconciles; if everything updated cleanly, nothing's left.
      if (install.installed.filter((r) => r.state === "outdated").length === 0) onClose();
    }
  }
</script>

<Modal open title={i18n.t("agentUpdates.title", { count: outdated.length })} onClose={onClose}>
  <p class="sub">{i18n.t("agentUpdates.sub")}</p>

  {#if outdated.length === 0}
    <p class="empty">{i18n.t("agentUpdates.empty")}</p>
  {:else}
    <div class="head">
      <label class="all">
        <input
          type="checkbox"
          checked={allChecked}
          indeterminate={!allChecked && !noneChecked}
          onchange={toggleAll}
        />
        {i18n.t("agentUpdates.selectAll")}
      </label>
      <span class="n">{i18n.t("common.selected", { count: chosen.length })}</span>
    </div>

    <ul class="list">
      {#each outdated as r (key(r))}
        <li class="row">
          <label class="lbl">
            <input type="checkbox" checked={isChecked(r)} onchange={() => toggle(r)} />
            <span class="emoji">{bySlug.get(r.slug)?.emoji ?? "○"}</span>
            <span class="name">{r.name}</span>
          </label>
          <span class="tool">{install.toolLabel(r.tool)}</span>
          {#if r.projectPath}<span class="proj" title={r.projectPath}>{projLabel(r.projectPath)}</span>{/if}
        </li>
      {/each}
    </ul>
  {/if}

  {#snippet actions()}
    <span class="foot-hint"><ArrowUpCircle size={13} /> {i18n.t("agentUpdates.footHint")}</span>
    <Button variant="secondary" onclick={onClose}>{i18n.t("common.close")}</Button>
    <Button variant="primary" disabled={busy || chosen.length === 0} onclick={updateChosen}>
      {busy ? i18n.t("common.working") : i18n.t("agentUpdates.updateN", { count: chosen.length })}
    </Button>
  {/snippet}
</Modal>

<style>
  .sub { font-size: var(--text-body-sm); color: var(--color-text-muted); margin-bottom: var(--space-3); }
  .empty { font-size: var(--text-body-sm); color: var(--color-text-muted); }

  .head {
    display: flex; align-items: center; gap: var(--space-3);
    padding-bottom: var(--space-2); margin-bottom: var(--space-2);
    border-bottom: 1px solid var(--color-border);
  }
  .all { display: inline-flex; align-items: center; gap: 7px; font-size: var(--text-body-sm); color: var(--color-text-secondary); cursor: pointer; }
  .n { margin-left: auto; font-size: var(--text-caption); color: var(--color-text-muted); font-variant-numeric: tabular-nums; }

  .list { list-style: none; margin: 0; padding: 0; max-height: 46vh; overflow-y: auto; display: flex; flex-direction: column; }
  .row {
    display: flex; align-items: center; gap: var(--space-2);
    padding: 6px 2px; border-bottom: 1px solid var(--color-border);
    font-size: var(--text-body-sm);
  }
  .row:last-child { border-bottom: none; }
  .lbl { flex: 1; min-width: 0; display: flex; align-items: center; gap: 8px; cursor: pointer; }
  .emoji { flex: none; }
  .name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--color-text-primary); }
  .tool { flex: none; font-size: var(--text-caption); color: var(--color-text-secondary); background: var(--color-surface-sunken); padding: 2px 8px; border-radius: var(--radius-full); }
  .proj { flex: none; font-size: var(--text-caption); color: var(--color-text-muted); max-width: 160px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

  input[type="checkbox"] { width: 15px; height: 15px; accent-color: var(--color-brand); cursor: pointer; }

  .foot-hint { display: inline-flex; align-items: center; gap: 6px; margin-right: auto; font-size: var(--text-caption); color: var(--color-text-muted); }
  .foot-hint :global(svg) { color: var(--color-brand); }
</style>
