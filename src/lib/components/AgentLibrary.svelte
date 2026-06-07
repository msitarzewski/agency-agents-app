<script lang="ts">
  /**
   * Agent Library — the reconciled cross-tool view of everything installed,
   * resolved into the 5 states (Current / Outdated / Modified / Removed /
   * Foreign). This is the surface that makes the install-tracking layer
   * visible: one row per (agent, tool, project), attention-needing rows first.
   */
  import Pill from "./Pill.svelte";
  import EmptyState from "./EmptyState.svelte";
  import LoadingState from "./LoadingState.svelte";
  import DiffModal from "./DiffModal.svelte";
  import RefreshIcon from "@lucide/svelte/icons/refresh-cw";
  import TrashIcon from "@lucide/svelte/icons/trash-2";
  import DownloadIcon from "@lucide/svelte/icons/download";
  import PlusIcon from "@lucide/svelte/icons/plus";
  import GitCompareIcon from "@lucide/svelte/icons/git-compare";

  import { onMount } from "svelte";
  import { install } from "$lib/stores/install.svelte";
  import { corpus } from "$lib/stores/corpus.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import type { InstalledAgent, InstallState } from "$lib/types";

  // Pure reader: the reconcile is triggered globally in +layout. Triggering it
  // here (read + mutate the same store during this component's lifecycle) froze
  // reactivity. corpus.ensureLoaded() is safe (its own internal guard).
  onMount(() => corpus.ensureLoaded());

  // Attention-needing first; current last. Stable within a bucket by name.
  const ORDER: Record<InstallState, number> = {
    outdated: 0, modified: 1, foreign: 2, removed: 3, current: 4,
  };

  // SINGLE source of truth for this component: snapshot the install store's
  // reactive array into LOCAL $state via ONE $effect, and read ONLY these
  // locals below. Previously the template read install.installed in ~8 scattered
  // places (.length, .filter, .slice, …); each became its own subscription and
  // they updated independently — the count said 0 while the list said 189. One
  // effect → one update → everything moves together. Re-runs when
  // install.installed / reconciling change.
  let rows = $state<InstalledAgent[]>([]);
  let attention = $state(0);
  let scanning = $state(true);
  $effect(() => {
    const list = install.installed;
    rows = list.slice().sort(
      (a, b) =>
        (ORDER[a.state] ?? 9) - (ORDER[b.state] ?? 9) ||
        (a.name ?? "").localeCompare(b.name ?? ""),
    );
    attention = list.filter((r) => r.state !== "current").length;
    scanning = install.reconciling || !install.reconciled;
  });

  // Build the slug→emoji lookup once (not a .find() per row — the Library can
  // hold 180+ rows when a Foreign sweep finds a full install.sh deployment).
  const emojiBySlug = $derived(
    new Map(corpus.agents.map((a) => [a.slug, a.emoji] as const)),
  );
  function emoji(slug: string): string {
    return emojiBySlug.get(slug) ?? "🧩";
  }

  function tone(s: InstallState): "success" | "warning" | "danger" | "info" {
    if (s === "current") return "success";
    if (s === "outdated" || s === "modified") return "warning";
    if (s === "removed") return "danger";
    return "info";
  }

  function stateLabel(r: InstalledAgent): string {
    if (r.state === "outdated") {
      return r.updateKind === "cosmetic" ? "Update · cosmetic" : "Update available";
    }
    return r.state.charAt(0).toUpperCase() + r.state.slice(1);
  }

  async function act(fn: () => Promise<unknown>, ok: string) {
    try {
      await fn();
      toast.success(ok);
    } catch (e) {
      toast.error("Action failed", String(e));
    }
  }

  // Diff viewer — for rows where the filename matches a catalog agent but the
  // bytes differ (foreign / modified / outdated). "What's actually different?"
  let diffTarget = $state<{ slug: string; tool: InstalledAgent["tool"]; projectPath: string | null; name: string } | null>(null);
  const DIFFABLE: InstallState[] = ["foreign", "modified", "outdated"];
  function canDiff(s: InstallState): boolean {
    return DIFFABLE.includes(s);
  }
  function openDiff(r: InstalledAgent) {
    diffTarget = { slug: r.slug, tool: r.tool, projectPath: r.projectPath, name: r.name };
  }
</script>

<section class="lib">
  <header class="lib-head">
    <div class="lib-titles">
      <p class="lib-sub">
        {rows.length} install{rows.length === 1 ? "" : "s"}
        {#if attention > 0}· <span class="warn">{attention} need attention</span>{/if}
      </p>
    </div>
    <button class="ghost-btn" onclick={() => install.reconcile()} title="Re-scan">
      <RefreshIcon size={15} /><span>Rescan</span>
    </button>
  </header>

  {#if rows.length === 0 && scanning}
    <LoadingState rows={6} label="Scanning your tools…" />
  {:else if rows.length === 0}
    <EmptyState title="Nothing installed yet">
      {#snippet icon()}<DownloadIcon size={48} />{/snippet}
      Open an agent in the catalog and choose <strong>Install into…</strong> to deploy it
      into Claude Code, Cursor, Codex and more. Everything you install shows up here.
    </EmptyState>
  {:else}
    <ul class="rows">
      {#each rows as r (r.dest)}
        {@const busy = install.busy === `${r.slug}:${r.tool}`}
        <li class="row" class:busy>
          <span class="r-emoji" aria-hidden="true">{emoji(r.slug)}</span>
          <div class="r-main">
            <span class="r-name">{r.name}</span>
            <span class="r-meta">
              {install.toolLabel(r.tool)}
              {#if r.projectPath}· <span class="r-proj" title={r.projectPath}>{r.projectPath.split("/").pop()}</span>{/if}
            </span>
          </div>
          <Pill tone={tone(r.state)}>{stateLabel(r)}</Pill>
          <div class="r-actions">
            {#if canDiff(r.state)}
              <button class="act" title="See what's different from the catalog" onclick={() => openDiff(r)}>
                <GitCompareIcon size={14} /><span>Diff</span>
              </button>
            {/if}
            {#if r.state === "outdated"}
              <button class="act" disabled={busy} onclick={() => act(() => install.update(r.slug, r.tool, r.projectPath), `Updated ${r.name}`)}>
                <RefreshIcon size={14} /><span>Update</span>
              </button>
            {:else if r.state === "modified"}
              <!-- Restore the canonical render; the user's edited file is backed
                   up first (backups/ under app data) so this is reversible. -->
              <button class="act" disabled={busy} title="Back up your edits and restore the catalog version" onclick={() => act(() => install.update(r.slug, r.tool, r.projectPath), `Restored ${r.name} (your copy was backed up)`)}>
                <RefreshIcon size={14} /><span>Restore</span>
              </button>
            {:else if r.state === "removed"}
              <button class="act" disabled={busy} onclick={() => act(() => install.install(r.slug, r.tool, r.projectPath), `Reinstalled ${r.name}`)}>
                <DownloadIcon size={14} /><span>Reinstall</span>
              </button>
            {:else if r.state === "foreign"}
              <!-- Update: replace the on-disk file with the catalog/repo version
                   (your copy is backed up to backups/ first). The common "pull
                   what the repo has" action when a local install drifts behind. -->
              <button class="act" disabled={busy} title="Replace with the catalog version (your copy is backed up)" onclick={() => act(() => install.update(r.slug, r.tool, r.projectPath), `Updated ${r.name} from the catalog (your copy was backed up)`)}>
                <RefreshIcon size={14} /><span>Update</span>
              </button>
              <!-- Track is non-destructive: records provenance, keeps your file. -->
              <button class="act primary" disabled={busy} title="Keep your file as-is — just start tracking it (no changes written)" onclick={() => act(() => install.track(r.slug, r.tool, r.projectPath), `Tracking ${r.name}`)}>
                <PlusIcon size={14} /><span>Track</span>
              </button>
            {/if}
            {#if r.state !== "foreign"}
              <button class="act danger" disabled={busy} title="Remove" onclick={() => act(() => install.uninstall(r.slug, r.tool, r.projectPath), `Removed ${r.name}`)}>
                <TrashIcon size={14} />
              </button>
            {/if}
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</section>

{#if diffTarget}
  <DiffModal
    slug={diffTarget.slug}
    tool={diffTarget.tool}
    projectPath={diffTarget.projectPath}
    name={diffTarget.name}
    onClose={() => (diffTarget = null)}
  />
{/if}

<style>
  .lib { display: flex; flex-direction: column; height: 100%; min-height: 0; }
  .lib-head {
    flex: none;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space-4);
    border-bottom: 1px solid var(--color-border);
  }
  .lib-sub { color: var(--color-text-secondary); font-size: var(--text-body-sm); }
  .lib-sub .warn { color: var(--color-warning); font-weight: var(--fw-medium); }
  .ghost-btn {
    display: inline-flex; align-items: center; gap: 6px;
    height: 30px; padding: 0 var(--space-3);
    border: 1px solid var(--color-border); border-radius: var(--radius-md);
    background: transparent; color: var(--color-text-secondary);
    font-size: var(--text-body-sm); cursor: pointer;
  }
  .ghost-btn:hover { color: var(--color-text-primary); background: var(--color-surface-sunken); }
  .rows { overflow-y: auto; padding: var(--space-3) var(--space-4); display: flex; flex-direction: column; gap: 1px; }
  .row {
    display: flex; align-items: center; gap: var(--space-3);
    padding: var(--space-3); border-radius: var(--radius-md);
  }
  .row:hover { background: var(--color-surface-sunken); }
  .row.busy { opacity: 0.6; }
  .r-emoji { font-size: 20px; line-height: 1; flex: none; }
  .r-main { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
  .r-name { font-weight: var(--fw-medium); color: var(--color-text-primary); }
  .r-meta { font-size: var(--text-caption); color: var(--color-text-muted); }
  .r-proj { color: var(--color-text-secondary); }
  .r-actions { display: flex; align-items: center; gap: 6px; flex: none; }
  .act {
    display: inline-flex; align-items: center; gap: 5px;
    height: 28px; padding: 0 var(--space-2);
    border: 1px solid var(--color-border); border-radius: var(--radius-sm);
    background: transparent; color: var(--color-text-secondary);
    font-size: var(--text-caption); cursor: pointer;
  }
  .act:hover:not(:disabled) { color: var(--color-text-primary); background: var(--color-surface-raised); }
  .act:disabled { opacity: 0.5; cursor: default; }
  .act.primary { color: var(--color-brand); border-color: var(--color-brand); }
  .act.danger:hover:not(:disabled) { color: var(--color-danger); border-color: var(--color-danger); }
</style>
