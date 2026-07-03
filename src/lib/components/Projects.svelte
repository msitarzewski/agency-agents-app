<script lang="ts">
  /**
   * Projects — the 4th pillar (Agents / Tools / Teams / Projects). The home for
   * per-project deployments: every folder you've installed agents into.
   *
   * Master/detail (not inline disclosures — a big roster gets unruly):
   *  • List: a row per project (folder · label · path · count). Clicking a row
   *    navigates into its detail via `ui.selectProject(path)` — a nav location,
   *    so the title-bar Back button returns to the list.
   *  • Detail: that project's path, actions (Deploy… · Reveal · Remove from
   *    list), and its roster grouped by division (collapsible), reusing the
   *    division-group pattern from Tools/Teams.
   *
   * "Deploy…" opens the two-pane DeployBrowser scoped to the project (pick an
   * agent, division, or team on the left; install into the project's tools on
   * the right) — so an empty project can be filled.
   */
  import { onMount } from "svelte";
  import EmptyState from "./EmptyState.svelte";
  import Pill from "./Pill.svelte";
  import Button from "./Button.svelte";
  import DeployBrowser from "./DeployBrowser.svelte";
  import FolderIcon from "@lucide/svelte/icons/folder";
  import FolderPlus from "@lucide/svelte/icons/folder-plus";
  import FolderOpen from "@lucide/svelte/icons/folder-open";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import LayersIcon from "@lucide/svelte/icons/layers";
  import Trash2 from "@lucide/svelte/icons/trash-2";

  import { install } from "$lib/stores/install.svelte";
  import { corpus } from "$lib/stores/corpus.svelte";
  import { projects } from "$lib/stores/projects.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import { resolveCategoryIcon } from "$lib/util/categoryIcon";
  import { i18n } from "$lib/stores/i18n.svelte";
  import type { InstalledAgent } from "$lib/types";

  onMount(() => {
    corpus.ensureLoaded();
    projects.refresh();
  });

  // ── Per-project roster: rows we (or anyone) deployed into that exact path. ──
  // Keyed by the project's absolute path; null projectPath = global, excluded.
  const rowsByProject = $derived.by(() => {
    const m = new Map<string, InstalledAgent[]>();
    for (const r of install.installed) {
      if (r.state === "removed") continue; // ledger says installed but file gone
      const p = r.projectPath;
      if (p == null) continue; // global scope lives in Teams/Tools, not here
      const arr = m.get(p);
      if (arr) arr.push(r);
      else m.set(p, [r]);
    }
    return m;
  });

  function rosterFor(path: string): InstalledAgent[] {
    return rowsByProject.get(path) ?? [];
  }

  // ── Selected project (detail pane). Resolve against the live list so a stale
  //    path (e.g. removed) falls back to the list rather than an empty detail. ──
  const selected = $derived(projects.list.find((p) => p.path === ui.projectsSelected) ?? null);

  // ── Group the selected project's roster by division (collapsible). ──
  const OTHER = "__other";
  const detailGroups = $derived.by(() => {
    if (!selected) return [];
    const divOf = new Map(corpus.agents.map((a) => [a.slug, a.category]));
    const m = new Map<string, InstalledAgent[]>();
    for (const r of rosterFor(selected.path)) {
      const div = divOf.get(r.slug) ?? OTHER;
      const arr = m.get(div);
      if (arr) arr.push(r);
      else m.set(div, [r]);
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

  // Division groups are CLOSED by default; initialize the collapse set to every
  // division once per project (guarded so installs/removes don't re-collapse).
  let collapsed = $state<Set<string>>(new Set());
  function toggleGroup(slug: string) {
    const next = new Set(collapsed);
    if (next.has(slug)) next.delete(slug);
    else next.add(slug);
    collapsed = next;
  }
  let collapseInitFor: string | null = null;
  $effect(() => {
    const p = ui.projectsSelected;
    // Wait for the roster to populate before seeding (cold corpus load) so the
    // divisions stay closed by default rather than seeding from an empty list.
    if (p === collapseInitFor || detailGroups.length === 0) return;
    collapseInitFor = p;
    collapsed = new Set(detailGroups.map((g) => g.slug));
  });

  // ── Deploy into a project: the two-pane DeployBrowser. ──
  let browseFor = $state<string | null>(null); // project path, or null = closed

  async function reveal(path: string) {
    try {
      await install.revealPath(path);
    } catch (e) {
      toast.error(i18n.t("common.couldNotOpenFolder"), String(e));
    }
  }

  function removeFromList(path: string) {
    projects.unregister(path);
    if (ui.projectsSelected === path) ui.selectProject(null);
  }

  let adding = $state(false);
  async function addProject() {
    if (adding) return;
    adding = true;
    try {
      const p = await projects.addViaPicker();
      if (!p) return;
      await projects.refresh();
      ui.selectProject(p); // land in the new project's detail (Deploy… from there)
    } finally {
      adding = false;
    }
  }
</script>

<section class="pr">
  {#if selected}
    <!-- ── Detail ── -->
    <header class="pr-head detail">
      <span class="dh-ic"><FolderIcon size={20} /></span>
      <div class="dh-id">
        <h2 class="dh-label">{selected.label}</h2>
        <button class="dh-path" title={selected.path} onclick={() => reveal(selected.path)}>{selected.path}</button>
      </div>
      <span class="dh-count">{i18n.count(selected.installedCount, "common.agent.one", "common.agent.many")}</span>
      <button class="btn" onclick={() => reveal(selected.path)}><FolderOpen size={15} /><span>{i18n.t("common.reveal")}</span></button>
      <button class="btn primary" onclick={() => (browseFor = selected.path)}>{i18n.t("teams.deploy")}</button>
      <button class="btn danger-ic" title={i18n.t("projects.removeTitle")} aria-label={i18n.t("projects.removeAria")} onclick={() => removeFromList(selected.path)}><Trash2 size={15} /></button>
    </header>

    <div class="scroll">
      {#if detailGroups.length === 0}
        <div class="d-empty">
          <LayersIcon size={40} />
          <p>{i18n.t("projects.emptyHere")}</p>
          <Button variant="primary" onclick={() => (browseFor = selected.path)}>{i18n.t("projects.deployDivisionTeam")}</Button>
        </div>
      {:else}
        <div class="groups">
          {#each detailGroups as g (g.slug)}
            {@const Icon = resolveCategoryIcon(g.icon)}
            {@const isOpen = !collapsed.has(g.slug)}
            <section class="grp">
              <button class="grp-head" onclick={() => toggleGroup(g.slug)} aria-expanded={isOpen}>
                <ChevronDown size={15} class={isOpen ? "pr-chev open" : "pr-chev"} />
                <span class="grp-ic" style="color:{g.color}"><Icon size={15} /></span>
                <span class="grp-label">{g.label}</span>
                <span class="grp-count">{g.rows.length}</span>
              </button>
              {#if isOpen}
                <ul class="roster">
                  {#each g.rows as r (r.slug + r.tool)}
                    <li class="r-row">
                      <span class="r-name">{r.name}</span>
                      <Pill tone="neutral">{install.toolLabel(r.tool)}</Pill>
                    </li>
                  {/each}
                </ul>
              {/if}
            </section>
          {/each}
        </div>
      {/if}
    </div>
  {:else}
    <!-- ── List ── -->
    <header class="pr-head">
      <p class="pr-count">{i18n.t("projects.count", { count: projects.list.length })}</p>
      <div class="pr-actions">
        <button class="btn primary" disabled={adding} onclick={addProject}>
          <FolderPlus size={15} /><span>{i18n.t("projects.add")}</span>
        </button>
      </div>
    </header>

    {#if projects.list.length === 0}
      <div class="scroll">
        <EmptyState title={i18n.t("projects.emptyTitle")}>
          {#snippet icon()}<FolderIcon size={48} />{/snippet}
          {i18n.t("projects.emptyBody")}
          {#snippet cta()}
            <div class="empty-cta">
              <Button variant="primary" disabled={adding} onclick={addProject}>
                {#snippet icon()}<FolderPlus size={15} />{/snippet}
                {i18n.t("projects.add")}
              </Button>
              <button class="link-btn" onclick={() => ui.openPlaybook()}>{i18n.t("projects.openPlaybook")}</button>
            </div>
          {/snippet}
        </EmptyState>
      </div>
    {:else}
      <ul class="rows">
        {#each projects.list as project (project.path)}
          <li class="proj">
            <button class="proj-row" onclick={() => ui.selectProject(project.path)}>
              <span class="proj-ic"><FolderIcon size={18} /></span>
              <span class="proj-body">
                <span class="proj-label">{project.label}</span>
                <span class="proj-path" title={project.path}>{project.path}</span>
              </span>
              <span class="proj-count">{i18n.count(project.installedCount, "common.agent.one", "common.agent.many")}</span>
              <ChevronRight size={16} class="proj-go" />
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  {/if}
</section>

{#if browseFor !== null}
  <DeployBrowser projectPath={browseFor} onClose={() => (browseFor = null)} />
{/if}

<style>
  .pr { display: flex; flex-direction: column; height: 100%; min-height: 0; }
  .pr-head {
    flex: none; display: flex; align-items: center; justify-content: space-between; gap: var(--space-3);
    padding: var(--space-3) var(--space-4); border-bottom: 1px solid var(--color-border);
  }
  .pr-count { color: var(--color-text-secondary); font-size: var(--text-body-sm); }
  .pr-actions { display: flex; gap: var(--space-2); }

  /* ── Detail header ── */
  .pr-head.detail { justify-content: flex-start; }
  .dh-ic {
    flex: none; display: inline-flex; align-items: center; justify-content: center;
    width: 40px; height: 40px; border-radius: var(--radius-md);
    background: var(--color-surface-sunken); color: var(--color-text-secondary);
  }
  .dh-id { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 1px; }
  .dh-label { font-size: var(--text-h2); font-weight: var(--fw-semibold); color: var(--color-text-primary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .dh-path {
    font-size: var(--text-caption); color: var(--color-text-muted); background: transparent;
    text-align: left; cursor: pointer; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 100%;
  }
  .dh-path:hover { color: var(--color-brand); text-decoration: underline; }
  .dh-count { flex: none; font-size: var(--text-body-sm); color: var(--color-text-muted); font-variant-numeric: tabular-nums; white-space: nowrap; }

  .btn {
    display: inline-flex; align-items: center; gap: 6px;
    height: 32px; padding: 0 var(--space-3);
    border: 1px solid var(--color-border); border-radius: var(--radius-md);
    background: transparent; color: var(--color-text-secondary);
    font-size: var(--text-body-sm); cursor: pointer; flex: none;
  }
  .btn:hover:not(:disabled) { color: var(--color-text-primary); background: var(--color-surface-sunken); }
  .btn:disabled { opacity: 0.5; cursor: default; }
  .btn.primary { background: var(--color-brand); color: var(--color-text-inverse); border-color: transparent; }
  .btn.primary:hover:not(:disabled) { filter: brightness(1.08); background: var(--color-brand); }
  .btn.danger-ic { padding: 0; width: 32px; justify-content: center; }
  .btn.danger-ic:hover { color: var(--color-danger); border-color: var(--color-danger); background: color-mix(in srgb, var(--color-danger) 10%, transparent); }

  .scroll { flex: 1; min-height: 0; overflow-y: auto; }

  .empty-cta { display: flex; flex-direction: column; align-items: center; gap: var(--space-2); }
  .link-btn { background: transparent; color: var(--color-text-link, var(--color-brand)); font-size: var(--text-body-sm); cursor: pointer; padding: 2px; }
  .link-btn:hover { text-decoration: underline; }

  /* ── Project list rows ── */
  .rows { flex: 1; min-height: 0; overflow-y: auto; list-style: none; margin: 0; padding: var(--space-3); display: flex; flex-direction: column; gap: var(--space-2); }
  .proj { border: 1px solid var(--color-border); border-radius: var(--radius-lg); background: var(--color-surface-raised); overflow: hidden; }
  .proj:hover { border-color: var(--color-border-strong, var(--color-text-muted)); }
  .proj-row {
    width: 100%; display: flex; align-items: center; gap: var(--space-3);
    padding: var(--space-3); background: transparent; cursor: pointer; text-align: left;
  }
  .proj-row:hover { background: var(--color-surface-sunken); }
  .proj-ic { flex: none; display: inline-flex; align-items: center; justify-content: center; width: 36px; height: 36px; border-radius: var(--radius-md); background: var(--color-surface-sunken); color: var(--color-text-secondary); }
  .proj-body { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 1px; }
  .proj-label { font-weight: var(--fw-semibold); color: var(--color-text-primary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .proj-path { font-size: var(--text-caption); color: var(--color-text-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .proj-count { flex: none; font-size: var(--text-body-sm); color: var(--color-text-muted); font-variant-numeric: tabular-nums; white-space: nowrap; }
  :global(.proj-go) { flex: none; color: var(--color-text-muted); }

  /* ── Division groups (detail roster) ── */
  .groups { padding: var(--space-3); display: flex; flex-direction: column; gap: 2px; }
  .grp { display: flex; flex-direction: column; }
  .grp-head {
    display: flex; align-items: center; gap: var(--space-2);
    width: 100%; padding: var(--space-2); border-radius: var(--radius-sm);
    background: transparent; cursor: pointer; text-align: left;
  }
  .grp-head:hover { background: var(--color-surface-sunken); }
  :global(.pr-chev) { color: var(--color-text-muted); transition: transform var(--motion-duration-fast, 120ms) ease; transform: rotate(-90deg); flex: none; }
  :global(.pr-chev.open) { transform: rotate(0deg); }
  .grp-ic { flex: none; display: inline-flex; }
  .grp-label { flex: 1; min-width: 0; font-weight: var(--fw-semibold); color: var(--color-text-primary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .grp-count { flex: none; min-width: 20px; text-align: center; font-size: var(--text-caption); color: var(--color-text-muted); font-variant-numeric: tabular-nums; background: var(--color-surface-sunken); border-radius: var(--radius-full); padding: 1px 7px; }

  .roster { list-style: none; margin: 0; padding: 2px 0 var(--space-2) var(--space-4); display: flex; flex-direction: column; gap: 1px; }
  .r-row { display: flex; align-items: center; gap: var(--space-3); padding: var(--space-2) var(--space-3); border-radius: var(--radius-md); }
  .r-row:hover { background: var(--color-surface-sunken); }
  .r-name { flex: 1; min-width: 0; font-weight: var(--fw-medium); color: var(--color-text-primary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

  .d-empty { height: 100%; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: var(--space-3); color: var(--color-text-muted); padding: var(--space-6); }
  .d-empty p { font-size: var(--text-body-sm); }

</style>
