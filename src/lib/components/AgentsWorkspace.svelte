<script lang="ts">
  /**
   * AgentsWorkspace — the unified Agents surface. Replaces the old split between
   * PersonaDiscover (catalog browse) and AgentLibrary (installed view): an agent
   * and its cross-tool deployment are ONE object now.
   *
   * Three panes: the app sidebar (Nav, in +page) · a list pane (filter lens +
   * search + category + bulk select) · a persistent, resizable detail pane
   * (persona + the DeploymentMatrix). Install state is a FILTER over one list,
   * not a separate destination — so "what an agent does" and "where it's
   * installed" are finally visible together.
   */
  import { onMount } from "svelte";
  import SearchIcon from "@lucide/svelte/icons/search";
  import RefreshIcon from "@lucide/svelte/icons/refresh-cw";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import TrashIcon from "@lucide/svelte/icons/trash-2";
  import PlusIcon from "@lucide/svelte/icons/plus";
  import XIcon from "@lucide/svelte/icons/x";
  import AlertTriangle from "@lucide/svelte/icons/triangle-alert";
  import LayersIcon from "@lucide/svelte/icons/layers";

  import Input from "./Input.svelte";
  import Pill from "./Pill.svelte";
  import EmptyState from "./EmptyState.svelte";
  import LoadingState from "./LoadingState.svelte";
  import ResizeHandle from "./ResizeHandle.svelte";
  import PersonaBody from "./PersonaBody.svelte";
  import DeploymentMatrix from "./DeploymentMatrix.svelte";
  import DiffModal from "./DiffModal.svelte";

  import { corpus } from "$lib/stores/corpus.svelte";
  import { install } from "$lib/stores/install.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import {
    ui,
    DETAIL_PANE_MIN_WIDTH,
    DETAIL_PANE_DEFAULT_WIDTH,
    clampDetailPaneWidth,
  } from "$lib/stores/ui.svelte";
  import { resolveCategoryIcon } from "$lib/util/categoryIcon";
  import type { Agent, AgentsFilter, InstalledAgent, InstallState, Tool } from "$lib/types";

  onMount(() => corpus.ensureLoaded());

  // ── Install rows grouped by agent slug (reactive over the reconcile) ──
  const installsBySlug = $derived.by(() => {
    const m = new Map<string, InstalledAgent[]>();
    for (const r of install.installed) {
      const a = m.get(r.slug);
      if (a) a.push(r);
      else m.set(r.slug, [r]);
    }
    return m;
  });

  // ── Filter lenses (counts power the segmented control badges) ──
  const FILTERS: { id: AgentsFilter; label: string }[] = [
    { id: "all", label: "All" },
    { id: "installed", label: "Installed" },
    { id: "notInstalled", label: "Not installed" },
    { id: "attention", label: "Needs attention" },
    { id: "untracked", label: "Untracked" },
  ];

  function lensMatch(filter: AgentsFilter, rows: InstalledAgent[] | undefined): boolean {
    if (filter === "all") return true;
    // "Not installed" is the complement of installed — and it's the one lens
    // that MATCHES agents with no rows, so it's checked before the empty guard.
    if (filter === "notInstalled") return !rows || !rows.some((r) => r.state !== "removed");
    if (!rows || rows.length === 0) return false;
    if (filter === "installed") return rows.some((r) => r.state !== "removed");
    if (filter === "attention")
      return rows.some((r) => r.state === "outdated" || r.state === "modified" || r.state === "removed");
    if (filter === "untracked") return rows.some((r) => r.state === "foreign");
    return true;
  }

  // Counts respect the active division — so the lens badges narrow when a
  // category is selected (All N / Installed N within that division, etc.).
  const counts = $derived.by(() => {
    const cat = ui.agentsCategory;
    const pool = cat ? corpus.agents.filter((a) => a.category === cat) : corpus.agents;
    let installed = 0,
      attention = 0,
      untracked = 0;
    for (const a of pool) {
      const rows = installsBySlug.get(a.slug);
      if (!rows || rows.length === 0) continue;
      if (rows.some((r) => r.state !== "removed")) installed++;
      if (rows.some((r) => r.state === "outdated" || r.state === "modified" || r.state === "removed")) attention++;
      if (rows.some((r) => r.state === "foreign")) untracked++;
    }
    return { all: pool.length, installed, notInstalled: pool.length - installed, attention, untracked };
  });
  function countFor(id: AgentsFilter): number {
    return id === "all"
      ? counts.all
      : id === "installed"
        ? counts.installed
        : id === "notInstalled"
          ? counts.notInstalled
          : id === "attention"
            ? counts.attention
            : counts.untracked;
  }
  // Keep the lens row quiet: show "All" always, the active lens always (so it
  // never disappears under you), and any other lens only when it has matches.
  function showLens(id: AgentsFilter): boolean {
    return id === "all" || ui.agentsFilter === id || countFor(id) > 0;
  }

  // ── List state ── (filter + category live in ui so back/forward + division
  // deep-links drive them; search query stays local — not a navigation.)
  let query = $state("");
  let catMenuOpen = $state(false);

  const visible = $derived(
    corpus.filtered(ui.agentsCategory, query).filter((a) => lensMatch(ui.agentsFilter, installsBySlug.get(a.slug))),
  );

  function setFilter(f: AgentsFilter) {
    ui.setAgentsFilter(f);
  }
  function pickCategory(slug: string | null) {
    ui.setAgentsCategory(slug);
    catMenuOpen = false;
  }
  const categoryLabel = $derived(ui.agentsCategory ? corpus.labelOf(ui.agentsCategory) : "All categories");

  // Compact per-row state dots (one per install row, colored by state).
  function dotTone(s: InstallState): string {
    if (s === "current") return "ok";
    if (s === "outdated" || s === "modified") return "warn";
    if (s === "foreign") return "info";
    return "danger"; // removed
  }

  // ── Detail selection (persistent pane) ──
  // Driven by ui.agentsSelected so back/forward + deep-links restore the open
  // agent. The effect shows the list-view stub instantly, then loads the body.
  let detailStub = $state<Agent | null>(null);
  let detail = $state<Agent | null>(null);
  let detailLoading = $state(false);
  const panelAgent = $derived(detail ?? detailStub);

  $effect(() => {
    const slug = ui.agentsSelected;
    if (!slug) {
      detailStub = null;
      detail = null;
      detailLoading = false;
      return;
    }
    const stub = corpus.agents.find((a) => a.slug === slug) ?? null;
    detailStub = stub;
    if (stub?.body) {
      detail = stub;
      detailLoading = false;
      return;
    }
    detail = null;
    detailLoading = true;
    void corpus.get(slug).then((full) => {
      if (ui.agentsSelected === slug) {
        detail = full;
        detailLoading = false;
      }
    });
  });

  function openAgent(a: Agent) {
    ui.selectAgent(a.slug);
  }
  function closeDetail() {
    ui.selectAgent(null);
  }

  // ── Diff modal (opened from the DeploymentMatrix) ──
  let diffTarget = $state<{ slug: string; tool: Tool; projectPath: string | null; name: string } | null>(null);

  // ── Bulk select (lifted from the old Library, now over the unified list) ──
  let selectMode = $state(false);
  let selected = $state<Set<string>>(new Set());
  let menuOpen = $state(false);
  let bulkBusy = $state(false);
  let confirmDelete = $state(false);

  function enterSelect() { selectMode = true; }
  function exitSelect() { selectMode = false; menuOpen = false; selected = new Set(); }
  function toggleRow(slug: string) {
    const next = new Set(selected);
    if (next.has(slug)) next.delete(slug);
    else next.add(slug);
    selected = next;
  }
  const allVisibleSelected = $derived(visible.length > 0 && visible.every((a) => selected.has(a.slug)));
  const someSelected = $derived(selected.size > 0 && !allVisibleSelected);
  function toggleAll() {
    if (allVisibleSelected) selected = new Set();
    else selected = new Set(visible.map((a) => a.slug));
  }
  // Prune selection to agents that still exist after a reconcile/reload.
  $effect(() => {
    const live = new Set(corpus.agents.map((a) => a.slug));
    if ([...selected].some((s) => !live.has(s))) {
      selected = new Set([...selected].filter((s) => live.has(s)));
    }
  });

  const selInstalls = $derived([...selected].flatMap((slug) => installsBySlug.get(slug) ?? []));
  const canBulkUpdate = $derived(selInstalls.some((i) => i.state !== "current"));
  const canBulkTrack = $derived(selInstalls.some((i) => i.state === "foreign"));

  async function runBulk(action: "update" | "track" | "uninstall", verb: string) {
    let picked = selInstalls;
    if (action === "update") picked = selInstalls.filter((i) => i.state !== "current");
    else if (action === "track") picked = selInstalls.filter((i) => i.state === "foreign");
    const targets = picked.map((i) => ({ slug: i.slug, tool: i.tool, projectPath: i.projectPath }));
    if (targets.length === 0) return;
    menuOpen = false;
    bulkBusy = true;
    try {
      const { ok, fail } = await install.bulk(action, targets);
      if (fail === 0) toast.success(`${verb} ${ok} install${ok === 1 ? "" : "s"}`);
      else toast.error(`${verb}: ${ok} ok, ${fail} failed`);
      selected = new Set();
    } finally {
      bulkBusy = false;
    }
  }

  const scanning = $derived(install.reconciling && !install.reconciled);
</script>

<section class="ws" class:sel={!!panelAgent}>
  <!-- ── List pane ── -->
  <div class="list-pane">
    <div class="lp-head">
      <div class="lp-search-row">
        <Input bind:value={query} variant="search" placeholder="Search agents by name, role, or vibe…" ariaLabel="Search agents" />
        {#if visible.length > 0}
          {#if selectMode}
            <button class="ghost" onclick={exitSelect}>Done</button>
          {:else}
            <button class="ghost" onclick={enterSelect}>Select</button>
          {/if}
        {/if}
        <button class="ghost icon" title="Rescan tools + installs" aria-label="Rescan" onclick={() => install.reconcile()}>
          <RefreshIcon size={15} />
        </button>
      </div>

      <div class="lp-filters">
        <div class="seg" role="tablist" aria-label="Filter agents">
          {#each FILTERS as f (f.id)}
            {#if showLens(f.id)}
              <button
                class="seg-btn"
                class:on={ui.agentsFilter === f.id}
                role="tab"
                aria-selected={ui.agentsFilter === f.id}
                onclick={() => setFilter(f.id)}
              >
                {f.label}<span class="seg-n">{countFor(f.id)}</span>
              </button>
            {/if}
          {/each}
        </div>

        <div class="cat-wrap">
          <button class="ghost cat-btn" class:active={!!ui.agentsCategory} onclick={() => (catMenuOpen = !catMenuOpen)}>
            <span class="truncate">{categoryLabel}</span><ChevronDown size={13} />
          </button>
          {#if catMenuOpen}
            <div class="cat-menu" role="menu">
              <button class="cat-opt" role="menuitem" class:on={!ui.agentsCategory} onclick={() => pickCategory(null)}>
                <LayersIcon size={14} /><span class="truncate">All categories</span><span class="cat-c">{corpus.agents.length}</span>
              </button>
              {#each corpus.tiles as c (c.slug)}
                {@const Icon = resolveCategoryIcon(c.icon)}
                <button class="cat-opt" role="menuitem" class:on={ui.agentsCategory === c.slug} onclick={() => pickCategory(c.slug)}>
                  <Icon size={14} /><span class="truncate">{c.label}</span><span class="cat-c">{c.count}</span>
                </button>
              {/each}
            </div>
          {/if}
        </div>
      </div>

      {#if selectMode}
        <div class="bulk-bar">
          <input
            type="checkbox"
            class="check"
            checked={allVisibleSelected}
            indeterminate={someSelected}
            onchange={toggleAll}
            aria-label="Select all visible"
          />
          <span class="bulk-count">{selected.size} selected</span>
          {#if selected.size > 0}
            <div class="bulk-menu-wrap">
              <button class="ghost" disabled={bulkBusy} onclick={() => (menuOpen = !menuOpen)}>
                {bulkBusy ? "Working…" : "With selected"}<ChevronDown size={14} />
              </button>
              {#if menuOpen}
                <div class="bulk-menu" role="menu">
                  <button class="bulk-opt" role="menuitem" disabled={!canBulkUpdate} title={canBulkUpdate ? "" : "All selected are in sync"} onclick={() => runBulk("update", "Updated")}>
                    <RefreshIcon size={14} /><span>Update — replace with catalog version</span>
                  </button>
                  <button class="bulk-opt" role="menuitem" disabled={!canBulkTrack} title={canBulkTrack ? "" : "Nothing untracked in the selection"} onclick={() => runBulk("track", "Tracked")}>
                    <PlusIcon size={14} /><span>Track — keep file, start managing</span>
                  </button>
                  <button class="bulk-opt danger" role="menuitem" onclick={() => { menuOpen = false; confirmDelete = true; }}>
                    <TrashIcon size={14} /><span>Delete — remove files from disk</span>
                  </button>
                </div>
              {/if}
            </div>
          {/if}
        </div>
      {/if}
    </div>

    <div class="lp-list">
      {#if corpus.loading && corpus.agents.length === 0}
        <LoadingState rows={6} label="Loading agents…" />
      {:else if corpus.error && corpus.agents.length === 0}
        <EmptyState title="Corpus unavailable" body="The agent catalog isn't ready yet.">
          {#snippet icon()}<SearchIcon size={48} />{/snippet}
        </EmptyState>
      {:else if visible.length === 0}
        <EmptyState
          title={query.trim() ? `Nothing matches “${query.trim()}”.` : ui.agentsFilter === "all" ? "No agents in this category." : "Nothing here yet."}
          body={ui.agentsFilter === "all" ? "Try a different search or category." : "Switch to All to browse the full catalog."}
        >
          {#snippet icon()}<SearchIcon size={48} />{/snippet}
        </EmptyState>
      {:else}
        <ul class="rows">
          {#each visible as a (a.slug)}
            {@const rows = installsBySlug.get(a.slug) ?? []}
            {@const isSel = panelAgent?.slug === a.slug}
            <li class="row" class:active={isSel} class:picked={selectMode && selected.has(a.slug)}>
              {#if selectMode}
                <input type="checkbox" class="check" checked={selected.has(a.slug)} onchange={() => toggleRow(a.slug)} aria-label={`Select ${a.name}`} />
              {/if}
              <button class="row-main" onclick={() => openAgent(a)} aria-current={isSel ? "true" : undefined}>
                <span class="row-emoji" aria-hidden="true">{a.emoji ?? "🧩"}</span>
                <span class="row-text">
                  <span class="row-name truncate">{a.name}</span>
                  {#if a.vibe}<span class="row-vibe truncate">{a.vibe}</span>{/if}
                </span>
                {#if rows.length > 0}
                  <span class="row-dots" aria-hidden="true">
                    {#each rows as r (r.dest)}
                      <span class="dot" data-tone={dotTone(r.state)} title={`${install.toolLabel(r.tool)} · ${r.state}`}></span>
                    {/each}
                  </span>
                {/if}
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  </div>

  <!-- ── Resize handle (grows the detail pane when dragged left) ── -->
  <div class="ws-resize">
    <ResizeHandle
      width={ui.detailPaneWidth}
      min={DETAIL_PANE_MIN_WIDTH}
      max={900}
      defaultWidth={DETAIL_PANE_DEFAULT_WIDTH}
      direction="left"
      label="Resize detail pane"
      onChange={(w) => (ui.detailPaneWidth = clampDetailPaneWidth(w))}
      onCommit={(w) => ui.setDetailPaneWidth(w)}
    />
  </div>

  <!-- ── Detail pane (persistent) ── -->
  <aside class="detail-pane" style="width: {ui.detailPaneWidth}px" aria-label="Agent detail">
    {#if panelAgent}
      <div class="dp-bar">
        <button class="dp-close" onclick={closeDetail} aria-label="Close detail" title="Close detail"><XIcon size={16} /></button>
      </div>
      <div class="dp-scroll">
        <PersonaBody agent={panelAgent} loading={detailLoading} onCategory={(slug) => ui.openDivision(slug)}>
          {#snippet deploy()}
            {#if panelAgent}
              <DeploymentMatrix agent={panelAgent} onDiff={(t) => (diffTarget = t)} />
            {/if}
          {/snippet}
        </PersonaBody>
      </div>
    {:else}
      <div class="dp-empty">
        <div class="dpe-card">
          <span class="dpe-emoji" aria-hidden="true">🧭</span>
          <h2>Pick an agent</h2>
          <p>Select an agent to read its persona and deploy it across your tools.</p>
          <div class="dpe-stats">
            <button class="dpe-stat" onclick={() => setFilter("all")}><span>{counts.all}</span><small>available</small></button>
            <button class="dpe-stat" onclick={() => setFilter("installed")}><span>{counts.installed}</span><small>installed</small></button>
            {#if counts.attention > 0}
              <button class="dpe-stat warn" onclick={() => setFilter("attention")}><span>{counts.attention}</span><small>need attention</small></button>
            {/if}
          </div>
        </div>
      </div>
    {/if}
  </aside>

  <!-- Narrow-window overlay scrim: only shown via CSS under the breakpoint when a
       detail is open. Clicking it dismisses the overlaid detail pane. -->
  <button class="ws-scrim" aria-label="Close detail" onclick={closeDetail}></button>
</section>

{#if diffTarget}
  <DiffModal
    slug={diffTarget.slug}
    tool={diffTarget.tool}
    projectPath={diffTarget.projectPath}
    name={diffTarget.name}
    onClose={() => (diffTarget = null)}
  />
{/if}

{#if confirmDelete}
  <button class="cd-scrim" aria-label="Cancel" onclick={() => (confirmDelete = false)}></button>
  <div class="cd-box" role="alertdialog" aria-modal="true" aria-label="Confirm delete">
    <div class="cd-head"><AlertTriangle size={20} /><h2>Delete {selected.size} agent{selected.size === 1 ? "" : "s"}?</h2></div>
    <p class="cd-body">
      This <strong>permanently removes {selInstalls.length} file{selInstalls.length === 1 ? "" : "s"} from disk</strong>
      (every tool these agents are installed in) — including any installed outside this app.
      Modified files are backed up before removal; catalog-identical files can be installed again.
    </p>
    <p class="cd-note">Tip: to swap in the catalog version instead, use <strong>Update</strong> — it backs your copy up first.</p>
    <div class="cd-actions">
      <button class="cd-cancel" onclick={() => (confirmDelete = false)}>Cancel</button>
      <button class="cd-delete" disabled={bulkBusy} onclick={() => { confirmDelete = false; runBulk("uninstall", "Deleted"); }}>
        <TrashIcon size={14} /> Delete {selInstalls.length}
      </button>
    </div>
  </div>
{/if}

<style>
  .ws { display: flex; height: 100%; min-height: 0; }

  /* ── List pane ── */
  .list-pane { flex: 1; min-width: 0; display: flex; flex-direction: column; min-height: 0; }
  .lp-head {
    flex: none; padding: var(--space-3) var(--space-4);
    border-bottom: 1px solid var(--color-border);
    display: flex; flex-direction: column; gap: var(--space-3);
  }
  .lp-search-row { display: flex; align-items: center; gap: var(--space-2); }
  .lp-search-row :global(.wrap) { flex: 1; min-width: 0; }

  .ghost {
    display: inline-flex; align-items: center; gap: 6px; flex: none;
    height: 32px; padding: 0 var(--space-3);
    border: 1px solid var(--color-border); border-radius: var(--radius-md);
    background: transparent; color: var(--color-text-secondary);
    font-size: var(--text-body-sm); cursor: pointer;
  }
  .ghost:hover:not(:disabled) { color: var(--color-text-primary); background: var(--color-surface-sunken); }
  .ghost:disabled { opacity: 0.6; cursor: default; }
  .ghost.icon { padding: 0; width: 32px; justify-content: center; }

  .lp-filters { display: flex; align-items: center; gap: var(--space-2); flex-wrap: wrap; }
  .seg {
    display: inline-flex; align-items: center; gap: 2px; flex-wrap: wrap;
    padding: 2px; background: var(--color-surface-sunken);
    border: 1px solid var(--color-border); border-radius: var(--radius-md);
  }
  .seg-btn {
    display: inline-flex; align-items: center; gap: 6px;
    height: 26px; padding: 0 10px; border-radius: var(--radius-sm);
    background: transparent; color: var(--color-text-secondary);
    font-size: var(--text-body-sm); cursor: pointer; white-space: nowrap;
  }
  .seg-btn:hover { color: var(--color-text-primary); }
  .seg-btn.on { background: var(--color-surface-raised); color: var(--color-text-primary); box-shadow: var(--shadow-sm, 0 1px 2px rgba(0,0,0,0.08)); }
  .seg-n {
    display: inline-flex; align-items: center; justify-content: center;
    min-width: 18px; height: 16px; padding: 0 5px; border-radius: 999px;
    background: var(--color-surface); color: var(--color-text-muted);
    font-size: 10px; font-weight: var(--fw-semibold);
  }
  .seg-btn.on .seg-n { background: var(--color-brand); color: var(--color-text-inverse); }

  .cat-wrap { position: relative; margin-left: auto; }
  .cat-btn { max-width: 180px; }
  .cat-btn.active { color: var(--color-brand); border-color: var(--color-brand); }
  .cat-menu {
    position: absolute; top: calc(100% + 4px); right: 0; z-index: 30;
    min-width: 220px; max-height: 320px; overflow-y: auto; padding: 4px;
    background: var(--color-surface-raised); border: 1px solid var(--color-border);
    border-radius: var(--radius-md); box-shadow: var(--shadow-lg);
    display: flex; flex-direction: column; gap: 1px;
  }
  .cat-opt {
    display: flex; align-items: center; gap: var(--space-2);
    padding: 6px 8px; border-radius: var(--radius-sm);
    background: transparent; color: var(--color-text-primary);
    font-size: var(--text-body-sm); text-align: left; cursor: pointer;
  }
  .cat-opt:hover { background: var(--color-surface-sunken); }
  .cat-opt.on { color: var(--color-brand); }
  .cat-opt .truncate { flex: 1; min-width: 0; }
  .cat-c { font-size: var(--text-caption); color: var(--color-text-muted); }

  .bulk-bar { display: flex; align-items: center; gap: var(--space-2); }
  .bulk-count { font-size: var(--text-body-sm); color: var(--color-brand); font-weight: var(--fw-medium); }
  .bulk-menu-wrap { position: relative; margin-left: auto; }
  .bulk-menu {
    position: absolute; top: calc(100% + 6px); right: 0; z-index: 30;
    min-width: 280px; padding: 4px;
    background: var(--color-surface-raised); border: 1px solid var(--color-border);
    border-radius: var(--radius-md); box-shadow: var(--shadow-lg);
    display: flex; flex-direction: column; gap: 1px;
  }
  .bulk-opt {
    display: flex; align-items: center; gap: var(--space-2);
    padding: 8px 10px; border-radius: var(--radius-sm);
    background: transparent; color: var(--color-text-primary);
    font-size: var(--text-body-sm); text-align: left; cursor: pointer;
  }
  .bulk-opt:hover:not(:disabled) { background: var(--color-surface-sunken); }
  .bulk-opt:disabled { opacity: 0.4; cursor: default; }
  .bulk-opt.danger { color: var(--color-danger); }
  .bulk-opt.danger:hover { background: color-mix(in srgb, var(--color-danger) 12%, transparent); }

  .check { accent-color: var(--color-brand); cursor: pointer; width: 15px; height: 15px; flex: none; }

  /* ── Rows ── */
  .lp-list { flex: 1; overflow-y: auto; min-height: 0; padding: var(--space-2) var(--space-3); }
  .rows { display: flex; flex-direction: column; gap: 1px; }
  .row { display: flex; align-items: center; gap: var(--space-2); border-radius: var(--radius-md); padding-left: var(--space-2); }
  .row:hover { background: var(--color-surface-sunken); }
  .row.active { background: var(--color-brand-subtle); }
  .row.picked { background: color-mix(in srgb, var(--color-brand) 10%, transparent); }
  .row-main {
    flex: 1; min-width: 0; display: flex; align-items: center; gap: var(--space-3);
    padding: var(--space-2) var(--space-2); background: transparent; cursor: pointer; text-align: left;
  }
  .row-emoji { font-size: 19px; line-height: 1; flex: none; }
  .row-text { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 1px; }
  .row-name { font-size: var(--text-body-sm); font-weight: var(--fw-medium); color: var(--color-text-primary); }
  .row-vibe { font-size: var(--text-caption); color: var(--color-text-muted); }
  .row-dots { display: inline-flex; align-items: center; gap: 3px; flex: none; }
  .row-dots .dot { width: 7px; height: 7px; border-radius: 999px; background: var(--color-text-muted); }
  .dot[data-tone="ok"]     { background: var(--color-success); }
  .dot[data-tone="warn"]   { background: var(--color-warning); }
  .dot[data-tone="info"]   { background: var(--color-brand); }
  .dot[data-tone="danger"] { background: var(--color-danger); }

  /* ── Resize handle wrapper ── */
  .ws-resize { display: flex; flex: none; }

  /* ── Detail pane ── */
  .detail-pane {
    flex: none; max-width: 90vw;
    display: flex; flex-direction: column; min-height: 0;
    background: var(--color-surface-raised);
    border-left: 1px solid var(--color-border);
  }
  .dp-bar { flex: none; display: flex; justify-content: flex-end; padding: 6px 8px 0; }
  .dp-close {
    display: inline-flex; align-items: center; justify-content: center;
    width: 28px; height: 28px; border-radius: var(--radius-sm);
    color: var(--color-text-muted); background: transparent; cursor: pointer;
  }
  .dp-close:hover { background: var(--color-surface-sunken); color: var(--color-text-primary); }
  .dp-scroll { flex: 1; overflow-y: auto; min-height: 0; }

  .dp-empty { flex: 1; display: flex; align-items: center; justify-content: center; padding: var(--space-5); }
  .dpe-card { text-align: center; display: flex; flex-direction: column; align-items: center; gap: var(--space-2); max-width: 280px; }
  .dpe-emoji { font-size: 34px; }
  .dpe-card h2 { font-size: var(--text-h3); font-weight: var(--fw-semibold); color: var(--color-text-primary); }
  .dpe-card p { font-size: var(--text-body-sm); color: var(--color-text-muted); line-height: var(--lh-normal); }
  .dpe-stats { display: flex; gap: var(--space-2); margin-top: var(--space-3); }
  .dpe-stat {
    display: flex; flex-direction: column; align-items: center; gap: 2px;
    padding: var(--space-2) var(--space-3); border-radius: var(--radius-md);
    border: 1px solid var(--color-border); background: transparent; cursor: pointer;
  }
  .dpe-stat:hover { border-color: var(--color-brand); }
  .dpe-stat span { font-size: var(--text-h3); font-weight: var(--fw-bold); color: var(--color-text-primary); }
  .dpe-stat small { font-size: 10px; color: var(--color-text-muted); }
  .dpe-stat.warn span { color: var(--color-warning); }

  /* Narrow-window overlay scrim — hidden by default, shown only under the
     breakpoint when a detail is open (see media query). */
  .ws-scrim { display: none; }

  @media (max-width: 860px) {
    .ws-resize { display: none; }
    .detail-pane {
      position: fixed; top: 36px; right: 0; bottom: 0; z-index: 41;
      width: min(var(--detail-w, 420px), 92vw) !important;
      box-shadow: var(--shadow-lg, -8px 0 24px rgba(0,0,0,0.18));
    }
    .ws:not(.sel) .detail-pane { display: none; }
    .ws.sel .ws-scrim {
      display: block; position: fixed; inset: 36px 0 0 0; z-index: 40;
      background: rgba(0,0,0,0.28); border: 0; cursor: default;
    }
  }
</style>
