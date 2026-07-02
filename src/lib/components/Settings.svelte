<script lang="ts">
  /**
   * Settings.svelte — Phase 12b
   *
   * Modal container for app-wide preferences. Opens via ⌘, or the sidebar
   * gear icon. Six sections rendered in a left-nav + right-pane layout,
   * inspired by the macOS System Settings shell.
   *
   * Focus-trap + Esc-to-close + click-outside-to-close mechanics borrowed
   * from `CommandPalette.svelte` (overlay + dialog box at z-index 80/81).
   * Settings sits one z-layer above the palette so a stray ⌘, while the
   * palette is open still surfaces correctly (we close the palette in the
   * keyboard handler, but the layering is defensive).
   *
   * NOTE: this component is deliberately self-contained — each section is a
   * sibling Svelte file rendered inline by `activeSection`. No store needed
   * for which section is active; that's purely modal-local state.
   */

  import { onMount } from "svelte";
  import X from "@lucide/svelte/icons/x";
  import Paintbrush from "@lucide/svelte/icons/paintbrush";
  import Library from "@lucide/svelte/icons/library";
  import Globe from "@lucide/svelte/icons/globe";
  import Github from "@lucide/svelte/icons/git-fork";
  import Activity from "@lucide/svelte/icons/activity";
  import Info from "@lucide/svelte/icons/info";

  import { ui } from "$lib/stores/ui.svelte";
  import { i18n, type TranslationKey } from "$lib/i18n.svelte";
  import SettingsSectionAppearance from "./SettingsSectionAppearance.svelte";
  import SettingsSectionCatalog from "./SettingsSectionCatalog.svelte";
  import SettingsSectionNetwork from "./SettingsSectionNetwork.svelte";
  import SettingsSectionGitHub from "./SettingsSectionGitHub.svelte";
  import SettingsSectionActivity from "./SettingsSectionActivity.svelte";
  import SettingsSectionAbout from "./SettingsSectionAbout.svelte";
  import type { SettingsSection } from "$lib/types";

  interface NavEntry {
    id: SettingsSection;
    labelKey: TranslationKey;
    icon: typeof Paintbrush;
  }

  const NAV: NavEntry[] = [
    { id: "appearance", labelKey: "settings.appearance", icon: Paintbrush },
    { id: "catalog",    labelKey: "settings.catalog",    icon: Library },
    { id: "network",    labelKey: "settings.network",    icon: Globe },
    { id: "github",     labelKey: "settings.github",     icon: Github },
    { id: "activity",   labelKey: "settings.activity",   icon: Activity },
    { id: "about",      labelKey: "settings.about",      icon: Info },
  ];

  let activeSection: SettingsSection = $state("appearance");
  let modalEl: HTMLDivElement | undefined = $state();

  /** Reset the section pick whenever the modal is reopened. Honors
      `ui.settingsInitialSection` when the caller deep-linked
      (e.g. PackageDetail intercepts a GitHub action while signed out);
      otherwise lands on Appearance. Focus the first focusable inside
      the modal so the user can immediately Tab through. */
  $effect(() => {
    if (ui.settingsOpen) {
      activeSection = ui.settingsInitialSection ?? "appearance";
      // Defer focus until DOM is updated.
      setTimeout(() => {
        if (!modalEl) return;
        const focusable = modalEl.querySelector<HTMLElement>(
          "button, [href], input, select, textarea, [tabindex]:not([tabindex='-1'])"
        );
        focusable?.focus();
      }, 0);
    }
  });

  /** Esc closes; Tab/Shift+Tab traps focus inside the modal box. Both
      mirror the CommandPalette behaviour and the Modal primitive. */
  function onKey(e: KeyboardEvent) {
    if (!ui.settingsOpen) return;
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      ui.closeSettings();
      return;
    }
    if (e.key === "Tab" && modalEl) {
      const focusables = modalEl.querySelectorAll<HTMLElement>(
        "button, [href], input, select, textarea, [tabindex]:not([tabindex='-1'])"
      );
      if (focusables.length === 0) return;
      const first = focusables[0];
      const last = focusables[focusables.length - 1];
      const active = document.activeElement as HTMLElement | null;
      if (e.shiftKey && active === first) { e.preventDefault(); last.focus(); }
      else if (!e.shiftKey && active === last) { e.preventDefault(); first.focus(); }
    }
  }

  onMount(() => {
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });
</script>

{#if ui.settingsOpen}
  <!-- Scrim catches click-outside to close. role="presentation" because the
       interactive surface is the modal box; the scrim is decorative. -->
  <div class="scrim" role="presentation" onclick={() => ui.closeSettings()}></div>
  <div
    class="settings-wrap"
    role="dialog"
    aria-modal="true"
    aria-label={i18n.t("settings.title")}
  >
    <div
      class="settings"
      bind:this={modalEl}
      data-tauri-drag-region="false"
    >
      <div class="nav" role="tablist" aria-label={i18n.t("settings.sections")}>
        <h1 class="nav-title">{i18n.t("settings.title")}</h1>
        <ul>
          {#each NAV as entry (entry.id)}
            {@const isActive = activeSection === entry.id}
            <li>
              <button
                type="button"
                class="nav-item"
                class:active={isActive}
                role="tab"
                aria-selected={isActive}
                aria-controls="settings-pane"
                onclick={() => (activeSection = entry.id)}
              >
                <span class="nav-icon" aria-hidden="true"><entry.icon size={14} /></span>
                <span>{i18n.t(entry.labelKey)}</span>
              </button>
            </li>
          {/each}
        </ul>
      </div>

      <div
        class="pane"
        id="settings-pane"
        role="tabpanel"
        aria-label={i18n.t("settings.paneLabel", { section: i18n.t(NAV.find((entry) => entry.id === activeSection)?.labelKey ?? "settings.appearance") })}
      >
        <button
          type="button"
          class="close"
          aria-label={i18n.t("settings.close")}
          onclick={() => ui.closeSettings()}
        >
          <X size={16} />
        </button>
        {#if activeSection === "appearance"}
          <SettingsSectionAppearance />
        {:else if activeSection === "catalog"}
          <SettingsSectionCatalog />
        {:else if activeSection === "network"}
          <SettingsSectionNetwork />
        {:else if activeSection === "github"}
          <SettingsSectionGitHub />
        {:else if activeSection === "activity"}
          <SettingsSectionActivity />
        {:else if activeSection === "about"}
          <SettingsSectionAbout />
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    /* Start below the 36px window title bar (the macOS drag region) so
       the window stays movable while Settings is open. A full `inset: 0`
       scrim covers the title bar's `data-tauri-drag-region`, swallowing
       the mousedown macOS needs to begin a window drag — so the window
       was stuck until Settings closed (issue #8). Leaving the title bar
       uncovered also matches native macOS, where the title bar stays at
       full opacity while a sheet is open. */
    inset: 36px 0 0 0;
    background: rgb(0 0 0 / 0.4);
    z-index: 90;
    animation: fadeIn var(--motion-duration-base) var(--motion-ease-out);
  }
  .settings-wrap {
    position: fixed;
    inset: 0;
    z-index: 91;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--space-4);
    pointer-events: none;
  }
  .settings {
    pointer-events: auto;
    width: 100%;
    max-width: 950px;
    height: 80vh;
    max-height: 700px;
    background: var(--color-surface-raised);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-modal);
    display: grid;
    grid-template-columns: 220px 1fr;
    overflow: hidden;
    animation: pop var(--motion-duration-base) var(--motion-ease-out);
  }

  /* ---------- Nav (left rail) ---------- */
  .nav {
    background: var(--color-surface-sunken);
    border-right: 1px solid var(--color-border);
    padding: var(--space-4) var(--space-2);
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .nav-title {
    font-size: var(--text-body);
    font-weight: var(--fw-semibold);
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    padding: 0 var(--space-3);
    margin-bottom: var(--space-1);
  }
  .nav ul { display: flex; flex-direction: column; gap: 1px; }
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
    cursor: pointer;
  }
  .nav-item:hover {
    background: var(--color-surface-raised);
    color: var(--color-text-primary);
  }
  .nav-item.active {
    background: var(--color-surface-raised);
    color: var(--color-text-primary);
    font-weight: var(--fw-semibold);
    box-shadow: var(--shadow-xs);
  }
  .nav-icon { display: inline-flex; color: inherit; }

  /* ---------- Right pane ---------- */
  .pane {
    position: relative;
    padding: var(--space-6) var(--space-6) var(--space-6) var(--space-6);
    overflow-y: auto;
    min-height: 0;
  }
  .close {
    position: absolute;
    top: var(--space-3);
    right: var(--space-3);
    color: var(--color-text-muted);
    padding: 4px;
    border-radius: var(--radius-sm);
    cursor: pointer;
  }
  .close:hover {
    color: var(--color-text-primary);
    background: var(--color-surface-sunken);
  }

  @keyframes fadeIn { from { opacity: 0; } to { opacity: 1; } }
  @keyframes pop {
    from { opacity: 0; transform: scale(0.96); }
    to   { opacity: 1; transform: scale(1); }
  }
  @media (prefers-reduced-motion: reduce) {
    .scrim, .settings { animation: none; }
  }
</style>
