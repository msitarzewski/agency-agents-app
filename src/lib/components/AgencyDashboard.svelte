<script lang="ts">
  /**
   * Agency Dashboard — the at-a-glance overview. Four rollup stats, then the
   * charts: install-health donut, coverage-by-tool bars, the cross-tool coverage
   * matrix (category × tool), and the catalog's category distribution. Every
   * surface deep-links into the Agents workspace (with the matching filter) or
   * the Tools view. All charts are dependency-free (SVG + CSS).
   */
  import { onMount } from "svelte";
  import { corpus } from "$lib/stores/corpus.svelte";
  import { install, SUPPORTED_TOOLS } from "$lib/stores/install.svelte";
  import { projects } from "$lib/stores/projects.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { i18n } from "$lib/i18n.svelte";
  import { toolAccent, toolLabel } from "$lib/data/toolRegistry";
  import HealthDonut from "./HealthDonut.svelte";
  import CoverageDonuts from "./CoverageDonuts.svelte";
  import CatalogByDivision from "./CatalogByDivision.svelte";
  import InstallSunburst from "./InstallSunburst.svelte";
  import type { InstalledAgent } from "$lib/types";

  // Pure reader — install state loaded globally in +layout.
  onMount(() => {
    corpus.ensureLoaded();
    projects.refresh();
  });

  const available = $derived(corpus.agents.length);
  const managed = $derived(install.installed.filter((i) => i.state !== "foreign").length);
  // Of that present-on-disk total, how many THIS app installed (ledger-tracked)
  // vs. picked up from elsewhere (a prior CLI `install.sh` run, byte-matched).
  const trackedByApp = $derived(install.installed.filter((i) => i.tracked && i.state !== "foreign").length);
  const fromOtherTools = $derived(managed - trackedByApp);
  const attention = $derived(
    install.installed.filter((i) => ["outdated", "modified", "removed"].includes(i.state)).length,
  );
  const foreign = $derived(install.installed.filter((i) => i.state === "foreign").length);
  const totalInstalls = $derived(install.installed.length);

  // ── Install-health donut (every install row, split by reconciled state) ──
  const byState = $derived.by(() => {
    const c = { current: 0, outdated: 0, modified: 0, foreign: 0, removed: 0 };
    for (const i of install.installed) c[i.state]++;
    return c;
  });
  const healthSegments = $derived([
    { label: i18n.t("state.current"),  value: byState.current,  color: "var(--color-success)", onClick: () => ui.openAgents(null, "current") },
    { label: i18n.t("state.outdated"), value: byState.outdated, color: "var(--color-warning)", onClick: () => ui.openAgents(null, "outdated") },
    { label: i18n.t("state.modified"), value: byState.modified, color: "color-mix(in srgb, var(--color-warning) 55%, var(--color-danger))", onClick: () => ui.openAgents(null, "outdated") },
    { label: i18n.t("state.foreign"),  value: byState.foreign,  color: "var(--color-brand)",   onClick: () => ui.openAgents(null, "foreign") },
    { label: i18n.t("state.removed"),  value: byState.removed,  color: "var(--color-danger)",  onClick: () => ui.openAgents(null, "removed") },
  ]);

  // ── Coverage by tool — only tools that actually hold agents (less noise) ──
  const perTool = $derived(
    SUPPORTED_TOOLS.map((t) => ({
      id: t.id,
      label: t.label,
      count: install.installed.filter((i) => i.tool === t.id).length,
      detected: install.tools.find((x) => x.tool === t.id)?.detected ?? false,
    })).filter((t) => t.count > 0),
  );
  const maxTool = $derived(Math.max(1, ...perTool.map((t) => t.count)));

  // Shared division highlight for the merged "Cross-tool coverage" card: the
  // per-tool donuts and the catalog-by-division bar light the same division.
  let divisionHover = $state<string | null>(null);

  // ── "Installed by you": two-ring donut — Global vs Projects, broken down by
  //    tool / by project on the outer ring. Uses the same `managed` set as the
  //    stat so the numbers reconcile. ──
  const managedRows = $derived(install.installed.filter((i) => i.state !== "foreign"));
  const globalRows = $derived(managedRows.filter((i) => i.projectPath == null));
  const projectRows = $derived(managedRows.filter((i) => i.projectPath != null));

  function basename(p: string): string {
    return p.replace(/[\\/]+$/, "").split(/[\\/]/).pop() || p;
  }
  // Distinct-but-cohesive hue per project (golden-angle spacing).
  function projectColor(i: number): string {
    return `hsl(${(i * 137.508 + 25) % 360} 60% 55%)`;
  }

  const globalByTool = $derived.by(() => {
    const m = new Map<string, number>();
    for (const r of globalRows) m.set(r.tool, (m.get(r.tool) ?? 0) + 1);
    return [...m.entries()]
      .sort((a, b) => b[1] - a[1])
      .map(([id, value]) => ({ label: toolLabel(id), value, color: toolAccent(id), onClick: () => ui.openTools(id) }));
  });
  const projByProject = $derived.by(() => {
    const m = new Map<string, number>();
    for (const r of projectRows) m.set(r.projectPath!, (m.get(r.projectPath!) ?? 0) + 1);
    return [...m.entries()]
      .sort((a, b) => b[1] - a[1])
      .map(([path, value], i) => ({ label: basename(path), value, color: projectColor(i), onClick: () => ui.selectProject(path) }));
  });
  const sunburstGroups = $derived([
    { label: i18n.t("common.global"), value: globalRows.length, color: "var(--color-brand)", onClick: () => ui.setSection("tools"), children: globalByTool },
    { label: i18n.t("common.projects"), value: projectRows.length, color: "var(--color-success)", onClick: () => ui.setSection("projects"), children: projByProject },
  ]);

  // ── Projects list with per-division breakdown (project-scoped installs). ──
  const slugCat = $derived(new Map(corpus.agents.map((a) => [a.slug, a.category])));
  const projectBreakdown = $derived.by(() => {
    const byPath = new Map<string, InstalledAgent[]>();
    for (const r of projectRows) {
      const arr = byPath.get(r.projectPath!);
      if (arr) arr.push(r);
      else byPath.set(r.projectPath!, [r]);
    }
    return [...byPath.entries()]
      .map(([path, rows]) => {
        const divs = new Map<string, number>();
        for (const r of rows) {
          const c = slugCat.get(r.slug) ?? "uncategorized";
          divs.set(c, (divs.get(c) ?? 0) + 1);
        }
        const divisions = [...divs.entries()]
          .map(([slug, count]) => ({ slug, label: corpus.labelOf(slug), color: corpus.colorOf(slug), count }))
          .sort((a, b) => b.count - a.count);
        return { path, label: basename(path), total: rows.length, divisions };
      })
      .sort((a, b) => b.total - a.total);
  });
</script>

<section class="dash">
  <div class="stats">
    <button class="stat" onclick={() => ui.openAgents()}>
      <span class="s-num">{available}</span>
      <span class="s-lbl">{i18n.t("dashboard.available")}</span>
    </button>
    <button class="stat" onclick={() => ui.openAgents()}>
      <span class="s-num">{managed}</span>
      <span class="s-lbl">{i18n.t("dashboard.totalInstalled")}</span>
      {#if fromOtherTools > 0}
        <span class="s-sub">{i18n.t("dashboard.viaThisApp", { tracked: trackedByApp, other: fromOtherTools })}</span>
      {/if}
    </button>
    {#if attention > 0}
      <button class="stat warn" onclick={() => ui.openAgents(null, "attention")}>
        <span class="s-num">{attention}</span>
        <span class="s-lbl">{i18n.t("dashboard.needAttention")}</span>
      </button>
    {/if}
    {#if foreign > 0}
      <button class="stat info" onclick={() => ui.openAgents(null, "foreign")}>
        <span class="s-num">{foreign}</span>
        <span class="s-lbl">{i18n.t("dashboard.foundToTrack")}</span>
      </button>
    {/if}
  </div>

  <div class="cols">
    <div class="card">
      <h3 class="c-title">{i18n.t("dashboard.totalInstalled")}</h3>
      <div class="card-fill center">
        {#if managed === 0}
          <p class="muted">{i18n.t("dashboard.emptyInstalled")}</p>
        {:else}
          <InstallSunburst groups={sunburstGroups} />
        {/if}
      </div>
    </div>

    <div class="card">
      <h3 class="c-title">{i18n.t("dashboard.projects")}</h3>
      <div class="card-fill">
        {#if projectBreakdown.length === 0}
        <p class="muted">
          {i18n.t("dashboard.noProjectInstalls")}
          <button class="link inline" onclick={() => ui.setSection("projects")}>{i18n.t("dashboard.openProjects")}</button>
        </p>
      {:else}
        <ul class="proj-list">
          {#each projectBreakdown as p (p.path)}
            <li>
              <button class="proj-row" onclick={() => ui.selectProject(p.path)} title={p.path}>
                <span class="proj-top">
                  <span class="proj-name">{p.label}</span>
                  <span class="proj-total">{i18n.t("dashboard.projectAgents", { count: p.total, agents: i18n.agents(p.total) })}</span>
                </span>
                <span class="proj-bar">
                  {#each p.divisions as d (d.slug)}
                    <span class="proj-seg" style="flex:{d.count};background:{d.color}" title="{d.label}: {d.count}"></span>
                  {/each}
                </span>
                <span class="proj-divs">
                  {#each p.divisions.slice(0, 4) as d (d.slug)}
                    <span class="proj-div"><span class="pd-dot" style="background:{d.color}"></span>{d.label} {d.count}</span>
                  {/each}
                  {#if p.divisions.length > 4}<span class="proj-div more">{i18n.t("common.more", { count: p.divisions.length - 4 })}</span>{/if}
                </span>
              </button>
            </li>
          {/each}
        </ul>
      {/if}
      </div>
    </div>
  </div>

  <div class="cols">
    <div class="card">
      <h3 class="c-title">{i18n.t("dashboard.installHealth")}</h3>
      <div class="card-fill center">
        {#if totalInstalls === 0}
          <p class="muted">{i18n.t("dashboard.emptyHealth")}</p>
        {:else}
          <HealthDonut segments={healthSegments} />
        {/if}
      </div>
    </div>

    <div class="card">
      <h3 class="c-title">{i18n.t("dashboard.coverageByTool")}</h3>
      <div class="card-fill">
        {#if perTool.length === 0}
        <p class="muted">{i18n.t("dashboard.emptyCoverage")}</p>
      {:else}
        <ul class="bars">
          {#each perTool as t (t.id)}
            <li>
              <button class="bar-btn" onclick={() => ui.openTools(t.id)} title={t.detected ? i18n.t("common.detectedDevice") : i18n.t("common.notDetectedDevice")}>
                <span class="tool-dot" class:off={!t.detected}></span>
                <span class="bar-label">{t.label}</span>
                <span class="bar-track"><span class="bar-fill" style="width:{(t.count / maxTool) * 100}%"></span></span>
                <span class="bar-count">{t.count}</span>
              </button>
            </li>
          {/each}
        </ul>
      {/if}
      </div>
      <button class="link" onclick={() => ui.setSection("tools")}>{i18n.t("dashboard.manageTools")}</button>
    </div>
  </div>

  <div class="card">
    <h3 class="c-title">{i18n.t("dashboard.crossToolCoverage")}</h3>
    <CoverageDonuts bind:hovered={divisionHover} />
    <div class="merge-sep">
      <span class="merge-cap">{i18n.t("dashboard.catalogByDivision")}</span>
    </div>
    <CatalogByDivision bind:hovered={divisionHover} />
  </div>
</section>

<style>
  .dash { height: 100%; overflow-y: auto; padding: var(--space-5); display: flex; flex-direction: column; gap: var(--space-4); }
  .stats { display: grid; grid-template-columns: repeat(auto-fit, minmax(160px, 1fr)); gap: var(--space-3); }
  .stat {
    display: flex; flex-direction: column; gap: 4px; align-items: flex-start;
    padding: var(--space-4); border: 1px solid var(--color-border); border-radius: var(--radius-lg);
    background: var(--color-surface-raised); cursor: pointer; text-align: left;
  }
  .stat:hover { border-color: var(--color-brand); }
  .s-num { font-size: 30px; font-weight: var(--fw-bold); color: var(--color-text-primary); line-height: 1; }
  .s-lbl { font-size: var(--text-body-sm); color: var(--color-text-muted); }
  .s-sub { font-size: 11px; color: var(--color-text-muted); opacity: 0.85; margin-top: 3px; }
  .stat.warn .s-num { color: var(--color-warning); }
  .stat.info .s-num { color: var(--color-info, var(--color-brand)); }

  /* Cards in a row stretch to equal height (default grid behavior) so neighbours
     never leave a dangling gap; their content fills via `.card-fill`. */
  .cols { display: grid; grid-template-columns: 1fr 1fr; gap: var(--space-4); align-items: stretch; }
  @media (max-width: 820px) { .cols { grid-template-columns: 1fr; } }
  .card {
    border: 1px solid var(--color-border); border-radius: var(--radius-lg);
    background: var(--color-surface-raised); padding: var(--space-4); min-width: 0;
    display: flex; flex-direction: column;
  }
  .c-title { flex: none; font-size: var(--text-body-sm); font-weight: var(--fw-semibold); color: var(--color-text-secondary); margin-bottom: var(--space-3); text-transform: uppercase; letter-spacing: 0.04em; }
  /* The body claims the leftover height. `.center` vertically centers a chart so
     a short donut sits balanced in a tall card instead of hugging the title. */
  .card-fill { flex: 1; min-height: 0; display: flex; flex-direction: column; }
  .card-fill.center { justify-content: center; }
  .muted { color: var(--color-text-muted); font-size: var(--text-body-sm); }

  /* ── Generic bar list (coverage-by-tool) ── */
  .bars { display: flex; flex-direction: column; gap: 2px; }
  .bar-btn {
    display: flex; align-items: center; gap: var(--space-2); width: 100%;
    padding: 6px var(--space-2); border-radius: var(--radius-sm);
    background: transparent; cursor: pointer; text-align: left;
  }
  .bar-btn:hover { background: var(--color-surface-sunken); }
  .bar-label { width: 116px; font-size: var(--text-body-sm); color: var(--color-text-primary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; flex: none; }
  .bar-track { flex: 1; height: 6px; background: var(--color-surface-sunken); border-radius: var(--radius-full); overflow: hidden; min-width: 24px; }
  .bar-fill { display: block; height: 100%; background: var(--color-brand); border-radius: var(--radius-full); }
  .bar-count { width: 30px; text-align: right; font-size: var(--text-caption); color: var(--color-text-muted); font-variant-numeric: tabular-nums; flex: none; }

  .tool-dot { width: 8px; height: 8px; border-radius: var(--radius-full); background: var(--color-success); flex: none; }
  .tool-dot.off { background: var(--color-text-muted); opacity: 0.5; }

  .link { background: transparent; color: var(--color-brand); font-size: var(--text-body-sm); cursor: pointer; padding: 0; margin-top: var(--space-3); }
  .link.inline { margin-top: 0; }
  .link:hover { text-decoration: underline; }

  /* ── Projects list with per-division breakdown ── */
  .proj-list { display: flex; flex-direction: column; gap: var(--space-2); }
  .proj-row {
    display: flex; flex-direction: column; gap: 6px; width: 100%;
    padding: var(--space-2) var(--space-3); border-radius: var(--radius-md);
    background: transparent; cursor: pointer; text-align: left;
  }
  .proj-row:hover { background: var(--color-surface-sunken); }
  .proj-top { display: flex; align-items: baseline; gap: var(--space-2); }
  .proj-name { flex: 1; min-width: 0; font-size: var(--text-body-sm); font-weight: var(--fw-semibold); color: var(--color-text-primary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .proj-total { flex: none; font-size: var(--text-caption); color: var(--color-text-muted); font-variant-numeric: tabular-nums; }
  .proj-bar { display: flex; height: 7px; border-radius: var(--radius-full); overflow: hidden; background: var(--color-surface-sunken); }
  .proj-seg { display: block; min-width: 2px; }
  .proj-divs { display: flex; flex-wrap: wrap; gap: 4px 10px; }
  .proj-div { display: inline-flex; align-items: center; gap: 5px; font-size: var(--text-caption); color: var(--color-text-secondary); font-variant-numeric: tabular-nums; }
  .proj-div.more { color: var(--color-text-muted); }
  .pd-dot { width: 7px; height: 7px; border-radius: 999px; flex: none; }

  /* Separator between the donuts and the catalog-by-division bar (the merged
     card's division reference). A hairline with a small inset caption. */
  .merge-sep {
    display: flex; align-items: center; gap: var(--space-2);
    margin: var(--space-4) 0 var(--space-2);
  }
  .merge-sep::after { content: ""; flex: 1; height: 1px; background: var(--color-border); }
  .merge-cap { font-size: var(--text-caption); font-weight: var(--fw-semibold); color: var(--color-text-muted); text-transform: uppercase; letter-spacing: 0.04em; }
</style>
