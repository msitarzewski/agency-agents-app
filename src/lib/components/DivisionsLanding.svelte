<script lang="ts">
  /**
   * DivisionsLanding — the Agents tab landing. Instead of dropping the user into
   * a flat list of every agent, it opens on the DIVISIONS (one row per division,
   * with its tinted icon and agent count). Clicking a row drills into that
   * division's agents (the existing AgentsWorkspace list, via ui.agentsCategory).
   *
   * "Select" mode multi-selects divisions; "Install Selected" opens the
   * DivisionInstallModal — a tri-state tool picker that deploys (or removes) every
   * agent in the chosen division(s) across user-scoped tools in one shot.
   *
   * Mirrors the agent list's own select-mode affordances so the two surfaces feel
   * like one tool at different zoom levels.
   */
  import LayersIcon from "@lucide/svelte/icons/layers";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";

  import InstallModal from "./InstallModal.svelte";
  import { corpus } from "$lib/stores/corpus.svelte";
  import { install } from "$lib/stores/install.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { i18n } from "$lib/i18n.svelte";
  import { resolveCategoryIcon } from "$lib/util/categoryIcon";

  // ── Division-level select mode (separate from the agent list's own) ──
  let selectMode = $state(false);
  let selected = $state<Set<string>>(new Set());
  let modalOpen = $state(false);

  function enterSelect() { selectMode = true; }
  function exitSelect() { selectMode = false; selected = new Set(); }
  function toggleRow(slug: string) {
    const next = new Set(selected);
    if (next.has(slug)) next.delete(slug);
    else next.add(slug);
    selected = next;
  }
  const tiles = $derived(corpus.tiles);
  const allSelected = $derived(tiles.length > 0 && tiles.every((c) => selected.has(c.slug)));
  const someSelected = $derived(selected.size > 0 && !allSelected);
  function toggleAll() {
    selected = allSelected ? new Set() : new Set(tiles.map((c) => c.slug));
  }
  // Drop selections for divisions that vanished after a catalog reload.
  $effect(() => {
    const live = new Set(tiles.map((c) => c.slug));
    if ([...selected].some((s) => !live.has(s))) {
      selected = new Set([...selected].filter((s) => live.has(s)));
    }
  });

  // Installed-agent count per division, for an at-a-glance "deployed" hint.
  const installedByDivision = $derived.by<Map<string, number>>(() => {
    const bySlug = new Map<string, string>(); // agent slug -> division
    for (const a of corpus.agents) bySlug.set(a.slug, a.category);
    const seen = new Set<string>(); // distinct agent slugs with any live install
    for (const r of install.installed) {
      if (r.state !== "removed") seen.add(r.slug);
    }
    const m = new Map<string, number>();
    for (const slug of seen) {
      const div = bySlug.get(slug);
      if (div) m.set(div, (m.get(div) ?? 0) + 1);
    }
    return m;
  });

  function openDivision(slug: string) {
    ui.setAgentsCategory(slug);
  }
  function onRow(slug: string) {
    if (selectMode) toggleRow(slug);
    else openDivision(slug);
  }
</script>

<div class="dl">
  <div class="dl-bar">
    {#if selectMode}
      <input
        type="checkbox"
        class="check"
        checked={allSelected}
        indeterminate={someSelected}
        onchange={toggleAll}
        aria-label={i18n.t("agents.selectAllVisible")}
      />
      <span class="count">{i18n.t("agents.selectedCount", { count: i18n.number(selected.size) })}</span>
      <button class="ghost cta" disabled={selected.size === 0} onclick={() => (modalOpen = true)}>
        {i18n.t("agents.installSelected")}
      </button>
      <button class="ghost" onclick={exitSelect}>{i18n.t("common.done")}</button>
    {:else}
      <span class="lead"><LayersIcon size={14} /> {i18n.t("common.divisions")}</span>
      <span class="spacer"></span>
      {#if tiles.length > 0}
        <button class="ghost" onclick={enterSelect}>{i18n.t("common.select")}</button>
      {/if}
    {/if}
  </div>

  <ul class="rows">
    {#each tiles as c (c.slug)}
      {@const Icon = resolveCategoryIcon(c.icon)}
      {@const deployed = installedByDivision.get(c.slug) ?? 0}
      <li class="row" class:picked={selectMode && selected.has(c.slug)}>
        {#if selectMode}
          <input
            type="checkbox"
            class="check"
            checked={selected.has(c.slug)}
            onchange={() => toggleRow(c.slug)}
            aria-label={`${i18n.t("common.select")} ${c.label}`}
          />
        {/if}
        <button class="row-main" onclick={() => onRow(c.slug)}>
          <span class="ic" style="color:{corpus.colorOf(c.slug)}"><Icon size={18} /></span>
          <span class="text">
            <span class="name truncate">{c.label}</span>
            <span class="meta">{i18n.number(c.count)} {i18n.agents(c.count)}{#if deployed > 0} · {i18n.number(deployed)} {i18n.t("common.deployed")}{/if}</span>
          </span>
          {#if !selectMode}<span class="chev" aria-hidden="true"><ChevronRight size={16} /></span>{/if}
        </button>
      </li>
    {/each}
  </ul>
</div>

{#if modalOpen}
  {@const slugs = corpus.agents.filter((a) => selected.has(a.category)).map((a) => a.slug)}
  {@const dTitle = selected.size === 1 ? i18n.t("agents.installDivisionTitle", { name: corpus.labelOf([...selected][0]) }) : i18n.t("agents.installDivisionsTitle", { count: i18n.number(selected.size), divisions: i18n.plural(selected.size, "division", "divisions", "раздел", "раздела", "разделов") })}
  <InstallModal title={dTitle} agentSlugs={slugs} onClose={() => (modalOpen = false)} />
{/if}

<style>
  .dl { display: flex; flex-direction: column; min-height: 0; height: 100%; }
  .dl-bar {
    display: flex; align-items: center; gap: var(--space-2);
    padding: var(--space-2) var(--space-3);
    border-bottom: 1px solid var(--color-border);
  }
  .lead { display: inline-flex; align-items: center; gap: 6px; font-size: var(--text-body-sm); color: var(--color-text-muted); font-weight: var(--fw-semibold); text-transform: uppercase; letter-spacing: 0.04em; }
  .spacer { flex: 1; }
  .count { font-size: var(--text-body-sm); color: var(--color-text-secondary); margin-right: auto; }

  .ghost {
    padding: 3px 10px; border-radius: var(--radius-md);
    background: transparent; color: var(--color-text-secondary);
    font-size: var(--text-body-sm); cursor: pointer;
  }
  .ghost:hover { background: var(--color-surface-sunken); color: var(--color-text-primary); }
  .cta { color: var(--color-brand); font-weight: var(--fw-medium); }
  .cta:disabled { color: var(--color-text-muted); cursor: not-allowed; background: transparent; }

  .rows { list-style: none; margin: 0; padding: 0; overflow-y: auto; flex: 1; min-height: 0; }
  .row {
    display: flex; align-items: center; gap: var(--space-2);
    padding: 0 var(--space-2);
    border-bottom: 1px solid var(--color-border);
  }
  .row.picked { background: var(--color-selection); }
  .row-main {
    flex: 1; min-width: 0;
    display: flex; align-items: center; gap: var(--space-3);
    padding: var(--space-3) var(--space-2);
    text-align: left; background: transparent; cursor: pointer;
  }
  .row-main:hover { background: var(--color-surface-sunken); }
  .ic { flex: none; display: inline-flex; }
  .text { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 1px; }
  .name { font-size: var(--text-body); font-weight: var(--fw-medium); color: var(--color-text-primary); }
  .meta { font-size: var(--text-caption); color: var(--color-text-muted); }
  .chev { flex: none; color: var(--color-text-muted); }
  .check { width: 16px; height: 16px; accent-color: var(--color-brand); cursor: pointer; }
</style>
