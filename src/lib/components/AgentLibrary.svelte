<script lang="ts">
  /**
   * Agent Library — the reconciled cross-tool view, grouped BY AGENT. One row
   * per agent; a pill per tool it's installed in. Each pill carries that
   * install's reconciled state (Current / Outdated / Modified / Removed /
   * Foreign) and its own actions: ✕ remove, ↻ update-from-catalog (when it's
   * drifted), and click-to-diff (when it differs). Bulk select acts per agent.
   */
  import EmptyState from "./EmptyState.svelte";
  import LoadingState from "./LoadingState.svelte";
  import DiffModal from "./DiffModal.svelte";
  import RefreshIcon from "@lucide/svelte/icons/refresh-cw";
  import TrashIcon from "@lucide/svelte/icons/trash-2";
  import DownloadIcon from "@lucide/svelte/icons/download";
  import PlusIcon from "@lucide/svelte/icons/plus";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import AlertTriangle from "@lucide/svelte/icons/triangle-alert";
  import X from "@lucide/svelte/icons/x";

  import { onMount } from "svelte";
  import { install } from "$lib/stores/install.svelte";
  import { corpus } from "$lib/stores/corpus.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import type { InstalledAgent, InstallState } from "$lib/types";

  onMount(() => corpus.ensureLoaded());

  interface Group {
    slug: string;
    name: string;
    installs: InstalledAgent[];
    attention: boolean; // any install not in sync
  }

  // Group the reconciled installs by agent. One row per agent; pills = tools.
  let groups = $state<Group[]>([]);
  let attention = $state(0);
  let installCount = $state(0);
  let scanning = $state(true);
  $effect(() => {
    const list = install.installed;
    const bySlug = new Map<string, InstalledAgent[]>();
    for (const r of list) {
      const arr = bySlug.get(r.slug);
      if (arr) arr.push(r);
      else bySlug.set(r.slug, [r]);
    }
    const gs: Group[] = [...bySlug.entries()].map(([slug, installs]) => ({
      slug,
      name: installs[0].name,
      installs: installs.slice().sort((a, b) => install.toolLabel(a.tool).localeCompare(install.toolLabel(b.tool))),
      attention: installs.some((i) => i.state !== "current"),
    }));
    // Agents needing attention first, then alphabetical.
    gs.sort((a, b) => Number(b.attention) - Number(a.attention) || a.name.localeCompare(b.name));
    groups = gs;
    installCount = list.length;
    attention = list.filter((r) => r.state !== "current").length;
    scanning = install.reconciling || !install.reconciled;
  });

  const emojiBySlug = $derived(new Map(corpus.agents.map((a) => [a.slug, a.emoji] as const)));
  function emoji(slug: string): string {
    return emojiBySlug.get(slug) ?? "🧩";
  }

  const DIFFABLE: InstallState[] = ["foreign", "modified", "outdated"];
  function canDiff(s: InstallState): boolean {
    return DIFFABLE.includes(s);
  }
  /** Tooltip for a pill, describing its state. */
  function stateHint(s: InstallState): string {
    switch (s) {
      case "current": return "In sync with the catalog";
      case "outdated": return "A newer catalog version is available";
      case "modified": return "Differs from the catalog (edited)";
      case "removed": return "Tracked but missing on disk";
      case "foreign": return "Recognized but not the catalog version";
    }
  }

  async function act(fn: () => Promise<unknown>, ok: string) {
    try {
      await fn();
      toast.success(ok);
    } catch (e) {
      toast.error("Action failed", String(e));
    }
  }

  // ── Diff ──
  let diffTarget = $state<InstalledAgent | null>(null);

  // ── Bulk selection (keyed by agent slug) ──
  let selected = $state<Set<string>>(new Set());
  let selectMode = $state(false);
  let menuOpen = $state(false);
  let bulkBusy = $state(false);
  let confirmDelete = $state(false);

  function enterSelect() { selectMode = true; }
  function exitSelect() { selectMode = false; menuOpen = false; clearSelection(); }

  const allSelected = $derived(groups.length > 0 && selected.size === groups.length);
  const someSelected = $derived(selected.size > 0 && selected.size < groups.length);

  function toggleRow(slug: string) {
    const next = new Set(selected);
    if (next.has(slug)) next.delete(slug);
    else next.add(slug);
    selected = next;
  }
  function selectAll() { selected = new Set(groups.map((g) => g.slug)); }
  function clearSelection() { selected = new Set(); }
  function toggleAll() { if (allSelected) clearSelection(); else selectAll(); }
  // Prune selection when the agent set changes (after a reconcile).
  $effect(() => {
    const live = new Set(groups.map((g) => g.slug));
    if ([...selected].some((s) => !live.has(s))) {
      selected = new Set([...selected].filter((s) => live.has(s)));
    }
  });

  // All installs of the selected agents — what the bulk ops act on.
  const selInstalls = $derived(groups.filter((g) => selected.has(g.slug)).flatMap((g) => g.installs));
  const canBulkUpdate = $derived(selInstalls.some((i) => i.state !== "current"));
  const canBulkTrack = $derived(selInstalls.some((i) => i.state === "foreign"));

  function toTarget(i: InstalledAgent) {
    return { slug: i.slug, tool: i.tool, projectPath: i.projectPath };
  }

  async function runBulk(action: "update" | "track" | "uninstall", verb: string) {
    let picked = selInstalls;
    if (action === "update") picked = selInstalls.filter((i) => i.state !== "current");
    else if (action === "track") picked = selInstalls.filter((i) => i.state === "foreign");
    const targets = picked.map(toTarget);
    if (targets.length === 0) return;
    menuOpen = false;
    bulkBusy = true;
    try {
      const { ok, fail } = await install.bulk(action, targets);
      if (fail === 0) toast.success(`${verb} ${ok} install${ok === 1 ? "" : "s"}`);
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
      {#if selectMode && groups.length > 0}
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
          {groups.length} agent{groups.length === 1 ? "" : "s"} · {installCount} install{installCount === 1 ? "" : "s"}
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
                <button class="bulk-opt" role="menuitem" disabled={!canBulkUpdate} title={canBulkUpdate ? "" : "All selected are already in sync"} onclick={() => runBulk("update", "Updated")}>
                  <RefreshIcon size={14} /><span>Update — replace with catalog version</span>
                </button>
                <button class="bulk-opt" role="menuitem" disabled={!canBulkTrack} title={canBulkTrack ? "" : "Nothing untracked in the selection"} onclick={() => runBulk("track", "Tracked")}>
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
        {#if groups.length > 0}
          <button class="ghost-btn" onclick={enterSelect}>Select</button>
        {/if}
        <button class="ghost-btn" onclick={() => install.reconcile()} title="Re-scan">
          <RefreshIcon size={15} /><span>Rescan</span>
        </button>
      {/if}
    </div>
  </header>

  {#if groups.length === 0 && scanning}
    <LoadingState rows={6} label="Scanning your tools…" />
  {:else if groups.length === 0}
    <EmptyState title="Nothing installed yet">
      {#snippet icon()}<DownloadIcon size={48} />{/snippet}
      Open an agent in the catalog and choose <strong>Install into…</strong> to deploy it
      into Claude Code, Cursor, Codex and more. Everything you install shows up here.
    </EmptyState>
  {:else}
    <ul class="rows">
      {#each groups as g (g.slug)}
        <li class="row" class:picked={selectMode && selected.has(g.slug)}>
          {#if selectMode}
            <input
              type="checkbox"
              class="r-check"
              checked={selected.has(g.slug)}
              onchange={() => toggleRow(g.slug)}
              aria-label={`Select ${g.name}`}
            />
          {/if}
          <span class="r-emoji" aria-hidden="true">{emoji(g.slug)}</span>
          <div class="r-main">
            <span class="r-name">{g.name}</span>
            <span class="r-pills">
              {#each g.installs as inst (inst.dest)}
                {@const busy = install.busy === `${inst.slug}:${inst.tool}`}
                {@const label = install.toolLabel(inst.tool)}
                <span class="tool-pill" data-state={inst.state} title={stateHint(inst.state)}>
                  {#if canDiff(inst.state) && !selectMode}
                    <button class="tp-body link" title="See what's different from the catalog" onclick={() => (diffTarget = inst)}>
                      {label}{#if inst.projectPath}<span class="tp-proj">{inst.projectPath.split("/").pop()}</span>{/if}
                    </button>
                  {:else}
                    <span class="tp-body">
                      {label}{#if inst.projectPath}<span class="tp-proj">{inst.projectPath.split("/").pop()}</span>{/if}
                    </span>
                  {/if}
                  {#if !selectMode}
                    {#if inst.state !== "current"}
                      <button
                        class="tp-act"
                        disabled={busy}
                        title={`Update from catalog — replace this ${label} copy (yours backed up)`}
                        aria-label="Update from catalog"
                        onclick={() => act(() => install.update(inst.slug, inst.tool, inst.projectPath), `Updated ${g.name} · ${label}`)}
                      ><RefreshIcon size={11} /></button>
                    {/if}
                    <button
                      class="tp-x"
                      disabled={busy}
                      title={`Remove from ${label}`}
                      aria-label={`Remove ${g.name} from ${label}`}
                      onclick={() => act(() => install.uninstall(inst.slug, inst.tool, inst.projectPath), `Removed ${g.name} from ${label}`)}
                    ><X size={11} /></button>
                  {/if}
                </span>
              {/each}
            </span>
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

{#if confirmDelete}
  <button class="cd-scrim" aria-label="Cancel" onclick={() => (confirmDelete = false)}></button>
  <div class="cd-box" role="alertdialog" aria-modal="true" aria-label="Confirm delete">
    <div class="cd-head"><AlertTriangle size={20} /><h2>Delete {selected.size} agent{selected.size === 1 ? "" : "s"}?</h2></div>
    <p class="cd-body">
      This <strong>permanently removes {selInstalls.length} file{selInstalls.length === 1 ? "" : "s"} from disk</strong>
      (every tool these agents are installed in) — including any installed outside this app
      (your CLI setup). <strong>There is no undo.</strong>
    </p>
    <p class="cd-note">Tip: to swap in the catalog version instead of deleting, use <strong>Update</strong> — it backs your copy up first.</p>
    <div class="cd-actions">
      <button class="cd-cancel" onclick={() => (confirmDelete = false)}>Cancel</button>
      <button class="cd-delete" disabled={bulkBusy} onclick={() => { confirmDelete = false; runBulk("uninstall", "Deleted"); }}>
        <TrashIcon size={14} /> Delete {selInstalls.length}
      </button>
    </div>
  </div>
{/if}

<style>
  .lib { display: flex; flex-direction: column; height: 100%; min-height: 0; }
  .lib-head {
    flex: none; display: flex; align-items: center; justify-content: space-between;
    padding: var(--space-4); border-bottom: 1px solid var(--color-border);
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
  .bulk-opt:hover:not(:disabled) { background: var(--color-surface-sunken); }
  .bulk-opt:disabled { opacity: 0.4; cursor: default; }
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
  .r-emoji { font-size: 20px; line-height: 1; flex: none; }
  .r-main { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 4px; }
  .r-name { font-weight: var(--fw-medium); color: var(--color-text-primary); }
  .r-pills { display: flex; flex-wrap: wrap; gap: 6px; }

  .tool-pill {
    display: inline-flex; align-items: center; gap: 4px;
    height: 20px; padding: 0 5px 0 9px;
    border: 1px solid var(--color-border); border-radius: 999px;
    background: var(--color-surface-sunken); color: var(--color-text-secondary);
    font-size: var(--text-caption); line-height: 1;
  }
  /* per-state coloring (current = neutral) */
  .tool-pill[data-state="foreign"] { border-color: color-mix(in srgb, var(--color-brand) 55%, var(--color-border)); color: var(--color-brand); }
  .tool-pill[data-state="outdated"], .tool-pill[data-state="modified"] { border-color: color-mix(in srgb, var(--color-warning) 55%, var(--color-border)); color: var(--color-warning); }
  .tool-pill[data-state="removed"] { border-color: color-mix(in srgb, var(--color-danger) 55%, var(--color-border)); color: var(--color-danger); }
  .tp-body { background: transparent; color: inherit; font-size: inherit; padding: 0; display: inline-flex; align-items: center; gap: 4px; }
  .tp-body.link { cursor: pointer; }
  .tp-body.link:hover { text-decoration: underline; }
  .tp-proj { color: var(--color-text-muted); }
  .tp-proj::before { content: "· "; }

  /* per-pill action buttons — hidden until the row is hovered (low-noise) */
  .tp-act, .tp-x {
    display: none; align-items: center; justify-content: center;
    width: 14px; height: 14px; flex: none; border-radius: 999px;
    background: transparent; color: var(--color-text-muted); cursor: pointer;
  }
  .row:hover .tp-act, .row:hover .tp-x { display: inline-flex; }
  .tp-act:hover:not(:disabled) { background: var(--color-brand); color: #fff; }
  .tp-x:hover:not(:disabled) { background: var(--color-danger); color: #fff; }
  .tp-act:disabled, .tp-x:disabled { opacity: 0.4; cursor: default; }

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
