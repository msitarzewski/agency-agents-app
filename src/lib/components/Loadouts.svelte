<script lang="ts">
  /**
   * Loadouts — save your current install set to a portable "Agentfile" and
   * restore it on another machine (a lockfile-style snapshot of installed
   * agents). Export reads the ledger; import installs every entry.
   */
  import EmptyState from "./EmptyState.svelte";
  import Pill from "./Pill.svelte";
  import UploadIcon from "@lucide/svelte/icons/upload";
  import DownloadIcon from "@lucide/svelte/icons/download";
  import ArchiveIcon from "@lucide/svelte/icons/archive";

  import { install } from "$lib/stores/install.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";

  // Pure reader — install state is reconciled globally in +layout.

  // The loadout = what WE installed (in the ledger). Foreign installs aren't
  // part of it until tracked, so they're excluded from the managed count.
  const managed = $derived(install.installed.filter((i) => i.state !== "foreign"));
  let busy = $state(false);

  async function exportLoadout() {
    const path = await saveDialog({
      title: "Save Agentfile",
      defaultPath: "Agentfile.json",
      filters: [{ name: "Agentfile", extensions: ["json"] }],
    });
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
    const picked = await openDialog({
      title: "Restore from Agentfile",
      multiple: false,
      filters: [{ name: "Agentfile", extensions: ["json"] }],
    });
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
    <p class="lo-sub">
      {managed.length} agent{managed.length === 1 ? "" : "s"} in your current loadout
    </p>
    <div class="lo-actions">
      <button class="btn" disabled={busy} onclick={importLoadout}>
        <DownloadIcon size={15} /><span>Restore…</span>
      </button>
      <button class="btn primary" disabled={busy || managed.length === 0} onclick={exportLoadout}>
        <UploadIcon size={15} /><span>Export…</span>
      </button>
    </div>
  </header>

  {#if managed.length === 0}
    <EmptyState title="No loadout yet">
      {#snippet icon()}<ArchiveIcon size={48} />{/snippet}
      Install some agents, then <strong>Export</strong> to save your setup as an Agentfile —
      and <strong>Restore</strong> it on a new Mac in one click. Already have an Agentfile?
      Restore it now.
    </EmptyState>
  {:else}
    <ul class="rows">
      {#each managed as r (r.slug + r.tool + (r.projectPath ?? ""))}
        <li class="row">
          <span class="r-name">{r.name}</span>
          <Pill tone="neutral">{install.toolLabel(r.tool)}</Pill>
          {#if r.projectPath}<span class="r-proj" title={r.projectPath}>{r.projectPath.split("/").pop()}</span>{/if}
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .lo { display: flex; flex-direction: column; height: 100%; min-height: 0; }
  .lo-head {
    flex: none; display: flex; align-items: center; justify-content: space-between;
    padding: var(--space-4); border-bottom: 1px solid var(--color-border);
  }
  .lo-sub { color: var(--color-text-secondary); font-size: var(--text-body-sm); }
  .lo-actions { display: flex; gap: var(--space-2); }
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
  .rows { overflow-y: auto; padding: var(--space-3) var(--space-4); display: flex; flex-direction: column; gap: 1px; }
  .row { display: flex; align-items: center; gap: var(--space-3); padding: var(--space-2) var(--space-3); border-radius: var(--radius-md); }
  .row:hover { background: var(--color-surface-sunken); }
  .r-name { flex: 1; min-width: 0; font-weight: var(--fw-medium); color: var(--color-text-primary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .r-proj { font-size: var(--text-caption); color: var(--color-text-muted); }
</style>
