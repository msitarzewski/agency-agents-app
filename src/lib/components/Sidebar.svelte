<script lang="ts">
  import LayoutDashboard from "@lucide/svelte/icons/layout-dashboard";
  import Bot from "@lucide/svelte/icons/bot";
  import Wrench from "@lucide/svelte/icons/wrench";
  import Users from "@lucide/svelte/icons/users";
  import FolderGit2 from "@lucide/svelte/icons/folder-git-2";
  import Activity from "@lucide/svelte/icons/activity";

  import { ui } from "$lib/stores/ui.svelte";
  import { corpus } from "$lib/stores/corpus.svelte";
  import { install } from "$lib/stores/install.svelte";
  import { i18n, type TranslationKey } from "$lib/i18n.svelte";
  import { shortcut } from "$lib/util/platform";
  import type { SidebarSection } from "$lib/types";

  interface NavItem {
    id: SidebarSection;
    labelKey: TranslationKey;
    shortcut: string;
    icon: typeof Bot;
  }

  // Agency-first navigation. Agents is the home screen and now the UNIFIED
  // surface — it absorbed the former Library, so install state lives there as a
  // filter, not a separate section. Shortcut glyphs adapt per platform
  // (⌘ on macOS, Ctrl elsewhere) since the app ships on macOS/Linux/Windows.
  const nav: NavItem[] = [
    { id: "dashboard", labelKey: "nav.dashboard", shortcut: shortcut("0"), icon: LayoutDashboard },
    { id: "personas",  labelKey: "nav.agents",    shortcut: shortcut("1"), icon: Bot },
    { id: "tools",     labelKey: "nav.tools",     shortcut: shortcut("2"), icon: Wrench },
    { id: "teams",     labelKey: "nav.teams",     shortcut: shortcut("3"), icon: Users },
    { id: "projects",  labelKey: "nav.projects",  shortcut: shortcut("4"), icon: FolderGit2 },
    { id: "activity",  labelKey: "nav.activity",  shortcut: shortcut("5"), icon: Activity },
  ];

  function badge(id: SidebarSection): string | null {
    if (id === "personas") {
      // Surface installs needing attention (outdated/modified/removed/foreign).
      const n = install.installed.filter((i) => i.state !== "current").length;
      return n > 0 ? String(n) : null;
    }
    return null;
  }

  /** Footer: live corpus size — the app's own at-a-glance status. */
  const agentCount = $derived(corpus.agents.length);
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
            title={`${i18n.t(item.labelKey)} (${item.shortcut})`}
          >
            <span class="ico" aria-hidden="true"><item.icon size={16} /></span>
            <span class="label">{i18n.t(item.labelKey)}</span>
            {#if b}<span class="badge">{b}</span>{/if}
          </button>
        </li>
      {/each}
    </ul>
  </nav>

  <footer class="foot">
    <div class="status status-ready" title={i18n.t("nav.catalogAgentsTitle", { count: agentCount, agents: i18n.agents(agentCount) })}>
      <span class="dot" aria-hidden="true"></span>
      <span class="status-label">{i18n.t("nav.catalogAgents", { count: agentCount, agents: i18n.agents(agentCount) })}</span>
    </div>
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
