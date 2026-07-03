<script lang="ts">
  /**
   * DiffModal — shows what's different between an on-disk agent file and the
   * canonical catalog render. Opened from the Library for `foreign` / `modified`
   * / `outdated` rows: the filename matches a catalog agent but the bytes don't,
   * and this is "what changed."
   *
   * Reads via the existing `agent_diff` command (install.diff). The on-disk
   * file is the OLD side, the catalog render is the NEW side, so `-` lines are
   * yours and `+` lines are what the catalog would write.
   */
  import { onMount } from "svelte";
  import X from "@lucide/svelte/icons/x";
  import { install } from "$lib/stores/install.svelte";
  import { i18n } from "$lib/stores/i18n.svelte";
  import { diffLines, diffStat, type DiffRow } from "$lib/util/diff";
  import type { AgentDiff, Tool } from "$lib/types";

  interface Props {
    slug: string;
    tool: Tool;
    projectPath: string | null;
    name: string;
    onClose: () => void;
  }
  let { slug, tool, projectPath, name, onClose }: Props = $props();

  let data = $state<AgentDiff | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(true);

  onMount(() => {
    void install
      .diff(slug, tool, projectPath)
      .then((d) => (data = d))
      .catch((e) => (error = String(e)))
      .finally(() => (loading = false));
  });

  const rows = $derived<DiffRow[]>(
    data ? diffLines(data.onDisk ?? "", data.proposed) : [],
  );
  const stat = $derived(diffStat(rows));

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") { e.preventDefault(); onClose(); }
  }
</script>

<svelte:window onkeydown={onKey} />

<button class="scrim" aria-label={i18n.t("diff.closeAria")} onclick={onClose}></button>
<div class="box" role="dialog" aria-modal="true" aria-label={`${name} diff`}>
  <header class="head">
    <div class="titles">
      <h2 class="title">{name}</h2>
      <span class="sub">{install.toolLabel(tool)}{projectPath ? ` · ${projectPath.split("/").pop()}` : ""}</span>
    </div>
    {#if data && !loading}
      <span class="stat"><span class="add">+{stat.added}</span> <span class="rem">−{stat.removed}</span></span>
    {/if}
    <button class="close" onclick={onClose} aria-label={i18n.t("common.close")}><X size={16} /></button>
  </header>

  {#if data}
    <p class="dest" title={data.dest}>{data.dest}</p>
  {/if}

  <div class="body">
    {#if loading}
      <p class="muted">{i18n.t("diff.comparing")}</p>
    {:else if error}
      <p class="err">{error}</p>
    {:else if data && data.onDisk === null}
      <p class="muted">{i18n.t("diff.fileMissing")}</p>
    {:else if data && !data.differs}
      <p class="muted">{i18n.t("diff.identical")}</p>
    {:else}
      <div class="legend">
        <span class="rem">{i18n.t("diff.onDisk")}</span>
        <span class="add">{i18n.t("diff.catalog")}</span>
      </div>
      <pre class="diff">{#each rows as r (`${r.oldNo ?? "x"}:${r.newNo ?? "x"}:${r.tag}`)}<span
            class="line {r.tag === '+' ? 'l-add' : r.tag === '-' ? 'l-rem' : 'l-ctx'}"
          ><span class="ln">{r.oldNo ?? ""}</span><span class="ln">{r.newNo ?? ""}</span><span class="gutter">{r.tag}</span><span class="txt">{r.text}</span></span>{/each}</pre>
    {/if}
  </div>
</div>

<style>
  .scrim {
    position: fixed; inset: 36px 0 0 0; z-index: 88;
    background: color-mix(in srgb, var(--color-bg) 60%, transparent);
    backdrop-filter: blur(4px); border: 0; cursor: default;
  }
  .box {
    position: fixed; z-index: 89;
    top: 60px; bottom: 60px; left: 50%; transform: translateX(-50%);
    width: min(820px, 92vw);
    display: flex; flex-direction: column;
    background: var(--color-surface-raised);
    border: 1px solid var(--color-border); border-radius: var(--radius-lg);
    box-shadow: var(--shadow-lg); overflow: hidden;
  }
  .head {
    flex: none; display: flex; align-items: center; gap: var(--space-3);
    padding: var(--space-3) var(--space-4); border-bottom: 1px solid var(--color-border);
  }
  .titles { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 1px; }
  .title { font-size: var(--text-h2); font-weight: var(--fw-semibold); color: var(--color-text-primary); }
  .sub { font-size: var(--text-caption); color: var(--color-text-muted); }
  .stat { font-family: var(--font-mono); font-size: var(--text-mono); display: inline-flex; gap: 8px; }
  .add { color: var(--color-success); }
  .rem { color: var(--color-danger); }
  .close {
    flex: none; display: inline-flex; align-items: center; justify-content: center;
    width: 28px; height: 28px; border-radius: var(--radius-md);
    background: transparent; color: var(--color-text-muted); cursor: pointer;
  }
  .close:hover { background: var(--color-surface-sunken); color: var(--color-text-primary); }
  .dest {
    flex: none; padding: 6px var(--space-4); margin: 0;
    font-family: var(--font-mono); font-size: var(--text-mono);
    color: var(--color-text-muted); border-bottom: 1px solid var(--color-border);
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .body { flex: 1; min-height: 0; overflow: auto; padding: var(--space-3) var(--space-4); }
  .muted { color: var(--color-text-muted); font-size: var(--text-body-sm); }
  .err { color: var(--color-danger); font-size: var(--text-body-sm); }
  .legend { display: flex; gap: var(--space-4); font-family: var(--font-mono); font-size: var(--text-caption); margin-bottom: var(--space-2); }
  .diff { margin: 0; font-family: var(--font-mono); font-size: var(--text-mono); line-height: 1.5; white-space: pre; }
  .line { display: block; }
  .ln {
    display: inline-block; width: 3.2em; text-align: right; padding-right: 8px;
    color: var(--color-text-muted); opacity: 0.6; user-select: none;
  }
  .gutter { display: inline-block; width: 1.2em; text-align: center; user-select: none; }
  .txt { white-space: pre-wrap; word-break: break-word; }
  .l-add { background: color-mix(in srgb, var(--color-success) 14%, transparent); }
  .l-add .gutter, .l-add .txt { color: var(--color-success); }
  .l-rem { background: color-mix(in srgb, var(--color-danger) 14%, transparent); }
  .l-rem .gutter, .l-rem .txt { color: var(--color-danger); }
  .l-ctx .txt { color: var(--color-text-secondary); }
</style>
