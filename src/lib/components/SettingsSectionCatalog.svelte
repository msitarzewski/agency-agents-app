<script lang="ts">
  /**
   * SettingsSectionCatalog.svelte — Catalog source + sync management (#1).
   *
   * The home for "manage the repo": which source is active, its git provenance
   * (commit / branch / dirty), how far behind upstream it is (fetch + diffstat),
   * GitHub repo stats + sign-in, and the controls to pull, switch source, or
   * point at your own clone. Reuses the existing `github` store (device-flow
   * auth + repo stats) — no new auth code.
   */
  import { onMount } from "svelte";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import Search from "@lucide/svelte/icons/search";
  import FolderGit2 from "@lucide/svelte/icons/folder-git-2";
  import Sparkles from "@lucide/svelte/icons/sparkles";
  import Package from "@lucide/svelte/icons/package";
  import GitCommitHorizontal from "@lucide/svelte/icons/git-commit-horizontal";
  import Star from "@lucide/svelte/icons/star";
  import GitFork from "@lucide/svelte/icons/git-fork";
  import CircleDot from "@lucide/svelte/icons/circle-dot";
  import ExternalLink from "@lucide/svelte/icons/external-link";

  import { catalog } from "$lib/stores/catalog.svelte";
  import { github, type RepoStatsOutcome } from "$lib/stores/github.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import { safeOpenUrl } from "$lib/util/url";
  import { i18n } from "$lib/stores/i18n.svelte";
  import type { CatalogCandidate } from "$lib/types";

  let manage = $state(true);
  let repoStats = $state<RepoStatsOutcome>({ kind: "loading" });

  onMount(() => {
    void catalog.load();
    void catalog.loadStatus();
    void catalog.detect(false);
    void github.loadStatus();
  });

  // Catalog repo homepage (for GitHub stats / links), derived from the remote.
  const repoSlug = $derived(catalog.status?.repoSlug ?? null);
  const repoUrl = $derived(repoSlug ? `https://github.com/${repoSlug}` : null);

  // Fetch GitHub stats whenever we learn the catalog's repo slug.
  $effect(() => {
    if (repoUrl) void github.getRepoStats(repoUrl).then((o) => (repoStats = o));
  });

  async function run(fn: () => Promise<unknown>, ok: string) {
    try {
      await fn();
      toast.success(ok);
    } catch (e) {
      toast.error(i18n.t("catalog.actionFailed"), String(e));
    }
  }

  async function pickFolder() {
    const picked = await openDialog({ directory: true, multiple: false, title: i18n.t("catalog.chooseCloneTitle") });
    if (typeof picked === "string") {
      await run(() => catalog.useClone(picked, manage), i18n.t("catalog.switchedClone"));
    }
  }

  function useCandidate(c: CatalogCandidate) {
    void run(() => catalog.useClone(c.path, manage), i18n.t("catalog.switchedPath", { path: c.path }));
  }

  const isReadOnly = $derived(catalog.source.kind === "userClone" && !catalog.source.manage);
  const st = $derived(catalog.status);
  const uc = $derived(catalog.updateCheck);
  const sourceLabel = $derived.by(() => {
    switch (catalog.source.kind) {
      case "bundled":
        return i18n.t("catalog.source.bundled");
      case "managed":
        return i18n.t("catalog.source.managed");
      case "userClone":
        return i18n.t(catalog.source.manage ? "catalog.source.userManaged" : "catalog.source.userReadOnly");
    }
  });

  function shortDate(iso: string | null): string {
    if (!iso) return "—";
    const d = new Date(iso);
    return Number.isNaN(d.getTime()) ? iso : d.toLocaleDateString();
  }
</script>

<div class="section">
  <h2>{i18n.t("catalog.title")}</h2>

  <!-- Source + git provenance -->
  <dl class="meta">
    <div class="row"><dt>{i18n.t("catalog.source")}</dt><dd>{sourceLabel}</dd></div>
    {#if catalog.sourcePath}
      <div class="row"><dt>{i18n.t("catalog.path")}</dt><dd class="mono">{catalog.sourcePath}</dd></div>
    {/if}
    <div class="row"><dt>{i18n.t("catalog.agents")}</dt><dd>{st?.agentCount ?? catalog.status?.agentCount ?? "—"}</dd></div>
    {#if st?.isGit}
      <div class="row">
        <dt>{i18n.t("catalog.commit")}</dt>
        <dd class="mono"><GitCommitHorizontal size={13} /> {st.commit}{st.branch ? ` (${st.branch})` : ""}</dd>
      </div>
      <div class="row">
        <dt>{i18n.t("catalog.lastChange")}</dt>
        <dd>{st.lastCommitSubject ?? "—"} <span class="muted">· {shortDate(st.lastCommitDate)}</span></dd>
      </div>
      {#if st.dirtyCount > 0}
        <div class="row"><dt>{i18n.t("catalog.localEdits")}</dt><dd class="warn">{i18n.count(st.dirtyCount, "common.change.one", "common.change.many")}</dd></div>
      {/if}
    {:else}
      <div class="row"><dt>{i18n.t("catalog.version")}</dt><dd class="mono">{st?.version ?? "—"}</dd></div>
      <div class="row"><dt>{i18n.t("catalog.fetched")}</dt><dd>{shortDate(st?.fetchedAt ?? null)}</dd></div>
    {/if}
  </dl>

  <!-- Sync / updates -->
  <div class="sync">
    <div class="sync-actions">
      {#if st?.isGit}
        <button class="ghost" disabled={catalog.checking || isReadOnly} onclick={() => catalog.checkUpdates()}>
          <Search size={14} /><span>{catalog.checking ? i18n.t("common.checking") : i18n.t("catalog.checkUpdates")}</span>
        </button>
      {/if}
      <button class="primary" disabled={catalog.busy || isReadOnly} onclick={() => run(() => catalog.pull(), i18n.t("catalog.updated"))}>
        <RefreshCw size={14} /><span>{catalog.busy ? i18n.t("common.working") : st?.isGit ? i18n.t("catalog.pullLatest") : i18n.t("catalog.refreshSnapshot")}</span>
      </button>
      {#if isReadOnly}<span class="hint">{i18n.t("catalog.readOnlyHint")}</span>{/if}
    </div>

    {#if uc}
      {#if !uc.isGit}
        <p class="hint">{i18n.t("catalog.snapshotHint")}</p>
      {:else if uc.upToDate}
        <p class="ok">{i18n.t("catalog.upToDate")}{uc.ahead > 0 ? i18n.t("catalog.ahead", { count: uc.ahead }) : ""}.</p>
      {:else}
        <div class="diff">
          <p class="diff-head">
            {i18n.t("catalog.behind", { commits: i18n.count(uc.behind, "common.commit.one", "common.commit.many"), files: i18n.count(uc.changedFiles, "common.file.one", "common.file.many") })}
          </p>
          {#if uc.diffstat}<pre class="diffstat">{uc.diffstat}</pre>{/if}
        </div>
      {/if}
    {/if}
  </div>

  <!-- GitHub -->
  {#if repoSlug}
    <h3>{i18n.t("settings.github")}</h3>
    <div class="gh">
      <div class="gh-repo">
        <button class="link" onclick={() => repoUrl && void safeOpenUrl(repoUrl)}>
          <code>{repoSlug}</code><ExternalLink size={12} />
        </button>
        {#if repoStats.kind === "loaded"}
          <div class="gh-stats">
            <span title={i18n.t("catalog.stars")}><Star size={13} /> {repoStats.stats.stars.toLocaleString()}</span>
            <span title={i18n.t("catalog.forks")}><GitFork size={13} /> {repoStats.stats.forks.toLocaleString()}</span>
            <span title={i18n.t("catalog.openIssues")}><CircleDot size={13} /> {repoStats.stats.openIssues.toLocaleString()}</span>
            {#if repoStats.stats.lastReleaseTag}<span class="rel">{repoStats.stats.lastReleaseTag}</span>{/if}
          </div>
        {:else if repoStats.kind === "rateLimited"}
          <span class="hint">{i18n.t("catalog.githubRateLimit")}</span>
        {/if}
      </div>

      <div class="gh-auth">
        {#if github.status?.signedIn}
          <span class="signed">{i18n.t("catalog.signedInAs", { username: github.status.username ?? "" })}</span>
        {:else}
          <button class="ghost" disabled={github.signinState.kind !== "idle"} onclick={() => github.signIn()}>
            {i18n.t("catalog.signIn")}
          </button>
          <span class="hint">{i18n.t("catalog.signInHint")}</span>
        {/if}
      </div>
    </div>
  {/if}

  <!-- Switch source -->
  <h3>{i18n.t("catalog.switchSource")}</h3>
  <div class="cards">
    <button class="card" disabled={catalog.busy} onclick={() => run(() => catalog.provisionManaged(), i18n.t("catalog.setupManaged"))}>
      <Sparkles size={20} />
      <div class="ct">
        <span class="t">{i18n.t("catalog.setupManaged")}</span>
        <span class="d">{i18n.t("catalog.setupManagedDesc")}</span>
      </div>
    </button>
    <button class="card" disabled={catalog.busy} onclick={() => run(() => catalog.useBundled(), i18n.t("catalog.usingBundled"))}>
      <Package size={20} />
      <div class="ct">
        <span class="t">{i18n.t("catalog.bundled")}</span>
        <span class="d">{i18n.t("catalog.bundledDesc")}</span>
      </div>
    </button>
  </div>

  <h3>{i18n.t("catalog.useOwnClone")}</h3>
  {#if catalog.detection?.candidates.length}
    <ul class="cands">
      {#each catalog.detection.candidates as c (c.path)}
        <li>
          <button class="cand" disabled={catalog.busy} onclick={() => useCandidate(c)}>
            <FolderGit2 size={15} />
            <div class="cand-main">
              <span class="cand-path">{c.path}</span>
              <span class="cand-meta">{i18n.count(c.agentCount, "common.agent.one", "common.agent.many")}{c.hasGit ? " · git" : ""}</span>
            </div>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
  <label class="manage">
    <input type="checkbox" bind:checked={manage} />
    {i18n.t("catalog.manageClone")}
  </label>
  <div class="row-actions">
    <button class="ghost" disabled={catalog.scanning} onclick={() => catalog.detect(true)}>
      <Search size={14} /><span>{catalog.scanning ? i18n.t("common.searching") : i18n.t("catalog.find")}</span>
    </button>
    <button class="ghost" disabled={catalog.busy} onclick={pickFolder}>{i18n.t("catalog.chooseFolder")}</button>
  </div>

  {#if catalog.error}<p class="err">{catalog.error}</p>{/if}
</div>

<style>
  .section { display: flex; flex-direction: column; gap: var(--space-4); max-width: 580px; }
  h2 { font-size: var(--text-h1); font-weight: var(--fw-semibold); color: var(--color-text-primary); margin-bottom: var(--space-2); }
  h3 { font-size: var(--text-h2); font-weight: var(--fw-semibold); color: var(--color-text-primary); margin-top: var(--space-2); }
  .meta {
    display: flex; flex-direction: column; gap: var(--space-1);
    padding: var(--space-3) var(--space-4); background: var(--color-surface-sunken);
    border: 1px solid var(--color-border); border-radius: var(--radius-md);
  }
  .row { display: grid; grid-template-columns: 96px 1fr; gap: var(--space-3); padding: 4px 0; align-items: baseline; }
  dt { font-size: var(--text-body-sm); color: var(--color-text-muted); font-weight: var(--fw-medium); }
  dd { font-size: var(--text-body); color: var(--color-text-primary); overflow: hidden; text-overflow: ellipsis; display: inline-flex; align-items: center; gap: 5px; }
  .mono { font-family: var(--font-mono); font-size: var(--text-mono); }
  .muted { color: var(--color-text-muted); }
  .warn { color: var(--color-warning); }
  .sync { display: flex; flex-direction: column; gap: var(--space-3); }
  .sync-actions { display: flex; align-items: center; gap: var(--space-2); flex-wrap: wrap; }
  .hint { font-size: var(--text-caption); color: var(--color-text-muted); }
  .ok { font-size: var(--text-body-sm); color: var(--color-success); }
  .diff { border: 1px solid var(--color-border); border-radius: var(--radius-md); padding: var(--space-3); background: var(--color-surface-sunken); }
  .diff-head { font-size: var(--text-body-sm); color: var(--color-text-primary); margin-bottom: var(--space-2); }
  .diffstat { font-family: var(--font-mono); font-size: var(--text-mono); color: var(--color-text-secondary); white-space: pre-wrap; max-height: 200px; overflow-y: auto; margin: 0; }
  .primary {
    display: inline-flex; align-items: center; gap: 6px; height: 32px; padding: 0 var(--space-4);
    border: 1px solid var(--color-brand); border-radius: var(--radius-md);
    background: var(--color-brand); color: var(--color-on-brand, #fff); font-size: var(--text-body-sm); cursor: pointer;
  }
  .primary:disabled { opacity: 0.5; cursor: default; }
  .gh { display: flex; flex-direction: column; gap: var(--space-2); padding: var(--space-3) var(--space-4); border: 1px solid var(--color-border); border-radius: var(--radius-md); background: var(--color-surface); }
  .gh-repo { display: flex; align-items: center; justify-content: space-between; gap: var(--space-3); flex-wrap: wrap; }
  .gh-stats { display: flex; align-items: center; gap: var(--space-3); font-size: var(--text-body-sm); color: var(--color-text-secondary); }
  .gh-stats span { display: inline-flex; align-items: center; gap: 4px; }
  .gh-stats .rel { font-family: var(--font-mono); font-size: var(--text-mono); color: var(--color-text-muted); }
  .gh-auth { display: flex; align-items: center; gap: var(--space-3); flex-wrap: wrap; }
  .signed { font-size: var(--text-body-sm); color: var(--color-text-secondary); }
  .link { display: inline-flex; align-items: center; gap: 6px; color: var(--color-text-link); font-size: var(--text-body-sm); cursor: pointer; background: transparent; padding: 0; }
  .link:hover { text-decoration: underline; }
  .link code { font-family: var(--font-mono); font-size: var(--text-mono); color: inherit; }
  .cards { display: flex; flex-direction: column; gap: var(--space-2); }
  .card {
    display: flex; align-items: flex-start; gap: var(--space-3); padding: var(--space-3) var(--space-4);
    border: 1px solid var(--color-border); border-radius: var(--radius-md); background: var(--color-surface);
    text-align: left; cursor: pointer; color: inherit;
  }
  .card:hover:not(:disabled) { background: var(--color-surface-sunken); border-color: var(--color-brand); }
  .card:disabled { opacity: 0.5; cursor: default; }
  .ct { display: flex; flex-direction: column; gap: 3px; min-width: 0; }
  .ct .t { font-weight: var(--fw-medium); color: var(--color-text-primary); }
  .ct .d { font-size: var(--text-caption); color: var(--color-text-muted); line-height: var(--lh-normal); }
  .cands { display: flex; flex-direction: column; gap: 4px; }
  .cand {
    width: 100%; display: flex; align-items: center; gap: var(--space-2);
    padding: var(--space-2) var(--space-3); border: 1px solid var(--color-border);
    border-radius: var(--radius-sm); background: var(--color-surface-sunken); cursor: pointer; color: inherit;
  }
  .cand:hover:not(:disabled) { border-color: var(--color-brand); color: var(--color-brand); }
  .cand-main { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; text-align: left; }
  .cand-path { font-family: var(--font-mono); font-size: var(--text-mono); color: var(--color-text-primary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .cand-meta { font-size: var(--text-caption); color: var(--color-text-muted); }
  .manage { display: flex; align-items: center; gap: 8px; font-size: var(--text-caption); color: var(--color-text-secondary); }
  .row-actions { display: flex; gap: var(--space-2); }
  .ghost {
    display: inline-flex; align-items: center; gap: 6px; height: 30px; padding: 0 var(--space-3);
    border: 1px solid var(--color-border); border-radius: var(--radius-md);
    background: transparent; color: var(--color-text-secondary); font-size: var(--text-body-sm); cursor: pointer;
  }
  .ghost:hover:not(:disabled) { color: var(--color-text-primary); background: var(--color-surface-sunken); }
  .ghost:disabled { opacity: 0.5; cursor: default; }
  code { font-family: var(--font-mono); font-size: 0.92em; }
  .err { font-size: var(--text-body-sm); color: var(--color-danger); }
</style>
