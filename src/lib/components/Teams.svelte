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
  import DeployModal from "./DeployModal.svelte";
  import UploadIcon from "@lucide/svelte/icons/upload";
  import DownloadIcon from "@lucide/svelte/icons/download";
  import ArchiveIcon from "@lucide/svelte/icons/archive";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import SaveIcon from "@lucide/svelte/icons/bookmark-plus";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import UsersIcon from "@lucide/svelte/icons/users";
  import RocketIcon from "@lucide/svelte/icons/rocket";

  import { install } from "$lib/stores/install.svelte";
  import { corpus } from "$lib/stores/corpus.svelte";
  import { teams, type SavedTeam } from "$lib/stores/teams.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import { resolveCategoryIcon } from "$lib/util/categoryIcon";
  import { PRESET_TEAMS } from "$lib/data/presetTeams";
  import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
  import type { InstalledAgent } from "$lib/types";

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
      label: slug === OTHER ? "Other" : corpus.labelOf(slug),
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
  const allCollapsed = $derived(groups.length > 0 && groups.every((g) => collapsed.has(g.slug)));
  function toggleAll() {
    collapsed = allCollapsed ? new Set() : new Set(groups.map((g) => g.slug));
  }

  // ── Deploy modal (presets + saved teams reuse the division tri-state) ──
  let deployTarget = $state<{ title: string; agents: string[] } | null>(null);
  function deploy(title: string, agents: string[]) {
    deployTarget = { title, agents };
  }

  // How many of a team's agents exist in the corpus / are currently installed.
  const corpusSlugs = $derived(new Set(corpus.agents.map((a) => a.slug)));
  const installedSlugs = $derived(new Set(install.installed.filter((i) => i.state !== "removed").map((i) => i.slug)));
  function teamStats(agents: string[]) {
    const present = agents.filter((s) => corpusSlugs.has(s));
    const deployed = present.filter((s) => installedSlugs.has(s)).length;
    return { count: present.length, deployed };
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
    toast.success(`Saved “${t.name}”`, `${t.agents.length} agent${t.agents.length === 1 ? "" : "s"}`);
  }

  function deleteSaved(t: SavedTeam) {
    teams.remove(t.id);
    toast.success(`Deleted “${t.name}”`);
  }

  // ── Agentfile export / restore (your current team) ──
  async function exportLoadout() {
    const path = await saveDialog({ title: "Save Agentfile", defaultPath: "Agentfile.json", filters: [{ name: "Agentfile", extensions: ["json"] }] });
    if (!path) return;
    busy = true;
    try {
      const n = await install.exportLoadout(path);
      toast.success(`Exported ${n} agent${n === 1 ? "" : "s"}`, path);
    } catch (e) {
      toast.error("Export failed", String(e));
    } finally {
      busy = false;
    }
  }
  async function importLoadout() {
    const picked = await openDialog({ title: "Restore from Agentfile", multiple: false, filters: [{ name: "Agentfile", extensions: ["json"] }] });
    if (!picked || Array.isArray(picked)) return;
    busy = true;
    try {
      const recs = await install.importLoadout(picked);
      toast.success(`Restored ${recs.length} agent${recs.length === 1 ? "" : "s"}`, picked);
    } catch (e) {
      toast.error("Restore failed", String(e));
    } finally {
      busy = false;
    }
  }
</script>

<section class="lo">
  <header class="lo-head">
    <div class="seg" role="tablist" aria-label="Teams view">
      <button class="seg-btn" class:on={tab === "current"} role="tab" aria-selected={tab === "current"} onclick={() => (tab = "current")}>
        <UsersIcon size={14} /> Your team
      </button>
      <button class="seg-btn" class:on={tab === "presets"} role="tab" aria-selected={tab === "presets"} onclick={() => (tab = "presets")}>
        <RocketIcon size={14} /> Team presets
      </button>
    </div>

    {#if tab === "current"}
      <div class="lo-actions">
        {#if managed.length > 0}
          <button class="btn ghost" onclick={toggleAll}>{allCollapsed ? "Expand all" : "Collapse all"}</button>
          <button class="btn" onclick={openSave}><SaveIcon size={15} /><span>Save as team…</span></button>
        {/if}
        <button class="btn" disabled={busy} onclick={importLoadout}><DownloadIcon size={15} /><span>Restore…</span></button>
        <button class="btn primary" disabled={busy || managed.length === 0} onclick={exportLoadout}><UploadIcon size={15} /><span>Export…</span></button>
      </div>
    {/if}
  </header>

  {#if tab === "current"}
    {#if managed.length === 0}
      <EmptyState title="No team yet">
        {#snippet icon()}<ArchiveIcon size={48} />{/snippet}
        Install some agents — or deploy a <strong>preset</strong> — then save your setup as a team
        and <strong>Export</strong> it to move to a new Mac in one click.
      </EmptyState>
    {:else}
      <p class="lo-sub">
        {managed.length} agent{managed.length === 1 ? "" : "s"}{#if groups.length > 1} · {groups.length} divisions{/if}
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
  {:else}
    <div class="cards">
      {#if teams.saved.length > 0}
        <h2 class="cards-h">Your saved teams</h2>
        <ul class="card-list">
          {#each teams.saved as t (t.id)}
            {@const st = teamStats(t.agents)}
            <li class="card">
              <span class="card-ic saved"><UsersIcon size={18} /></span>
              <span class="card-body">
                <span class="card-title">{t.name}</span>
                <span class="card-desc">{st.count} agent{st.count === 1 ? "" : "s"}{#if st.deployed > 0} · {st.deployed} deployed{/if}</span>
              </span>
              <button class="card-del" title="Delete team" aria-label="Delete team" onclick={() => deleteSaved(t)}><Trash2 size={14} /></button>
              <Button size="sm" variant="secondary" onclick={() => deploy(`Deploy ${t.name}`, t.agents)}>Deploy…</Button>
            </li>
          {/each}
        </ul>
      {/if}

      <h2 class="cards-h">Presets</h2>
      <ul class="card-list">
        {#each PRESET_TEAMS as p (p.slug)}
          {@const st = teamStats(p.agents)}
          {@const Icon = p.icon}
          <li class="card">
            <span class="card-ic" style="color:{p.color}"><Icon size={18} /></span>
            <span class="card-body">
              <span class="card-title">{p.label}</span>
              <span class="card-desc">{p.description}</span>
              <span class="card-meta">{st.count} agent{st.count === 1 ? "" : "s"}{#if st.deployed > 0} · {st.deployed} deployed{/if}</span>
            </span>
            <Button size="sm" variant="secondary" onclick={() => deploy(`Deploy ${p.label}`, p.agents)}>Deploy…</Button>
          </li>
        {/each}
      </ul>
    </div>
  {/if}
</section>

{#if deployTarget}
  <DeployModal title={deployTarget.title} agentSlugs={deployTarget.agents} onClose={() => (deployTarget = null)} />
{/if}

{#if saveOpen}
  <Modal open title="Save as team" defaultFocus="first" onClose={() => (saveOpen = false)}>
    <p class="save-sub">Snapshots your {managedSlugs.length} installed agent{managedSlugs.length === 1 ? "" : "s"} as a reusable team.</p>
    <Input bind:value={saveName} placeholder="Team name (e.g. “My SaaS stack”)" ariaLabel="Team name" />
    {#snippet actions()}
      <Button variant="secondary" modalAction="cancel" onclick={() => (saveOpen = false)}>Cancel</Button>
      <Button variant="primary" modalAction="confirm" disabled={managedSlugs.length === 0} onclick={confirmSave}>Save team</Button>
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
    display: flex; align-items: center; gap: var(--space-3);
    padding: var(--space-3); border: 1px solid var(--color-border); border-radius: var(--radius-lg);
    background: var(--color-surface-raised);
  }
  .card-ic { flex: none; display: inline-flex; align-items: center; justify-content: center; width: 36px; height: 36px; border-radius: var(--radius-md); background: var(--color-surface-sunken); }
  .card-ic.saved { color: var(--color-text-secondary); }
  .card-body { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
  .card-title { font-weight: var(--fw-semibold); color: var(--color-text-primary); }
  .card-desc { font-size: var(--text-body-sm); color: var(--color-text-secondary); }
  .card-meta { font-size: var(--text-caption); color: var(--color-text-muted); }
  .card-del { flex: none; display: inline-flex; align-items: center; justify-content: center; width: 28px; height: 28px; border-radius: var(--radius-md); color: var(--color-text-muted); cursor: pointer; }
  .card-del:hover { background: var(--color-surface-sunken); color: var(--color-danger); }

  .save-sub { font-size: var(--text-body-sm); color: var(--color-text-secondary); margin-bottom: var(--space-3); }
</style>
