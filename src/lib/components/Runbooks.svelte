<script lang="ts">
  /**
   * Runbooks — the NEXUS scenario launcher (own nav pillar). Each runbook is a
   * proven agent roster for a specific job (from the catalog's
   * `strategy/runbooks.json`); the app resolves each roster slug to a real
   * catalog agent and lets you deploy the whole team into a project in one step.
   *
   *  • Deploy team… → the shared InstallModal (destinations × tools), preloaded
   *    with the runbook's resolved roster — so "install this scenario's team into
   *    a project" reuses the exact install flow.
   *  • Copy activation prompt → a prompt synthesised from the runbook (mode +
   *    roster), mirroring NEXUS's own activation format.
   *
   * `strategy/` only ships in a synced catalog, so an empty manifest shows a
   * "sync to unlock" state, not an error.
   */
  import { onMount } from "svelte";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import RocketIcon from "@lucide/svelte/icons/rocket";
  import DownloadIcon from "@lucide/svelte/icons/download";
  import CopyIcon from "@lucide/svelte/icons/copy";
  import InstallModal from "./InstallModal.svelte";
  import EmptyState from "./EmptyState.svelte";
  import { corpus } from "$lib/stores/corpus.svelte";
  import { runbooks } from "$lib/stores/runbooks.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import { i18n } from "$lib/stores/i18n.svelte";
  import type { Agent, Runbook } from "$lib/types";

  onMount(() => {
    corpus.ensureLoaded();
    runbooks.load();
  });

  // Slug → agent, from the loaded corpus (rebuilds as the corpus resolves).
  const bySlug = $derived(new Map(corpus.agents.map((a) => [a.slug, a])));

  const rosterSlugs = (rb: Runbook) => rb.roster.flatMap((g) => g.agents);
  const resolvedSlugs = (rb: Runbook) => rosterSlugs(rb).filter((s) => bySlug.has(s));
  function counts(rb: Runbook): { total: number; found: number } {
    const all = rosterSlugs(rb);
    return { total: all.length, found: all.filter((s) => bySlug.has(s)).length };
  }
  function resolve(slugs: string[]): { slug: string; agent: Agent | undefined }[] {
    return slugs.map((slug) => ({ slug, agent: bySlug.get(slug) }));
  }
  function keyPart(value: string): string {
    return value
      .toLowerCase()
      .replace(/&/g, " and ")
      .replace(/\+/g, " plus ")
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "");
  }
  const runbookTitle = (rb: Runbook) => i18n.optional(`runbooks.item.${rb.slug}.title`, rb.title);
  const runbookDuration = (rb: Runbook) => i18n.optional(`runbooks.item.${rb.slug}.duration`, rb.duration);
  const runbookSummary = (rb: Runbook) => i18n.optional(`runbooks.item.${rb.slug}.summary`, rb.summary);
  const runbookGroup = (group: string) => i18n.optional(`runbooks.group.${keyPart(group)}`, group);
  const runbookActivation = (activation: string) => i18n.optional(`runbooks.activation.${keyPart(activation)}`, activation);

  let openSlug = $state<string | null>(null);
  function toggle(slug: string) {
    openSlug = openSlug === slug ? null : slug;
  }

  // Deploy: the shared InstallModal, preloaded with the runbook's resolved roster.
  let deployRb = $state<Runbook | null>(null);

  /** A NEXUS-style activation prompt built from the runbook — no doc scraping. */
  function activationPrompt(rb: Runbook): string {
    const roster = rb.roster
      .map((g) => {
        const names = g.agents.map((s) => bySlug.get(s)?.name ?? s).join(", ");
        return `- ${runbookGroup(g.group)} (${runbookActivation(g.activation)}): ${names}`;
      })
      .join("\n");
    return i18n.optional(
      "runbooks.activationPrompt",
      [
        `Activate the "${rb.title}" runbook in ${rb.mode} mode.`,
        rb.summary,
        "",
        "Roster:",
        roster,
        "",
        "Coordinate this team through the runbook's phases. At each phase, verify the work with evidence before advancing to the next.",
      ].join("\n"),
      { title: runbookTitle(rb), mode: rb.mode, summary: runbookSummary(rb), roster },
    );
  }

  async function copyPrompt(rb: Runbook) {
    try {
      await navigator.clipboard.writeText(activationPrompt(rb));
      toast.success(i18n.t("runbooks.promptCopied", { runbook: runbookTitle(rb) }));
    } catch (e) {
      toast.error(i18n.t("common.copyFailed"), String(e));
    }
  }
</script>

<section class="rbv">
  <header class="rbv-head" data-tauri-drag-region>
    <div class="rbv-titles" data-tauri-drag-region="false">
      <h1 class="rbv-title">{i18n.t("nav.runbooks")}</h1>
      <p class="rbv-sub">{i18n.t("runbooks.subtitle")}</p>
    </div>
  </header>

  <div class="rbv-scroll">
    {#if !runbooks.loaded || runbooks.loading}
      <p class="rbv-status">{i18n.t("common.loading")}</p>
    {:else if runbooks.list.length === 0}
      <EmptyState title={i18n.t("runbooks.needSyncTitle")}>
        {#snippet icon()}<RocketIcon size={44} />{/snippet}
        {i18n.t("runbooks.needSync")}
        {#snippet cta()}
          <button class="link" onclick={() => ui.openSettings("catalog")}>{i18n.t("runbooks.openCatalog")}</button>
        {/snippet}
      </EmptyState>
    {:else}
      <ul class="rb-list">
        {#each runbooks.list as rb (rb.slug)}
          {@const c = counts(rb)}
          {@const open = openSlug === rb.slug}
          {@const title = runbookTitle(rb)}
          <li class="rb-item" class:open>
            <div class="rb-top">
              <button class="rb-expand" onclick={() => toggle(rb.slug)} aria-expanded={open}>
                <ChevronDown size={16} class={open ? "rbv-chev open" : "rbv-chev"} />
                <span class="rb-id">
                  <span class="rb-title-row">
                    <span class="rb-title">{title}</span>
                    <span class="rb-mode">{rb.mode}</span>
                    <span class="rb-dur">{runbookDuration(rb)}</span>
                  </span>
                  <span class="rb-sum">{runbookSummary(rb)}</span>
                </span>
              </button>
              <span class="rb-actions">
                <span class="rb-count" title={i18n.t("runbooks.resolvedTitle", { found: c.found, total: c.total })}>{c.found}/{c.total}</span>
                <button class="btn ghost" onclick={() => copyPrompt(rb)}>
                  <CopyIcon size={14} /><span>{i18n.t("runbooks.copyPrompt")}</span>
                </button>
                <button class="btn primary" disabled={c.found === 0} onclick={() => (deployRb = rb)}>
                  <DownloadIcon size={14} /><span>{i18n.t("runbooks.deploy")}</span>
                </button>
              </span>
            </div>

            {#if open}
              <div class="rb-detail">
                {#each rb.roster as g (g.group)}
                  <div class="rb-grp">
                    <div class="rb-grp-head">
                      <span class="rb-grp-name">{runbookGroup(g.group)}</span>
                      <span class="rb-grp-act">{runbookActivation(g.activation)}</span>
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
  </div>
</section>

{#if deployRb}
  <InstallModal
    title={i18n.t("runbooks.deployTitle", { runbook: runbookTitle(deployRb) })}
    agentSlugs={resolvedSlugs(deployRb)}
    onClose={() => (deployRb = null)}
  />
{/if}

<style>
  .rbv { display: flex; flex-direction: column; height: 100%; min-height: 0; }
  .rbv-head { flex: none; padding: var(--space-3) var(--space-4); border-bottom: 1px solid var(--color-border); }
  .rbv-title { font-size: var(--text-h2); font-weight: var(--fw-semibold); color: var(--color-text-primary); }
  .rbv-sub { font-size: var(--text-body-sm); color: var(--color-text-secondary); margin-top: 1px; }
  .rbv-scroll { flex: 1; min-height: 0; overflow-y: auto; padding: var(--space-3); }
  .rbv-status { font-size: var(--text-body-sm); color: var(--color-text-muted); padding: var(--space-3); }
  .link { background: transparent; color: var(--color-brand); font-size: var(--text-body-sm); cursor: pointer; padding: 2px; }
  .link:hover { text-decoration: underline; }

  .rb-list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: var(--space-2); }
  .rb-item { border: 1px solid var(--color-border); border-radius: var(--radius-lg); background: var(--color-surface-raised); overflow: hidden; }
  .rb-item.open { border-color: var(--color-border-strong, var(--color-text-muted)); }

  .rb-top { display: flex; align-items: center; gap: var(--space-3); padding: var(--space-3); }
  .rb-expand { flex: 1; min-width: 0; display: flex; align-items: center; gap: var(--space-3); background: transparent; cursor: pointer; text-align: left; }
  :global(.rbv-chev) { flex: none; color: var(--color-text-muted); transition: transform var(--motion-duration-fast, 120ms) ease; transform: rotate(-90deg); }
  :global(.rbv-chev.open) { transform: rotate(0deg); }
  .rb-id { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
  .rb-title-row { display: flex; align-items: baseline; gap: var(--space-2); min-width: 0; }
  .rb-title { min-width: 0; font-weight: var(--fw-semibold); color: var(--color-text-primary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .rb-mode { font-family: var(--font-mono, ui-monospace, monospace); font-size: 10px; letter-spacing: 0.03em; color: var(--color-brand); background: color-mix(in srgb, var(--color-brand) 12%, transparent); padding: 2px 7px; border-radius: var(--radius-full); white-space: nowrap; }
  .rb-dur { font-size: var(--text-caption); color: var(--color-text-muted); white-space: nowrap; }
  .rb-sum { font-size: var(--text-body-sm); color: var(--color-text-secondary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

  .rb-actions { flex: none; display: flex; align-items: center; gap: var(--space-2); }
  .rb-count { font-family: var(--font-mono, ui-monospace, monospace); font-size: 11px; color: var(--color-text-muted); font-variant-numeric: tabular-nums; }
  .btn { display: inline-flex; align-items: center; gap: 6px; height: 30px; padding: 0 var(--space-3); border: 1px solid var(--color-border); border-radius: var(--radius-md); background: transparent; color: var(--color-text-secondary); font-size: var(--text-body-sm); cursor: pointer; white-space: nowrap; }
  .btn:hover:not(:disabled) { color: var(--color-text-primary); background: var(--color-surface-sunken); }
  .btn:disabled { opacity: 0.5; cursor: default; }
  .btn.primary { background: var(--color-brand); color: var(--color-text-inverse); border-color: transparent; }
  .btn.primary:hover:not(:disabled) { filter: brightness(1.08); background: var(--color-brand); }

  .rb-detail { padding: 0 var(--space-3) var(--space-3) 36px; display: flex; flex-direction: column; gap: var(--space-3); border-top: 1px solid var(--color-border); padding-top: var(--space-3); }
  .rb-grp-head { display: flex; align-items: baseline; gap: var(--space-2); margin-bottom: 5px; }
  .rb-grp-name { font-size: var(--text-body-sm); font-weight: var(--fw-semibold); color: var(--color-text-primary); }
  .rb-grp-act { font-size: var(--text-caption); color: var(--color-text-muted); }
  .rb-agents { list-style: none; margin: 0; padding: 0; display: grid; grid-template-columns: repeat(auto-fill, minmax(190px, 1fr)); gap: 3px var(--space-4); }
  .rb-agent { display: flex; align-items: center; gap: 7px; padding: 2px 0; font-size: var(--text-body-sm); color: var(--color-text-secondary); min-width: 0; }
  .rb-emoji { flex: none; }
  .rb-name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .rb-agent.missing .rb-name { color: var(--color-text-muted); text-decoration: line-through; text-decoration-color: var(--color-text-muted); }
  .rb-flag { flex: none; font-size: 9.5px; text-transform: uppercase; letter-spacing: 0.04em; color: var(--color-warning); background: color-mix(in srgb, var(--color-warning) 14%, transparent); padding: 1px 5px; border-radius: var(--radius-full); }
</style>
