<script lang="ts">
  /**
   * CoverageMatrix — the cross-tool install registry as a heatmap. Rows are the
   * agent categories you've deployed; columns are every supported tool; each
   * cell is how many agents in that category are installed in that tool, shaded
   * by COVERAGE — `installed ÷ that division's catalog size`, so every cell is
   * its own 0–100% scale (a fully-deployed small division reads as strong as a
   * fully-deployed big one, instead of intensity tracking raw category size).
   * Empty columns are the point — they show where you're NOT deployed (e.g.
   * "everything's in Claude Code, nothing in Cursor").
   *
   * Dependency-free (CSS grid + color-mix). Clicking a populated cell jumps to
   * the Agents workspace filtered to your installed set.
   */
  import EmptyState from "./EmptyState.svelte";
  import LayersIcon from "@lucide/svelte/icons/layers";
  import { corpus } from "$lib/stores/corpus.svelte";
  import { install, SUPPORTED_TOOLS } from "$lib/stores/install.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import type { Tool } from "$lib/types";

  // Short column headers (full label in the tooltip).
  const SHORT: Record<Tool, string> = {
    claudeCode: "Claude", codex: "Codex", geminiCli: "Gemini", copilot: "Copilot",
    qwen: "Qwen", cursor: "Cursor", opencode: "opencode",
    windsurf: "Windsurf", aider: "Aider", openclaw: "openclaw", antigravity: "antigravity",
  };

  const slugCat = $derived(new Map(corpus.agents.map((a) => [a.slug, a.category])));

  // Catalog size per category — the coverage denominator (how many agents in
  // this division exist to deploy, regardless of how many you've installed).
  const catalogTotal = $derived.by(() => {
    const m = new Map<string, number>();
    for (const a of corpus.agents) m.set(a.category, (m.get(a.category) ?? 0) + 1);
    return m;
  });

  const data = $derived.by(() => {
    const toolIds = SUPPORTED_TOOLS.map((t) => t.id);
    const byCat = new Map<string, { cat: string; counts: Record<string, number> }>();
    for (const r of install.installed) {
      const cat = slugCat.get(r.slug) ?? "uncategorized";
      let row = byCat.get(cat);
      if (!row) {
        row = { cat, counts: Object.fromEntries(toolIds.map((t) => [t, 0])) };
        byCat.set(cat, row);
      }
      row.counts[r.tool] = (row.counts[r.tool] ?? 0) + 1;
    }
    const rows = [...byCat.values()]
      .map((r) => ({ ...r, catTotal: catalogTotal.get(r.cat) ?? 0 }))
      .sort((a, b) => b.catTotal - a.catTotal);
    const toolTotals = Object.fromEntries(
      toolIds.map((t) => [t, rows.reduce((s, r) => s + (r.counts[t] ?? 0), 0)]),
    ) as Record<string, number>;
    // Only show tool columns that actually hold agents — empty columns are noise.
    const tools = SUPPORTED_TOOLS.filter((t) => (toolTotals[t.id] ?? 0) > 0);
    return { rows, toolTotals, tools };
  });

  /** Fraction of a division's catalog agents deployed in a tool (0..1). */
  function coverage(count: number, catTotal: number): number {
    return catTotal > 0 ? Math.min(count / catTotal, 1) : 0;
  }
  function cellStyle(frac: number): string {
    if (frac <= 0) return "";
    const pct = 20 + Math.round(frac * 55); // 20%..75% brand, by coverage
    return `background: color-mix(in srgb, var(--color-brand) ${pct}%, transparent);`;
  }
  function strong(frac: number): boolean {
    return frac > 0.5;
  }
</script>

{#if data.rows.length === 0}
  <EmptyState title="No coverage yet" body="Install agents across your tools to see the cross-tool map here.">
    {#snippet icon()}<LayersIcon size={40} />{/snippet}
  </EmptyState>
{:else}
  <div class="cm" style="--cols:{data.tools.length}">
    <!-- header -->
    <div class="cm-row cm-head">
      <div class="cm-cat cm-corner">Division</div>
      {#each data.tools as t (t.id)}
        <div class="cm-th" title={`${t.label} · ${data.toolTotals[t.id] ?? 0} installed`}>
          <span class="cm-th-l">{SHORT[t.id] ?? t.label}</span>
          <span class="cm-th-n">{data.toolTotals[t.id] ?? 0}</span>
        </div>
      {/each}
    </div>
    <!-- body -->
    {#each data.rows as r (r.cat)}
      <div class="cm-row">
        <button class="cm-cat" title={`See all ${corpus.labelOf(r.cat)} agents`} onclick={() => ui.openDivision(r.cat)}>
          <span class="truncate">{corpus.labelOf(r.cat)}</span>
          <span class="cm-cat-n">{r.catTotal}</span>
        </button>
        {#each data.tools as t (t.id)}
          {@const n = r.counts[t.id] ?? 0}
          {@const frac = coverage(n, r.catTotal)}
          <button
            class="cm-cell"
            class:strong={strong(frac)}
            class:empty={n === 0}
            style={cellStyle(frac)}
            title={`${corpus.labelOf(r.cat)} × ${t.label}: ${n} of ${r.catTotal} (${Math.round(frac * 100)}%)`}
            disabled={n === 0}
            onclick={() => ui.openDivision(r.cat)}
          >{n > 0 ? n : ""}</button>
        {/each}
      </div>
    {/each}
  </div>
{/if}

<style>
  .cm { display: flex; flex-direction: column; gap: 3px; overflow-x: auto; }
  .cm-row {
    display: grid;
    grid-template-columns: minmax(120px, 1.5fr) repeat(var(--cols), minmax(40px, 1fr));
    gap: 3px; align-items: stretch;
  }
  .cm-head { position: sticky; top: 0; }
  .cm-th {
    display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 1px;
    padding: 4px 2px; border-radius: var(--radius-sm);
    background: var(--color-surface-sunken);
  }
  .cm-th-l { font-size: 10px; font-weight: var(--fw-semibold); color: var(--color-text-secondary); text-align: center; line-height: 1.1; }
  .cm-th-n { font-size: 10px; color: var(--color-text-muted); font-variant-numeric: tabular-nums; }
  .cm-corner { background: transparent; }

  .cm-cat {
    display: flex; align-items: center; justify-content: space-between; gap: var(--space-2);
    padding: 0 var(--space-2); min-height: 28px; border-radius: var(--radius-sm);
    background: var(--color-surface-sunken); color: var(--color-text-primary);
    font-size: var(--text-caption); text-align: left; cursor: pointer;
  }
  .cm-cat:hover { background: var(--color-surface); }
  .cm-cat .truncate { min-width: 0; }
  .cm-cat-n { font-size: 10px; color: var(--color-text-muted); font-variant-numeric: tabular-nums; flex: none; }

  .cm-cell {
    display: flex; align-items: center; justify-content: center; min-height: 28px;
    border-radius: var(--radius-sm); background: var(--color-surface-sunken);
    color: var(--color-text-primary); font-size: var(--text-caption); font-weight: var(--fw-semibold);
    font-variant-numeric: tabular-nums; cursor: pointer;
    transition: outline-color var(--motion-duration-fast) var(--motion-ease-out);
    outline: 1.5px solid transparent; outline-offset: -1.5px;
  }
  .cm-cell.empty { background: var(--color-surface-sunken); opacity: 0.45; cursor: default; }
  .cm-cell.strong { color: #fff; }
  .cm-cell:not(.empty):hover { outline-color: var(--color-brand); }
</style>
