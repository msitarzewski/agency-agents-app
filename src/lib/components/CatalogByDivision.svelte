<script lang="ts">
  /**
   * CatalogByDivision — the agent catalog as ONE proportional bar: a single
   * full-width track split into a colored segment per division (width ∝ that
   * division's agent count), each painted with its brand color from the catalog
   * metadata (`corpus.colorOf`). Labels are spread across FOUR lanes — two above
   * the bar, two below — so all ~16 divisions are labelled without crowding:
   *
   *   ┌ top-outer ─ the next tier, spread across the (otherwise empty) right side
   *   ├ top-inner ─ the majors (≥ MAJOR_PCT), centered over their wide segments
   *   ╞════════════ the bar ════════════
   *   ├ bot-inner ─┐ the long tail, split into two interleaved fan-rows that
   *   └ bot-outer ─┘ distribute full-width, each cell tied to its (right-clustered)
   *                  segment by an angled dotted leader in the division's color.
   *
   * Dependency-free: the bar + labels are HTML (crisp text, easy hit targets);
   * only the leader lines are SVG, drawn in a 0–100 × 0–H space stretched to the
   * container (preserveAspectRatio=none) with non-scaling strokes so a 1px line
   * stays 1px under the non-uniform scale. The SVG's y-space is in px and the
   * element is exactly H px tall, so y maps 1:1 while x maps percent → width.
   */
  import { corpus } from "$lib/stores/corpus.svelte";
  import { i18n } from "$lib/stores/i18n.svelte";
  import { ui } from "$lib/stores/ui.svelte";

  // Lane geometry (px; the SVG viewBox y-space matches these 1:1).
  const ROW_H = 22;
  const TOP_OUTER_Y = 2; // farthest top label row
  const TOP_INNER_Y = TOP_OUTER_Y + ROW_H + 4; // majors, just above the bar
  const BAR_Y = TOP_INNER_Y + ROW_H + 4;
  const BAR_H = 30;
  const BAR_BOTTOM = BAR_Y + BAR_H;
  const BOT_INNER_Y = BAR_BOTTOM + 16; // nearest bottom label row
  const BOT_OUTER_Y = BOT_INNER_Y + ROW_H + 4; // farthest bottom label row
  const H = BOT_OUTER_Y + ROW_H;

  const MAJOR_PCT = 6; // segments ≥ this are centered on the inner-top lane
  const TOP_TIER = 4; // how many of the remaining divisions ride the top-outer lane

  type Seg = { slug: string; label: string; color: string; count: number; pct: number; mid: number };

  const model = $derived.by(() => {
    const divs = corpus.tiles.filter((c) => c.count > 0).sort((a, b) => b.count - a.count);
    const total = divs.reduce((s, c) => s + c.count, 0);
    let acc = 0;
    const list: Seg[] = divs.map((c) => {
      const pct = total > 0 ? (c.count / total) * 100 : 0;
      const x0 = total > 0 ? (acc / total) * 100 : 0;
      acc += c.count;
      return { slug: c.slug, label: c.label, color: corpus.colorOf(c.slug), count: c.count, pct, mid: x0 + pct / 2 };
    });
    return { total, list };
  });

  // Majors: centered over their segment on the inner-top lane.
  const majors = $derived(model.list.filter((s) => s.pct >= MAJOR_PCT));

  // Everything else is a "fan" label (label x ≠ segment mid, tied by a leader).
  // Spread positions: evenly across [x0..x1] of the lane's width. `rank` (the
  // within-row index) staggers each leader's rail so no two horizontals share a
  // y, and the column phase (x0 offset) keeps the two bottom rows' verticals out
  // of each other's labels — together that's what lets the elbows never cross.
  function fan(items: Seg[], y: number, x0 = 0, x1 = 100) {
    const n = items.length;
    return items.map((s, i) => ({ ...s, y, rank: i, lx: x0 + ((i + 0.5) / n) * (x1 - x0) }));
  }
  type Fan = ReturnType<typeof fan>[number];

  const minors = $derived(model.list.filter((s) => s.pct < MAJOR_PCT));
  // Top-outer: the next tier, kept over the right side so its leaders stay short
  // and fill the space the majors leave empty.
  const topOuter = $derived(fan(minors.slice(0, TOP_TIER), TOP_OUTER_Y, 50, 99));
  // The remaining tail splits across two interleaved bottom fan-rows whose
  // columns are phase-shifted so each row's verticals fall between the other's.
  const tail = $derived(minors.slice(TOP_TIER));
  const botInner = $derived(fan(tail.filter((_, i) => i % 2 === 0), BOT_INNER_Y, 2, 88));
  const botOuter = $derived(fan(tail.filter((_, i) => i % 2 === 1), BOT_OUTER_Y, 14, 99));
  const fans = $derived([...topOuter, ...botInner, ...botOuter]);

  // Z-elbow leader: vertical off the segment mid, a short horizontal at a
  // rank-staggered rail near the bar, then a vertical down the label's column.
  function elbow(f: Fan): string {
    const above = f.y < BAR_Y;
    const rail = above ? BAR_Y - (5 + f.rank * 3) : BAR_BOTTOM + (5 + f.rank * 3);
    const barY = above ? BAR_Y : BAR_BOTTOM;
    const labY = above ? f.y + ROW_H : f.y;
    return `${f.mid},${barY} ${f.mid},${rail} ${f.lx},${rail} ${f.lx},${labY}`;
  }

  // Linked highlight: a division slug hovered anywhere (segment or label) lights
  // that division and dims the rest. Bindable so it shares the highlight with the
  // sibling CoverageDonuts in the merged card — hover here, the donuts light up.
  let { hovered = $bindable(null) }: { hovered?: string | null } = $props();
  const dim = (slug: string) => hovered !== null && hovered !== slug;
</script>

{#if model.total === 0}
  <p class="cbd-empty">{i18n.t("catalog.empty")}</p>
{:else}
  <div class="cbd">
    <div class="cbd-head">
      <span class="cbd-total">{i18n.count(model.total, "common.agent.one", "common.agent.many")}</span>
      <span class="cbd-pct">100%</span>
    </div>

    <div class="cbd-plot" style="height:{H}px">
      <!-- Leader lines: short stems for the centered majors, angled dotted
           leaders from each fan label to its segment mid. Drawn first so the
           bar + labels sit on top. -->
      <svg class="cbd-svg" viewBox="0 0 100 {H}" preserveAspectRatio="none" aria-hidden="true">
        {#each majors as m (m.slug)}
          <line
            x1={m.mid} y1={BAR_Y - 8} x2={m.mid} y2={BAR_Y}
            stroke={m.color} stroke-width="1" vector-effect="non-scaling-stroke" class="cbd-leader"
          />
        {/each}
        {#each fans as f (f.slug)}
          <polyline
            points={elbow(f)} fill="none"
            stroke={f.color} stroke-width="1" stroke-dasharray="2 3"
            vector-effect="non-scaling-stroke" class="cbd-leader" class:dim={dim(f.slug)}
          />
        {/each}
      </svg>

      <!-- The proportional bar -->
      <div class="cbd-bar" style="top:{BAR_Y}px; height:{BAR_H}px">
        {#each model.list as s (s.slug)}
          <button
            class="cbd-seg" class:dim={dim(s.slug)}
            style="width:{s.pct}%; background:{s.color}"
            title={`${s.label}: ${i18n.count(s.count, "common.agent.one", "common.agent.many")} (${s.pct.toFixed(1)}%)`}
            aria-label={`${s.label}: ${i18n.count(s.count, "common.agent.one", "common.agent.many")}`}
            onmouseenter={() => (hovered = s.slug)}
            onmouseleave={() => (hovered = null)}
            onclick={() => ui.openDivision(s.slug)}
          ></button>
        {/each}
      </div>

      <!-- Majors: centered over their segment -->
      {#each majors as m (m.slug)}
        <button
          class="cbd-lbl major" class:dim={dim(m.slug)}
          style="left:{m.mid}%; top:{TOP_INNER_Y}px"
          onmouseenter={() => (hovered = m.slug)}
          onmouseleave={() => (hovered = null)}
          onclick={() => ui.openDivision(m.slug)}
        >
          <span class="cbd-name">{m.label}</span>
          <span class="cbd-n">{m.count}</span>
        </button>
      {/each}

      <!-- Fan labels: top-outer + the two bottom rows -->
      {#each fans as f (f.slug)}
        <button
          class="cbd-lbl fan" class:dim={dim(f.slug)}
          style="left:{f.lx}%; top:{f.y}px"
          onmouseenter={() => (hovered = f.slug)}
          onmouseleave={() => (hovered = null)}
          onclick={() => ui.openDivision(f.slug)}
          title={`${f.label}: ${i18n.count(f.count, "common.agent.one", "common.agent.many", { count: f.count })}`}
        >
          <span class="cbd-swatch" style="background:{f.color}"></span>
          <span class="cbd-name truncate">{f.label}</span>
          <span class="cbd-n">{f.count}</span>
        </button>
      {/each}
    </div>
  </div>
{/if}

<style>
  .cbd-empty { color: var(--color-text-muted); font-size: var(--text-body-sm); }

  .cbd { display: flex; flex-direction: column; gap: var(--space-2); }
  .cbd-head { display: flex; justify-content: space-between; align-items: baseline; }
  .cbd-total { font-size: var(--text-body-sm); font-weight: var(--fw-semibold); color: var(--color-text-primary); }
  .cbd-pct { font-size: var(--text-caption); color: var(--color-text-muted); font-variant-numeric: tabular-nums; }

  .cbd-plot { position: relative; width: 100%; }
  .cbd-svg { position: absolute; inset: 0; width: 100%; height: 100%; overflow: visible; pointer-events: none; }
  .cbd-leader { opacity: 0.5; transition: opacity var(--motion-duration-fast) var(--motion-ease-out); }
  .cbd-leader.dim { opacity: 0.12; }

  .cbd-bar {
    position: absolute; left: 0; right: 0;
    display: flex; border-radius: var(--radius-sm); overflow: hidden;
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--color-text-primary) 8%, transparent);
  }
  .cbd-seg {
    height: 100%; min-width: 2px; border: none; padding: 0; cursor: pointer;
    box-shadow: inset -1px 0 0 var(--color-surface-raised);
    transition: filter var(--motion-duration-fast) var(--motion-ease-out),
                opacity var(--motion-duration-fast) var(--motion-ease-out);
  }
  .cbd-seg:last-child { box-shadow: none; }
  .cbd-seg:hover { filter: brightness(1.12) saturate(1.1); }
  .cbd-seg.dim { opacity: 0.22; }

  .cbd-lbl {
    position: absolute; transform: translateX(-50%);
    display: inline-flex; align-items: center; gap: 5px; height: 18px;
    padding: 0 5px; border-radius: var(--radius-sm);
    background: transparent; cursor: pointer; white-space: nowrap;
    transition: background var(--motion-duration-fast) var(--motion-ease-out),
                opacity var(--motion-duration-fast) var(--motion-ease-out);
  }
  .cbd-lbl:hover { background: var(--color-surface-sunken); }
  .cbd-lbl.dim { opacity: 0.32; }
  .cbd-lbl.fan { max-width: 22%; }
  .cbd-name { font-size: var(--text-caption); color: var(--color-text-secondary); }
  .cbd-lbl.major .cbd-name { font-weight: var(--fw-semibold); color: var(--color-text-primary); }
  .cbd-lbl.fan .cbd-name { overflow: hidden; text-overflow: ellipsis; min-width: 0; }
  .cbd-n { font-size: 10px; color: var(--color-text-muted); font-variant-numeric: tabular-nums; flex: none; }
  .cbd-swatch { width: 8px; height: 8px; border-radius: 2px; flex: none; }
</style>
