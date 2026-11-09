<script lang="ts">
  import LayoutDashboard from "@lucide/svelte/icons/layout-dashboard";
  import Bot from "@lucide/svelte/icons/bot";
  import Wrench from "@lucide/svelte/icons/wrench";
  import Users from "@lucide/svelte/icons/users";
  import FolderGit2 from "@lucide/svelte/icons/folder-git-2";
  import Rocket from "@lucide/svelte/icons/rocket";
  import Activity from "@lucide/svelte/icons/activity";

  import { ui } from "$lib/stores/ui.svelte";
  import { corpus } from "$lib/stores/corpus.svelte";
  import { catalog } from "$lib/stores/catalog.svelte";
  import { install } from "$lib/stores/install.svelte";
  import { i18n } from "$lib/stores/i18n.svelte";
  import { shortcut } from "$lib/util/platform";
  import type { SidebarSection } from "$lib/types";

  interface NavItem {
    id: SidebarSection;
    shortcut: string;
    icon: typeof Bot;
  }

  // Agency-first navigation. Agents is the home screen and now the UNIFIED
  // surface — it absorbed the former Library, so install state lives there as a
  // filter, not a separate section. Shortcut glyphs adapt per platform
  // (⌘ on macOS, Ctrl elsewhere) since the app ships on macOS/Linux/Windows.
  const nav: NavItem[] = [
    { id: "dashboard", shortcut: shortcut("0"), icon: LayoutDashboard },
    { id: "personas",  shortcut: shortcut("1"), icon: Bot },
    { id: "tools",     shortcut: shortcut("2"), icon: Wrench },
    { id: "teams",     shortcut: shortcut("3"), icon: Users },
    { id: "projects",  shortcut: shortcut("4"), icon: FolderGit2 },
    { id: "runbooks",  shortcut: shortcut("5"), icon: Rocket },
    { id: "activity",  shortcut: shortcut("6"), icon: Activity },
  ];

  function label(id: SidebarSection): string {
    if (id === "dashboard") return i18n.t("nav.dashboard");
    if (id === "personas") return i18n.t("nav.agents");
    if (id === "tools") return i18n.t("nav.tools");
    if (id === "teams") return i18n.t("nav.teams");
    if (id === "projects") return i18n.t("nav.projects");
    if (id === "runbooks") return i18n.t("nav.runbooks");
    return i18n.t("nav.activity");
  }

  function badge(id: SidebarSection): string | null {
    if (id === "personas") {
      // Installed agents with a newer version in the catalog — "updates
      // available". (Matches the "N updates" action on the Agents view. Other
      // non-current states — local edits, untracked, missing — are surfaced
      // per-agent when you drill in, not as a catch-all attention count.)
      const n = install.installed.filter((i) => i.state === "outdated").length;
      return n > 0 ? String(n) : null;
    }
    return null;
  }

  /** Footer: live corpus size — the app's own at-a-glance status. */
  const agentCount = $derived(corpus.agents.length);
  const statusText = $derived(i18n.t("nav.catalogStatus", { count: agentCount }));
  /** Catalog files that look like agents but couldn't be indexed (#79). */
  const skippedCount = $derived(catalog.status?.skipped.length ?? 0);
  const skippedText = $derived(
    i18n.t("nav.catalogStatusSkipped", {
      skipped: i18n.count(skippedCount, "common.file.one", "common.file.many"),
    }),
  );
</script>

<aside
  class="sidebar"
  class:collapsed={ui.sidebarCollapsed}
  style="width: {ui.sidebarCollapsed ? 56 : ui.sidebarWidth}px"
  aria-label={i18n.t("nav.primary")}
>
  <button class="brand" onclick={() => ui.setSection("personas")} title={i18n.t("nav.homeTitle")}>
    <span class="brand-mark" aria-hidden="true">🤖</span>
    <span class="brand-name">Agency Agents</span>
  </button>

  <nav>
    <ul>
      {#each nav as item (item.id)}
        {@const isActive = ui.section === item.id}
        {@const b = badge(item.id)}
        <li>
          <button
            class="nav-item"
            class:active={isActive}
            aria-current={isActive ? "page" : undefined}
            onclick={() => ui.setSection(item.id)}
            title={`${label(item.id)} (${item.shortcut})`}
          >
            <span class="ico" aria-hidden="true"><item.icon size={16} /></span>
            <span class="label">{label(item.id)}</span>
            {#if b}<span class="badge" title={i18n.t("agentUpdates.badgeTitle", { count: Number(b) })}>{b}</span>{/if}
          </button>
        </li>
      {/each}
    </ul>
  </nav>

  <footer class="foot">
    {#if skippedCount > 0}
      <button
        class="status status-warn"
        onclick={() => ui.openSettings("catalog")}
        title={`${statusText} · ${skippedText}. ${i18n.t("nav.catalogSkippedTitle")}`}
      >
        <span class="dot" aria-hidden="true"></span>
        <span class="status-lines">
          <span class="status-label">{statusText}</span>
          <span class="status-label status-sub">{skippedText}</span>
        </span>
      </button>
    {:else}
      <div class="status status-ready" title={statusText}>
        <span class="dot" aria-hidden="true"></span>
        <span class="status-label">{statusText}</span>
      </div>
    {/if}
  </footer>
</aside>

<style>
  .sidebar {
    /* width is set inline from ui.sidebarWidth (or 56px collapsed) so the
       resize handle in +page.svelte can drive it live. */
    flex: none;
    background: var(--color-surface-raised);
    border-right: 1px solid var(--color-border);
    display: flex;
    flex-direction: column;
    min-height: 0;
    transition: width var(--motion-duration-base, 180ms) var(--motion-ease-out, ease);
  }
  @media (prefers-reduced-motion: reduce) {
    .sidebar { transition: none; }
  }

  /* Brand row — the app's home affordance. Click → Agents home. */
  .brand {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    width: 100%;
    padding: var(--space-3);
    background: transparent;
    color: var(--color-text-primary);
    cursor: pointer;
    text-align: left;
  }
  .brand-mark { font-size: 18px; line-height: 1; }
  .brand-name { font-weight: var(--fw-semibold); font-size: var(--text-body); }

  nav { flex: 1; padding: var(--space-2); overflow-y: auto; }
  ul { display: flex; flex-direction: column; gap: 1px; }

  .nav-item {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    width: 100%;
    padding: var(--space-2) var(--space-3);
    border-radius: var(--radius-md);
    color: var(--color-text-secondary);
    font-size: var(--text-body);
    font-weight: var(--fw-medium);
    line-height: 1;
    text-align: left;
    transition: background-color var(--motion-duration-fast) var(--motion-ease-out);
  }
  .nav-item:hover { background: var(--color-surface-sunken); color: var(--color-text-primary); }
  .nav-item.active {
    background: var(--color-surface-sunken);
    color: var(--color-text-primary);
    font-weight: var(--fw-semibold);
  }
  .nav-item .label { flex: 1; }
  .ico { display: inline-flex; }
  .badge {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    height: 16px;
    min-width: 16px;
    padding: 0 var(--space-1);
    border-radius: var(--radius-full);
    background: var(--color-brand);
    color: var(--color-text-inverse);
    font-size: var(--text-caption);
    font-weight: var(--fw-semibold);
  }

  .foot {
    border-top: 1px solid var(--color-border);
    padding: var(--space-3);
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .status {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    font-size: var(--text-caption);
    color: var(--color-text-muted);
    padding: 2px var(--space-1);
    margin: -2px calc(-1 * var(--space-1));
    border-radius: var(--radius-sm);
    background: transparent;
    text-align: left;
    white-space: nowrap;
  }
  .dot {
    width: 8px; height: 8px; border-radius: var(--radius-full);
    background: var(--color-text-muted);
  }
  .status-ready .dot { background: var(--color-success); }
  /* Some catalog files couldn't be read (#79): a second line says so, and a
     click opens Settings → Catalog, which lists them. */
  .status-warn { cursor: pointer; align-items: flex-start; max-width: 100%; }
  .status-warn:hover { background: var(--color-surface-sunken); color: var(--color-text-primary); }
  .status-warn .dot { flex: none; margin-top: 0.35em; background: var(--color-warning); }
  .status-lines { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .status-label { overflow: hidden; text-overflow: ellipsis; }
  .status-sub { color: var(--color-warning); }

  /* ── Collapsed sidebar (icon-rail mode) ── */
  .sidebar.collapsed { width: 56px; }
  .sidebar.collapsed .brand-name { display: none; }
  .sidebar.collapsed .brand { justify-content: center; }
  .sidebar.collapsed .nav-item {
    justify-content: center;
    padding-left: 0;
    padding-right: 0;
    position: relative;
  }
  .sidebar.collapsed .nav-item .label { display: none; }
  .sidebar.collapsed .nav-item .badge {
    position: absolute;
    top: 2px;
    right: 4px;
    min-width: 14px;
    height: 14px;
    padding: 0 4px;
    font-size: 9px;
    line-height: 1;
  }
  .sidebar.collapsed .foot {
    align-items: center;
    padding-left: var(--space-2);
    padding-right: var(--space-2);
  }
  .sidebar.collapsed .status { justify-content: center; margin: 0; padding: 4px; }
  .sidebar.collapsed .status-label { display: none; }
</style>
