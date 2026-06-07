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
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import AlertTriangle from "@lucide/svelte/icons/triangle-alert";

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

  // ── Bulk selection ──────────────────────────────────────────────
  // Rows keyed by `dest` (the unique {#each} key). Reassign the Set on every
  // change so Svelte 5 reactivity fires reliably.
  let selected = $state<Set<string>>(new Set());
  let selectMode = $state(false);
  let menuOpen = $state(false);
  let bulkBusy = $state(false);
  let confirmDelete = $state(false);

  function enterSelect() {
    selectMode = true;
  }
  function exitSelect() {
    selectMode = false;
    menuOpen = false;
    clearSelection();
  }

  const allSelected = $derived(rows.length > 0 && selected.size === rows.length);
  const someSelected = $derived(selected.size > 0 && selected.size < rows.length);

  function toggleRow(dest: string) {
    const next = new Set(selected);
    if (next.has(dest)) next.delete(dest);
    else next.add(dest);
    selected = next;
  }
  function selectAll() {
    selected = new Set(rows.map((r) => r.dest));
  }
  function clearSelection() {
    selected = new Set();
  }
  function toggleAll() {
    if (allSelected) clearSelection();
    else selectAll();
  }
  // Prune selection when the row set changes (after a reconcile).
  $effect(() => {
    const live = new Set(rows.map((r) => r.dest));
    if ([...selected].some((d) => !live.has(d))) {
      selected = new Set([...selected].filter((d) => live.has(d)));
    }
  });

  function selectedTargets() {
    return rows
      .filter((r) => selected.has(r.dest))
      .map((r) => ({ slug: r.slug, tool: r.tool, projectPath: r.projectPath }));
  }

  async function runBulk(action: "update" | "track" | "uninstall", verb: string) {
    const targets = selectedTargets();
    if (targets.length === 0) return;
    menuOpen = false;
    bulkBusy = true;
    try {
      const { ok, fail } = await install.bulk(action, targets);
      if (fail === 0) toast.success(`${verb} ${ok} agent${ok === 1 ? "" : "s"}`);
      else toast.error(`${verb}: ${ok} ok, ${fail} failed`);
      clearSelection();
    } finally {
      bulkBusy = false;
    }
  }
</script>

<section class="lib">
  <header class="lib-head">
    <div class="lib-titles">
      {#if selectMode && rows.length > 0}
        <input
          type="checkbox"
          class="head-check"
          checked={allSelected}
          indeterminate={someSelected}
          onchange={toggleAll}
          title={allSelected ? "Deselect all" : "Select all"}
          aria-label="Select all"
        />
      {/if}
      <p class="lib-sub">
        {#if selectMode}
          <span class="sel">{selected.size} selected</span>
        {:else}
          {rows.length} install{rows.length === 1 ? "" : "s"}
          {#if attention > 0}· <span class="warn">{attention} need attention</span>{/if}
        {/if}
      </p>
    </div>
    <div class="head-actions">
      {#if selectMode}
        {#if selected.size > 0}
          <div class="bulk-wrap">
            <button class="ghost-btn" disabled={bulkBusy} onclick={() => (menuOpen = !menuOpen)}>
              <span>{bulkBusy ? "Working…" : "With selected"}</span>
              <ChevronDown size={14} />
            </button>
            {#if menuOpen}
              <div class="bulk-menu" role="menu">
                <button class="bulk-opt" role="menuitem" onclick={() => runBulk("update", "Updated")}>
                  <RefreshIcon size={14} /><span>Update — replace with catalog version</span>
                </button>
                <button class="bulk-opt" role="menuitem" onclick={() => runBulk("track", "Tracked")}>
                  <PlusIcon size={14} /><span>Track — keep file, start managing</span>
                </button>
                <button class="bulk-opt danger" role="menuitem" onclick={() => { menuOpen = false; confirmDelete = true; }}>
                  <TrashIcon size={14} /><span>Delete — remove files from disk</span>
                </button>
              </div>
            {/if}
          </div>
        {/if}
        <button class="ghost-btn primary" onclick={exitSelect}>Done</button>
      {:else}
        {#if rows.length > 0}
          <button class="ghost-btn" onclick={enterSelect}>Select</button>
        {/if}
        <button class="ghost-btn" onclick={() => install.reconcile()} title="Re-scan">
          <RefreshIcon size={15} /><span>Rescan</span>
        </button>
      {/if}
    </div>
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
        <li class="row" class:busy class:picked={selectMode && selected.has(r.dest)}>
          {#if selectMode}
            <input
              type="checkbox"
              class="r-check"
              checked={selected.has(r.dest)}
              onchange={() => toggleRow(r.dest)}
              aria-label={`Select ${r.name}`}
            />
          {/if}
          <span class="r-emoji" aria-hidden="true">{emoji(r.slug)}</span>
          <div class="r-main">
            <span class="r-name">{r.name}</span>
            <span class="r-meta">
              {install.toolLabel(r.tool)}
              {#if r.projectPath}· <span class="r-proj" title={r.projectPath}>{r.projectPath.split("/").pop()}</span>{/if}
            </span>
          </div>
          <Pill tone={tone(r.state)}>{stateLabel(r)}</Pill>
          {#if !selectMode}
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
          {/if}
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

{#if confirmDelete}
  <button class="cd-scrim" aria-label="Cancel" onclick={() => (confirmDelete = false)}></button>
  <div class="cd-box" role="alertdialog" aria-modal="true" aria-label="Confirm delete">
    <div class="cd-head"><AlertTriangle size={20} /><h2>Delete {selected.size} agent{selected.size === 1 ? "" : "s"}?</h2></div>
    <p class="cd-body">
      This <strong>permanently removes the file{selected.size === 1 ? "" : "s"} from disk</strong> —
      including any installed outside this app (your CLI setup). <strong>There is no undo.</strong>
    </p>
    <p class="cd-note">Tip: to swap in the catalog version instead of deleting, use <strong>Update</strong> — it backs your copy up first.</p>
    <div class="cd-actions">
      <button class="cd-cancel" onclick={() => (confirmDelete = false)}>Cancel</button>
      <button class="cd-delete" disabled={bulkBusy} onclick={() => { confirmDelete = false; runBulk("uninstall", "Deleted"); }}>
        <TrashIcon size={14} /> Delete {selected.size}
      </button>
    </div>
  </div>
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
  .lib-titles { display: flex; align-items: center; gap: var(--space-3); }
  .lib-sub { color: var(--color-text-secondary); font-size: var(--text-body-sm); }
  .lib-sub .warn { color: var(--color-warning); font-weight: var(--fw-medium); }
  .lib-sub .sel { color: var(--color-brand); font-weight: var(--fw-medium); }
  .head-check, .r-check { accent-color: var(--color-brand); cursor: pointer; width: 15px; height: 15px; flex: none; }
  .head-actions { display: flex; align-items: center; gap: var(--space-2); }
  .bulk-wrap { position: relative; }
  .bulk-menu {
    position: absolute; top: calc(100% + 6px); right: 0; z-index: 30;
    min-width: 280px; padding: 4px;
    background: var(--color-surface-raised); border: 1px solid var(--color-border);
    border-radius: var(--radius-md); box-shadow: var(--shadow-lg);
    display: flex; flex-direction: column; gap: 1px;
  }
  .bulk-opt {
    display: flex; align-items: center; gap: var(--space-2);
    padding: 8px 10px; border-radius: var(--radius-sm);
    background: transparent; color: var(--color-text-primary);
    font-size: var(--text-body-sm); text-align: left; cursor: pointer;
  }
  .bulk-opt:hover { background: var(--color-surface-sunken); }
  .bulk-opt.danger { color: var(--color-danger); }
  .bulk-opt.danger:hover { background: color-mix(in srgb, var(--color-danger) 12%, transparent); }
  .ghost-btn {
    display: inline-flex; align-items: center; gap: 6px;
    height: 30px; padding: 0 var(--space-3);
    border: 1px solid var(--color-border); border-radius: var(--radius-md);
    background: transparent; color: var(--color-text-secondary);
    font-size: var(--text-body-sm); cursor: pointer;
  }
  .ghost-btn:hover { color: var(--color-text-primary); background: var(--color-surface-sunken); }
  .ghost-btn.primary { color: var(--color-brand); border-color: var(--color-brand); }
  .ghost-btn.primary:hover { background: color-mix(in srgb, var(--color-brand) 12%, transparent); color: var(--color-brand); }
  .rows { overflow-y: auto; padding: var(--space-3) var(--space-4); display: flex; flex-direction: column; gap: 1px; }
  .row {
    display: flex; align-items: center; gap: var(--space-3);
    padding: var(--space-3); border-radius: var(--radius-md);
  }
  .row:hover { background: var(--color-surface-sunken); }
  .row.picked { background: color-mix(in srgb, var(--color-brand) 10%, transparent); }
  .row.picked:hover { background: color-mix(in srgb, var(--color-brand) 16%, transparent); }
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

  /* ── Delete confirmation ── */
  .cd-scrim {
    position: fixed; inset: 36px 0 0 0; z-index: 92; border: 0; cursor: default;
    background: color-mix(in srgb, var(--color-bg) 60%, transparent); backdrop-filter: blur(4px);
  }
  .cd-box {
    position: fixed; z-index: 93; top: 50%; left: 50%; transform: translate(-50%, -50%);
    width: min(460px, 92vw); padding: var(--space-5);
    background: var(--color-surface-raised); border: 1px solid var(--color-border);
    border-radius: var(--radius-lg); box-shadow: var(--shadow-lg);
    display: flex; flex-direction: column; gap: var(--space-3);
  }
  .cd-head { display: flex; align-items: center; gap: var(--space-2); color: var(--color-danger); }
  .cd-head h2 { font-size: var(--text-h2); font-weight: var(--fw-semibold); color: var(--color-text-primary); }
  .cd-body { font-size: var(--text-body-sm); color: var(--color-text-secondary); line-height: var(--lh-normal); }
  .cd-note { font-size: var(--text-caption); color: var(--color-text-muted); }
  .cd-actions { display: flex; justify-content: flex-end; gap: var(--space-2); margin-top: var(--space-1); }
  .cd-cancel {
    height: 32px; padding: 0 var(--space-4); border-radius: var(--radius-md);
    border: 1px solid var(--color-border); background: transparent;
    color: var(--color-text-secondary); font-size: var(--text-body-sm); cursor: pointer;
  }
  .cd-cancel:hover { color: var(--color-text-primary); background: var(--color-surface-sunken); }
  .cd-delete {
    display: inline-flex; align-items: center; gap: 6px;
    height: 32px; padding: 0 var(--space-4); border-radius: var(--radius-md);
    border: 1px solid var(--color-danger); background: var(--color-danger);
    color: #fff; font-size: var(--text-body-sm); font-weight: var(--fw-medium); cursor: pointer;
  }
  .cd-delete:hover:not(:disabled) { filter: brightness(1.08); }
  .cd-delete:disabled { opacity: 0.5; cursor: default; }
</style>
