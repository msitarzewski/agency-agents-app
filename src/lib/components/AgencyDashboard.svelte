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
  import { ui } from "$lib/stores/ui.svelte";
  import HealthDonut from "./HealthDonut.svelte";
  import CoverageDonuts from "./CoverageDonuts.svelte";
  import CatalogByDivision from "./CatalogByDivision.svelte";

  // Pure reader — install state loaded globally in +layout.
  onMount(() => corpus.ensureLoaded());

  const available = $derived(corpus.agents.length);
  const managed = $derived(install.installed.filter((i) => i.state !== "foreign").length);
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
    { label: "In sync",   value: byState.current,  color: "var(--color-success)", onClick: () => ui.openAgents() },
    { label: "Outdated",  value: byState.outdated,  color: "var(--color-warning)", onClick: () => ui.openAgents() },
    { label: "Modified",  value: byState.modified,  color: "color-mix(in srgb, var(--color-warning) 55%, var(--color-danger))", onClick: () => ui.openAgents() },
    { label: "Untracked", value: byState.foreign,   color: "var(--color-brand)",   onClick: () => ui.openAgents() },
    { label: "Missing",   value: byState.removed,   color: "var(--color-danger)",  onClick: () => ui.openAgents() },
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
</script>

<section class="dash">
  <div class="stats">
    <button class="stat" onclick={() => ui.openAgents()}>
      <span class="s-num">{available}</span>
      <span class="s-lbl">agents available</span>
    </button>
    <button class="stat" onclick={() => ui.openAgents()}>
      <span class="s-num">{managed}</span>
      <span class="s-lbl">installed by you</span>
    </button>
    {#if attention > 0}
      <button class="stat warn" onclick={() => ui.openAgents()}>
        <span class="s-num">{attention}</span>
        <span class="s-lbl">need attention</span>
      </button>
    {/if}
    {#if foreign > 0}
      <button class="stat info" onclick={() => ui.openAgents()}>
        <span class="s-num">{foreign}</span>
        <span class="s-lbl">found to track</span>
      </button>
    {/if}
  </div>

  <div class="cols">
    <div class="card">
      <h3 class="c-title">Install health</h3>
      {#if totalInstalls === 0}
        <p class="muted">Nothing installed yet — deploy an agent to see its health here.</p>
      {:else}
        <HealthDonut segments={healthSegments} centerLabel={String(totalInstalls)} centerSub="installs" />
      {/if}
    </div>

    <div class="card">
      <h3 class="c-title">Coverage by tool</h3>
      {#if perTool.length === 0}
        <p class="muted">No agents installed yet — deploy one and it'll show up here.</p>
      {:else}
        <ul class="bars">
          {#each perTool as t (t.id)}
            <li>
              <button class="bar-btn" onclick={() => ui.openTools(t.id)} title={t.detected ? "Detected on this device" : "Not detected on this device"}>
                <span class="tool-dot" class:off={!t.detected}></span>
                <span class="bar-label">{t.label}</span>
                <span class="bar-track"><span class="bar-fill" style="width:{(t.count / maxTool) * 100}%"></span></span>
                <span class="bar-count">{t.count}</span>
              </button>
            </li>
          {/each}
        </ul>
      {/if}
      <button class="link" onclick={() => ui.setSection("tools")}>Manage tools →</button>
    </div>
  </div>

  <div class="card">
    <h3 class="c-title">Cross-tool coverage</h3>
    <CoverageDonuts />
  </div>

  <div class="card">
    <h3 class="c-title">Catalog by division</h3>
    <CatalogByDivision />
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
  .stat.warn .s-num { color: var(--color-warning); }
  .stat.info .s-num { color: var(--color-info, var(--color-brand)); }

  .cols { display: grid; grid-template-columns: 1fr 1fr; gap: var(--space-4); align-items: start; }
  @media (max-width: 820px) { .cols { grid-template-columns: 1fr; } }
  .card { border: 1px solid var(--color-border); border-radius: var(--radius-lg); background: var(--color-surface-raised); padding: var(--space-4); min-width: 0; }
  .c-title { font-size: var(--text-body-sm); font-weight: var(--fw-semibold); color: var(--color-text-secondary); margin-bottom: var(--space-3); text-transform: uppercase; letter-spacing: 0.04em; }
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
</style>
