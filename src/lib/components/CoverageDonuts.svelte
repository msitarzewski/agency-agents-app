<script lang="ts">
  /**
   * CoverageDonuts — the cross-tool registry as small-multiple donuts: one donut
   * per tool you've deployed into, each sliced by division (segment size = agents
   * of that division installed in that tool), the tool's badge in the hole. A
   * single shared legend maps division → color; hovering a slice (or a legend
   * row) highlights that division everywhere and dims the rest, so you can read
   * "Marketing lives mostly in Claude" at a glance. Click a slice or legend row
   * to jump to that division in the Agents workspace.
   *
   * Dependency-free: SVG arcs via stroke-dasharray (same technique as
   * HealthDonut), rotated -90° so segment 0 starts at twelve o'clock. Division
   * colors are derived (golden-angle hue spacing) since divisions carry no color
   * in the catalog metadata — stable per division, auto-covers new ones.
   */
  import EmptyState from "./EmptyState.svelte";
  import LayersIcon from "@lucide/svelte/icons/layers";
  import { corpus } from "$lib/stores/corpus.svelte";
  import { i18n } from "$lib/stores/i18n.svelte";
  import { install, SUPPORTED_TOOLS } from "$lib/stores/install.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { toolAccent, toolMark, toolIcon } from "$lib/util/toolBadge";

  // Geometry (viewBox 0 0 120 120, matching HealthDonut's spec exactly).
  const STROKE = 16;
  const R = 50;
  const CIRC = 2 * Math.PI * R;

  const slugCat = $derived(new Map(corpus.agents.map((a) => [a.slug, a.category])));

  // Divisions present across all installs, ordered by install size (stable), each
  // with its shared color + total — drives both the legend and slice colors.
  const divisions = $derived.by(() => {
    const present = new Map<string, number>();
    for (const r of install.installed) {
      const cat = slugCat.get(r.slug) ?? "uncategorized";
      present.set(cat, (present.get(cat) ?? 0) + 1);
    }
    return [...present.keys()]
      .sort((a, b) => (present.get(b)! - present.get(a)!) || a.localeCompare(b))
      .map((slug) => ({ slug, label: corpus.labelOf(slug), color: corpus.colorOf(slug), total: present.get(slug)! }));
  });

  // One donut per tool with installs; segments follow the legend's division order
  // so colors line up across every donut.
  const donuts = $derived.by(() => {
    const byTool = new Map<string, Map<string, number>>();
    for (const r of install.installed) {
      const cat = slugCat.get(r.slug) ?? "uncategorized";
      let m = byTool.get(r.tool);
      if (!m) { m = new Map(); byTool.set(r.tool, m); }
      m.set(cat, (m.get(cat) ?? 0) + 1);
    }
    return SUPPORTED_TOOLS.filter((t) => byTool.has(t.id)).map((t) => {
      const m = byTool.get(t.id)!;
      const segs = divisions
        .filter((d) => (m.get(d.slug) ?? 0) > 0)
        .map((d) => ({ slug: d.slug, label: d.label, color: d.color, value: m.get(d.slug)! }));
      const total = segs.reduce((s, x) => s + x.value, 0);
      return { tool: t.id, label: t.label, total, segs };
    });
  });

  function arcsFor(segs: { slug: string; label: string; color: string; value: number }[], total: number) {
    const out: { slug: string; label: string; color: string; value: number; len: number; offset: number }[] = [];
    if (total <= 0) return out;
    let acc = 0;
    for (const s of segs) {
      const len = (s.value / total) * CIRC;
      out.push({ ...s, len, offset: -acc });
      acc += len;
    }
    return out;
  }

  // Linked highlight: a division slug hovered anywhere. Bindable so a sibling
  // (the merged "Catalog by division" bar) shares the same highlight — hover a
  // donut slice and that division lights up in the bar, and vice versa.
  let { hovered = $bindable(null) }: { hovered?: string | null } = $props();
  const hoveredLabel = $derived(hovered ? (divisions.find((d) => d.slug === hovered)?.label ?? "") : "");
  function countIn(tool: string, slug: string): number {
    return donuts.find((d) => d.tool === tool)?.segs.find((s) => s.slug === slug)?.value ?? 0;
  }
</script>

{#if donuts.length === 0}
  <EmptyState title={i18n.t("coverage.emptyTitle")} body={i18n.t("coverage.donutsEmptyBody")}>
    {#snippet icon()}<LayersIcon size={40} />{/snippet}
  </EmptyState>
{:else}
  <div class="cd">
    <div class="cd-donuts">
      {#each donuts as d (d.tool)}
        {@const arcs = arcsFor(d.segs, d.total)}
        {@const hot = hovered ? countIn(d.tool, hovered) : 0}
        <div class="cd-cell">
          <div class="cd-chart">
            <svg width="132" height="132" viewBox="0 0 120 120" role="img" aria-label={i18n.t("coverage.donutAria", { tool: d.label, count: d.total, divisions: d.segs.length })}>
              <g transform="rotate(-90 60 60)">
                <circle cx="60" cy="60" r={R} fill="none" style="stroke: var(--color-surface-sunken)" stroke-width={STROKE} />
                {#each arcs as a (a.slug)}
                  <!-- Slice click is a mouse convenience; the legend below is the
                       keyboard-accessible control (a focusable button per division). -->
                  <!-- svelte-ignore a11y_click_events_have_key_events -->
                  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
                  <circle
                    cx="60" cy="60" r={R} fill="none"
                    style={`stroke:${a.color}`} stroke-width={STROKE}
                    stroke-dasharray={`${a.len} ${CIRC - a.len}`}
                    stroke-dashoffset={a.offset}
                    class="seg"
                    class:dim={hovered !== null && hovered !== a.slug}
                    role="img"
                    aria-label={i18n.t("coverage.sliceAria", { division: a.label, count: a.value, tool: d.label })}
                    onmouseenter={() => (hovered = a.slug)}
                    onmouseleave={() => (hovered = null)}
                    onclick={() => ui.openDivision(a.slug)}
                  ><title>{i18n.t("coverage.sliceAria", { division: a.label, count: a.value, tool: d.label })}</title></circle>
                {/each}
              </g>
            </svg>
            <button
              class="cd-badge"
              style="--accent:{toolAccent(d.tool)}"
              title={i18n.t("coverage.openTool", { tool: d.label })}
              aria-label={i18n.t("coverage.openTool", { tool: d.label })}
              onclick={() => ui.openTools(d.tool)}
            >{#if toolIcon(d.tool)}<span class="glyph">{@html toolIcon(d.tool)}</span>{:else}{toolMark(d.label)}{/if}</button>
          </div>
          <div class="cd-meta">
            <span class="cd-tool">{d.label}</span>
            {#if hovered && hot > 0}
              <span class="cd-sub">{hot} {hoveredLabel}</span>
            {:else}
              <span class="cd-sub">{i18n.count(d.total, "common.agent.one", "common.agent.many")}</span>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  </div>
{/if}

<style>
  .cd { display: flex; flex-direction: column; gap: var(--space-4); }

  .cd-donuts {
    display: flex; flex-wrap: wrap; gap: var(--space-4) var(--space-5);
    align-items: flex-start;
  }
  .cd-cell { display: flex; flex-direction: column; align-items: center; gap: var(--space-2); flex: none; }
  /* flex: none — like HealthDonut's .hd-chart — so the flex row never resizes the
     donut; every donut stays a fixed 132×132 regardless of label length. */
  .cd-chart { position: relative; width: 132px; height: 132px; flex: none; }
  /* Pin the svg to a fixed px size (not 100%) so the donut can never expand to
     the card width if .cd-chart's scoped style fails to apply / races on load —
     a viewBox-only svg otherwise grows to full width and its 1:1 ratio balloons
     it into one giant circle. The width/height attributes are the last-resort
     floor; this rule keeps it crisp when the styles are present. */
  .cd-chart svg { width: 132px; height: 132px; display: block; }

  .seg { cursor: pointer; transition: opacity var(--motion-duration-fast) var(--motion-ease-out); }
  .seg.dim { opacity: 0.18; }
  .seg:hover { opacity: 1; }

  .cd-badge {
    position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%);
    width: 46px; height: 46px; border-radius: 13px;
    display: inline-flex; align-items: center; justify-content: center;
    background: linear-gradient(145deg, var(--accent), color-mix(in srgb, var(--accent) 70%, black));
    color: #fff; font-weight: var(--fw-bold); font-size: 22px;
    box-shadow: inset 0 1px 0 color-mix(in srgb, white 25%, transparent);
    cursor: pointer;
    transition: transform var(--motion-duration-fast) var(--motion-ease-out),
                filter var(--motion-duration-fast) var(--motion-ease-out);
  }
  .cd-badge:hover { transform: translate(-50%, -50%) scale(1.06); filter: brightness(1.08); }
  .cd-badge:focus-visible { outline: 2px solid var(--color-brand); outline-offset: 2px; }
  .cd-badge .glyph { display: inline-flex; align-items: center; justify-content: center; }
  .cd-badge .glyph :global(svg) { width: 1em; height: 1em; }

  .cd-meta { display: flex; flex-direction: column; align-items: center; gap: 1px; min-height: 30px; }
  .cd-tool { font-size: var(--text-body-sm); font-weight: var(--fw-semibold); color: var(--color-text-primary); }
  .cd-sub { font-size: var(--text-caption); color: var(--color-text-muted); font-variant-numeric: tabular-nums; }

</style>
