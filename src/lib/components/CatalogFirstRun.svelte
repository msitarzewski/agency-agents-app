<script lang="ts">
  /**
   * CatalogFirstRun.svelte — first-launch catalog-source picker (#1).
   *
   * Shown once, before anything else, when the user hasn't chosen where the
   * agent catalog lives (`catalog.configured === false`). Three paths:
   *   1. Use my own clone  — detect/Find + folder picker (manage-with-permission)
   *   2. Set one up for me  — provision ~/.agency-agents (git clone or snapshot)
   *   3. Bundled snapshot   — the always-works default
   *
   * Posture: nothing is written until the user picks. Choosing any option
   * persists the choice (configured → true), which dismisses this modal.
   */
  import { onMount } from "svelte";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import FolderGit2 from "@lucide/svelte/icons/folder-git-2";
  import Sparkles from "@lucide/svelte/icons/sparkles";
  import Package from "@lucide/svelte/icons/package";
  import Search from "@lucide/svelte/icons/search";
  import Check from "@lucide/svelte/icons/check";

  import { catalog } from "$lib/stores/catalog.svelte";
  import { i18n } from "$lib/stores/i18n.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import type { CatalogCandidate } from "$lib/types";

  let expanded = $state<"clone" | null>(null);
  let manage = $state(true);

  onMount(() => void catalog.detect(false));

  async function choose(fn: () => Promise<unknown>, ok: string) {
    try {
      await fn();
      toast.success(ok);
    } catch (e) {
      toast.error(i18n.t("firstRun.error"), String(e));
    }
  }

  async function pickFolder() {
    const picked = await openDialog({ directory: true, multiple: false, title: i18n.t("catalog.chooseCloneTitle") });
    if (typeof picked === "string") {
      await choose(() => catalog.useClone(picked, manage), i18n.t("firstRun.usingClone"));
    }
  }

  function useCandidate(c: CatalogCandidate) {
    void choose(() => catalog.useClone(c.path, manage), i18n.t("firstRun.usingPath", { path: c.path }));
  }
</script>

<div class="scrim">
  <div class="box" role="dialog" aria-modal="true" aria-label={i18n.t("firstRun.dialogAria")}>
    <header>
      <h1>{i18n.t("firstRun.title")}</h1>
      <p class="lede">{i18n.t("firstRun.lede")}</p>
    </header>

    <div class="cards">
      <!-- 1. Use my own clone -->
      <div class="card" class:open={expanded === "clone"}>
        <button class="card-head" onclick={() => (expanded = expanded === "clone" ? null : "clone")}>
          <FolderGit2 size={22} />
          <div class="ct">
            <span class="t">{i18n.t("firstRun.useClone")}</span>
            <span class="d">{i18n.t("firstRun.useCloneDesc")}</span>
          </div>
        </button>

        {#if expanded === "clone"}
          <div class="card-body">
            {#if catalog.detection?.candidates.length}
              <ul class="cands">
                {#each catalog.detection.candidates as c (c.path)}
                  <li>
                    <button class="cand" disabled={catalog.busy} onclick={() => useCandidate(c)}>
                      <div class="cand-main">
                        <span class="cand-path">{c.path}</span>
                        <span class="cand-meta">
                          {i18n.count(c.agentCount, "common.agent.one", "common.agent.many")}{c.hasGit ? " · git" : ""} · {c.kind === "managed" ? "~/.agency-agents" : i18n.t("common.detected")}
                        </span>
                      </div>
                      <Check size={15} />
                    </button>
                  </li>
                {/each}
              </ul>
            {:else}
              <p class="empty">{i18n.t("firstRun.noClone")}</p>
            {/if}

            <label class="manage">
              <input type="checkbox" bind:checked={manage} />
              {i18n.t("firstRun.manage")}
            </label>

            <div class="row-actions">
              <button class="ghost" disabled={catalog.scanning} onclick={() => catalog.detect(true)}>
                <Search size={14} /><span>{catalog.scanning ? i18n.t("common.searching") : i18n.t("catalog.find")}</span>
              </button>
              <button class="ghost" disabled={catalog.busy} onclick={pickFolder}>{i18n.t("catalog.chooseFolder")}</button>
            </div>
          </div>
        {/if}
      </div>

      <!-- 2. Set one up for me -->
      <button class="card simple" disabled={catalog.busy} onclick={() => choose(() => catalog.provisionManaged(), i18n.t("catalog.setupManaged"))}>
        <Sparkles size={22} />
        <div class="ct">
          <span class="t">{i18n.t("firstRun.setup")}</span>
          <span class="d">{i18n.t("firstRun.setupDesc")}</span>
        </div>
      </button>

      <!-- 3. Bundled snapshot -->
      <button class="card simple" disabled={catalog.busy} onclick={() => choose(() => catalog.useBundled(), i18n.t("catalog.usingBundled"))}>
        <Package size={22} />
        <div class="ct">
          <span class="t">{i18n.t("firstRun.bundled")}</span>
          <span class="d">{i18n.t("firstRun.bundledDesc")}</span>
        </div>
      </button>
    </div>

    {#if catalog.error}<p class="err">{catalog.error}</p>{/if}
  </div>
</div>

<style>
  .scrim {
    position: fixed; inset: 36px 0 0 0; z-index: 90;
    display: flex; align-items: center; justify-content: center;
    background: color-mix(in srgb, var(--color-bg) 70%, transparent);
    backdrop-filter: blur(6px);
  }
  .box {
    width: min(560px, 92vw); max-height: 86vh; overflow-y: auto;
    background: var(--color-surface-raised);
    border: 1px solid var(--color-border); border-radius: var(--radius-lg);
    box-shadow: var(--shadow-lg); padding: var(--space-6);
    display: flex; flex-direction: column; gap: var(--space-5);
  }
  header h1 { font-size: var(--text-h1); font-weight: var(--fw-semibold); color: var(--color-text-primary); }
  .lede { margin-top: var(--space-2); font-size: var(--text-body-sm); color: var(--color-text-secondary); line-height: var(--lh-normal); }
  .cards { display: flex; flex-direction: column; gap: var(--space-3); }
  .card {
    border: 1px solid var(--color-border); border-radius: var(--radius-md);
    background: var(--color-surface); overflow: hidden;
  }
  .card.simple {
    display: flex; align-items: flex-start; gap: var(--space-3);
    padding: var(--space-4); text-align: left; cursor: pointer; color: inherit;
  }
  .card.simple:hover:not(:disabled) { background: var(--color-surface-sunken); border-color: var(--color-brand); }
  .card.simple:disabled { opacity: 0.5; cursor: default; }
  .card-head {
    width: 100%; display: flex; align-items: flex-start; gap: var(--space-3);
    padding: var(--space-4); background: transparent; cursor: pointer; text-align: left; color: inherit;
  }
  .card-head:hover { background: var(--color-surface-sunken); }
  .card.open { border-color: var(--color-brand); }
  .ct { display: flex; flex-direction: column; gap: 3px; min-width: 0; }
  .ct .t { font-weight: var(--fw-medium); color: var(--color-text-primary); }
  .ct .d { font-size: var(--text-caption); color: var(--color-text-muted); line-height: var(--lh-normal); }
  .card-body { padding: 0 var(--space-4) var(--space-4); display: flex; flex-direction: column; gap: var(--space-3); }
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
  .empty { font-size: var(--text-body-sm); color: var(--color-text-muted); }
  .manage { display: flex; align-items: center; gap: 8px; font-size: var(--text-caption); color: var(--color-text-secondary); }
  .row-actions { display: flex; gap: var(--space-2); }
  .ghost {
    display: inline-flex; align-items: center; gap: 6px; height: 30px; padding: 0 var(--space-3);
    border: 1px solid var(--color-border); border-radius: var(--radius-md);
    background: transparent; color: var(--color-text-secondary); font-size: var(--text-body-sm); cursor: pointer;
  }
  .ghost:hover:not(:disabled) { color: var(--color-text-primary); background: var(--color-surface-sunken); }
  .ghost:disabled { opacity: 0.5; cursor: default; }
  .err { font-size: var(--text-body-sm); color: var(--color-danger); }
</style>
