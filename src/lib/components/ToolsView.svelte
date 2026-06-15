<script lang="ts">
  /**
   * Tools — the "by tool" axis (the workspace is "by agent"), as a list/detail
   * two-pane mirroring the Agents workspace. The left pane lists each supported
   * AI tool (brand accent, detected state, version, health bar); selecting one
   * opens its console on the right: reveal its agents folder, flip it as a
   * default install target, and run tool-wide actions — Sync to catalog, Track
   * all, Remove all — plus per-agent controls.
   */
  import { onMount } from "svelte";
  import RefreshIcon from "@lucide/svelte/icons/refresh-cw";
  import FolderOpen from "@lucide/svelte/icons/folder-open";
  import TrashIcon from "@lucide/svelte/icons/trash-2";
  import PlusIcon from "@lucide/svelte/icons/plus";
  import DiffIcon from "@lucide/svelte/icons/file-diff";
  import XIcon from "@lucide/svelte/icons/x";
  import AlertTriangle from "@lucide/svelte/icons/triangle-alert";
  import WrenchIcon from "@lucide/svelte/icons/wrench";

  import Switch from "./Switch.svelte";
  import Input from "./Input.svelte";
  import DiffModal from "./DiffModal.svelte";
  import ResizeHandle from "./ResizeHandle.svelte";
  import { install } from "$lib/stores/install.svelte";
  import { corpus } from "$lib/stores/corpus.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import { toolAccent, toolMark } from "$lib/util/toolBadge";
  import type { InstalledAgent, InstallState, Tool, ToolInfo } from "$lib/types";

  // ── List-pane width (resizable, persisted) ──
  const LW_KEY = "agency-agents:tools-list-width";
  const LW_MIN = 240;
  const LW_MAX = 520;
  let listWidth = $state(300);
  function clampLW(w: number): number {
    return Math.min(Math.max(Math.round(w), LW_MIN), LW_MAX);
  }
  function setListWidth(w: number): void {
    listWidth = clampLW(w);
    try {
      localStorage.setItem(LW_KEY, String(listWidth));
    } catch {
      /* ignore */
    }
  }

  onMount(() => {
    corpus.ensureLoaded();
    void install.loadTools();
    void install.loadVersions(); // best-effort; spawns `<bin> --version`
    try {
      const raw = localStorage.getItem(LW_KEY);
      if (raw) {
        const n = Number(raw);
        if (Number.isFinite(n)) listWidth = clampLW(n);
      }
      const lens = localStorage.getItem(TLENS_KEY);
      if (lens === "installed" || lens === "uninstalled" || lens === "all") toolLens = lens;
    } catch {
      /* ignore */
    }
  });

  const tools = $derived(install.tools);

  // ── Tool lens: default to only installed/discovered tools, with a toggle to
  //    reveal the not-installed (supported-but-absent) ones, mirroring the
  //    Agents workspace filter lens. ──
  type ToolLens = "installed" | "uninstalled" | "all";
  const TLENS_KEY = "agency-agents:tools-lens";
  let toolLens = $state<ToolLens>("installed");
  function setToolLens(l: ToolLens): void {
    toolLens = l;
    try {
      localStorage.setItem(TLENS_KEY, l);
    } catch {
      /* ignore */
    }
  }
  /** A tool counts as "installed/discovered" if its config dir is present on
      this machine, or we already have agents deployed in it. */
  function toolPresent(t: ToolInfo): boolean {
    return t.detected || health(t.tool).total > 0;
  }
  const TLENS: { id: ToolLens; label: string }[] = [
    { id: "installed", label: "Installed" },
    { id: "uninstalled", label: "Not installed" },
    { id: "all", label: "All" },
  ];
  function lensMatch(l: ToolLens, t: ToolInfo): boolean {
    if (l === "all") return true;
    return l === "installed" ? toolPresent(t) : !toolPresent(t);
  }
  const visibleTools = $derived(tools.filter((t) => lensMatch(toolLens, t)));

  const STATE_COLOR: Record<InstallState, string> = {
    current: "var(--color-success)",
    outdated: "var(--color-warning)",
    modified: "color-mix(in srgb, var(--color-warning) 55%, var(--color-danger))",
    foreign: "var(--color-brand)",
    removed: "var(--color-danger)",
  };
  const STATE_LABEL: Record<InstallState, string> = {
    current: "In sync",
    outdated: "Outdated",
    modified: "Modified",
    foreign: "Untracked",
    removed: "Missing",
  };
  const DIFFABLE: InstallState[] = ["foreign", "modified", "outdated"];
  const ORDER: InstallState[] = ["current", "outdated", "modified", "foreign", "removed"];

  function rowsFor(toolId: Tool): InstalledAgent[] {
    return install.installed.filter((i) => i.tool === toolId);
  }
  function health(toolId: Tool) {
    const c = { current: 0, outdated: 0, modified: 0, foreign: 0, removed: 0 };
    for (const r of rowsFor(toolId)) c[r.state]++;
    const total = c.current + c.outdated + c.modified + c.foreign + c.removed;
    return { ...c, total };
  }
  // Catalog coverage: how many distinct catalog agents are deployed (present on
  // disk, i.e. not "removed") in a tool, vs the whole catalog. Drives the bar.
  const catalogTotal = $derived(Math.max(corpus.agents.length, 1));
  function installedCount(toolId: Tool): number {
    const s = new Set<string>();
    for (const r of rowsFor(toolId)) if (r.state !== "removed") s.add(r.slug);
    return s.size;
  }

  const emojiBySlug = $derived(new Map(corpus.agents.map((a) => [a.slug, a.emoji] as const)));
  function emoji(slug: string): string {
    return emojiBySlug.get(slug) ?? "🧩";
  }

  // ── Selection (master-detail) ──
  let selectedTool = $state<Tool | null>(null);
  let autoPicked = false;
  $effect(() => {
    if (!autoPicked && tools.length > 0) {
      autoPicked = true;
      selectedTool = [...tools].sort((a, b) => b.installedCount - a.installedCount)[0]?.tool ?? null;
    }
  });
  // Resolve against the VISIBLE (lens-filtered) list, so switching the lens to
  // one that excludes the selected tool closes its detail panel rather than
  // leaving a stale tool shown that isn't in the list.
  const sel = $derived<ToolInfo | null>(visibleTools.find((t) => t.tool === selectedTool) ?? null);
  const selRows = $derived(
    selectedTool
      ? install.installed.filter((i) => i.tool === selectedTool).slice().sort((a, b) => a.name.localeCompare(b.name))
      : [],
  );
  const selHealth = $derived(selectedTool ? health(selectedTool) : null);

  let agentFilter = $state("");
  const selVisible = $derived(
    agentFilter.trim()
      ? selRows.filter((r) => r.name.toLowerCase().includes(agentFilter.trim().toLowerCase()))
      : selRows,
  );

  const selProjects = $derived.by(() => {
    const m = new Map<string, number>();
    for (const r of selRows) if (r.projectPath) m.set(r.projectPath, (m.get(r.projectPath) ?? 0) + 1);
    return [...m.entries()].map(([path, count]) => ({ path, count }));
  });

  // ── Actions ──
  let busy = $state(false);
  let diffTarget = $state<InstalledAgent | null>(null);
  let confirmRemove = $state(false);

  async function rescan() {
    busy = true;
    try {
      await Promise.all([install.loadTools(), install.reconcile(), install.loadVersions()]);
      toast.success("Rescanned tools", `${install.tools.filter((t) => t.detected).length} detected`);
    } finally {
      busy = false;
    }
  }

  function toTarget(i: InstalledAgent) {
    return { slug: i.slug, tool: i.tool, projectPath: i.projectPath };
  }
  const canSync = $derived(selRows.some((r) => r.state !== "current"));
  const canTrack = $derived(selRows.some((r) => r.state === "foreign"));

  async function runToolBulk(action: "update" | "track" | "uninstall", verb: string) {
    let picked = selRows;
    if (action === "update") picked = selRows.filter((r) => r.state !== "current");
    else if (action === "track") picked = selRows.filter((r) => r.state === "foreign");
    const targets = picked.map(toTarget);
    if (targets.length === 0) return;
    busy = true;
    try {
      const { ok, fail } = await install.bulk(action, targets);
      if (fail === 0) toast.success(`${verb} ${ok} agent${ok === 1 ? "" : "s"}`, sel?.label);
      else toast.error(`${verb}: ${ok} ok, ${fail} failed`);
    } finally {
      busy = false;
    }
  }

  async function quick(fn: () => Promise<unknown>, ok: string) {
    try {
      await fn();
      toast.success(ok);
    } catch (e) {
      toast.error("Action failed", String(e));
    }
  }
  async function reveal(path: string | null | undefined) {
    if (!path) return;
    try {
      await install.revealPath(path);
    } catch (e) {
      toast.error("Could not open folder", String(e));
    }
  }
  function homePath(p: string): string {
    return p.replace(/^.*\/Users\/[^/]+/, "~").replace(/^.*\\Users\\[^\\]+/, "~");
  }
</script>

<section class="tw">
  <!-- ── List pane ── -->
  <div class="list-pane" style="width:{listWidth}px">
    <header class="lp-head">
      <div class="seg" role="tablist" aria-label="Filter tools">
        {#each TLENS as f (f.id)}
          <button
            class="seg-btn"
            class:on={toolLens === f.id}
            role="tab"
            aria-selected={toolLens === f.id}
            onclick={() => setToolLens(f.id)}
          >
            {f.label}
          </button>
        {/each}
      </div>
      <button class="ghost icon" disabled={busy} onclick={rescan} title="Re-detect tools, versions + installs" aria-label="Rescan">
        <RefreshIcon size={15} />
      </button>
    </header>
    <ul class="tlist">
      {#if visibleTools.length === 0}
        <li class="tlist-empty">
          {toolLens === "installed"
            ? "No tools detected on this device yet."
            : "Every supported tool is installed."}
        </li>
      {/if}
      {#each visibleTools as t (t.tool)}
        {@const h = health(t.tool)}
        {@const ver = install.versionOf(t.tool)}
        {@const inst = installedCount(t.tool)}
        <li>
          <button class="trow" class:sel={selectedTool === t.tool} class:dim={!t.detected && h.total === 0} onclick={() => (selectedTool = t.tool)}>
            <span class="badge" style="--accent:{toolAccent(t.tool)}">{toolMark(t.label)}</span>
            <span class="trow-id">
              <span class="trow-top">
                <span class="trow-name">{t.label}</span>
                <span class="c-dot" class:on={t.detected} title={t.detected ? "Detected" : "Not detected"}></span>
              </span>
              <span class="hbar" title="{inst} of {catalogTotal} catalog agents installed">
                <span class="hseg" style="flex:{inst};background:var(--color-success)"></span>
                <span class="hseg" style="flex:{Math.max(catalogTotal - inst, 0)}"></span>
              </span>
              <span class="trow-sub">
                {h.total > 0 ? `${h.total} agent${h.total === 1 ? "" : "s"}` : "No agents"}{#if ver} · <span class="trow-ver" title={ver}>{ver}</span>{/if}
              </span>
            </span>
          </button>
        </li>
      {/each}
    </ul>
  </div>

  <div class="tw-resize">
    <ResizeHandle
      width={listWidth}
      min={LW_MIN}
      max={LW_MAX}
      defaultWidth={300}
      direction="right"
      label="Resize tool list"
      onChange={(w) => (listWidth = clampLW(w))}
      onCommit={setListWidth}
    />
  </div>

  <!-- ── Detail pane (console) ── -->
  <div class="detail-pane">
    {#if sel}
      <div class="con">
        <div class="con-head">
          <span class="badge lg" style="--accent:{toolAccent(sel.tool)}">{toolMark(sel.label)}</span>
          <div class="con-id">
            <h2>{sel.label}</h2>
            <span class="con-meta">
              {sel.scope === "user" ? "user-global" : "project-scoped"}
              {#if install.versionOf(sel.tool)}· {install.versionOf(sel.tool)}{/if}
              {#if !sel.detected}· <span class="warn">not detected</span>{/if}
            </span>
          </div>
          <label class="def-target" title="Preselect this tool in the agent “Use with” menu">
            <Switch checked={install.isSelected(sel.tool)} ariaLabel="Default install target" onToggle={() => install.toggleSelected(sel.tool)} />
            <span>Default target</span>
          </label>
          {#if sel.userDest}
            <button class="ghost" onclick={() => reveal(sel.userDest)} title={sel.userDest}>
              <FolderOpen size={15} /><span>Reveal</span>
            </button>
          {/if}
        </div>

        {#if sel.userDest}
          <code class="con-path" title={sel.userDest}>{homePath(sel.userDest)}</code>
        {/if}

        {#if selHealth && selHealth.total > 0}
          <div class="legend">
            {#each ORDER as s (s)}
              {#if selHealth[s] > 0}
                <span class="leg"><span class="dot" style="background:{STATE_COLOR[s]}"></span>{selHealth[s]} {STATE_LABEL[s]}</span>
              {/if}
            {/each}
          </div>
        {/if}

        <div class="actions">
          <button class="act" disabled={busy || !canSync} title={canSync ? "Update every drifted agent to the catalog version (backs up yours)" : "Everything here is in sync"} onclick={() => runToolBulk("update", "Synced")}>
            <RefreshIcon size={14} /> Sync to catalog
          </button>
          <button class="act" disabled={busy || !canTrack} title={canTrack ? "Adopt the untracked agents (no files changed)" : "Nothing untracked here"} onclick={() => runToolBulk("track", "Tracked")}>
            <PlusIcon size={14} /> Track all
          </button>
          <button class="act danger" disabled={busy || selRows.length === 0} onclick={() => (confirmRemove = true)}>
            <TrashIcon size={14} /> Remove all
          </button>
        </div>

        {#if selProjects.length > 0}
          <div class="projects">
            <h3 class="sub">Projects</h3>
            {#each selProjects as p (p.path)}
              <div class="proj">
                <FolderOpen size={14} />
                <span class="proj-name" title={p.path}>{p.path.split("/").pop()}</span>
                <span class="proj-count">{p.count}</span>
                <button class="mini" onclick={() => reveal(p.path)} title="Reveal in file manager"><FolderOpen size={13} /></button>
              </div>
            {/each}
          </div>
        {/if}

        {#if selRows.length === 0}
          <p class="empty">No agents deployed in {sel.label} yet. Open an agent and use the <strong>Use with</strong> switch to deploy it here.</p>
        {:else}
          <div class="list-head">
            <h3 class="sub">{selRows.length} agent{selRows.length === 1 ? "" : "s"}</h3>
            <div class="filter"><Input bind:value={agentFilter} variant="search" placeholder="Filter…" ariaLabel="Filter agents" /></div>
          </div>
          <ul class="agents">
            {#each selVisible as r (r.dest)}
              {@const isBusy = install.busy === `${r.slug}:${r.tool}`}
              <li class="agent">
                <span class="a-emoji" aria-hidden="true">{emoji(r.slug)}</span>
                <span class="a-dot" style="background:{STATE_COLOR[r.state]}" title={STATE_LABEL[r.state]}></span>
                <span class="a-name">{r.name}</span>
                {#if r.projectPath}<span class="a-proj" title={r.projectPath}>{r.projectPath.split("/").pop()}</span>{/if}
                <span class="a-acts">
                  {#if DIFFABLE.includes(r.state)}
                    <button class="mini" title="See what differs" onclick={() => (diffTarget = r)}><DiffIcon size={13} /></button>
                  {/if}
                  {#if r.state === "foreign"}
                    <button class="mini" title="Track" disabled={isBusy} onclick={() => quick(() => install.track(r.slug, r.tool, r.projectPath), `Tracking ${r.name}`)}><PlusIcon size={13} /></button>
                  {/if}
                  {#if r.state !== "current"}
                    <button class="mini" title="Update from catalog" disabled={isBusy} onclick={() => quick(() => install.update(r.slug, r.tool, r.projectPath), `Updated ${r.name}`)}><RefreshIcon size={13} /></button>
                  {/if}
                  <button class="mini danger" title="Remove" disabled={isBusy} onclick={() => quick(() => install.uninstall(r.slug, r.tool, r.projectPath), `Removed ${r.name}`)}><XIcon size={13} /></button>
                </span>
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    {:else}
      <div class="d-empty">
        <WrenchIcon size={40} />
        <p>Select a tool to manage the agents deployed in it.</p>
      </div>
    {/if}
  </div>
</section>

{#if diffTarget}
  <DiffModal slug={diffTarget.slug} tool={diffTarget.tool} projectPath={diffTarget.projectPath} name={diffTarget.name} onClose={() => (diffTarget = null)} />
{/if}

{#if confirmRemove && sel}
  <button class="cd-scrim" aria-label="Cancel" onclick={() => (confirmRemove = false)}></button>
  <div class="cd-box" role="alertdialog" aria-modal="true" aria-label="Confirm remove all">
    <div class="cd-head"><AlertTriangle size={20} /><h2>Remove all from {sel.label}?</h2></div>
    <p class="cd-body">This <strong>deletes {selRows.length} agent file{selRows.length === 1 ? "" : "s"} from disk</strong> — including any installed outside this app. Modified files are backed up before removal; catalog-identical files can be installed again.</p>
    <div class="cd-actions">
      <button class="cd-cancel" onclick={() => (confirmRemove = false)}>Cancel</button>
      <button class="cd-delete" disabled={busy} onclick={() => { confirmRemove = false; runToolBulk("uninstall", "Removed"); }}>
        <TrashIcon size={14} /> Remove {selRows.length}
      </button>
    </div>
  </div>
{/if}

<style>
  .tw { display: flex; height: 100%; min-height: 0; }

  /* ── List pane ── */
  .list-pane { flex: none; display: flex; flex-direction: column; min-height: 0; min-width: 0; }
  .lp-head {
    flex: none; display: flex; align-items: center; justify-content: space-between;
    gap: var(--space-2);
    padding: var(--space-3);
    border-bottom: 1px solid var(--color-border);
  }
  /* ── Tool filter lens (mirrors the Agents workspace) ── */
  .seg {
    display: flex; align-items: center; gap: 2px; flex-wrap: wrap; min-width: 0;
    padding: 2px; background: var(--color-surface-sunken);
    border: 1px solid var(--color-border); border-radius: var(--radius-md);
  }
  .seg-btn {
    display: inline-flex; align-items: center; gap: 6px;
    height: 26px; padding: 0 10px; border-radius: var(--radius-sm);
    background: transparent; color: var(--color-text-secondary);
    font-size: var(--text-body-sm); cursor: pointer; white-space: nowrap;
  }
  .seg-btn:hover { color: var(--color-text-primary); }
  .seg-btn.on { background: var(--color-surface-raised); color: var(--color-text-primary); box-shadow: var(--shadow-sm, 0 1px 2px rgba(0,0,0,0.08)); }
  .tlist-empty {
    padding: var(--space-4) var(--space-3);
    font-size: var(--text-body-sm); color: var(--color-text-muted); text-align: center;
  }
  .ghost {
    display: inline-flex; align-items: center; gap: 6px;
    height: 30px; padding: 0 var(--space-3);
    border: 1px solid var(--color-border); border-radius: var(--radius-md);
    background: transparent; color: var(--color-text-secondary);
    font-size: var(--text-body-sm); cursor: pointer; flex: none;
  }
  .ghost:hover:not(:disabled) { color: var(--color-text-primary); background: var(--color-surface-sunken); }
  .ghost:disabled { opacity: 0.6; cursor: default; }
  .ghost.icon { padding: 0; width: 30px; justify-content: center; }

  .tlist { flex: 1; overflow-y: auto; min-height: 0; padding: var(--space-2); display: flex; flex-direction: column; gap: 2px; }
  .trow {
    display: flex; align-items: center; gap: var(--space-2); width: 100%;
    padding: var(--space-2); border-radius: var(--radius-md);
    background: transparent; cursor: pointer; text-align: left;
  }
  .trow:hover { background: var(--color-surface-sunken); }
  .trow.sel { background: var(--color-brand-subtle); }
  .trow.dim { opacity: 0.55; }
  .trow-id { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 3px; }
  .trow-top { display: flex; align-items: center; gap: var(--space-2); }
  .trow-name { flex: 1; min-width: 0; font-weight: var(--fw-medium); color: var(--color-text-primary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .c-dot { width: 8px; height: 8px; border-radius: 999px; background: var(--color-text-muted); opacity: 0.4; flex: none; }
  .c-dot.on { background: var(--color-success); opacity: 1; }
  .trow-sub { font-size: var(--text-caption); color: var(--color-text-muted); display: flex; gap: 4px; min-width: 0; }
  .trow-ver { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .hbar { display: flex; height: 5px; border-radius: 999px; overflow: hidden; background: var(--color-surface-sunken); }
  .hseg { display: block; }

  /* ── badges ── */
  .badge {
    display: inline-flex; align-items: center; justify-content: center;
    width: 32px; height: 32px; flex: none; border-radius: 9px;
    background: linear-gradient(145deg, var(--accent), color-mix(in srgb, var(--accent) 70%, black));
    color: #fff; font-weight: var(--fw-bold); font-size: 15px;
    box-shadow: inset 0 1px 0 color-mix(in srgb, white 25%, transparent);
  }
  .badge.lg { width: 44px; height: 44px; border-radius: 12px; font-size: 20px; }

  /* ── resize ── */
  .tw-resize { display: flex; flex: none; }

  /* ── Detail pane (console) ── */
  .detail-pane { flex: 1; min-width: 0; overflow-y: auto; border-left: 1px solid var(--color-border); }
  .con { padding: var(--space-4); display: flex; flex-direction: column; gap: var(--space-3); }
  .con-head { display: flex; align-items: center; gap: var(--space-3); flex-wrap: wrap; }
  .con-id { flex: 1; min-width: 0; }
  .con-id h2 { font-size: var(--text-h2); font-weight: var(--fw-semibold); color: var(--color-text-primary); }
  .con-meta { font-size: var(--text-caption); color: var(--color-text-muted); }
  .con-meta .warn { color: var(--color-warning); }
  .def-target { display: inline-flex; align-items: center; gap: var(--space-2); font-size: var(--text-body-sm); color: var(--color-text-secondary); cursor: pointer; }
  .con-path {
    font-family: var(--font-mono, monospace); font-size: var(--text-caption);
    color: var(--color-text-secondary); background: var(--color-surface-sunken);
    padding: 3px 8px; border-radius: var(--radius-sm); width: max-content; max-width: 100%;
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }

  .legend { display: flex; flex-wrap: wrap; gap: var(--space-3); }
  .leg { display: inline-flex; align-items: center; gap: 5px; font-size: var(--text-caption); color: var(--color-text-secondary); }
  .leg .dot, .a-dot { width: 8px; height: 8px; border-radius: 999px; flex: none; }

  .actions { display: flex; flex-wrap: wrap; gap: var(--space-2); }
  .act {
    display: inline-flex; align-items: center; gap: 6px;
    height: 32px; padding: 0 var(--space-3); border-radius: var(--radius-md);
    border: 1px solid var(--color-border); background: var(--color-surface-sunken);
    color: var(--color-text-primary); font-size: var(--text-body-sm); cursor: pointer;
  }
  .act:hover:not(:disabled) { border-color: var(--color-brand); }
  .act:disabled { opacity: 0.45; cursor: default; }
  .act.danger { color: var(--color-danger); }
  .act.danger:hover:not(:disabled) { background: color-mix(in srgb, var(--color-danger) 12%, transparent); border-color: var(--color-danger); }

  .projects { display: flex; flex-direction: column; gap: 2px; }
  .sub { font-size: var(--text-caption); font-weight: var(--fw-semibold); color: var(--color-text-secondary); text-transform: uppercase; letter-spacing: 0.04em; }
  .proj { display: flex; align-items: center; gap: var(--space-2); padding: 4px var(--space-2); color: var(--color-text-secondary); font-size: var(--text-body-sm); }
  .proj-name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .proj-count { font-size: var(--text-caption); color: var(--color-text-muted); }

  .empty { font-size: var(--text-body-sm); color: var(--color-text-muted); line-height: var(--lh-normal); }
  .list-head { display: flex; align-items: center; justify-content: space-between; gap: var(--space-3); }
  .filter { width: 180px; }
  .filter :global(.wrap) { width: 100%; }

  .agents { display: flex; flex-direction: column; gap: 1px; }
  .agent { display: flex; align-items: center; gap: var(--space-2); padding: 5px var(--space-2); border-radius: var(--radius-sm); }
  .agent:hover { background: var(--color-surface-sunken); }
  .a-emoji { font-size: 15px; line-height: 1; flex: none; }
  .a-name { flex: 1; min-width: 0; font-size: var(--text-body-sm); color: var(--color-text-primary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .a-proj { font-size: var(--text-caption); color: var(--color-text-muted); }
  .a-acts { display: inline-flex; align-items: center; gap: 2px; flex: none; }
  .mini {
    display: inline-flex; align-items: center; justify-content: center;
    width: 24px; height: 24px; border-radius: var(--radius-sm);
    background: transparent; color: var(--color-text-muted); cursor: pointer;
  }
  .agent:hover .mini, .proj .mini { color: var(--color-text-secondary); }
  .mini:hover:not(:disabled) { background: var(--color-surface); color: var(--color-text-primary); }
  .mini.danger:hover:not(:disabled) { background: var(--color-danger); color: #fff; }
  .mini:disabled { opacity: 0.4; cursor: default; }

  .d-empty { height: 100%; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: var(--space-3); color: var(--color-text-muted); }
  .d-empty p { font-size: var(--text-body-sm); }

  /* confirm */
  .cd-scrim { position: fixed; inset: 36px 0 0 0; z-index: 92; border: 0; cursor: default; background: color-mix(in srgb, var(--color-bg) 60%, transparent); backdrop-filter: blur(4px); }
  .cd-box {
    position: fixed; z-index: 93; top: 50%; left: 50%; transform: translate(-50%, -50%);
    width: min(440px, 92vw); padding: var(--space-5);
    background: var(--color-surface-raised); border: 1px solid var(--color-border);
    border-radius: var(--radius-lg); box-shadow: var(--shadow-lg);
    display: flex; flex-direction: column; gap: var(--space-3);
  }
  .cd-head { display: flex; align-items: center; gap: var(--space-2); color: var(--color-danger); }
  .cd-head h2 { font-size: var(--text-h2); font-weight: var(--fw-semibold); color: var(--color-text-primary); }
  .cd-body { font-size: var(--text-body-sm); color: var(--color-text-secondary); line-height: var(--lh-normal); }
  .cd-actions { display: flex; justify-content: flex-end; gap: var(--space-2); }
  .cd-cancel { height: 32px; padding: 0 var(--space-4); border-radius: var(--radius-md); border: 1px solid var(--color-border); background: transparent; color: var(--color-text-secondary); font-size: var(--text-body-sm); cursor: pointer; }
  .cd-cancel:hover { color: var(--color-text-primary); background: var(--color-surface-sunken); }
  .cd-delete { display: inline-flex; align-items: center; gap: 6px; height: 32px; padding: 0 var(--space-4); border-radius: var(--radius-md); border: 1px solid var(--color-danger); background: var(--color-danger); color: #fff; font-size: var(--text-body-sm); font-weight: var(--fw-medium); cursor: pointer; }
  .cd-delete:hover:not(:disabled) { filter: brightness(1.08); }
  .cd-delete:disabled { opacity: 0.5; cursor: default; }
</style>
