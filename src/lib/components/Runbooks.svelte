<script lang="ts">
  /**
   * Runbooks (P0) — NEXUS scenario runbooks surfaced from the catalog's
   * `strategy/runbooks.json`. Lists each runbook and, on expand, its roster
   * grouped by team — every agent resolved from its slug to a real catalog
   * agent (name + emoji). Unresolved slugs are flagged, never dropped.
   *
   * P0 proves the pipeline: read manifest → resolve rosters → render. Deploy +
   * copy-prompt actions land in P1. `strategy/` only exists in a synced catalog,
   * so an empty manifest shows a "sync to unlock" nudge, not an error.
   */
  import { onMount } from "svelte";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import { corpus } from "$lib/stores/corpus.svelte";
  import { runbooks } from "$lib/stores/runbooks.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { i18n } from "$lib/stores/i18n.svelte";
  import type { Agent, Runbook } from "$lib/types";

  onMount(() => {
    corpus.ensureLoaded();
    runbooks.load();
  });

  // Slug → agent, from the loaded corpus. Rebuilds as the corpus resolves.
  const bySlug = $derived(new Map(corpus.agents.map((a) => [a.slug, a])));

  function resolve(slugs: string[]): { slug: string; agent: Agent | undefined }[] {
    return slugs.map((slug) => ({ slug, agent: bySlug.get(slug) }));
  }
  function counts(rb: Runbook): { total: number; found: number } {
    const all = rb.roster.flatMap((g) => g.agents);
    return { total: all.length, found: all.filter((s) => bySlug.has(s)).length };
  }

  let openSlug = $state<string | null>(null);
  function toggle(slug: string) {
    openSlug = openSlug === slug ? null : slug;
  }

  function unlock() {
    ui.closePlaybook();
    ui.openSettings("catalog");
  }
</script>

<section class="rb">
  {#if !runbooks.loaded || runbooks.loading}
    <p class="rb-status">{i18n.t("common.loading")}</p>
  {:else if runbooks.list.length === 0}
    <div class="rb-empty">
      <p>{i18n.t("runbooks.needSync")}</p>
      <button class="link" onclick={unlock}>{i18n.t("runbooks.openCatalog")}</button>
    </div>
  {:else}
    <ul class="rb-list">
      {#each runbooks.list as rb (rb.slug)}
        {@const c = counts(rb)}
        {@const open = openSlug === rb.slug}
        <li class="rb-item" class:open>
          <button class="rb-head" onclick={() => toggle(rb.slug)} aria-expanded={open}>
            <ChevronDown size={15} class={open ? "rb-chev open" : "rb-chev"} />
            <span class="rb-id">
              <span class="rb-title">{rb.title}</span>
              <span class="rb-sum">{rb.summary}</span>
            </span>
            <span class="rb-meta">
              <span class="rb-mode">{rb.mode}</span>
              <span class="rb-dur">{rb.duration}</span>
              <span class="rb-count" title={i18n.t("runbooks.resolvedTitle", { found: c.found, total: c.total })}>{c.found}/{c.total}</span>
            </span>
          </button>
          {#if open}
            <div class="rb-detail">
              {#each rb.roster as g (g.group)}
                <div class="rb-grp">
                  <div class="rb-grp-head">
                    <span class="rb-grp-name">{g.group}</span>
                    <span class="rb-grp-act">{g.activation}</span>
                  </div>
                  <ul class="rb-agents">
                    {#each resolve(g.agents) as r (r.slug)}
                      <li class="rb-agent" class:missing={!r.agent}>
                        <span class="rb-emoji">{r.agent?.emoji ?? "○"}</span>
                        <span class="rb-name">{r.agent?.name ?? r.slug}</span>
                        {#if !r.agent}<span class="rb-flag">{i18n.t("runbooks.notInCatalog")}</span>{/if}
                      </li>
                    {/each}
                  </ul>
                </div>
              {/each}
            </div>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .rb { display: flex; flex-direction: column; gap: var(--space-2); }
  .rb-status { font-size: var(--text-body-sm); color: var(--color-text-muted); }

  .rb-empty { border: 1px dashed var(--color-border); border-radius: var(--radius-md); padding: var(--space-3) var(--space-4); }
  .rb-empty p { font-size: var(--text-body-sm); color: var(--color-text-secondary); margin: 0 0 4px; }
  .link { background: transparent; color: var(--color-brand); font-size: var(--text-body-sm); cursor: pointer; padding: 0; }
  .link:hover { text-decoration: underline; }

  .rb-list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: var(--space-2); }
  .rb-item { border: 1px solid var(--color-border); border-radius: var(--radius-lg); background: var(--color-surface-raised); overflow: hidden; }
  .rb-item.open { border-color: var(--color-border-strong, var(--color-text-muted)); }

  .rb-head { width: 100%; display: flex; align-items: center; gap: var(--space-3); padding: var(--space-3); background: transparent; cursor: pointer; text-align: left; }
  .rb-head:hover { background: var(--color-surface-sunken); }
  :global(.rb-chev) { flex: none; color: var(--color-text-muted); transition: transform var(--motion-duration-fast, 120ms) ease; transform: rotate(-90deg); }
  :global(.rb-chev.open) { transform: rotate(0deg); }
  .rb-id { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 1px; }
  .rb-title { font-weight: var(--fw-semibold); color: var(--color-text-primary); }
  .rb-sum { font-size: var(--text-caption); color: var(--color-text-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .rb-meta { flex: none; display: flex; align-items: center; gap: var(--space-2); }
  .rb-mode { font-family: var(--font-mono, ui-monospace, monospace); font-size: 10px; letter-spacing: 0.03em; color: var(--color-brand); background: color-mix(in srgb, var(--color-brand) 12%, transparent); padding: 2px 7px; border-radius: var(--radius-full); white-space: nowrap; }
  .rb-dur { font-size: var(--text-caption); color: var(--color-text-muted); white-space: nowrap; }
  .rb-count { font-family: var(--font-mono, ui-monospace, monospace); font-size: 11px; color: var(--color-text-muted); font-variant-numeric: tabular-nums; }

  .rb-detail { padding: 0 var(--space-3) var(--space-3) 34px; display: flex; flex-direction: column; gap: var(--space-3); border-top: 1px solid var(--color-border); padding-top: var(--space-2); }
  .rb-grp-head { display: flex; align-items: baseline; gap: var(--space-2); margin-bottom: 4px; }
  .rb-grp-name { font-size: var(--text-body-sm); font-weight: var(--fw-semibold); color: var(--color-text-primary); }
  .rb-grp-act { font-size: var(--text-caption); color: var(--color-text-muted); }
  .rb-agents { list-style: none; margin: 0; padding: 0; display: grid; grid-template-columns: repeat(auto-fill, minmax(180px, 1fr)); gap: 2px var(--space-3); }
  .rb-agent { display: flex; align-items: center; gap: 7px; padding: 2px 0; font-size: var(--text-body-sm); color: var(--color-text-secondary); min-width: 0; }
  .rb-emoji { flex: none; }
  .rb-name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .rb-agent.missing .rb-name { color: var(--color-text-muted); text-decoration: line-through; text-decoration-color: var(--color-text-muted); }
  .rb-flag { flex: none; font-size: 9.5px; text-transform: uppercase; letter-spacing: 0.04em; color: var(--color-warning); background: color-mix(in srgb, var(--color-warning) 14%, transparent); padding: 1px 5px; border-radius: var(--radius-full); }
</style>
