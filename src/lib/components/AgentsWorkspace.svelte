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
  import DivisionsLanding from "./DivisionsLanding.svelte";
  import InstallModal from "./InstallModal.svelte";
  import StarterPrompt from "./StarterPrompt.svelte";
  import { divisionPrompt } from "$lib/data/playbook";
  import DownloadIcon from "@lucide/svelte/icons/download";

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
  import { i18n } from "$lib/stores/i18n.svelte";
  import type { MessageKey } from "$lib/i18n/messages";
  import type { Agent, InstalledAgent, InstallState, Tool } from "$lib/types";

  onMount(() => corpus.ensureLoaded());

  // ── OS-style dropdown dismissal: click anywhere outside (or Escape) closes the
  //    open menu. Each trigger button is excluded so clicking it just toggles. ──
  let catBtn = $state<HTMLElement>();
  let catMenu = $state<HTMLElement>();
  let bulkBtn = $state<HTMLElement>();
  let bulkMenu = $state<HTMLElement>();
  function onDocClick(e: MouseEvent) {
    const t = e.target as Node | null;
    if (!t) return;
    if (catMenuOpen && !catBtn?.contains(t) && !catMenu?.contains(t)) catMenuOpen = false;
    if (menuOpen && !bulkBtn?.contains(t) && !bulkMenu?.contains(t)) menuOpen = false;
  }
  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      catMenuOpen = false;
      menuOpen = false;
    }
  }
  onMount(() => {
    document.addEventListener("click", onDocClick);
    window.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("click", onDocClick);
      window.removeEventListener("keydown", onKey);
    };
  });

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


  // ── List state ── (category lives in ui so back/forward + division
  // deep-links drive it; search query stays local — not a navigation.)
  let query = $state("");
  let catMenuOpen = $state(false);

  // ── Install-state lens ── (filter the agent list by deployment state, the
  // same dot tones shown per row). Local + localStorage like the Tools lens —
  // not nav state. An agent matches a bucket if ANY of its install rows is in
  // it; "none" = no install rows anywhere.
  type Lens = "all" | "attention" | "current" | "outdated" | "foreign" | "removed" | "none";
  // The lens lives in nav (ui.agentsLens) — the single source of truth — so the
  // Dashboard can deep-link a filter ("5 need attention" → the flat attention list)
  // and back/forward restores it. It no longer persists across launches: a sticky
  // filter would hijack the divisions landing, since an active lens now switches the
  // list to a flat all-divisions view (see showDivisions).
  const lens: Lens = $derived(ui.agentsLens as Lens);
  function setLens(l: Lens) { ui.setAgentsLens(l); }

  /** Which state buckets an agent falls into, across all its install rows. */
  function buckets(slug: string): Set<Lens> {
    const rows = installsBySlug.get(slug) ?? [];
    const s = new Set<Lens>();
    if (rows.length === 0) { s.add("none"); return s; }
    for (const r of rows) {
      if (r.state === "current") s.add("current");
      else if (r.state === "outdated" || r.state === "modified") { s.add("outdated"); s.add("attention"); }
      else if (r.state === "foreign") s.add("foreign");
      else if (r.state === "removed") { s.add("removed"); s.add("attention"); }
    }
    return s;
  }

  // Division + search filtered (pre-lens) — lens counts are computed over THIS
  // so they reflect the current division/search, not the selected lens.
  const base = $derived(corpus.filtered(ui.agentsCategory, query));
  const visible = $derived(lens === "all" ? base : base.filter((a) => buckets(a.slug).has(lens)));

  const lensCounts = $derived.by<Record<Lens, number>>(() => {
    const c: Record<Lens, number> = { all: base.length, attention: 0, current: 0, outdated: 0, foreign: 0, removed: 0, none: 0 };
    for (const a of base) for (const b of buckets(a.slug)) c[b]++;
    return c;
  });

  // Lens definitions paired with the row dot tones so the color story matches.
  const LENSES: { id: Lens; key: MessageKey; tone: string }[] = [
    { id: "all", key: "state.all", tone: "" },
    { id: "attention", key: "state.attention", tone: "warn" },
    { id: "current", key: "state.current", tone: "ok" },
    { id: "outdated", key: "state.outdated", tone: "warn" },
    { id: "foreign", key: "state.foreign", tone: "info" },
    { id: "removed", key: "state.removed", tone: "danger" },
    { id: "none", key: "state.none", tone: "none" },
  ];
  // Show "All" plus any bucket present in the current view (zero-count lenses hide).
  const visibleLenses = $derived(LENSES.filter((f) => f.id === "all" || lensCounts[f.id] > 0));
  // If the active lens empties out (e.g. switching to a division with none of
  // that state), fall back to All so the selection isn't an invisible filter.
  $effect(() => {
    if (lens !== "all" && lensCounts[lens] === 0) setLens("all");
  });

  // Close the detail pane when the open agent falls out of the list — e.g.
  // switching divisions or refining search leaves the detail showing an agent
  // you can no longer see in the picker. Only strands a real (in-corpus) agent;
  // an unresolved slug is left to the loader below.
  $effect(() => {
    const slug = ui.agentsSelected;
    if (!slug) return;
    const inCorpus = corpus.agents.some((a) => a.slug === slug);
    if (inCorpus && !base.some((a) => a.slug === slug)) ui.selectAgent(null);
  });

  // The Agents tab LANDS on the division list (not a flat agent list): only when
  // no division is drilled into AND there's no active search. Picking a division
  // or typing a query switches to the agent list below.
  const showDivisions = $derived(ui.agentsCategory === null && !query.trim() && lens === "all");
  // Leaving the landing for the agent list shouldn't carry a stale agent-select
  // session; entering it shouldn't either (the landing has its own selection).
  $effect(() => {
    if (showDivisions && selectMode) exitSelect();
  });

  function pickCategory(slug: string | null) {
    ui.setAgentsCategory(slug);
    catMenuOpen = false;
  }
  const categoryLabel = $derived(ui.agentsCategory ? corpus.labelOf(ui.agentsCategory) : i18n.t("agents.allDivisions"));

  // ── Division overview banner — shown atop a division's agent list (not while
  //    searching or selecting): what the division is for + deploy-the-whole-
  //    division + a starter prompt. null = hidden. ──
  const divisionMeta = $derived.by(() => {
    const slug = ui.agentsCategory;
    if (!slug || query.trim() || selectMode) return null;
    const slugs = corpus.agents.filter((a) => a.category === slug).map((a) => a.slug);
    if (slugs.length === 0) return null;
    const label = corpus.labelOf(slug);
    const fallback = divisionPrompt(slug, label);
    const prompt = i18n.optional(`divisionPrompt.${slug}`, fallback, { division: label });
    return { slug, label, color: corpus.colorOf(slug), icon: corpus.iconOf(slug), slugs, prompt };
  });
  let divisionInstallOpen = $state(false);

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

  // ── Diff modal (opened from the deployment pills) ──
  let diffTarget = $state<{ slug: string; tool: Tool; projectPath: string | null; name: string } | null>(null);
  // ── Install modal (the shared destinations × tools grid) for the open agent ──
  let installOpen = $state(false);

  // ── Bulk select (lifted from the old Library, now over the unified list) ──
  let selectMode = $state(false);
  let selected = $state<Set<string>>(new Set());
  let menuOpen = $state(false);
  let bulkBusy = $state(false);
  let confirmDelete = $state(false);
  // Bulk deploy: the shared InstallModal opened over the current selection.
  let bulkInstallOpen = $state(false);

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
  // Foreign rows = files we don't manage ("not ours"). When the selection has any,
  // the destructive action is a genuine delete; otherwise it's a reversible
  // uninstall (catalog agents re-install; any edits are backed up first).
  const selHasForeign = $derived(selInstalls.some((i) => i.state === "foreign"));

  async function runBulk(action: "update" | "track" | "uninstall", verbKey: MessageKey) {
    let picked = selInstalls;
    if (action === "update") picked = selInstalls.filter((i) => i.state !== "current");
    else if (action === "track") picked = selInstalls.filter((i) => i.state === "foreign");
    const targets = picked.map((i) => ({ slug: i.slug, tool: i.tool, projectPath: i.projectPath }));
    if (targets.length === 0) return;
    menuOpen = false;
    bulkBusy = true;
    try {
      const { ok, fail } = await install.bulk(action, targets);
      const verb = i18n.t(verbKey);
      if (fail === 0) toast.success(i18n.t("agents.bulkSuccess", { verb, count: ok }));
      else toast.error(i18n.t("agents.bulkError", { verb, ok, fail }));
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
        <div class="cat-wrap">
          <button class="ghost cat-btn" bind:this={catBtn} onclick={() => (catMenuOpen = !catMenuOpen)}>
            <span class="truncate">{categoryLabel}</span><ChevronDown size={13} />
          </button>
          {#if catMenuOpen}
            <div class="cat-menu" role="menu" bind:this={catMenu}>
              <button class="cat-opt" role="menuitem" class:on={!ui.agentsCategory} onclick={() => pickCategory(null)}>
                <LayersIcon size={14} /><span class="truncate">{i18n.t("agents.allDivisions")}</span><span class="cat-c">{corpus.agents.length}</span>
              </button>
              {#each corpus.tiles as c (c.slug)}
                {@const Icon = resolveCategoryIcon(c.icon)}
                <button class="cat-opt" role="menuitem" class:on={ui.agentsCategory === c.slug} onclick={() => pickCategory(c.slug)}>
                  <span class="cat-ic" style="color:{corpus.colorOf(c.slug)}"><Icon size={14} /></span><span class="truncate">{c.label}</span><span class="cat-c">{c.count}</span>
                </button>
              {/each}
            </div>
          {/if}
        </div>
        <Input bind:value={query} variant="search" placeholder={i18n.t("agents.searchPlaceholder")} ariaLabel={i18n.t("agents.searchLabel")} />
        {#if visible.length > 0 && !showDivisions}
          {#if selectMode}
            <button class="ghost" onclick={exitSelect}>{i18n.t("common.done")}</button>
          {:else}
            <button class="ghost" onclick={enterSelect}>{i18n.t("common.select")}</button>
          {/if}
        {/if}
        <button class="ghost icon" title={i18n.t("agents.rescanTitle")} aria-label={i18n.t("agents.rescanTitle")} onclick={() => install.reconcile()}>
          <RefreshIcon size={15} />
        </button>
      </div>

      {#if !showDivisions && !selectMode && base.length > 0}
        <div class="seg" role="tablist" aria-label={i18n.t("agents.filterByInstallState")}>
          {#each visibleLenses as f (f.id)}
            <button
              class="seg-btn"
              class:on={lens === f.id}
              role="tab"
              aria-selected={lens === f.id}
              onclick={() => setLens(f.id)}
            >
              {#if f.tone}<span class="seg-dot" data-tone={f.tone}></span>{/if}
              {i18n.t(f.key)}
              <span class="seg-c">{lensCounts[f.id]}</span>
            </button>
          {/each}
        </div>
      {/if}

      {#if selectMode}
        <div class="bulk-bar">
          <input
            type="checkbox"
            class="check"
            checked={allVisibleSelected}
            indeterminate={someSelected}
            onchange={toggleAll}
            aria-label={i18n.t("agents.selectAllVisible")}
          />
          <span class="bulk-count">{i18n.t("common.selected", { count: selected.size })}</span>
          {#if selected.size > 0}
            <div class="bulk-menu-wrap">
              <button class="ghost" bind:this={bulkBtn} disabled={bulkBusy} onclick={() => (menuOpen = !menuOpen)}>
                {bulkBusy ? i18n.t("common.working") : i18n.t("agents.withSelected")}<ChevronDown size={14} />
              </button>
              {#if menuOpen}
                <div class="bulk-menu" role="menu" bind:this={bulkMenu}>
                  <button class="bulk-opt" role="menuitem" onclick={() => { menuOpen = false; bulkInstallOpen = true; }}>
                    <DownloadIcon size={14} /><span>{i18n.t("agents.installSelected")}</span>
                  </button>
                  <div class="bulk-div"></div>
                  <button class="bulk-opt" role="menuitem" disabled={!canBulkUpdate} title={canBulkUpdate ? "" : i18n.t("agents.allSelectedInSync")} onclick={() => runBulk("update", "agents.updatedVerb")}>
                    <RefreshIcon size={14} /><span>{i18n.t("agents.updateSelected")}</span>
                  </button>
                  <button class="bulk-opt" role="menuitem" disabled={!canBulkTrack} title={canBulkTrack ? "" : i18n.t("agents.nothingUntrackedSelection")} onclick={() => runBulk("track", "agents.trackedVerb")}>
                    <PlusIcon size={14} /><span>{i18n.t("agents.trackSelected")}</span>
                  </button>
                  <button class="bulk-opt" class:danger={selHasForeign} role="menuitem" onclick={() => { menuOpen = false; confirmDelete = true; }}>
                    <TrashIcon size={14} /><span>{i18n.t(selHasForeign ? "agents.deleteSelected" : "agents.uninstallSelected")}</span>
                  </button>
                </div>
              {/if}
            </div>
          {/if}
        </div>
      {/if}
    </div>

    <div class="lp-list">
      {#if divisionMeta}
        {@const Icon = resolveCategoryIcon(divisionMeta.icon)}
        <div class="dov">
          <div class="dov-head">
            <span class="dov-ic" style="color:{divisionMeta.color}"><Icon size={18} /></span>
            <div class="dov-id">
              <span class="dov-name">{divisionMeta.label}</span>
              <span class="dov-sub">{i18n.t("agents.divisionOverview", { count: divisionMeta.slugs.length })}</span>
            </div>
            <button class="dov-deploy" onclick={() => (divisionInstallOpen = true)}><DownloadIcon size={14} /> {i18n.t("agents.deployDivision")}</button>
          </div>
          <StarterPrompt template={divisionMeta.prompt} />
        </div>
      {/if}
      {#if corpus.loading && corpus.agents.length === 0}
        <LoadingState rows={6} label={i18n.t("agents.loading")} />
      {:else if corpus.error && corpus.agents.length === 0}
        <EmptyState title={i18n.t("agents.corpusUnavailableTitle")} body={i18n.t("agents.corpusUnavailableBody")}>
          {#snippet icon()}<SearchIcon size={48} />{/snippet}
        </EmptyState>
      {:else if showDivisions}
        <DivisionsLanding />
      {:else if visible.length === 0}
        <EmptyState
          title={lens !== "all"
            ? i18n.t("agents.emptyStateLens", { state: i18n.t(LENSES.find((l) => l.id === lens)?.key ?? "state.all").toLowerCase() })
            : query.trim()
              ? i18n.t("agents.emptySearch", { query: query.trim() })
              : i18n.t("agents.emptyDivision")}
          body={lens !== "all" ? i18n.t("agents.emptyFilteredBody") : i18n.t("agents.emptyBody")}
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
                <input type="checkbox" class="check" checked={selected.has(a.slug)} onchange={() => toggleRow(a.slug)} aria-label={`${i18n.t("common.select")} ${a.name}`} />
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

  {#if panelAgent}
    <!-- ── Resize handle (grows the detail pane when dragged left) ── -->
    <div class="ws-resize">
      <ResizeHandle
        width={ui.detailPaneWidth}
        min={DETAIL_PANE_MIN_WIDTH}
        max={900}
        defaultWidth={DETAIL_PANE_DEFAULT_WIDTH}
        direction="left"
        label={i18n.t("common.resizeDetailPane")}
        onChange={(w) => (ui.detailPaneWidth = clampDetailPaneWidth(w))}
        onCommit={(w) => ui.setDetailPaneWidth(w)}
      />
    </div>

    <!-- ── Detail pane (only when an agent is selected) ── -->
    <aside class="detail-pane" style="width: {ui.detailPaneWidth}px" aria-label={i18n.t("agents.agentDetail")}>
      <div class="dp-bar">
        <button class="dp-close" onclick={closeDetail} aria-label={i18n.t("agents.closeDetail")} title={i18n.t("agents.closeDetail")}><XIcon size={16} /></button>
      </div>
      <div class="dp-scroll">
        <PersonaBody agent={panelAgent} loading={detailLoading} onCategory={(slug) => ui.openDivision(slug)}>
          {#snippet headerAction()}
            {#if panelAgent}
              <button class="dp-install" onclick={() => (installOpen = true)}>
                <DownloadIcon size={14} /> {i18n.t("agents.installAgent")}
              </button>
            {/if}
          {/snippet}
          {#snippet deploy()}
            {#if panelAgent}
              <DeploymentMatrix agent={panelAgent} onDiff={(t) => (diffTarget = t)} />
            {/if}
          {/snippet}
        </PersonaBody>
      </div>
    </aside>

    <!-- Narrow-window overlay scrim: clicking dismisses the overlaid detail pane. -->
    <button class="ws-scrim" aria-label={i18n.t("agents.closeDetail")} onclick={closeDetail}></button>
  {/if}
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

{#if installOpen && panelAgent}
  <InstallModal title={i18n.t("agents.installAgentTitle", { name: panelAgent.name })} agentSlugs={[panelAgent.slug]} onClose={() => (installOpen = false)} />
{/if}

{#if bulkInstallOpen && selected.size > 0}
  <InstallModal
    title={i18n.t("agents.installSelectedTitle", { count: selected.size })}
    agentSlugs={[...selected]}
    onClose={() => (bulkInstallOpen = false)}
  />
{/if}

{#if divisionInstallOpen && divisionMeta}
  <InstallModal
    title={i18n.t("agents.installDivisionTitle", { division: divisionMeta.label })}
    agentSlugs={divisionMeta.slugs}
    onClose={() => (divisionInstallOpen = false)}
  />
{/if}

{#if confirmDelete}
  <button class="cd-scrim" aria-label={i18n.t("common.cancel")} onclick={() => (confirmDelete = false)}></button>
  <div class="cd-box" role="alertdialog" aria-modal="true" aria-label={i18n.t("agents.confirmDeleteAria")}>
    <div class="cd-head"><AlertTriangle size={20} /><h2>{i18n.t(selHasForeign ? "agents.confirmDelete" : "agents.confirmUninstall", { count: selected.size })}</h2></div>
    <p class="cd-body">
      {#if selHasForeign}
        {i18n.t("agents.deleteBody", { count: selInstalls.length })}
      {:else}
        {i18n.t("agents.uninstallBody", { count: selInstalls.length })}
      {/if}
    </p>
    <p class="cd-note">{i18n.t("agents.confirmTip")}</p>
    <div class="cd-actions">
      <button class="cd-cancel" onclick={() => (confirmDelete = false)}>{i18n.t("common.cancel")}</button>
      <button class="cd-delete" disabled={bulkBusy} onclick={() => { confirmDelete = false; runBulk("uninstall", selHasForeign ? "agents.deletedVerb" : "agents.uninstalledVerb"); }}>
        <TrashIcon size={14} /> {i18n.t(selHasForeign ? "common.delete" : "common.uninstall")} {selInstalls.length}
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

  .cat-wrap { position: relative; }
  .cat-btn { max-width: 180px; }
  .cat-menu {
    position: absolute; top: calc(100% + 4px); left: 0; z-index: 30;
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
  /* Division icon tinted with the division's brand color; dim to neutral when
     the row is the active selection so the brand-blue "on" state stays legible. */
  .cat-ic { display: inline-flex; flex: none; }
  .cat-opt.on .cat-ic { color: var(--color-brand) !important; }
  .cat-c { font-size: var(--text-caption); color: var(--color-text-muted); }

  .bulk-bar { display: flex; align-items: center; gap: var(--space-2); }
  .bulk-count { font-size: var(--text-body-sm); color: var(--color-brand); font-weight: var(--fw-medium); }

  /* ── Install-state lens (mirrors the Tools view segmented filter) ── */
  .seg {
    display: flex; align-items: center; gap: 2px; flex-wrap: wrap; min-width: 0;
    margin-top: var(--space-2); padding: 2px;
    background: var(--color-surface-sunken);
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
  .seg-dot { width: 7px; height: 7px; border-radius: 999px; background: var(--color-text-muted); flex: none; }
  .seg-dot[data-tone="ok"]     { background: var(--color-success); }
  .seg-dot[data-tone="warn"]   { background: var(--color-warning); }
  .seg-dot[data-tone="info"]   { background: var(--color-brand); }
  .seg-dot[data-tone="danger"] { background: var(--color-danger); }
  .seg-c { color: var(--color-text-muted); font-variant-numeric: tabular-nums; }
  .seg-btn.on .seg-c { color: var(--color-text-secondary); }
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
  .bulk-div { height: 1px; margin: 4px 0; background: var(--color-border); }

  .check { accent-color: var(--color-brand); cursor: pointer; width: 15px; height: 15px; flex: none; }

  /* ── Division overview banner ── */
  .dov {
    margin: var(--space-1) var(--space-1) var(--space-3);
    padding: var(--space-3);
    border: 1px solid var(--color-border); border-radius: var(--radius-lg);
    background: var(--color-surface-raised);
    display: flex; flex-direction: column; gap: var(--space-3);
  }
  .dov-head { display: flex; align-items: center; gap: var(--space-3); }
  .dov-ic { flex: none; display: inline-flex; align-items: center; justify-content: center; width: 34px; height: 34px; border-radius: var(--radius-md); background: var(--color-surface-sunken); }
  .dov-id { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 1px; }
  .dov-name { font-size: var(--text-body); font-weight: var(--fw-semibold); color: var(--color-text-primary); }
  .dov-sub { font-size: var(--text-caption); color: var(--color-text-muted); }
  .dov-deploy {
    flex: none; display: inline-flex; align-items: center; gap: 6px;
    height: 30px; padding: 0 12px; border-radius: var(--radius-md);
    border: 1px solid var(--color-border); background: var(--color-surface-sunken);
    color: var(--color-text-primary); font-size: var(--text-body-sm); cursor: pointer;
  }
  .dov-deploy:hover { border-color: var(--color-brand); }

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
  .dp-install {
    display: inline-flex; align-items: center; gap: 6px;
    height: 30px; padding: 0 12px; border-radius: var(--radius-md);
    background: var(--color-brand); color: var(--color-text-inverse);
    font-size: var(--text-body-sm); font-weight: var(--fw-medium); cursor: pointer;
  }
  .dp-install:hover { filter: brightness(1.08); }
  .dp-scroll { flex: 1; overflow-y: auto; min-height: 0; }

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
