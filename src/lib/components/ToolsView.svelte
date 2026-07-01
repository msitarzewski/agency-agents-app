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
  import ChevronDown from "@lucide/svelte/icons/chevron-down";

  import Switch from "./Switch.svelte";
  import Input from "./Input.svelte";
  import DiffModal from "./DiffModal.svelte";
  import ResizeHandle from "./ResizeHandle.svelte";
  import { install } from "$lib/stores/install.svelte";
  import { corpus } from "$lib/stores/corpus.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { toolAccent, toolMark, toolIcon } from "$lib/util/toolBadge";
  import { TOOLS, isInstallable } from "$lib/data/toolRegistry";
  import { open as openFolderDialog } from "@tauri-apps/plugin-dialog";
  import { settingsGet, settingsSet } from "$lib/api";
  import { resolveCategoryIcon } from "$lib/util/categoryIcon";
  import { i18n } from "$lib/stores/i18n.svelte";
  import type { MessageKey } from "$lib/i18n/messages";
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

  // The list is the WHOLE registry (all 12 tools), overlaid with backend
  // detection/scope for the wired ones. Non-wired tools (recognized upstream but
  // no native renderer yet) show as a dimmed "not yet installable" row.
  type ToolRow = ToolInfo & { wired: boolean };
  const tools = $derived.by<ToolRow[]>(() =>
    TOOLS.map((m) => {
      const be = install.tools.find((it) => it.tool === m.id);
      return {
        tool: m.id,
        label: m.label,
        detected: be?.detected ?? false,
        scope: be?.scope ?? (m.scope?.user ? "user" : "project"),
        userDest: be?.userDest ?? null,
        installedCount: be?.installedCount ?? 0,
        customPath: be?.customPath ?? null,
        wired: isInstallable(m),
      };
    }),
  );

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
  const TLENS: { id: ToolLens; key: MessageKey }[] = [
    { id: "installed", key: "tools.lens.installed" },
    { id: "uninstalled", key: "tools.lens.uninstalled" },
    { id: "all", key: "tools.lens.all" },
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
  const STATE_LABEL: Record<InstallState, MessageKey> = {
    current: "state.current",
    outdated: "state.outdated",
    modified: "state.modified",
    foreign: "state.foreign",
    removed: "state.removed",
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
  // A Dashboard "Coverage by tool" click sets ui.toolsSelected — honor it so the
  // console opens on that tool (overriding the auto-pick).
  $effect(() => {
    if (ui.toolsSelected) {
      selectedTool = ui.toolsSelected;
      autoPicked = true;
    }
  });
  // Resolve against the VISIBLE (lens-filtered) list, so switching the lens to
  // one that excludes the selected tool closes its detail panel rather than
  // leaving a stale tool shown that isn't in the list.
  const sel = $derived<ToolRow | null>(visibleTools.find((t) => t.tool === selectedTool) ?? null);
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

  // ── Group the agent list by division (collapsible disclosures), mirroring the
  //    Agents workspace + Teams. Agents missing from the corpus fall into "Other".
  const OTHER = "__other";
  const selGroups = $derived.by(() => {
    const divOf = new Map(corpus.agents.map((a) => [a.slug, a.category]));
    const m = new Map<string, InstalledAgent[]>();
    for (const r of selVisible) {
      const div = divOf.get(r.slug) ?? OTHER;
      const arr = m.get(div);
      if (arr) arr.push(r);
      else m.set(div, [r]);
    }
    const out = [...m.entries()].map(([slug, rows]) => ({
      slug,
      label: slug === OTHER ? i18n.t("common.other") : corpus.labelOf(slug),
      color: slug === OTHER ? "#94A3B8" : corpus.colorOf(slug),
      icon: slug === OTHER ? "HelpCircle" : corpus.iconOf(slug),
      rows,
    }));
    out.sort((a, b) => (a.slug === OTHER ? 1 : b.slug === OTHER ? -1 : a.label.localeCompare(b.label)));
    return out;
  });

  // Every division present in the tool (over the UNFILTERED rows, so this is
  // stable while you type in the agent filter).
  const selDivisionSlugs = $derived.by(() => {
    const divOf = new Map(corpus.agents.map((a) => [a.slug, a.category]));
    const s = new Set<string>();
    for (const r of selRows) s.add(divOf.get(r.slug) ?? OTHER);
    return s;
  });

  let collapsed = $state<Set<string>>(new Set());
  function toggleGroup(slug: string) {
    const next = new Set(collapsed);
    if (next.has(slug)) next.delete(slug);
    else next.add(slug);
    collapsed = next;
  }
  const allCollapsed = $derived(selGroups.length > 0 && selGroups.every((g) => collapsed.has(g.slug)));
  function toggleAllGroups() {
    collapsed = allCollapsed ? new Set() : new Set(selGroups.map((g) => g.slug));
  }
  // Default to collapsed: initialize the collapse set to every division the
  // first time we land on a tool (guarded so installs/removes don't re-collapse
  // a tool you're actively managing). A search auto-expands (see isOpen below).
  let collapseInitFor: Tool | null = null;
  $effect(() => {
    const t = selectedTool;
    // Wait for the division set to populate (cold corpus load) before seeding,
    // so groups stay collapsed by default rather than seeding from an empty set.
    if (t === collapseInitFor || selDivisionSlugs.size === 0) return;
    collapseInitFor = t;
    collapsed = new Set(selDivisionSlugs);
  });

  // ── Actions ──
  let busy = $state(false);
  let diffTarget = $state<InstalledAgent | null>(null);
  let confirmRemove = $state(false);

  async function rescan() {
    busy = true;
    try {
      await Promise.all([install.loadTools(), install.reconcile(), install.loadVersions()]);
      toast.success(i18n.t("tools.rescanned"), i18n.t("common.detectedCount", { count: install.tools.filter((t) => t.detected).length }));
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
      if (fail === 0) toast.success(i18n.t("tools.bulkSuccess", { verb, count: ok }), sel?.label);
      else toast.error(i18n.t("tools.bulkError", { verb, ok, fail }));
    } finally {
      busy = false;
    }
  }

  async function quick(fn: () => Promise<unknown>, ok: string) {
    try {
      await fn();
      toast.success(ok);
    } catch (e) {
      toast.error(i18n.t("common.actionFailed"), String(e));
    }
  }
  async function reveal(path: string | null | undefined) {
    if (!path) return;
    try {
      await install.revealPath(path);
    } catch (e) {
      toast.error(i18n.t("common.couldNotOpenFolder"), String(e));
    }
  }

  // ── Per-tool custom install location (e.g. a WSL home from the Windows app) ──
  let editingLocation = $state(false);
  let locationBusy = $state(false);
  async function saveToolPath(tool: Tool, path: string | null): Promise<void> {
    locationBusy = true;
    try {
      const s = await settingsGet();
      const next = { ...(s.toolPaths ?? {}) };
      if (path) next[tool] = path;
      else delete next[tool];
      await settingsSet({ ...s, toolPaths: next });
      await install.loadTools(); // re-detect against the new base
    } catch (e) {
      toast.error("Could not save install location", String(e));
    } finally {
      locationBusy = false;
    }
  }
  async function chooseToolPath(tool: Tool): Promise<void> {
    try {
      const picked = await openFolderDialog({ directory: true, multiple: false });
      if (typeof picked === "string") await saveToolPath(tool, picked);
    } catch (e) {
      toast.error("Could not open the folder picker", String(e));
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
      <div class="seg" role="tablist" aria-label={i18n.t("tools.filterTools")}>
        {#each TLENS as f (f.id)}
          <button
            class="seg-btn"
            class:on={toolLens === f.id}
            role="tab"
            aria-selected={toolLens === f.id}
            onclick={() => setToolLens(f.id)}
          >
            {i18n.t(f.key)}
          </button>
        {/each}
      </div>
      <button class="ghost icon" disabled={busy} onclick={rescan} title={i18n.t("tools.rescanTitle")} aria-label={i18n.t("tools.rescanTitle")}>
        <RefreshIcon size={15} />
      </button>
    </header>
    <ul class="tlist">
      {#if visibleTools.length === 0}
        <li class="tlist-empty">
          {toolLens === "installed"
            ? i18n.t("tools.emptyInstalled")
            : i18n.t("tools.emptyUninstalled")}
        </li>
      {/if}
      {#each visibleTools as t (t.tool)}
        {@const h = health(t.tool)}
        {@const ver = install.versionOf(t.tool)}
        {@const inst = installedCount(t.tool)}
        <li>
          <button class="trow" class:sel={selectedTool === t.tool} class:dim={!t.detected && h.total === 0} onclick={() => { selectedTool = t.tool; ui.toolsSelected = t.tool; }}>
            <span class="badge" style="--accent:{toolAccent(t.tool)}">{#if toolIcon(t.tool)}<span class="glyph">{@html toolIcon(t.tool)}</span>{:else}{toolMark(t.label)}{/if}</span>
            <span class="trow-id">
              <span class="trow-top">
                <span class="trow-name">{t.label}</span>
                {#if t.wired}<span class="c-dot" class:on={t.detected} title={t.detected ? i18n.t("common.detected") : i18n.t("common.notDetected")}></span>{/if}
              </span>
              {#if t.wired}
                <span class="hbar" title={i18n.t("tools.hbarTitle", { installed: inst, total: catalogTotal })}>
                  <span class="hseg" style="flex:{inst};background:var(--color-success)"></span>
                  <span class="hseg" style="flex:{Math.max(catalogTotal - inst, 0)}"></span>
                </span>
                <span class="trow-sub">
                  {h.total > 0 ? i18n.count(h.total, "common.agent.one", "common.agent.many") : i18n.t("common.noAgents")}{#if ver} · <span class="trow-ver" title={ver}>{ver}</span>{/if}
                </span>
              {:else}
                <span class="trow-sub recognized">{i18n.t("tools.recognizedSub")}</span>
              {/if}
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
        label={i18n.t("common.resizeToolList")}
      onChange={(w) => (listWidth = clampLW(w))}
      onCommit={setListWidth}
    />
  </div>

  <!-- ── Detail pane (console) ── -->
  <div class="detail-pane">
    {#if sel}
      <div class="con">
        <div class="con-head">
          <span class="badge lg" style="--accent:{toolAccent(sel.tool)}">{#if toolIcon(sel.tool)}<span class="glyph">{@html toolIcon(sel.tool)}</span>{:else}{toolMark(sel.label)}{/if}</span>
          <div class="con-id">
            <h2>{sel.label}</h2>
            <span class="con-meta">
              {sel.scope === "user" ? i18n.t("tools.userGlobal") : i18n.t("tools.projectScoped")}
              {#if install.versionOf(sel.tool)}· {install.versionOf(sel.tool)}{/if}
              {#if !sel.detected}· <span class="warn">{i18n.t("common.notDetected")}</span>{/if}
              {#if sel.customPath}· <span class="cust" title={sel.customPath}>{i18n.t("tools.customLocation")}</span>{/if}
            </span>
          </div>
          {#if sel.wired}
            <label class="def-target" title={i18n.t("tools.defaultTargetTitle")}>
              <Switch checked={install.isSelected(sel.tool)} ariaLabel={i18n.t("tools.defaultTargetAria")} onToggle={() => install.toggleSelected(sel.tool)} />
              <span>{i18n.t("tools.defaultTarget")}</span>
            </label>
          {/if}
          {#if sel.userDest}
            <button class="ghost" onclick={() => reveal(sel.userDest)} title={sel.userDest}>
              <FolderOpen size={15} /><span>{i18n.t("common.reveal")}</span>
            </button>
          {/if}
          {#if sel.wired}
            <button class="ghost" class:on={editingLocation} onclick={() => (editingLocation = !editingLocation)} title={i18n.t("tools.locationTitle", { tool: sel.label })}>
              <WrenchIcon size={15} /><span>{i18n.t("tools.location")}</span>
            </button>
          {/if}
        </div>

        {#if editingLocation && sel.wired}
          <div class="loc-editor">
            <div class="loc-row">
              <span class="loc-label">{i18n.t("tools.installLocation")}</span>
              <code class="con-path" title={sel.customPath ?? i18n.t("tools.locationDefaultTitle")}>{sel.customPath ? sel.customPath : i18n.t("tools.locationDefault")}</code>
            </div>
            <p class="loc-hint">{i18n.t("tools.locationHint", { tool: sel.label, wsl: "\\\\wsl.localhost\\…" })}</p>
            <div class="loc-actions">
              <button class="act" disabled={locationBusy} onclick={() => chooseToolPath(sel.tool)}>{i18n.t("tools.locationChoose")}</button>
              {#if sel.customPath}
                <button class="act subtle" disabled={locationBusy} onclick={() => saveToolPath(sel.tool, null)}>{i18n.t("tools.locationReset")}</button>
              {/if}
            </div>
          </div>
        {/if}

        {#if sel.wired}
        {#if sel.userDest}
          <code class="con-path" title={sel.userDest}>{homePath(sel.userDest)}</code>
        {/if}

        {#if selHealth && selHealth.total > 0}
          <div class="legend">
            {#each ORDER as s (s)}
              {#if selHealth[s] > 0}
                <span class="leg"><span class="dot" style="background:{STATE_COLOR[s]}"></span>{selHealth[s]} {i18n.t(STATE_LABEL[s])}</span>
              {/if}
            {/each}
          </div>
        {/if}

        <div class="actions">
          <button class="act" disabled={busy || !canSync} title={canSync ? i18n.t("tools.canSyncTitle") : i18n.t("tools.cannotSyncTitle")} onclick={() => runToolBulk("update", i18n.t("tools.syncedVerb"))}>
            <RefreshIcon size={14} /> {i18n.t("tools.syncToCatalog")}
          </button>
          <button class="act" disabled={busy || !canTrack} title={canTrack ? i18n.t("tools.canTrackTitle") : i18n.t("tools.cannotTrackTitle")} onclick={() => runToolBulk("track", i18n.t("tools.trackedVerb"))}>
            <PlusIcon size={14} /> {i18n.t("tools.trackAll")}
          </button>
          <button class="act danger" disabled={busy || selRows.length === 0} onclick={() => (confirmRemove = true)}>
            <TrashIcon size={14} /> {i18n.t("tools.removeAll")}
          </button>
        </div>

        {#if selProjects.length > 0}
          <div class="projects">
            <h3 class="sub">{i18n.t("tools.projects")}</h3>
            {#each selProjects as p (p.path)}
              <div class="proj">
                <FolderOpen size={14} />
                <span class="proj-name" title={p.path}>{p.path.split("/").pop()}</span>
                <span class="proj-count">{p.count}</span>
                <button class="mini" onclick={() => reveal(p.path)} title={i18n.t("tools.revealInFileManager")}><FolderOpen size={13} /></button>
              </div>
            {/each}
          </div>
        {/if}

        {#if selRows.length === 0}
          <p class="empty">{i18n.t("tools.noAgentsDeployed", { tool: sel.label })}</p>
        {:else}
          <div class="list-head">
            <h3 class="sub">
              {i18n.count(selRows.length, "common.agent.one", "common.agent.many")}{#if selGroups.length > 1} · {i18n.count(selGroups.length, "common.division.one", "common.division.many")}{/if}
            </h3>
            {#if selGroups.length > 1}
              <button class="ghost sm" onclick={toggleAllGroups}>{i18n.t(allCollapsed ? "common.expandAll" : "common.collapseAll")}</button>
            {/if}
            <div class="filter"><Input bind:value={agentFilter} variant="search" placeholder={i18n.t("common.filterEllipsis")} ariaLabel={i18n.t("common.filterAgents")} /></div>
          </div>
          {#if selVisible.length === 0}
            <p class="empty">{i18n.t("tools.noAgentsMatch", { query: agentFilter.trim() })}</p>
          {:else}
            <div class="groups">
              {#each selGroups as g (g.slug)}
                {@const Icon = resolveCategoryIcon(g.icon)}
                {@const isOpen = agentFilter.trim() ? true : !collapsed.has(g.slug)}
                <section class="grp">
                  <button class="grp-head" onclick={() => toggleGroup(g.slug)} aria-expanded={isOpen}>
                    <ChevronDown size={15} class={isOpen ? "tv-chev open" : "tv-chev"} />
                    <span class="grp-ic" style="color:{g.color}"><Icon size={15} /></span>
                    <span class="grp-label">{g.label}</span>
                    <span class="grp-count">{g.rows.length}</span>
                  </button>
                  {#if isOpen}
                    <ul class="agents">
                      {#each g.rows as r (r.dest)}
                        {@const isBusy = install.busy === `${r.slug}:${r.tool}`}
                        <li class="agent">
                          <span class="a-emoji" aria-hidden="true">{emoji(r.slug)}</span>
                          <span class="a-dot" style="background:{STATE_COLOR[r.state]}" title={i18n.t(STATE_LABEL[r.state])}></span>
                          <span class="a-name">{r.name}</span>
                          {#if r.projectPath}<span class="a-proj" title={r.projectPath}>{r.projectPath.split("/").pop()}</span>{/if}
                          <span class="a-acts">
                            {#if DIFFABLE.includes(r.state)}
                              <button class="mini" title={i18n.t("tools.diffTitle")} onclick={() => (diffTarget = r)}><DiffIcon size={13} /></button>
                            {/if}
                            {#if r.state === "foreign"}
                              <button class="mini" title={i18n.t("tools.trackTitle")} disabled={isBusy} onclick={() => quick(() => install.track(r.slug, r.tool, r.projectPath), i18n.t("tools.quickTracking", { name: r.name }))}><PlusIcon size={13} /></button>
                            {/if}
                            {#if r.state !== "current"}
                              <button class="mini" title={i18n.t("tools.updateTitle")} disabled={isBusy} onclick={() => quick(() => install.update(r.slug, r.tool, r.projectPath), i18n.t("tools.quickUpdated", { name: r.name }))}><RefreshIcon size={13} /></button>
                            {/if}
                            <button class="mini danger" title={i18n.t("tools.removeTitle")} disabled={isBusy} onclick={() => quick(() => install.uninstall(r.slug, r.tool, r.projectPath), i18n.t("tools.quickRemoved", { name: r.name }))}><XIcon size={13} /></button>
                          </span>
                        </li>
                      {/each}
                    </ul>
                  {/if}
                </section>
              {/each}
            </div>
          {/if}
        {/if}
        {:else}
          <div class="recognized-detail">
            <span class="rd-badge">{i18n.t("tools.recognized")}</span>
            <p>{i18n.t("tools.recognizedBody")}</p>
          </div>
        {/if}
      </div>
    {:else}
      <div class="d-empty">
        <WrenchIcon size={40} />
        <p>{i18n.t("tools.selectTool")}</p>
      </div>
    {/if}
  </div>
</section>

{#if diffTarget}
  <DiffModal slug={diffTarget.slug} tool={diffTarget.tool} projectPath={diffTarget.projectPath} name={diffTarget.name} onClose={() => (diffTarget = null)} />
{/if}

{#if confirmRemove && sel}
  <button class="cd-scrim" aria-label={i18n.t("common.cancel")} onclick={() => (confirmRemove = false)}></button>
  <div class="cd-box" role="alertdialog" aria-modal="true" aria-label={i18n.t("tools.removeAllTitle", { tool: sel.label })}>
    <div class="cd-head"><AlertTriangle size={20} /><h2>{i18n.t("tools.removeAllTitle", { tool: sel.label })}</h2></div>
    <p class="cd-body">{i18n.t("tools.removeAllBody", { count: selRows.length })}</p>
    <div class="cd-actions">
      <button class="cd-cancel" onclick={() => (confirmRemove = false)}>{i18n.t("common.cancel")}</button>
      <button class="cd-delete" disabled={busy} onclick={() => { confirmRemove = false; runToolBulk("uninstall", i18n.t("tools.removedVerb")); }}>
        <TrashIcon size={14} /> {i18n.t("common.remove")} {selRows.length}
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
  .trow-sub.recognized { font-style: italic; opacity: 0.85; }
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
  .badge .glyph { display: inline-flex; align-items: center; justify-content: center; }
  .badge .glyph :global(svg) { width: 1em; height: 1em; }

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
  .con-meta .cust { color: var(--color-brand); }
  .ghost.on { color: var(--color-brand); background: var(--color-surface-sunken); }

  .loc-editor {
    display: flex; flex-direction: column; gap: var(--space-2);
    padding: var(--space-3); border: 1px solid var(--color-border);
    border-radius: var(--radius-md); background: var(--color-surface-sunken);
  }
  .loc-row { display: flex; align-items: center; gap: var(--space-2); flex-wrap: wrap; }
  .loc-label { font-size: var(--text-body-sm); font-weight: var(--fw-semibold); color: var(--color-text-secondary); }
  .loc-hint { font-size: var(--text-caption); color: var(--color-text-muted); line-height: var(--lh-normal); }
  .loc-hint code { font-family: var(--font-mono, monospace); }
  .loc-actions { display: flex; gap: var(--space-2); }
  .act.subtle { background: transparent; color: var(--color-text-secondary); }
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
  .list-head { display: flex; align-items: center; gap: var(--space-3); }
  .list-head .sub { flex: 1; min-width: 0; }
  .ghost.sm { height: 28px; }
  .filter { width: 180px; }
  .filter :global(.wrap) { width: 100%; }

  /* ── Division groups (collapsible) ── */
  .groups { display: flex; flex-direction: column; gap: 2px; }
  .grp { display: flex; flex-direction: column; }
  .grp-head {
    display: flex; align-items: center; gap: var(--space-2);
    width: 100%; padding: var(--space-2);
    background: transparent; cursor: pointer; text-align: left; border-radius: var(--radius-sm);
  }
  .grp-head:hover { background: var(--color-surface-sunken); }
  :global(.tv-chev) { color: var(--color-text-muted); transition: transform var(--motion-duration-fast, 120ms) ease; transform: rotate(-90deg); flex: none; }
  :global(.tv-chev.open) { transform: rotate(0deg); }
  .grp-ic { flex: none; display: inline-flex; }
  .grp-label { flex: 1; min-width: 0; font-weight: var(--fw-semibold); color: var(--color-text-primary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .grp-count { flex: none; min-width: 20px; text-align: center; font-size: var(--text-caption); color: var(--color-text-muted); font-variant-numeric: tabular-nums; background: var(--color-surface-sunken); border-radius: var(--radius-full); padding: 1px 7px; }

  .agents { display: flex; flex-direction: column; gap: 1px; padding-left: var(--space-4); }
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

  .recognized-detail { display: flex; flex-direction: column; align-items: flex-start; gap: var(--space-2); padding: var(--space-2) 0; }
  .rd-badge { font-size: var(--text-caption); font-weight: var(--fw-semibold); text-transform: uppercase; letter-spacing: 0.04em; color: var(--color-text-secondary); background: var(--color-surface-sunken); border-radius: var(--radius-full); padding: 2px 10px; }
  .recognized-detail p { font-size: var(--text-body-sm); color: var(--color-text-secondary); line-height: var(--lh-normal); max-width: 52ch; }

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
