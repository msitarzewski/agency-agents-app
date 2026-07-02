<script lang="ts">
  /**
   * DeploymentMatrix — now just the at-a-glance DEPLOYMENT PILLS for an agent:
   * one pill per place it's installed (tool · project), toned by state. Actually
   * changing where an agent is deployed happens in the shared InstallModal
   * (opened from the "Install…" button in the detail header); this is the
   * read-only summary that sits on its own row beneath the title.
   *
   * Pills for `foreign`/`modified`/`outdated` installs are clickable — they open
   * the diff so you can see how the on-disk file departs from the catalog.
   */
  import { install } from "$lib/stores/install.svelte";
  import { i18n } from "$lib/stores/i18n.svelte";
  import type { Agent, InstallState, Tool } from "$lib/types";

  let {
    agent,
    onDiff,
  }: {
    agent: Agent;
    onDiff: (t: { slug: string; tool: Tool; projectPath: string | null; name: string }) => void;
  } = $props();

  const rows = $derived(
    install
      .forSlug(agent.slug)
      .slice()
      .sort((a, b) => install.toolLabel(a.tool).localeCompare(install.toolLabel(b.tool))),
  );

  const DIFFABLE: InstallState[] = ["foreign", "modified", "outdated"];
  function tone(s: InstallState): string {
    if (s === "current") return "ok";
    if (s === "outdated" || s === "modified") return "warn";
    if (s === "foreign") return "info";
    return "danger";
  }
  function basename(p: string): string {
    return p.replace(/\/+$/, "").split("/").pop() || p;
  }
  function stateLabel(state: InstallState): string {
    if (state === "current") return i18n.t("state.current");
    if (state === "outdated") return i18n.t("state.outdated");
    if (state === "modified") return i18n.t("state.modified");
    if (state === "foreign") return i18n.t("state.foreign");
    return i18n.t("state.removed");
  }
  function pillTitle(tool: string, project: string | null, state: InstallState, diffable: boolean): string {
    const target = `${tool}${project ? " · " + basename(project) : ""} · ${stateLabel(state)}`;
    return diffable ? i18n.t("deployment.diffTitle", { target }) : target;
  }
</script>

{#if rows.length > 0}
  <div class="pills">
    {#each rows as r (r.dest)}
      {@const diffable = DIFFABLE.includes(r.state)}
      <button
        class="pill"
        class:link={diffable}
        data-tone={tone(r.state)}
        disabled={!diffable}
        title={pillTitle(install.toolLabel(r.tool), r.projectPath, r.state, diffable)}
        onclick={() => diffable && onDiff({ slug: r.slug, tool: r.tool, projectPath: r.projectPath, name: agent.name })}
      >
        <span class="pdot" data-tone={tone(r.state)}></span>
        <span class="plabel">{install.toolLabel(r.tool)}{#if r.projectPath}<span class="pproj">{basename(r.projectPath)}</span>{/if}</span>
      </button>
    {/each}
  </div>
{:else}
  <p class="none">{i18n.t("deployment.notDeployed")}</p>
{/if}

<style>
  .pills { display: flex; flex-wrap: wrap; gap: 6px; }
  .none { font-size: var(--text-body-sm); color: var(--color-text-muted); }
  .pill {
    display: inline-flex; align-items: center; gap: 5px;
    height: 24px; padding: 0 10px; border-radius: 999px;
    border: 1px solid var(--color-border); background: var(--color-surface-sunken);
    font-size: var(--text-body-sm); color: var(--color-text-secondary);
  }
  .pill.link { cursor: pointer; }
  .pill.link:hover { border-color: var(--color-border-strong, var(--color-text-muted)); color: var(--color-text-primary); }
  .pill:disabled { cursor: default; }
  .pdot { width: 7px; height: 7px; border-radius: 999px; background: var(--color-text-muted); flex: none; }
  .pdot[data-tone="ok"]     { background: var(--color-success); }
  .pdot[data-tone="warn"]   { background: var(--color-warning); }
  .pdot[data-tone="info"]   { background: var(--color-brand); }
  .pdot[data-tone="danger"] { background: var(--color-danger); }
  .plabel { display: inline-flex; align-items: center; gap: 5px; font-weight: var(--fw-medium); color: var(--color-text-primary); }
  .pproj { font-size: var(--text-caption); color: var(--color-text-muted); }
</style>
