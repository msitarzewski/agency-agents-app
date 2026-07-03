<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { ui, watchSystemTheme } from "$lib/stores/ui.svelte";
  import { install } from "$lib/stores/install.svelte";
  import { activity } from "$lib/stores/activity.svelte";
  import { settings } from "$lib/stores/settings.svelte";
  import { catalog } from "$lib/stores/catalog.svelte";
  import { i18n } from "$lib/stores/i18n.svelte";
  import CatalogFirstRun from "$lib/components/CatalogFirstRun.svelte";

  let { children } = $props();

  onMount(() => {
    i18n.init();
    ui.loadThemeFromStorage();
    // Settings (Phase 12b) — all read with enum/numeric validation so a
    // corrupt or hostile localStorage entry can't poison runtime state.
    ui.loadDefaultSectionFromStorage();
    ui.loadVibrancyMaterialFromStorage();
    ui.loadConfirmDestructiveFromStorage();
    ui.loadActivitySettingsFromStorage();
    ui.loadSidebarCollapsedFromStorage();
    ui.loadSidebarWidthFromStorage();
    ui.loadDetailPaneWidthFromStorage();
    // Seed back/forward history with the landing location (after the default
    // section has been resolved above), so the first entry is real.
    ui.initNav();
    activity.hydrate();
    // Install state — reconcile ONCE here at the app root, not inside the view
    // components. A view that both reads install.* state AND triggers a mutation
    // (reconcile) during its own setup froze its reactivity (Library bug). Views
    // are now pure readers; Rescan buttons re-trigger on user action.
    void install.reconcile();
    void install.loadTools();
    install.loadSelection();
    // Phase 12d — hydrate the persisted settings.json into the renderer
    // so the Network section, the Catalog stale banner, and the cask
    // icon mode all read from one source of truth.
    void settings.load();
    // Catalog source (#1) — load the persisted choice; if none has been made
    // the first-run picker renders over the app until the user chooses.
    void catalog.load();
    // NOTE: GitHub sign-in status is intentionally NOT hydrated here.
    // `github.loadStatus()` reads from macOS Keychain, which prompts
    // the user the first time a new app binary tries to access an
    // existing entry. Probing on every app launch trains users to
    // dismiss the prompt without reading it, and is intrusive when
    // they have no intention of using GitHub features.
    // Instead: probe lazily — `requireGithubSignIn()` (in PackageDetail)
    // awaits loadStatus on first action click, and Settings → GitHub
    // calls loadStatus when its panel mounts. Both contexts are
    // user-initiated, so a Keychain prompt is contextual and expected.

    // Native macOS menu bridge — Rust emits `menu:about` / `menu:settings`
    // when the user picks those items from the App menu in the system menu
    // bar; we just open the corresponding modal. The Cmd+, accelerator is
    // also bound on the Settings menu item so both surfaces stay in sync
    // with the in-app shortcut already handled in `+page.svelte`.
    let unlistenAbout: UnlistenFn | undefined;
    let unlistenSettings: UnlistenFn | undefined;
    void listen("menu:about", () => { ui.openAbout(); }).then((u) => { unlistenAbout = u; });
    void listen("menu:settings", () => { ui.openSettings(); }).then((u) => { unlistenSettings = u; });

    const unwatch = watchSystemTheme(() => ui.theme);
    return () => {
      unwatch();
      unlistenAbout?.();
      unlistenSettings?.();
    };
  });
</script>

<!--
  Window dragging in Tauri 2 with titleBarStyle: "Overlay" is wired via the
  `data-tauri-drag-region` attribute on regular DOM elements (Sidebar brand
  area + each panel-head). Tauri's WebView handles click-vs-drag detection
  natively, so interactive children inside drag regions still receive their
  clicks. Avoids the fixed-overlay pattern (which intercepts scroll-wheel
  events at the top of the window).
-->

{@render children()}

{#if !catalog.configured}
  <CatalogFirstRun />
{/if}
