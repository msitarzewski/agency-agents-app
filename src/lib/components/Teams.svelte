<script lang="ts">
  /**
   * Teams — your roster of agents, the way you actually think about them.
   *
   * Two tabs:
   *  • "Your team" — what you have installed right now, grouped by division
   *    (collapsible). Save it as a named team, export it as an Agentfile, or
   *    restore one.
   *  • "Team presets" — app-bundled curated squads (PRESET_TEAMS) plus your own
   *    saved teams. Deploy any of them as a unit via the tri-state DeployModal.
   *
   * "Your team" reads the live install ledger (reconciled in +layout). Saved
   * teams live in the teams store (localStorage). Presets are bundled data.
   */
  import { onMount } from "svelte";
  import EmptyState from "./EmptyState.svelte";
  import Pill from "./Pill.svelte";
  import Modal from "./Modal.svelte";
  import Button from "./Button.svelte";
  import Input from "./Input.svelte";
  import InstallModal from "./InstallModal.svelte";
  import UploadIcon from "@lucide/svelte/icons/upload";
  import DownloadIcon from "@lucide/svelte/icons/download";
  import ArchiveIcon from "@lucide/svelte/icons/archive";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import SaveIcon from "@lucide/svelte/icons/bookmark-plus";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import UsersIcon from "@lucide/svelte/icons/users";
  import RocketIcon from "@lucide/svelte/icons/rocket";

  import StarterPrompt from "./StarterPrompt.svelte";
  import { install } from "$lib/stores/install.svelte";
  import { corpus } from "$lib/stores/corpus.svelte";
  import { teams, type SavedTeam } from "$lib/stores/teams.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { i18n } from "$lib/i18n.svelte";
  import { resolveCategoryIcon } from "$lib/util/categoryIcon";
  import { PRESET_TEAMS, presetTeamText } from "$lib/data/presetTeams";
  import { teamExamples } from "$lib/data/playbook";
  import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
  import type { InstalledAgent, Agent } from "$lib/types";
  import type { Component } from "svelte";

  onMount(() => {
    corpus.ensureLoaded();
    teams.hydrate();
  });

  let tab = $state<"current" | "presets">("current");
  let busy = $state(false);

  // ── "Your team" = what WE installed (foreign isn't part of it until tracked). ──
  const managed = $derived(install.installed.filter((i) => i.state !== "foreign"));
  const managedSlugs = $derived([...new Set(managed.map((m) => m.slug))]);

  // Group the loadout by division (collapsible). Agents missing from the current
  // corpus (e.g. removed upstream) fall into "Other".
  const OTHER = "__other";
  const groups = $derived.by(() => {
    const divOf = new Map(corpus.agents.map((a) => [a.slug, a.category]));
    const m = new Map<string, InstalledAgent[]>();
    for (const r of managed) {
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
      rows: rows.slice().sort((a, b) => a.name.localeCompare(b.name)),
    }));
    out.sort((a, b) => (a.slug === OTHER ? 1 : b.slug === OTHER ? -1 : a.label.localeCompare(b.label)));
    return out;
  });

  let collapsed = $state<Set<string>>(new Set());
  function toggle(slug: string) {
    const next = new Set(collapsed);
    if (next.has(slug)) next.delete(slug);
    else next.add(slug);
    collapsed = next;
  }
  // Division groups are CLOSED by default — seed the collapse set with every
  // division once the roster first loads.
  let collapseSeeded = false;
  $effect(() => {
    if (collapseSeeded || groups.length === 0) return;
    collapseSeeded = true;
    collapsed = new Set(groups.map((g) => g.slug));
  });
  const allCollapsed = $derived(groups.length > 0 && groups.every((g) => collapsed.has(g.slug)));
  function toggleAll() {
    collapsed = allCollapsed ? new Set() : new Set(groups.map((g) => g.slug));
  }

  // ── Deploy modal (presets + saved teams reuse the division tri-state) ──
  let deployTarget = $state<{ title: string; agents: string[] } | null>(null);
  function deploy(title: string, agents: string[]) {
    deployTarget = { title, agents };
  }

  // ── Team detail (master/detail over the presets tab) ──
  type TeamView = {
    key: string;
    label: string;
    description: string;
    color: string | null;
    icon: Component;
    saved: boolean;
    agents: string[];
    examples: string[];
  };
  // The opened team is a nav location (ui.teamsSelected) so the system-wide
  // title-bar back arrow returns to the list — no in-view back button.
  function openPreset(p: (typeof PRESET_TEAMS)[number]) { tab = "presets"; ui.selectTeam(`preset:${p.slug}`); }
  function openSaved(t: SavedTeam) { tab = "presets"; ui.selectTeam(`saved:${t.id}`); }
  const openedTeam = $derived.by<TeamView | null>(() => {
    const key = ui.teamsSelected;
    if (!key) return null;
    if (key.startsWith("preset:")) {
      const slug = key.slice("preset:".length);
      const p = PRESET_TEAMS.find((x) => x.slug === slug);
      if (!p) return null;
      const text = presetTeamText(p, i18n.locale);
      return { key, label: text.label, description: text.description, color: p.color, icon: p.icon, saved: false, agents: p.agents, examples: teamExamples(p.slug, i18n.locale) };
    }
    if (key.startsWith("saved:")) {
      const id = key.slice("saved:".length);
      const t = teams.saved.find((x) => x.id === id);
      return t ? { key, label: t.name, description: i18n.t("teams.savedTeam", { count: t.agents.length, agents: i18n.agents(t.agents.length) }), color: null, icon: UsersIcon as unknown as Component, saved: true, agents: t.agents, examples: [] } : null;
    }
    return null;
  });

  // The opened team's agents, grouped by division (disclosures, closed by default).
  const TOTHER = "__other";
  const detailGroups = $derived.by(() => {
    if (!openedTeam) return [];
    const present = corpus.agents.filter((a) => openedTeam!.agents.includes(a.slug));
    const m = new Map<string, Agent[]>();
    for (const a of present) {
      const d = a.category || TOTHER;
      const arr = m.get(d);
      if (arr) arr.push(a);
      else m.set(d, [a]);
    }
    const out = [...m.entries()].map(([slug, rows]) => ({
      slug,
      label: slug === TOTHER ? i18n.t("common.other") : corpus.labelOf(slug),
      color: slug === TOTHER ? "#94A3B8" : corpus.colorOf(slug),
      icon: slug === TOTHER ? "HelpCircle" : corpus.iconOf(slug),
      rows: rows.slice().sort((a, b) => a.name.localeCompare(b.name)),
    }));
    out.sort((a, b) => (a.slug === TOTHER ? 1 : b.slug === TOTHER ? -1 : a.label.localeCompare(b.label)));
    return out;
  });
  const detailMissing = $derived(openedTeam ? openedTeam.agents.length - detailGroups.reduce((n, g) => n + g.rows.length, 0) : 0);
  // Curated examples when present; otherwise a generic loop seeded with the name.
  const detailExamples = $derived(
    openedTeam
      ? openedTeam.examples.length > 0
        ? openedTeam.examples
        : [i18n.t("teams.genericExample", { team: openedTeam.label })]
      : [],
  );

  let teamCollapsed = $state<Set<string>>(new Set());
  function toggleTeamGroup(slug: string) {
    const next = new Set(teamCollapsed);
    if (next.has(slug)) next.delete(slug);
    else next.add(slug);
    teamCollapsed = next;
  }
  let tcInitFor = "";
  $effect(() => {
    const k = openedTeam?.key ?? "";
    // Guard on an empty group set too: if the corpus loads after the team opens
    // (e.g. a cold back/forward restore), don't seed from an empty list — wait
    // for detailGroups so divisions stay closed by default.
    if (k === tcInitFor || detailGroups.length === 0) return;
    tcInitFor = k;
    teamCollapsed = new Set(detailGroups.map((g) => g.slug));
  });

  // How many of a team's agents exist in the corpus / are currently installed.
  const corpusSlugs = $derived(new Set(corpus.agents.map((a) => a.slug)));
  const installedSlugs = $derived(new Set(install.installed.filter((i) => i.state !== "removed").map((i) => i.slug)));
  function teamStats(agents: string[]) {
    const present = agents.filter((s) => corpusSlugs.has(s));
    const deployed = present.filter((s) => installedSlugs.has(s)).length;
    return { count: present.length, deployed };
  }

  function teamMeta(count: number, deployed: number): string {
    const base = `${i18n.number(count)} ${i18n.agents(count)}`;
    return deployed > 0 ? `${base} · ${i18n.number(deployed)} ${i18n.t("common.deployed")}` : base;
  }

  // ── Save current installs as a named team ──
  let saveOpen = $state(false);
  let saveName = $state("");
  function openSave() {
    saveName = "";
    saveOpen = true;
  }
  function confirmSave() {
    if (managedSlugs.length === 0) return;
    const t = teams.save(saveName, managedSlugs);
    saveOpen = false;
    tab = "presets";
    toast.success(i18n.t("teams.savedToast", { name: t.name }), `${i18n.number(t.agents.length)} ${i18n.agents(t.agents.length)}`);
  }

  function deleteSaved(t: SavedTeam) {
    teams.remove(t.id);
    toast.success(i18n.t("teams.deletedToast", { name: t.name }));
  }

  // ── Agentfile export / restore (your current team) ──
  async function exportLoadout() {
    const path = await saveDialog({ title: i18n.t("teams.saveAgentfile"), defaultPath: "Agentfile.json", filters: [{ name: "Agentfile", extensions: ["json"] }] });
    if (!path) return;
    busy = true;
    try {
      const n = await install.exportLoadout(path);
      toast.success(i18n.t("teams.exportedToast", { count: i18n.number(n), agents: i18n.agents(n) }), path);
    } catch (e) {
      toast.error(i18n.t("teams.exportFailed"), String(e));
    } finally {
      busy = false;
    }
  }
  async function importLoadout() {
    const picked = await openDialog({ title: i18n.t("teams.restoreAgentfile"), multiple: false, filters: [{ name: "Agentfile", extensions: ["json"] }] });
    if (!picked || Array.isArray(picked)) return;
    busy = true;
    try {
      const recs = await install.importLoadout(picked);
      toast.success(i18n.t("teams.restoredToast", { count: i18n.number(recs.length), agents: i18n.agents(recs.length) }), picked);
    } catch (e) {
      toast.error(i18n.t("teams.restoreFailed"), String(e));
    } finally {
      busy = false;
    }
  }
</script>

<section class="lo">
  <header class="lo-head">
    <div class="seg" role="tablist" aria-label={i18n.t("teams.viewAria")}>
      <button class="seg-btn" class:on={tab === "current"} role="tab" aria-selected={tab === "current"} onclick={() => (tab = "current")}>
        <UsersIcon size={14} /> {i18n.t("teams.yourTeam")}
      </button>
      <button class="seg-btn" class:on={tab === "presets"} role="tab" aria-selected={tab === "presets"} onclick={() => (tab = "presets")}>
        <RocketIcon size={14} /> {i18n.t("teams.presets")}
      </button>
    </div>

    {#if tab === "current"}
      <div class="lo-actions">
        {#if managed.length > 0}
          <button class="btn ghost" onclick={toggleAll}>{allCollapsed ? i18n.t("common.expandAll") : i18n.t("common.collapseAll")}</button>
          <button class="btn" onclick={openSave}><SaveIcon size={15} /><span>{i18n.t("teams.saveAs")}</span></button>
        {/if}
        <button class="btn" disabled={busy} onclick={importLoadout}><DownloadIcon size={15} /><span>{i18n.t("teams.restore")}</span></button>
        <button class="btn primary" disabled={busy || managed.length === 0} onclick={exportLoadout}><UploadIcon size={15} /><span>{i18n.t("teams.export")}</span></button>
      </div>
    {/if}
  </header>

  {#if tab === "current"}
    {#if managed.length === 0}
      <EmptyState title={i18n.t("teams.emptyTitle")} body={i18n.t("teams.emptyBody")}>
        {#snippet icon()}<ArchiveIcon size={48} />{/snippet}
        {#snippet cta()}
          <div class="empty-cta">
            <Button variant="primary" onclick={() => (tab = "presets")}>
              {#snippet icon()}<RocketIcon size={15} />{/snippet}
              {i18n.t("teams.browsePresets")}
            </Button>
            <button class="link-btn" onclick={() => ui.openPlaybook()}>{i18n.t("projects.newToAgents")}</button>
          </div>
        {/snippet}
      </EmptyState>
    {:else}
      <p class="lo-sub">
        {i18n.number(managed.length)} {i18n.agents(managed.length)}{#if groups.length > 1} · {i18n.number(groups.length)} {i18n.plural(groups.length, "division", "divisions", "раздел", "раздела", "разделов")}{/if}
      </p>
      <div class="groups">
        {#each groups as g (g.slug)}
          {@const Icon = resolveCategoryIcon(g.icon)}
          {@const isOpen = !collapsed.has(g.slug)}
          <section class="grp">
            <button class="grp-head" onclick={() => toggle(g.slug)} aria-expanded={isOpen}>
              <ChevronDown size={15} class={isOpen ? "grp-chev open" : "grp-chev"} />
              <span class="grp-ic" style="color:{g.color}"><Icon size={16} /></span>
              <span class="grp-label">{g.label}</span>
              <span class="grp-count">{g.rows.length}</span>
            </button>
            {#if isOpen}
              <ul class="rows">
                {#each g.rows as r (r.slug + r.tool + (r.projectPath ?? ""))}
                  <li class="row">
                    <span class="r-name">{r.name}</span>
                    <Pill tone="neutral">{install.toolLabel(r.tool)}</Pill>
                    {#if r.projectPath}<span class="r-proj" title={r.projectPath}>{r.projectPath.split("/").pop()}</span>{/if}
                  </li>
                {/each}
              </ul>
            {/if}
          </section>
        {/each}
      </div>
    {/if}
  {:else if openedTeam}
    {@const team = openedTeam}
    {@const st = teamStats(team.agents)}
    {@const HeadIcon = team.icon}
    <div class="tdetail">
      <div class="td-head">
        <span class="td-ic" style={team.color ? `color:${team.color}` : ""}><HeadIcon size={22} /></span>
        <div class="td-id">
          <h2 class="td-name">{team.label}</h2>
          <p class="td-desc">{team.description}</p>
        </div>
        <span class="td-count">{teamMeta(st.count, st.deployed)}</span>
        <Button variant="primary" onclick={() => deploy(`${i18n.t("common.install")} ${team.label}`, team.agents)}>{i18n.t("teams.deploy")}</Button>
      </div>

      <h3 class="td-sec">{i18n.t("teams.tryThese")}</h3>
      <div class="td-examples">
        {#each detailExamples as ex (ex)}
          <StarterPrompt template={ex} />
        {/each}
      </div>

      <h3 class="td-sec">{i18n.number(st.count)} {i18n.agents(st.count)}{#if detailGroups.length > 1} · {i18n.number(detailGroups.length)} {i18n.plural(detailGroups.length, "division", "divisions", "раздел", "раздела", "разделов")}{/if}</h3>
      <div class="groups">
        {#each detailGroups as g (g.slug)}
          {@const Icon = resolveCategoryIcon(g.icon)}
          {@const isOpen = !teamCollapsed.has(g.slug)}
          <section class="grp">
            <button class="grp-head" onclick={() => toggleTeamGroup(g.slug)} aria-expanded={isOpen}>
              <ChevronDown size={15} class={isOpen ? "grp-chev open" : "grp-chev"} />
              <span class="grp-ic" style="color:{g.color}"><Icon size={15} /></span>
              <span class="grp-label">{g.label}</span>
              <span class="grp-count">{g.rows.length}</span>
            </button>
            {#if isOpen}
              <ul class="rows">
                {#each g.rows as a (a.slug)}
                  <li class="row"><span class="row-emoji">{a.emoji ?? "🧩"}</span><span class="r-name">{a.name}</span></li>
                {/each}
              </ul>
            {/if}
          </section>
        {/each}
        {#if detailMissing > 0}
          <p class="td-missing">{i18n.t("teams.missing", { count: i18n.number(detailMissing), agents: i18n.agents(detailMissing) })}</p>
        {/if}
      </div>
    </div>
  {:else}
    <div class="cards">
      {#if teams.saved.length > 0}
        <h2 class="cards-h">{i18n.t("teams.savedTeams")}</h2>
        <ul class="card-list">
          {#each teams.saved as t (t.id)}
            {@const st = teamStats(t.agents)}
            <li class="card">
              <button class="card-main" onclick={() => openSaved(t)}>
                <span class="card-ic saved"><UsersIcon size={18} /></span>
                <span class="card-body">
                  <span class="card-title">{t.name}</span>
                  <span class="card-desc">{teamMeta(st.count, st.deployed)}</span>
                </span>
                <ChevronRight size={16} class="card-go" />
              </button>
              <button class="card-del" title={i18n.t("teams.deleteTeam")} aria-label={`${i18n.t("teams.deleteTeam")}: ${t.name}`} onclick={() => deleteSaved(t)}><Trash2 size={14} /></button>
            </li>
          {/each}
        </ul>
      {/if}

      <h2 class="cards-h">{i18n.t("teams.presetHeading")}</h2>
      <ul class="card-list">
        {#each PRESET_TEAMS as p (p.slug)}
          {@const st = teamStats(p.agents)}
          {@const Icon = p.icon}
          {@const text = presetTeamText(p, i18n.locale)}
          <li class="card">
            <button class="card-main" onclick={() => openPreset(p)}>
              <span class="card-ic" style="color:{p.color}"><Icon size={18} /></span>
              <span class="card-body">
                <span class="card-title">{text.label}</span>
                <span class="card-desc">{text.description}</span>
                <span class="card-meta">{teamMeta(st.count, st.deployed)}</span>
              </span>
              <ChevronRight size={16} class="card-go" />
            </button>
          </li>
        {/each}
      </ul>
    </div>
  {/if}
</section>

{#if deployTarget}
  <InstallModal title={deployTarget.title} agentSlugs={deployTarget.agents} onClose={() => (deployTarget = null)} />
{/if}

{#if saveOpen}
  <Modal open title={i18n.t("teams.saveTitle")} defaultFocus="first" onClose={() => (saveOpen = false)}>
    <p class="save-sub">{i18n.t("teams.saveSub", { count: i18n.number(managedSlugs.length), agents: i18n.agents(managedSlugs.length) })}</p>
    <Input bind:value={saveName} placeholder={i18n.t("teams.namePlaceholder")} ariaLabel={i18n.t("teams.nameAria")} />
    {#snippet actions()}
      <Button variant="secondary" modalAction="cancel" onclick={() => (saveOpen = false)}>{i18n.t("common.cancel")}</Button>
      <Button variant="primary" modalAction="confirm" disabled={managedSlugs.length === 0} onclick={confirmSave}>{i18n.t("teams.saveTeam")}</Button>
    {/snippet}
  </Modal>
{/if}

<style>
  .lo { display: flex; flex-direction: column; height: 100%; min-height: 0; }
  .lo-head {
    flex: none; display: flex; align-items: center; justify-content: space-between; gap: var(--space-3);
    padding: var(--space-3) var(--space-4); border-bottom: 1px solid var(--color-border);
  }
  .lo-sub { flex: none; padding: var(--space-2) var(--space-4) 0; color: var(--color-text-secondary); font-size: var(--text-body-sm); }
  .lo-actions { display: flex; gap: var(--space-2); }

  /* tabs */
  .seg { display: flex; align-items: center; gap: 2px; padding: 2px; background: var(--color-surface-sunken); border: 1px solid var(--color-border); border-radius: var(--radius-md); }
  .seg-btn { display: inline-flex; align-items: center; gap: 6px; height: 28px; padding: 0 12px; border-radius: var(--radius-sm); background: transparent; color: var(--color-text-secondary); font-size: var(--text-body-sm); cursor: pointer; white-space: nowrap; }
  .seg-btn:hover { color: var(--color-text-primary); }
  .seg-btn.on { background: var(--color-surface-raised); color: var(--color-text-primary); box-shadow: var(--shadow-sm, 0 1px 2px rgba(0,0,0,0.08)); }

  .btn {
    display: inline-flex; align-items: center; gap: 6px;
    height: 32px; padding: 0 var(--space-3);
    border: 1px solid var(--color-border); border-radius: var(--radius-md);
    background: transparent; color: var(--color-text-secondary);
    font-size: var(--text-body-sm); cursor: pointer;
  }
  .btn:hover:not(:disabled) { color: var(--color-text-primary); background: var(--color-surface-sunken); }
  .btn:disabled { opacity: 0.5; cursor: default; }
  .btn.primary { background: var(--color-brand); color: var(--color-text-inverse); border-color: transparent; }
  .btn.primary:hover:not(:disabled) { filter: brightness(1.08); background: var(--color-brand); }
  .btn.ghost { border-color: transparent; }

  /* ── Division groups (collapsible) ── */
  .groups { flex: 1; min-height: 0; overflow-y: auto; padding: var(--space-2) var(--space-3); }
  .grp { display: flex; flex-direction: column; }
  .grp-head {
    position: sticky; top: 0; z-index: 1;
    display: flex; align-items: center; gap: var(--space-2);
    width: 100%; padding: var(--space-2) var(--space-2);
    background: var(--color-surface); cursor: pointer; text-align: left;
    border-bottom: 1px solid var(--color-border);
  }
  .grp-head:hover { background: var(--color-surface-sunken); }
  :global(.grp-chev) { color: var(--color-text-muted); transition: transform var(--motion-duration-fast, 120ms) ease; transform: rotate(-90deg); flex: none; }
  :global(.grp-chev.open) { transform: rotate(0deg); }
  .grp-ic { flex: none; display: inline-flex; }
  .grp-label { flex: 1; min-width: 0; font-weight: var(--fw-semibold); color: var(--color-text-primary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .grp-count { flex: none; min-width: 20px; text-align: center; font-size: var(--text-caption); color: var(--color-text-muted); font-variant-numeric: tabular-nums; background: var(--color-surface-sunken); border-radius: var(--radius-full); padding: 1px 7px; }

  .rows { display: flex; flex-direction: column; gap: 1px; padding: 2px 0 var(--space-2) var(--space-4); }
  .row { display: flex; align-items: center; gap: var(--space-3); padding: var(--space-2) var(--space-3); border-radius: var(--radius-md); }
  .row:hover { background: var(--color-surface-sunken); }
  .r-name { flex: 1; min-width: 0; font-weight: var(--fw-medium); color: var(--color-text-primary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .r-proj { font-size: var(--text-caption); color: var(--color-text-muted); }

  /* ── Preset / saved team cards ── */
  .cards { flex: 1; min-height: 0; overflow-y: auto; padding: var(--space-3) var(--space-4); }
  .cards-h { font-size: var(--text-body-sm); font-weight: var(--fw-semibold); color: var(--color-text-muted); text-transform: uppercase; letter-spacing: 0.04em; margin: var(--space-3) 0 var(--space-2); }
  .cards-h:first-child { margin-top: 0; }
  .card-list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: var(--space-2); }
  .card {
    display: flex; align-items: stretch; gap: 0;
    border: 1px solid var(--color-border); border-radius: var(--radius-lg);
    background: var(--color-surface-raised); overflow: hidden;
  }
  .card:hover { border-color: var(--color-border-strong, var(--color-text-muted)); }
  .card-main {
    flex: 1; min-width: 0; display: flex; align-items: center; gap: var(--space-3);
    padding: var(--space-3); background: transparent; cursor: pointer; text-align: left;
  }
  .card-main:hover { background: var(--color-surface-sunken); }
  :global(.card-go) { flex: none; color: var(--color-text-muted); }
  .card-ic { flex: none; display: inline-flex; align-items: center; justify-content: center; width: 36px; height: 36px; border-radius: var(--radius-md); background: var(--color-surface-sunken); }
  .card-ic.saved { color: var(--color-text-secondary); }
  .card-body { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
  .card-title { font-weight: var(--fw-semibold); color: var(--color-text-primary); }
  .card-desc { font-size: var(--text-body-sm); color: var(--color-text-secondary); }
  .card-meta { font-size: var(--text-caption); color: var(--color-text-muted); }
  .card-del { flex: none; align-self: center; display: inline-flex; align-items: center; justify-content: center; width: 28px; height: 28px; margin-right: var(--space-2); border-radius: var(--radius-md); color: var(--color-text-muted); cursor: pointer; }
  .card-del:hover { background: var(--color-surface-sunken); color: var(--color-danger); }

  .save-sub { font-size: var(--text-body-sm); color: var(--color-text-secondary); margin-bottom: var(--space-3); }

  .empty-cta { display: flex; flex-direction: column; align-items: center; gap: var(--space-2); }
  .link-btn { background: transparent; color: var(--color-text-link, var(--color-brand)); font-size: var(--text-body-sm); cursor: pointer; padding: 2px; }
  .link-btn:hover { text-decoration: underline; }

  /* ── Team detail (master/detail over the presets tab) ── */
  .tdetail { flex: 1; min-height: 0; overflow-y: auto; padding: var(--space-3) var(--space-4) var(--space-5); display: flex; flex-direction: column; gap: var(--space-3); }
  .td-head { display: flex; align-items: center; gap: var(--space-3); }
  .td-ic { flex: none; display: inline-flex; align-items: center; justify-content: center; width: 44px; height: 44px; border-radius: var(--radius-md); background: var(--color-surface-sunken); }
  .td-id { flex: 1; min-width: 0; }
  .td-name { font-size: var(--text-h2); font-weight: var(--fw-semibold); color: var(--color-text-primary); }
  .td-desc { font-size: var(--text-body-sm); color: var(--color-text-secondary); }
  .td-count { flex: none; font-size: var(--text-body-sm); color: var(--color-text-muted); font-variant-numeric: tabular-nums; white-space: nowrap; }
  .td-sec { font-size: var(--text-caption); font-weight: var(--fw-semibold); color: var(--color-text-muted); text-transform: uppercase; letter-spacing: 0.04em; }
  .td-examples { display: flex; flex-direction: column; gap: var(--space-2); }
  .td-missing { font-size: var(--text-caption); color: var(--color-text-muted); padding: var(--space-2) var(--space-2) 0; }
  .row-emoji { flex: none; font-size: 15px; line-height: 1; }
  /* In the detail, the whole pane scrolls — don't make the group list its own
     scroll region (it inherits .groups from the "Your team" tab). */
  .tdetail .groups { flex: none; overflow: visible; padding: 0; }
</style>
