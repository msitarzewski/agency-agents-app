<script lang="ts">
  import ActivityIcon from "@lucide/svelte/icons/activity";
  import DownloadIcon from "@lucide/svelte/icons/download";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import RefreshIcon from "@lucide/svelte/icons/refresh-cw";
  import PlusIcon from "@lucide/svelte/icons/plus";
  import ToggleRightIcon from "@lucide/svelte/icons/toggle-right";
  import LayersIcon from "@lucide/svelte/icons/layers";

  import Button from "./Button.svelte";
  import EmptyState from "./EmptyState.svelte";
  import { activity, type JournalEntry } from "$lib/stores/activity.svelte";
  import { install } from "$lib/stores/install.svelte";

  /** Lucide icon per journal action. */
  const ACTION_ICON = {
    install: DownloadIcon,
    uninstall: Trash2,
    update: RefreshIcon,
    track: PlusIcon,
    switch: ToggleRightIcon,
    sync: RefreshIcon,
    bulk: LayersIcon,
  } as const;

  /** Sentence-case verb shown at the head of each row. */
  const ACTION_VERB: Record<JournalEntry["action"], string> = {
    install: "Installed",
    uninstall: "Removed",
    update: "Updated",
    track: "Tracked",
    switch: "Switched",
    sync: "Synced",
    bulk: "Bulk",
  };

  /** Basename of a project path, for the " · project" suffix. */
  function basename(p: string): string {
    const parts = p.replace(/[\\/]+$/, "").split(/[\\/]/);
    return parts[parts.length - 1] || p;
  }

  /** Human row text, tuned per action so target-less / summary ops read
      naturally instead of "Verb + sentence-fragment". */
  function rowText(e: JournalEntry): string {
    const tool = e.tool ? install.toolLabel(e.tool) : "";
    // Default-target toggle: the tool IS the subject, detail is the descriptor.
    if (e.action === "switch") {
      return tool ? `${tool} · ${e.detail ?? "default target changed"}` : (e.detail ?? "Default target changed");
    }
    // Batch sweeps: detail is already a self-contained phrase ("Updated 3 agents").
    if (e.action === "sync" || e.action === "bulk") {
      return e.detail ?? ACTION_VERB[e.action];
    }
    // Single-agent ops: "Verb agent → Tool · project".
    let s = `${ACTION_VERB[e.action]} ${e.agentName ?? e.agentSlug ?? ""}`.trim();
    if (tool) s += ` → ${tool}`;
    if (e.scope === "project" && e.projectPath) s += ` · ${basename(e.projectPath)}`;
    return s;
  }

  // ─── Day grouping (Today / Yesterday / locale date) ───────────────────────
  const DAY_MS = 86_400_000;
  function dayKey(iso: string): number {
    const d = new Date(iso);
    return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  }
  function dayLabel(key: number): string {
    const today = (() => {
      const n = new Date();
      return new Date(n.getFullYear(), n.getMonth(), n.getDate()).getTime();
    })();
    if (key === today) return "Today";
    if (key === today - DAY_MS) return "Yesterday";
    return new Date(key).toLocaleDateString(undefined, {
      weekday: "short",
      month: "short",
      day: "numeric",
    });
  }

  /** Entries are already newest-first; bucket them into ordered day groups. */
  const groups = $derived.by<{ key: number; label: string; entries: JournalEntry[] }[]>(() => {
    const out: { key: number; label: string; entries: JournalEntry[] }[] = [];
    let current: { key: number; label: string; entries: JournalEntry[] } | null = null;
    for (const e of activity.entries) {
      const k = dayKey(e.ts);
      if (!current || current.key !== k) {
        current = { key: k, label: dayLabel(k), entries: [] };
        out.push(current);
      }
      current.entries.push(e);
    }
    return out;
  });

  // ─── Relative timestamp ("just now" / "5m" / "3h" / locale time) ──────────
  function relTime(iso: string): string {
    const diff = Date.now() - new Date(iso).getTime();
    const sec = Math.floor(diff / 1000);
    if (sec < 45) return "just now";
    const min = Math.floor(sec / 60);
    if (min < 60) return `${min}m`;
    const hr = Math.floor(min / 60);
    if (hr < 24) return `${hr}h`;
    return new Date(iso).toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" });
  }
</script>

<section class="hist">
  {#if activity.entries.length > 0}
    <header class="panel-head" data-tauri-drag-region>
      <span class="action-wrap" data-tauri-drag-region="false">
        <Button size="sm" variant="ghost" onclick={() => activity.clear()}>
          {#snippet icon()}<Trash2 size={14} />{/snippet}
          Clear
        </Button>
      </span>
    </header>
  {/if}

  <div class="list-wrap">
    {#if activity.entries.length === 0}
      <EmptyState
        title="No activity yet"
        body="Installs, updates, and removes will show up here."
      >
        {#snippet icon()}<ActivityIcon size={48} />{/snippet}
      </EmptyState>
    {:else}
      {#each groups as group (group.key)}
        <h2 class="day">{group.label}</h2>
        <ul class="list">
          {#each group.entries as e (e.id)}
            {@const Icon = ACTION_ICON[e.action]}
            <li class="row">
              <span class="ico" aria-hidden="true"><Icon size={15} /></span>
              <span class="text truncate" title={e.outcome === "error" && e.detail ? e.detail : rowText(e)}>
                {rowText(e)}
              </span>
              <span class="time" title={new Date(e.ts).toLocaleString()}>{relTime(e.ts)}</span>
              <span
                class="status-dot"
                class:ok={e.outcome === "ok"}
                class:fail={e.outcome === "error"}
                aria-label={e.outcome === "error" ? "Failed" : "Succeeded"}
                title={e.outcome === "error" ? (e.detail ?? "Failed") : "Succeeded"}
              ></span>
            </li>
          {/each}
        </ul>
      {/each}
    {/if}
  </div>
</section>

<style>
  .hist { display: flex; flex-direction: column; min-height: 0; height: 100%; }
  .panel-head {
    display: flex; justify-content: flex-end; align-items: center;
    padding: var(--space-4);
    border-bottom: 1px solid var(--color-border);
  }

  .list-wrap { flex: 1; overflow-y: auto; min-height: 0; }

  .day {
    position: sticky;
    top: 0;
    z-index: 1;
    padding: var(--space-2) var(--space-3);
    background: var(--color-surface);
    color: var(--color-text-muted);
    font-size: var(--text-caption);
    font-weight: var(--fw-semibold);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    border-bottom: 1px solid var(--color-border);
  }

  .list { display: flex; flex-direction: column; }
  .row {
    display: grid;
    grid-template-columns: 24px 1fr auto 10px;
    align-items: center;
    width: 100%;
    padding: var(--space-2) var(--space-3);
    min-height: 36px;
    text-align: left;
    color: var(--color-text-primary);
    font-size: var(--text-body);
    border-bottom: 1px solid var(--color-border);
    gap: var(--space-3);
  }

  .ico { display: inline-flex; color: var(--color-text-muted); }
  .text { font-weight: var(--fw-medium); }
  .time {
    text-align: right;
    color: var(--color-text-muted);
    font-size: var(--text-body-sm);
    font-variant-numeric: tabular-nums;
  }

  .status-dot {
    width: 8px; height: 8px;
    border-radius: var(--radius-full);
    background: var(--color-text-muted);
  }
  .status-dot.ok   { background: var(--color-success); }
  .status-dot.fail { background: var(--color-danger); }
</style>
