<script lang="ts">
  /**
   * SettingsSectionNetwork.svelte — Agency Agents
   *
   * Privacy controls for the app's small, well-defined network surface. Agency
   * Agents talks to exactly two places: GitHub (to fetch/clone/pull the
   * agency-agents catalog and, optionally, read repo stats + sign in) and the
   * app's own update endpoint. Offline Mode blocks all of it.
   *
   * Backed by `settings.svelte.ts` (mirrors on-disk `settings.json`). The
   * corrupt-on-disk recovery path fails closed: until the file parses, every
   * outbound call is blocked and the only action is [Reset to defaults].
   */

  import AlertTriangle from "@lucide/svelte/icons/triangle-alert";
  import CheckCircle from "@lucide/svelte/icons/check-circle-2";
  import XCircle from "@lucide/svelte/icons/x-circle";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";

  import { settings } from "$lib/stores/settings.svelte";
  import SettingsSectionUpdates from "$lib/components/SettingsSectionUpdates.svelte";
  import { i18n } from "$lib/stores/i18n.svelte";

  const offlineModeDescription = $derived(i18n.t("network.offlineDescription"));

  function toggleParanoid(e: Event) {
    const v = (e.currentTarget as HTMLInputElement).checked;
    void settings.save({ paranoidMode: v });
  }

  function handleReset() {
    void settings.reset();
  }

  type PathStatus = { label: string; desc: string; allowed: boolean };
  let pathStatuses = $derived.by<PathStatus[]>(() => {
    const paranoid = settings.effective.paranoidMode;
    return [
      {
        label: "github.com · codeload.github.com",
        desc: i18n.t("network.githubSourceDesc"),
        allowed: !paranoid,
      },
      {
        label: "api.github.com",
        desc: i18n.t("network.githubApiDesc"),
        allowed: !paranoid,
      },
      {
        label: "raw.githubusercontent.com · objects.githubusercontent.com",
        desc: i18n.t("network.githubAssetsDesc"),
        allowed: !paranoid,
      },
      {
        label: "agencyagents.app",
        desc: i18n.t("network.updaterDesc"),
        allowed: !paranoid,
      },
      {
        label: i18n.t("network.defaultBrowser"),
        desc: i18n.t("network.browserDesc"),
        allowed: true,
      },
    ];
  });
</script>

<div class="section">
  <h2>{i18n.t("network.title")}</h2>

  {#if settings.loading && !settings.data}
    <p class="lead">{i18n.t("network.loading")}</p>
  {:else if settings.corruptOnDisk}
    <div class="callout corrupt" role="alert">
      <div class="callout-head">
        <AlertTriangle size={18} />
        <strong>{i18n.t("network.corruptTitle")}</strong>
      </div>
      <p class="callout-body">{i18n.t("network.corruptBody")}</p>
      {#if settings.error}<p class="callout-error">{settings.error}</p>{/if}
      <button type="button" class="btn-danger" onclick={handleReset} disabled={settings.loading}>
        <RefreshCw size={14} /> {i18n.t("network.reset")}
      </button>
    </div>
  {:else if settings.data}
    <div class="field">
      <label class="toggle" title={offlineModeDescription}>
        <input
          type="checkbox"
          checked={settings.data.paranoidMode}
          onchange={toggleParanoid}
          disabled={settings.loading}
          aria-describedby="offline-mode-hint"
        />
        <span class="toggle-track" aria-hidden="true"></span>
        <span class="toggle-label">{i18n.t("network.offlineMode")}</span>
      </label>
      <p class="hint" id="offline-mode-hint">{offlineModeDescription}</p>
      {#if settings.data.paranoidMode}
        <div class="callout warn" role="status">
          <AlertTriangle size={16} />
          <span>{i18n.t("network.offlineOn")}</span>
        </div>
      {/if}
    </div>

    <div class="field disclosure">
      <span class="field-label">{i18n.t("network.whereConnects")}</span>
      <ol class="paths">
        {#each pathStatuses as p, i (p.label)}
          <li>
            <span class="num">{i + 1}.</span>
            <span class="status" aria-label={p.allowed ? "allowed" : "blocked"}>
              {#if p.allowed}<CheckCircle size={14} class="ok" />{:else}<XCircle size={14} class="bad" />{/if}
            </span>
            <div>
              <code class="path-label">{p.label}</code>
              <p class="path-desc">{p.desc}</p>
            </div>
          </li>
        {/each}
      </ol>
      <p class="hint">{i18n.t("network.everyCall")}</p>
    </div>

    {#if settings.error}<p class="callout-error">{settings.error}</p>{/if}

    <SettingsSectionUpdates />
  {/if}
</div>

<style>
  .section { display: flex; flex-direction: column; gap: var(--space-5); max-width: 580px; }
  h2 { font-size: var(--text-h1); font-weight: var(--fw-semibold); color: var(--color-text-primary); margin-bottom: var(--space-2); }
  .lead { font-size: var(--text-body); color: var(--color-text-secondary); line-height: var(--lh-normal); }
  .field { display: flex; flex-direction: column; gap: var(--space-2); }
  .field-label { font-size: var(--text-body); font-weight: var(--fw-medium); color: var(--color-text-primary); }
  .hint { font-size: var(--text-body-sm); color: var(--color-text-muted); line-height: var(--lh-snug); }

  .toggle { display: inline-flex; align-items: center; gap: var(--space-2); cursor: pointer; user-select: none; }
  .toggle input { position: absolute; opacity: 0; pointer-events: none; }
  .toggle-track {
    width: 36px; height: 20px; background: var(--color-surface-sunken);
    border: 1px solid var(--color-border); border-radius: 999px; position: relative;
    transition: background-color var(--motion-duration-fast) var(--motion-ease-out);
  }
  .toggle-track::after {
    content: ""; position: absolute; top: 1px; left: 1px; width: 16px; height: 16px;
    background: var(--color-surface-raised); border-radius: 50%; box-shadow: var(--shadow-xs);
    transition: transform var(--motion-duration-fast) var(--motion-ease-out);
  }
  .toggle input:checked + .toggle-track { background: var(--color-brand); border-color: var(--color-brand); }
  .toggle input:checked + .toggle-track::after { transform: translateX(16px); background: white; }
  .toggle-label { font-size: var(--text-body); font-weight: var(--fw-medium); color: var(--color-text-primary); }

  .callout { display: flex; flex-direction: column; gap: var(--space-2); padding: var(--space-3); border-radius: var(--radius-md); border: 1px solid var(--color-border); }
  .callout-head { display: inline-flex; align-items: center; gap: var(--space-2); color: var(--color-text-primary); font-size: var(--text-body); }
  .callout-body { font-size: var(--text-body-sm); color: var(--color-text-secondary); line-height: var(--lh-snug); }
  .callout-error { font-family: var(--font-mono); font-size: var(--text-mono); color: var(--color-text-muted); word-break: break-word; margin-top: var(--space-1); }
  .corrupt { background: color-mix(in srgb, var(--color-danger) 8%, var(--color-surface-sunken)); border-color: color-mix(in srgb, var(--color-danger) 35%, var(--color-border)); }
  .warn { flex-direction: row; align-items: center; background: var(--color-surface-sunken); font-size: var(--text-body-sm); color: var(--color-text-secondary); }

  .btn-danger {
    display: inline-flex; align-items: center; gap: 6px; padding: 6px 12px; border-radius: var(--radius-md);
    background: var(--color-danger); color: white; font-size: var(--text-body-sm); font-weight: var(--fw-medium); cursor: pointer; width: max-content;
  }
  .btn-danger:disabled { opacity: 0.6; cursor: not-allowed; }
  .btn-danger:hover:not(:disabled) { filter: brightness(1.05); }

  .disclosure { gap: var(--space-3); }
  .paths { list-style: none; display: flex; flex-direction: column; gap: var(--space-3); padding: var(--space-4); background: var(--color-surface-sunken); border: 1px solid var(--color-border); border-radius: var(--radius-md); margin: 0; }
  .paths li { display: grid; grid-template-columns: 22px 18px 1fr; gap: var(--space-2); align-items: start; }
  .num { font-variant-numeric: tabular-nums; color: var(--color-text-muted); font-size: var(--text-body-sm); padding-top: 2px; }
  .status { display: inline-flex; align-items: center; padding-top: 2px; }
  .status :global(.ok) { color: #58a55c; }
  .status :global(.bad) { color: #d24a4a; }
  .path-label { font-family: var(--font-mono); font-size: var(--text-mono); color: var(--color-text-primary); background: var(--color-surface-raised); padding: 1px 6px; border-radius: var(--radius-sm); border: 1px solid var(--color-border); }
  .path-desc { margin-top: 4px; font-size: var(--text-body-sm); color: var(--color-text-secondary); line-height: var(--lh-snug); }
</style>
