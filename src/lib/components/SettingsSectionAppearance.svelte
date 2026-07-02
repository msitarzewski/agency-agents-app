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
  import { i18n, type TranslationKey } from "$lib/i18n.svelte";
  import type { LocalePreference, SidebarSection, ThemePreference } from "$lib/types";

  /** Sections the user can pick as their default landing page. Mirrors the
      sidebar nav order, plus Dashboard which lives in the brand button. */
  const SECTIONS: { value: SidebarSection; labelKey: TranslationKey }[] = [
    { value: "dashboard", labelKey: "nav.dashboard" },
    { value: "personas", labelKey: "nav.agents" },
    { value: "tools", labelKey: "nav.tools" },
    { value: "teams", labelKey: "nav.teams" },
    { value: "projects", labelKey: "nav.projects" },
    { value: "activity", labelKey: "nav.activity" },
  ];

  const LOCALES: { value: LocalePreference; labelKey: TranslationKey }[] = [
    { value: "system", labelKey: "locale.system" },
    { value: "en", labelKey: "locale.en" },
    { value: "ru", labelKey: "locale.ru" },
  ];

  function onSectionChange(e: Event) {
    const value = (e.currentTarget as HTMLSelectElement).value as SidebarSection;
    ui.setDefaultSection(value);
  }
  function onVibrancyChange(e: Event) {
    const value = (e.currentTarget as HTMLSelectElement).value as VibrancyMaterial;
    ui.setVibrancyMaterial(value);
  }
  function onLocaleChange(e: Event) {
    const value = (e.currentTarget as HTMLSelectElement).value as LocalePreference;
    i18n.setPreference(value);
  }
  function pickTheme(t: ThemePreference) { ui.setTheme(t); }
</script>

<div class="section">
  <h2>{i18n.t("appearance.title")}</h2>

  <div class="field">
    <label for="theme-group">{i18n.t("appearance.theme")}</label>
    <div id="theme-group" class="radio-row" role="radiogroup" aria-label={i18n.t("appearance.themeAria")}>
      <button
        type="button"
        class="radio-btn"
        class:on={ui.theme === "light"}
        role="radio"
        aria-checked={ui.theme === "light"}
        onclick={() => pickTheme("light")}
      >
        <Sun size={14} /> {i18n.t("theme.light")}
      </button>
      <button
        type="button"
        class="radio-btn"
        class:on={ui.theme === "dark"}
        role="radio"
        aria-checked={ui.theme === "dark"}
        onclick={() => pickTheme("dark")}
      >
        <Moon size={14} /> {i18n.t("theme.dark")}
      </button>
      <button
        type="button"
        class="radio-btn"
        class:on={ui.theme === "system"}
        role="radio"
        aria-checked={ui.theme === "system"}
        onclick={() => pickTheme("system")}
      >
        <Monitor size={14} /> {i18n.t("theme.system")}
      </button>
    </div>
    <p class="hint">{i18n.t("appearance.themeHint")}</p>
  </div>

  <div class="field">
    <label for="locale">{i18n.t("appearance.language")}</label>
    <select
      id="locale"
      class="select"
      value={i18n.preference}
      onchange={onLocaleChange}
    >
      {#each LOCALES as opt (opt.value)}
        <option value={opt.value}>{i18n.t(opt.labelKey)}</option>
      {/each}
    </select>
    <p class="hint">{i18n.t("appearance.languageHint")}</p>
  </div>

  <div class="field">
    <label for="default-section">{i18n.t("appearance.defaultLanding")}</label>
    <select
      id="default-section"
      class="select"
      value={ui.defaultSection}
      onchange={onSectionChange}
    >
      {#each SECTIONS as opt (opt.value)}
        <option value={opt.value}>{i18n.t(opt.labelKey)}</option>
      {/each}
    </select>
    <p class="hint">{i18n.t("appearance.defaultLandingHint")}</p>
  </div>

  <div class="field">
    <label for="vibrancy-material">{i18n.t("appearance.vibrancy")}</label>
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
    <p class="hint">{i18n.t("appearance.vibrancyHint")}</p>
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
