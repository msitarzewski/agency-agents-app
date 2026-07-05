<script lang="ts">
  /**
   * DeployBrowser — a two-pane "deploy into a project" picker (System-Settings
   * style). The LEFT pane is a scrollable list of things you can drop into the
   * project — the project's current roster, the app presets, and every division.
   * Clicking one shows it on the RIGHT: its agents + a per-tool installer scoped
   * to THIS project. Replaces the old flat chooser → modal hop.
   *
   * The right-pane toggles are tri-state over the selected set (all / some /
   * none) for each project-capable tool, scoped to this project's path; removal
   * of `foreign` files asks first.
   */
  import { onMount } from "svelte";
  import X from "@lucide/svelte/icons/x";
  import FolderIcon from "@lucide/svelte/icons/folder";
  import LayersIcon from "@lucide/svelte/icons/layers";
  import UsersIcon from "@lucide/svelte/icons/users";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";

  import Input from "./Input.svelte";
  import DestructiveConfirm from "./DestructiveConfirm.svelte";
  import { corpus } from "$lib/stores/corpus.svelte";
  import { install, SUPPORTED_TOOLS } from "$lib/stores/install.svelte";
  import { teams } from "$lib/stores/teams.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import { i18n } from "$lib/stores/i18n.svelte";
  import { resolveCategoryIcon } from "$lib/util/categoryIcon";
  import { PRESET_TEAMS } from "$lib/data/presetTeams";
  import type { MessageKey } from "$lib/i18n/messages";
  import type { Tool, InstalledAgent, Agent } from "$lib/types";
  import type { Component as SvelteComponent } from "svelte";

  interface Props {
    projectPath: string;
    onClose: () => void;
  }
  let { projectPath, onClose }: Props = $props();

  onMount(() => {
    corpus.ensureLoaded();
    teams.hydrate();
    void install.reconcile();
  });

  let query = $state("");

  function basename(p: string): string {
    return p.replace(/\/+$/, "").split("/").pop() || p;
  }
  const projectName = $derived(basename(projectPath));

  function presetLabel(slug: string): string {
    return i18n.t(`preset.${slug}.label` as MessageKey);
  }

  function presetDescription(slug: string): string {
    return i18n.t(`preset.${slug}.description` as MessageKey);
  }

  // ── Left list: current roster + presets + divisions ──
  type Item = {
    key: string;
    label: string;
    description: string;
    /** Lucide component icon OR an emoji string (agents). */
    icon?: SvelteComponent;
    emoji?: string;
    color: string | null;
    slugs: string[];
  };

  const rosterSlugs = $derived([
    ...new Set(
      install.installed
        .filter((r) => r.state !== "removed" && (r.projectPath ?? null) === projectPath)
        .map((r) => r.slug),
    ),
  ]);

  // Every deployable granularity — an agent, a division, a team (preset OR your
  // own saved team), or the project's current roster.
  const presetItems = $derived<Item[]>(
    PRESET_TEAMS.map((p) => ({ key: `preset:${p.slug}`, label: presetLabel(p.slug), description: presetDescription(p.slug), icon: p.icon as unknown as SvelteComponent, color: p.color, slugs: p.agents })),
  );
  const savedTeamItems = $derived<Item[]>(
    teams.saved.map((t) => ({ key: `team:${t.id}`, label: t.name, description: i18n.t("deploy.savedTeamDesc", { count: t.agents.length }), icon: UsersIcon as unknown as SvelteComponent, color: null, slugs: t.agents })),
  );
  const divisionItems = $derived<Item[]>(
    corpus.tiles.map((c) => ({ key: `div:${c.slug}`, label: c.label, description: i18n.t("deploy.divisionDesc", { count: c.count }), icon: resolveCategoryIcon(c.icon) as unknown as SvelteComponent, color: corpus.colorOf(c.slug), slugs: corpus.agents.filter((a) => a.category === c.slug).map((a) => a.slug) })),
  );
  const agentItems = $derived<Item[]>(
    corpus.agents.map((a) => ({ key: `agent:${a.slug}`, label: a.name, description: corpus.labelOf(a.category), emoji: a.emoji ?? "🧩", color: null, slugs: [a.slug] })),
  );

  function match(items: Item[]): Item[] {
    const q = query.trim().toLowerCase();
    if (!q) return items;
    return items.filter((i) => `${i.label} ${i.description}`.toLowerCase().includes(q));
  }

  const groups = $derived(
    [
      rosterSlugs.length > 0 && !query.trim()
        ? {
            head: i18n.t("deploy.thisProject"),
            items: [
              {
                key: "roster",
                label: i18n.t("deploy.currentRoster"),
                description: i18n.t("deploy.currentRosterDesc", { count: rosterSlugs.length }),
                icon: FolderIcon as unknown as SvelteComponent,
                color: null,
                slugs: rosterSlugs,
              } satisfies Item,
            ],
          }
        : null,
      { head: i18n.t("deploy.teams"), items: match([...savedTeamItems, ...presetItems]) },
      { head: i18n.t("deploy.divisions"), items: match(divisionItems) },
      { head: i18n.t("deploy.agents"), items: match(agentItems) },
    ].filter((g): g is { head: string; items: Item[] } => !!g && g.items.length > 0),
  );

  let selectedKey = $state("");
  const allItems = $derived(groups.flatMap((g) => g.items));
  // Keep a valid selection: default to the first item, and if filtering hides
  // the current pick, fall back to the first visible one.
  $effect(() => {
    if (allItems.length > 0 && !allItems.some((i) => i.key === selectedKey)) {
      selectedKey = allItems[0].key;
    }
  });
  const selected = $derived(allItems.find((i) => i.key === selectedKey) ?? null);

  // The selected set's agents that exist in the corpus.
  const setAgents = $derived(
    selected ? corpus.agents.filter((a) => selected.slugs.includes(a.slug)) : [],
  );
  const setTotal = $derived(setAgents.length);

  // ── Group the selected set's agents by division (collapsible, closed by
  //    default). A single-division/single-agent set renders flat (no disclosure). ──
  const OTHER = "__other";
  const agentGroups = $derived.by(() => {
    const m = new Map<string, Agent[]>();
    for (const a of setAgents) {
      const div = a.category || OTHER;
      const arr = m.get(div);
      if (arr) arr.push(a);
      else m.set(div, [a]);
    }
    const out = [...m.entries()].map(([slug, rows]) => ({
      slug,
      label: slug === OTHER ? i18n.t("common.other") : corpus.labelOf(slug),
      color: slug === OTHER ? "#94A3B8" : corpus.colorOf(slug),
      icon: slug === OTHER ? "HelpCircle" : corpus.iconOf(slug),
      rows: rows.slice().sort((a, b) => a.name.localeCompare(b.name)),
    }));
    out.sort((a, b) => (a.slug === OTHER ? 1 : b.slug === OTHER ? -1 : a.label.localeCompare(b.label)));
    return out;
  });

  let agentsCollapsed = $state<Set<string>>(new Set());
  function toggleAgentGroup(slug: string) {
    const next = new Set(agentsCollapsed);
    if (next.has(slug)) next.delete(slug);
    else next.add(slug);
    agentsCollapsed = next;
  }
  // Closed by default; re-seed whenever the selected item changes.
  let acInitFor = "__none__";
  $effect(() => {
    const k = selectedKey;
    if (k === acInitFor) return;
    acInitFor = k;
    agentsCollapsed = new Set(agentGroups.map((g) => g.slug));
  });

  // ── Right pane: per-tool installer, scoped to THIS project ──
  const projectTools = $derived(
    SUPPORTED_TOOLS.filter(
      (t) =>
        t.supportsProject &&
        (install.tools.length === 0 ||
          install.tools.some((ti) => ti.tool === t.id && ti.detected) ||
          install.installed.some(
            (r) => r.tool === t.id && r.state !== "removed" && (r.projectPath ?? null) === projectPath,
          )),
    ),
  );

  function cover(tool: Tool) {
    const rows = install.installed.filter(
      (r) =>
        r.state !== "removed" &&
        r.tool === tool &&
        (r.projectPath ?? null) === projectPath &&
        selected?.slugs.includes(r.slug),
    );
    const present = new Set(rows.map((r) => r.slug));
    return {
      rows,
      count: present.size,
      all: setTotal > 0 && present.size === setTotal,
      some: present.size > 0 && present.size < setTotal,
      hasForeign: rows.some((r) => r.state === "foreign"),
    };
  }

  let busy = $state<Tool | null>(null);
  let confirm = $state<{ tool: Tool; rows: InstalledAgent[] } | null>(null);

  async function toggle(tool: Tool) {
    if (busy || !selected) return;
    const cov = cover(tool);
    if (cov.all) {
      if (cov.hasForeign) {
        confirm = { tool, rows: cov.rows };
        return;
      }
      await remove(tool, cov.rows);
      return;
    }
    const present = new Set(cov.rows.map((r) => r.slug));
    const missing = setAgents.filter((a) => !present.has(a.slug));
    if (missing.length === 0) return;
    busy = tool;
    try {
      const { ok, fail } = await install.bulk(
        "install",
        missing.map((a) => ({ slug: a.slug, tool, projectPath })),
      );
      if (fail === 0) toast.success(i18n.t("install.installedToast", { count: ok, tool: install.toolLabel(tool), where: projectName }));
      else toast.error(i18n.t("install.installFailedToast", { tool: install.toolLabel(tool), ok, fail }));
    } finally {
      busy = null;
    }
  }

  async function remove(tool: Tool, rows: InstalledAgent[]) {
    busy = tool;
    try {
      const { ok, fail } = await install.bulk(
        "uninstall",
        rows.map((r) => ({ slug: r.slug, tool: r.tool, projectPath: r.projectPath })),
      );
      if (fail === 0) toast.success(i18n.t("install.removedToast", { count: ok, tool: install.toolLabel(tool) }));
      else toast.error(i18n.t("install.removeFailedToast", { tool: install.toolLabel(tool), ok, fail }));
    } finally {
      busy = null;
    }
  }

  async function confirmRemove() {
    if (!confirm) return;
    const { tool, rows } = confirm;
    confirm = null;
    await remove(tool, rows);
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape" && !confirm) {
      e.preventDefault();
      onClose();
    }
  }
</script>

<svelte:window onkeydown={onKey} />

<button class="scrim" aria-label={i18n.t("deploy.closeAria")} onclick={onClose}></button>
<div class="box" role="dialog" aria-modal="true" aria-label={i18n.t("deploy.title", { project: projectName })}>
  <header class="head">
    <div class="titles">
      <h2 class="title">{i18n.t("deploy.title", { project: projectName })}</h2>
      <span class="sub" title={projectPath}>{projectPath}</span>
    </div>
    <button class="close" onclick={onClose} aria-label={i18n.t("deploy.closeAria")}><X size={16} /></button>
  </header>

  <div class="panes">
    <!-- LEFT: picker (agents · divisions · teams · current roster) -->
    <nav class="list" aria-label={i18n.t("deploy.choose")}>
      <div class="list-search">
        <Input bind:value={query} variant="search" placeholder={i18n.t("deploy.searchPlaceholder")} ariaLabel={i18n.t("deploy.searchAria")} />
      </div>
      <div class="list-scroll">
        {#each groups as g (g.head)}
          <h3 class="list-h">{g.head}</h3>
          {#each g.items as it (it.key)}
            <button class="li" class:on={selectedKey === it.key} onclick={() => (selectedKey = it.key)}>
              <span class="li-ic" style={it.color ? `color:${it.color}` : ""}>
                {#if it.emoji}{it.emoji}{:else if it.icon}{@const Icon = it.icon}<Icon size={16} />{/if}
              </span>
              <span class="li-body"><span class="li-label">{it.label}</span><span class="li-desc">{it.description}</span></span>
            </button>
          {/each}
        {/each}
        {#if groups.length === 0}
          <p class="list-empty">{i18n.t("deploy.noMatches", { query: query.trim() })}</p>
        {/if}
      </div>
    </nav>

    <!-- RIGHT: detail + per-tool install for this project -->
    <section class="detail">
      {#if selected}
        <div class="d-head">
          <span class="d-ic" style={selected.color ? `color:${selected.color}` : ""}>
            {#if selected.emoji}{selected.emoji}{:else if selected.icon}{@const SelIcon = selected.icon}<SelIcon size={20} />{/if}
          </span>
          <div class="d-titles">
            <h3 class="d-name">{selected.label}</h3>
            <p class="d-desc">{selected.description}</p>
          </div>
        </div>

        <p class="d-section">{i18n.t("deploy.installInto", { project: projectName })}</p>
        {#if projectTools.length === 0}
          <p class="d-empty">{i18n.t("deploy.noTools")}</p>
        {:else}
          <div class="grid-wrap">
            <div class="grid" style="--cols: {projectTools.length}">
              {#each projectTools as t (t.id)}
                <div class="cell head tool" title={t.label}>{t.label}</div>
              {/each}
              {#each projectTools as t (t.id)}
                {@const cov = cover(t.id)}
                {@const isBusy = busy === t.id}
                <button
                  class="cell toggle"
                  class:on={cov.all}
                  class:partial={cov.some}
                  disabled={isBusy || setTotal === 0}
                  title={i18n.t("install.cellTitle", { tool: t.label, target: cov.all ? i18n.t("deploy.cellAll", { count: setTotal }) : cov.some ? i18n.t("deploy.cellPartial", { count: cov.count, total: setTotal }) : i18n.t("deploy.cellNone") })}
                  aria-label={i18n.t(cov.all ? "install.removeFromAria" : "install.installIntoAria", { tool: t.label, target: projectName })}
                  onclick={() => toggle(t.id)}
                >
                  {#if isBusy}<span class="dot busy"></span>
                  {:else if cov.all}<span class="dot full"></span>
                  {:else if cov.some}<span class="dot half"></span>
                  {:else}<span class="dot"></span>{/if}
                  <span class="t-count" class:cta={!cov.all && !cov.some}>{#if cov.all}{i18n.t("deploy.cellAll", { count: setTotal })}{:else if cov.some}{i18n.t("deploy.cellPartial", { count: cov.count, total: setTotal })}{:else}{i18n.t("deploy.cellInstall")}{/if}</span>
                </button>
              {/each}
            </div>
          </div>
          <p class="grid-hint">{i18n.t("deploy.pickTool")}</p>
        {/if}

        <p class="d-section">{i18n.count(setTotal, "common.agent.one", "common.agent.many")}{#if agentGroups.length > 1} · {i18n.count(agentGroups.length, "common.division.one", "common.division.many")}{/if}</p>
        {#if agentGroups.length <= 1}
          <ul class="agents">
            {#each setAgents as a (a.slug)}
              <li class="ag"><span class="ag-emoji">{a.emoji ?? "🧩"}</span>{a.name}</li>
            {/each}
          </ul>
        {:else}
          <div class="agroups">
            {#each agentGroups as g (g.slug)}
              {@const Icon = resolveCategoryIcon(g.icon)}
              {@const isOpen = !agentsCollapsed.has(g.slug)}
              <section class="agrp">
                <button class="agrp-head" onclick={() => toggleAgentGroup(g.slug)} aria-expanded={isOpen}>
                  <ChevronDown size={14} class={isOpen ? "db-chev open" : "db-chev"} />
                  <span class="agrp-ic" style="color:{g.color}"><Icon size={14} /></span>
                  <span class="agrp-label">{g.label}</span>
                  <span class="agrp-count">{g.rows.length}</span>
                </button>
                {#if isOpen}
                  <ul class="agents">
                    {#each g.rows as a (a.slug)}
                      <li class="ag"><span class="ag-emoji">{a.emoji ?? "🧩"}</span>{a.name}</li>
                    {/each}
                  </ul>
                {/if}
              </section>
            {/each}
          </div>
        {/if}
      {:else}
        <p class="d-empty"><LayersIcon size={16} /> {i18n.t("deploy.pickLeft")}</p>
      {/if}
    </section>
  </div>
</div>

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
  .scrim {
    position: fixed; inset: 36px 0 0 0; z-index: 88;
    background: color-mix(in srgb, var(--color-bg) 60%, transparent);
    backdrop-filter: blur(4px); border: 0; cursor: default;
  }
  .box {
    position: fixed; z-index: 89;
    top: 64px; bottom: 64px; left: 50%; transform: translateX(-50%);
    width: min(820px, 94vw);
    display: flex; flex-direction: column;
    background: var(--color-surface-raised);
    border: 1px solid var(--color-border); border-radius: var(--radius-lg);
    box-shadow: var(--shadow-lg); overflow: hidden;
  }
  .head { flex: none; display: flex; align-items: center; gap: var(--space-3); padding: var(--space-3) var(--space-4); border-bottom: 1px solid var(--color-border); }
  .titles { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 1px; }
  .title { font-size: var(--text-h2); font-weight: var(--fw-semibold); color: var(--color-text-primary); }
  .sub { font-size: var(--text-caption); color: var(--color-text-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .close { flex: none; display: inline-flex; align-items: center; justify-content: center; width: 28px; height: 28px; border-radius: var(--radius-md); color: var(--color-text-muted); cursor: pointer; }
  .close:hover { background: var(--color-surface-sunken); color: var(--color-text-primary); }

  .panes { flex: 1; min-height: 0; display: grid; grid-template-columns: 248px 1fr; }
  .list { display: flex; flex-direction: column; min-height: 0; border-right: 1px solid var(--color-border); }
  .list-search { flex: none; padding: var(--space-2); border-bottom: 1px solid var(--color-border); }
  .list-scroll { flex: 1; min-height: 0; overflow-y: auto; padding: var(--space-2); }
  .list-empty { padding: var(--space-3) var(--space-2); font-size: var(--text-body-sm); color: var(--color-text-muted); }
  .list-h { font-size: var(--text-caption); font-weight: var(--fw-semibold); color: var(--color-text-muted); text-transform: uppercase; letter-spacing: 0.04em; padding: var(--space-2) var(--space-2) var(--space-1); }
  .li { display: flex; align-items: center; gap: var(--space-2); width: 100%; padding: var(--space-2); border-radius: var(--radius-md); background: transparent; cursor: pointer; text-align: left; }
  .li:hover { background: var(--color-surface-sunken); }
  .li.on { background: var(--color-selection-strong); }
  .li.on .li-label, .li.on .li-desc { color: var(--color-text-inverse); }
  .li-ic { flex: none; display: inline-flex; align-items: center; justify-content: center; width: 18px; font-size: 14px; }
  .li-body { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 0; }
  .li-label { font-size: var(--text-body-sm); font-weight: var(--fw-medium); color: var(--color-text-primary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .li-desc { font-size: var(--text-caption); color: var(--color-text-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

  .detail { overflow-y: auto; padding: var(--space-4); }
  .d-head { display: flex; align-items: center; gap: var(--space-3); margin-bottom: var(--space-3); }
  .d-ic { flex: none; display: inline-flex; align-items: center; justify-content: center; width: 38px; height: 38px; border-radius: var(--radius-md); background: var(--color-surface-sunken); font-size: 20px; }
  .d-titles { flex: 1; min-width: 0; }
  .d-name { font-size: var(--text-h3); font-weight: var(--fw-semibold); color: var(--color-text-primary); }
  .d-desc { font-size: var(--text-body-sm); color: var(--color-text-secondary); }
  .d-section { font-size: var(--text-caption); font-weight: var(--fw-semibold); color: var(--color-text-muted); text-transform: uppercase; letter-spacing: 0.04em; margin: var(--space-3) 0 var(--space-1); }
  .d-empty { display: flex; align-items: center; gap: 6px; font-size: var(--text-body-sm); color: var(--color-text-muted); }

  /* ── Tools as an InstallModal-style table (only detected tools) ── */
  .grid-wrap { overflow-x: auto; border: 1px solid var(--color-border); border-radius: var(--radius-md); }
  .grid {
    display: grid;
    grid-template-columns: repeat(var(--cols), minmax(88px, 1fr));
    align-items: stretch;
  }
  .cell { display: flex; align-items: center; justify-content: center; padding: var(--space-2); border-bottom: 1px solid var(--color-border); }
  .grid .head { background: var(--color-surface-sunken); font-size: var(--text-caption); color: var(--color-text-muted); font-weight: var(--fw-semibold); min-height: 32px; }
  .grid .head.tool { text-align: center; padding: var(--space-2) 6px; line-height: 1.15; }
  .cell.toggle { flex-direction: column; gap: 4px; background: transparent; cursor: pointer; }
  .cell.toggle:hover:not(:disabled) { background: var(--color-surface-sunken); }
  .cell.toggle:disabled { cursor: default; }
  .t-count { font-size: 10px; color: var(--color-text-muted); font-variant-numeric: tabular-nums; line-height: 1; }
  .t-count.cta { color: var(--color-brand); font-weight: var(--fw-semibold); }
  .cell.toggle:disabled .t-count.cta { color: var(--color-text-muted); }

  .grid-hint { margin: var(--space-2) 0 0; font-size: var(--text-caption); color: var(--color-text-muted); line-height: 1.4; }

  .dot { width: 16px; height: 16px; border-radius: 999px; border: 1.5px solid var(--color-border-strong, var(--color-text-muted)); box-sizing: border-box; }
  .dot.full { background: var(--color-brand); border-color: var(--color-brand); }
  .dot.half { border-color: var(--color-brand); background: linear-gradient(90deg, var(--color-brand) 50%, transparent 50%); }
  .dot.busy { border-color: var(--color-text-muted); border-top-color: transparent; animation: spin 0.6s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }

  /* ── Agent roster, grouped by division (closed by default) ── */
  .agroups { display: flex; flex-direction: column; gap: 2px; }
  .agrp { display: flex; flex-direction: column; }
  .agrp-head { display: flex; align-items: center; gap: var(--space-2); width: 100%; padding: 5px var(--space-2); border-radius: var(--radius-sm); background: transparent; cursor: pointer; text-align: left; }
  .agrp-head:hover { background: var(--color-surface-sunken); }
  :global(.db-chev) { color: var(--color-text-muted); transition: transform var(--motion-duration-fast, 120ms) ease; transform: rotate(-90deg); flex: none; }
  :global(.db-chev.open) { transform: rotate(0deg); }
  .agrp-ic { flex: none; display: inline-flex; }
  .agrp-label { flex: 1; min-width: 0; font-size: var(--text-body-sm); font-weight: var(--fw-semibold); color: var(--color-text-primary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .agrp-count { flex: none; min-width: 18px; text-align: center; font-size: var(--text-caption); color: var(--color-text-muted); font-variant-numeric: tabular-nums; background: var(--color-surface-sunken); border-radius: var(--radius-full); padding: 1px 6px; }

  .agents { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 1px; }
  .agrp .agents { padding-left: var(--space-4); }
  .ag { display: flex; align-items: center; gap: var(--space-2); padding: 3px var(--space-2); font-size: var(--text-body-sm); color: var(--color-text-secondary); }
  .ag-emoji { flex: none; }
</style>
