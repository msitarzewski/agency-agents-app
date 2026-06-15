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
  import { install, SUPPORTED_TOOLS } from "$lib/stores/install.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { toolAccent, toolMark } from "$lib/util/toolBadge";

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

  // Linked highlight: a division slug hovered anywhere (slice or legend).
  let hovered = $state<string | null>(null);
  const hoveredLabel = $derived(hovered ? (divisions.find((d) => d.slug === hovered)?.label ?? "") : "");
  function countIn(tool: string, slug: string): number {
    return donuts.find((d) => d.tool === tool)?.segs.find((s) => s.slug === slug)?.value ?? 0;
  }
</script>

{#if donuts.length === 0}
  <EmptyState title="No coverage yet" body="Install agents across your tools to see the per-tool division mix here.">
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
            <svg width="132" height="132" viewBox="0 0 120 120" role="img" aria-label={`${d.label}: ${d.total} agents across ${d.segs.length} divisions`}>
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
                    aria-label={`${a.label}: ${a.value} in ${d.label}`}
                    onmouseenter={() => (hovered = a.slug)}
                    onmouseleave={() => (hovered = null)}
                    onclick={() => ui.openDivision(a.slug)}
                  ><title>{a.label}: {a.value} in {d.label}</title></circle>
                {/each}
              </g>
            </svg>
            <div class="cd-badge" style="--accent:{toolAccent(d.tool)}">{toolMark(d.label)}</div>
          </div>
          <div class="cd-meta">
            <span class="cd-tool">{d.label}</span>
            {#if hovered && hot > 0}
              <span class="cd-sub">{hot} {hoveredLabel}</span>
            {:else}
              <span class="cd-sub">{d.total} agent{d.total === 1 ? "" : "s"}</span>
            {/if}
          </div>
        </div>
      {/each}
    </div>

    <ul class="cd-legend">
      {#each divisions as div (div.slug)}
        <li>
          <button
            class="cd-leg"
            class:hot={hovered === div.slug}
            class:dim={hovered !== null && hovered !== div.slug}
            title={`See all ${div.label} agents`}
            onmouseenter={() => (hovered = div.slug)}
            onmouseleave={() => (hovered = null)}
            onclick={() => ui.openDivision(div.slug)}
          >
            <span class="cd-swatch" style="background:{div.color}"></span>
            <span class="cd-leg-label truncate">{div.label}</span>
            <span class="cd-leg-n">{div.total}</span>
          </button>
        </li>
      {/each}
    </ul>
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
    pointer-events: none;
  }

  .cd-meta { display: flex; flex-direction: column; align-items: center; gap: 1px; min-height: 30px; }
  .cd-tool { font-size: var(--text-body-sm); font-weight: var(--fw-semibold); color: var(--color-text-primary); }
  .cd-sub { font-size: var(--text-caption); color: var(--color-text-muted); font-variant-numeric: tabular-nums; }

  .cd-legend {
    display: flex; flex-wrap: wrap; gap: 2px 4px;
    padding-top: var(--space-3); border-top: 1px solid var(--color-border);
  }
  .cd-leg {
    display: inline-flex; align-items: center; gap: 6px; max-width: 200px;
    padding: 3px 8px; border-radius: var(--radius-full);
    background: transparent; cursor: pointer;
    transition: opacity var(--motion-duration-fast) var(--motion-ease-out),
                background var(--motion-duration-fast) var(--motion-ease-out);
  }
  .cd-leg:hover { background: var(--color-surface-sunken); }
  .cd-leg.hot { background: var(--color-surface-sunken); }
  .cd-leg.dim { opacity: 0.4; }
  .cd-swatch { width: 10px; height: 10px; border-radius: 3px; flex: none; }
  .cd-leg-label { font-size: var(--text-caption); color: var(--color-text-secondary); min-width: 0; }
  .cd-leg-n { font-size: 10px; color: var(--color-text-muted); font-variant-numeric: tabular-nums; }
</style>
