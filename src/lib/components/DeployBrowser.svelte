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

  import Input from "./Input.svelte";
  import DestructiveConfirm from "./DestructiveConfirm.svelte";
  import { corpus } from "$lib/stores/corpus.svelte";
  import { install, SUPPORTED_TOOLS } from "$lib/stores/install.svelte";
  import { teams } from "$lib/stores/teams.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import { resolveCategoryIcon } from "$lib/util/categoryIcon";
  import { PRESET_TEAMS } from "$lib/data/presetTeams";
  import type { Tool, InstalledAgent } from "$lib/types";
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
    PRESET_TEAMS.map((p) => ({ key: `preset:${p.slug}`, label: p.label, description: p.description, icon: p.icon as unknown as SvelteComponent, color: p.color, slugs: p.agents })),
  );
  const savedTeamItems = $derived<Item[]>(
    teams.saved.map((t) => ({ key: `team:${t.id}`, label: t.name, description: `${t.agents.length} agent${t.agents.length === 1 ? "" : "s"} · saved`, icon: UsersIcon as unknown as SvelteComponent, color: null, slugs: t.agents })),
  );
  const divisionItems = $derived<Item[]>(
    corpus.tiles.map((c) => ({ key: `div:${c.slug}`, label: c.label, description: `${c.count} agent${c.count === 1 ? "" : "s"}`, icon: resolveCategoryIcon(c.icon) as unknown as SvelteComponent, color: corpus.colorOf(c.slug), slugs: corpus.agents.filter((a) => a.category === c.slug).map((a) => a.slug) })),
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
            head: "This project",
            items: [
              {
                key: "roster",
                label: "Current roster",
                description: `${rosterSlugs.length} agent${rosterSlugs.length === 1 ? "" : "s"} already here`,
                icon: FolderIcon as unknown as SvelteComponent,
                color: null,
                slugs: rosterSlugs,
              } satisfies Item,
            ],
          }
        : null,
      { head: "Teams", items: match([...savedTeamItems, ...presetItems]) },
      { head: "Divisions", items: match(divisionItems) },
      { head: "Agents", items: match(agentItems) },
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
      if (fail === 0) toast.success(`Installed ${ok} → ${install.toolLabel(tool)} · ${projectName}`);
      else toast.error(`${install.toolLabel(tool)}: ${ok} installed, ${fail} failed`);
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
      if (fail === 0) toast.success(`Removed ${ok} from ${install.toolLabel(tool)}`);
      else toast.error(`${install.toolLabel(tool)}: ${ok} removed, ${fail} failed`);
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

<button class="scrim" aria-label="Close" onclick={onClose}></button>
<div class="box" role="dialog" aria-modal="true" aria-label={`Deploy into ${projectName}`}>
  <header class="head">
    <div class="titles">
      <h2 class="title">Deploy into {projectName}</h2>
      <span class="sub" title={projectPath}>{projectPath}</span>
    </div>
    <button class="close" onclick={onClose} aria-label="Close"><X size={16} /></button>
  </header>

  <div class="panes">
    <!-- LEFT: picker (agents · divisions · teams · current roster) -->
    <nav class="list" aria-label="Choose what to deploy">
      <div class="list-search">
        <Input bind:value={query} variant="search" placeholder="Search agents, divisions, teams…" ariaLabel="Search what to deploy" />
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
          <p class="list-empty">No matches for “{query.trim()}”.</p>
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

        <p class="d-section">Install into {projectName} —</p>
        {#if projectTools.length === 0}
          <p class="d-empty">No supported tools detected on this device.</p>
        {:else}
          <ul class="tools">
            {#each projectTools as t (t.id)}
              {@const cov = cover(t.id)}
              {@const isBusy = busy === t.id}
              <li class="tool">
                <button
                  class="t-toggle"
                  class:on={cov.all}
                  class:partial={cov.some}
                  disabled={isBusy || setTotal === 0}
                  aria-label={`${cov.all ? "Remove from" : "Install into"} ${t.label}`}
                  onclick={() => toggle(t.id)}
                >
                  {#if isBusy}<span class="dot busy"></span>
                  {:else if cov.all}<span class="dot full"></span>
                  {:else if cov.some}<span class="dot half"></span>
                  {:else}<span class="dot"></span>{/if}
                </button>
                <span class="t-label">{t.label}</span>
                <span class="t-count">{#if cov.all}all {setTotal}{:else if cov.some}{cov.count}/{setTotal}{:else}none{/if}</span>
              </li>
            {/each}
          </ul>
        {/if}

        <p class="d-section">{setTotal} agent{setTotal === 1 ? "" : "s"}</p>
        <ul class="agents">
          {#each setAgents as a (a.slug)}
            <li class="ag"><span class="ag-emoji">{a.emoji ?? "🧩"}</span>{a.name}</li>
          {/each}
        </ul>
      {:else}
        <p class="d-empty"><LayersIcon size={16} /> Pick something on the left to deploy.</p>
      {/if}
    </section>
  </div>
</div>

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

  .tools { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; }
  .tool { display: flex; align-items: center; gap: var(--space-3); padding: var(--space-2); border-radius: var(--radius-md); }
  .tool:hover { background: var(--color-surface-sunken); }
  .t-toggle { flex: none; display: inline-flex; align-items: center; justify-content: center; width: 24px; height: 24px; background: transparent; cursor: pointer; }
  .t-toggle:disabled { cursor: default; }
  .t-label { flex: 1; min-width: 0; font-size: var(--text-body); color: var(--color-text-primary); font-weight: var(--fw-medium); }
  .t-count { font-size: var(--text-body-sm); color: var(--color-text-muted); font-variant-numeric: tabular-nums; }
  .dot { width: 16px; height: 16px; border-radius: 999px; border: 1.5px solid var(--color-border-strong, var(--color-text-muted)); box-sizing: border-box; }
  .dot.full { background: var(--color-brand); border-color: var(--color-brand); }
  .dot.half { border-color: var(--color-brand); background: linear-gradient(90deg, var(--color-brand) 50%, transparent 50%); }
  .dot.busy { border-color: var(--color-text-muted); border-top-color: transparent; animation: spin 0.6s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }

  .agents { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 1px; }
  .ag { display: flex; align-items: center; gap: var(--space-2); padding: 3px var(--space-2); font-size: var(--text-body-sm); color: var(--color-text-secondary); }
  .ag-emoji { flex: none; }
</style>
