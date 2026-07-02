<script lang="ts">
  /**
   * SettingsSectionAppearance.svelte — Phase 12b
   *
   * Theme radio (Light/Dark/System), default landing dropdown, vibrancy
   * material dropdown. All settings persist via `ui.svelte.ts`
   * localStorage helpers.
   */

  import Sun from "@lucide/svelte/icons/sun";
  import Moon from "@lucide/svelte/icons/moon";
  import Monitor from "@lucide/svelte/icons/monitor";

  import { ui, VIBRANCY_MATERIALS, type VibrancyMaterial } from "$lib/stores/ui.svelte";
  import { i18n } from "$lib/stores/i18n.svelte";
  import { LOCALES, localeLabels, type Locale } from "$lib/i18n/messages";
  import type { SidebarSection, ThemePreference } from "$lib/types";

  /** Sections the user can pick as their default landing page. Mirrors the
      sidebar nav order, plus Dashboard which lives in the brand button. */
  const SECTIONS: SidebarSection[] = ["dashboard", "personas", "tools", "teams", "projects", "activity"];

  function sectionLabel(value: SidebarSection): string {
    if (value === "dashboard") return i18n.t("nav.dashboard");
    if (value === "personas") return i18n.t("nav.agents");
    if (value === "tools") return i18n.t("nav.tools");
    if (value === "teams") return i18n.t("nav.teams");
    if (value === "projects") return i18n.t("nav.projects");
    return i18n.t("nav.activity");
  }

  function onSectionChange(e: Event) {
    const value = (e.currentTarget as HTMLSelectElement).value as SidebarSection;
    ui.setDefaultSection(value);
  }
  function onLocaleChange(e: Event) {
    const value = (e.currentTarget as HTMLSelectElement).value as Locale;
    i18n.setLocale(value);
  }
  function onVibrancyChange(e: Event) {
    const value = (e.currentTarget as HTMLSelectElement).value as VibrancyMaterial;
    ui.setVibrancyMaterial(value);
  }
  function pickTheme(t: ThemePreference) { ui.setTheme(t); }
</script>

<div class="section">
  <h2>{i18n.t("settings.appearance.title")}</h2>

  <div class="field">
    <label for="theme-group">{i18n.t("settings.appearance.theme")}</label>
    <div id="theme-group" class="radio-row" role="radiogroup" aria-label={i18n.t("settings.appearance.theme")}>
      <button
        type="button"
        class="radio-btn"
        class:on={ui.theme === "light"}
        role="radio"
        aria-checked={ui.theme === "light"}
        onclick={() => pickTheme("light")}
      >
        <Sun size={14} /> {i18n.t("app.theme.light")}
      </button>
      <button
        type="button"
        class="radio-btn"
        class:on={ui.theme === "dark"}
        role="radio"
        aria-checked={ui.theme === "dark"}
        onclick={() => pickTheme("dark")}
      >
        <Moon size={14} /> {i18n.t("app.theme.dark")}
      </button>
      <button
        type="button"
        class="radio-btn"
        class:on={ui.theme === "system"}
        role="radio"
        aria-checked={ui.theme === "system"}
        onclick={() => pickTheme("system")}
      >
        <Monitor size={14} /> {i18n.t("app.theme.system")}
      </button>
    </div>
    <p class="hint">{i18n.t("settings.appearance.themeHint")}</p>
  </div>

  <div class="field">
    <label for="default-section">{i18n.t("settings.appearance.defaultLanding")}</label>
    <select
      id="default-section"
      class="select"
      value={ui.defaultSection}
      onchange={onSectionChange}
    >
      {#each SECTIONS as opt (opt)}
        <option value={opt}>{sectionLabel(opt)}</option>
      {/each}
    </select>
    <p class="hint">{i18n.t("settings.appearance.defaultLandingHint")}</p>
  </div>

  <div class="field">
    <label for="locale">{i18n.t("settings.appearance.language")}</label>
    <select
      id="locale"
      class="select"
      value={i18n.locale}
      onchange={onLocaleChange}
    >
      {#each LOCALES as locale (locale)}
        <option value={locale}>{localeLabels[locale]}</option>
      {/each}
    </select>
    <p class="hint">{i18n.t("settings.appearance.languageHint")}</p>
  </div>

  <div class="field">
    <label for="vibrancy-material">{i18n.t("settings.appearance.vibrancy")}</label>
    <select
      id="vibrancy-material"
      class="select"
      value={ui.vibrancyMaterial}
      onchange={onVibrancyChange}
    >
      {#each VIBRANCY_MATERIALS as m (m)}
        <option value={m}>{m}</option>
      {/each}
    </select>
    <p class="hint">{i18n.t("settings.appearance.vibrancyHint")}</p>
  </div>

</div>

<style>
  .section { display: flex; flex-direction: column; gap: var(--space-5); max-width: 520px; }
  h2 {
    font-size: var(--text-h1);
    font-weight: var(--fw-semibold);
    color: var(--color-text-primary);
    margin-bottom: var(--space-2);
  }
  .field { display: flex; flex-direction: column; gap: var(--space-2); }
  label {
    font-size: var(--text-body);
    font-weight: var(--fw-medium);
    color: var(--color-text-primary);
  }
  .hint {
    font-size: var(--text-body-sm);
    color: var(--color-text-muted);
    line-height: var(--lh-snug);
  }
  .radio-row {
    display: inline-flex;
    gap: 2px;
    padding: 2px;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background: var(--color-surface-sunken);
    width: max-content;
  }
  .radio-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    border-radius: var(--radius-sm);
    color: var(--color-text-secondary);
    font-size: var(--text-body-sm);
    font-weight: var(--fw-medium);
    cursor: pointer;
    transition: background-color var(--motion-duration-fast) var(--motion-ease-out);
  }
  .radio-btn:hover { color: var(--color-text-primary); }
  .radio-btn.on {
    background: var(--color-surface-raised);
    color: var(--color-text-primary);
    box-shadow: var(--shadow-xs);
  }
  .select {
    width: 100%;
    max-width: 260px;
    padding: 6px var(--space-3);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background: var(--color-surface-raised);
    color: var(--color-text-primary);
    font-size: var(--text-body);
    font-family: var(--font-sans);
    cursor: pointer;
  }
  .select:focus-visible {
    outline: none;
    border-color: var(--color-border-focus);
    box-shadow: var(--shadow-focus-ring);
  }

</style>
