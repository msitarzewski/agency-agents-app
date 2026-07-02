<script lang="ts">
  import { onMount } from "svelte";
  import X from "@lucide/svelte/icons/x";
  import Heart from "@lucide/svelte/icons/heart";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import Sparkles from "@lucide/svelte/icons/sparkles";

  import { ui } from "$lib/stores/ui.svelte";
  import { appVersion } from "$lib/api";
  import { safeOpenUrl } from "$lib/util/url";
  import { SPONSOR_URL } from "$lib/util/donate";
  import { i18n } from "$lib/i18n.svelte";
  import appIcon from "$lib/assets/app-icon.png";

  const REPO_URL = "https://github.com/msitarzewski/agency-agents-app";
  const LICENSE_URL = "https://github.com/msitarzewski/agency-agents-app/blob/main/LICENSE";

  let appVer = $state<string | null>(null);
  let modalEl: HTMLDivElement | undefined = $state();

  // Fetch app version lazily; only meaningful when this modal is open.
  $effect(() => {
    if (ui.aboutOpen && appVer === null) {
      void appVersion()
        .then((v) => { appVer = v; })
        .catch(() => { appVer = "unknown"; });
    }
  });

  // Esc-to-close + focus management — same pattern as Settings.svelte.
  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape" && ui.aboutOpen) {
      e.stopPropagation();
      ui.closeAbout();
    }
  }

  function onBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) ui.closeAbout();
  }

  onMount(() => {
    window.addEventListener("keydown", onKeydown);
    return () => window.removeEventListener("keydown", onKeydown);
  });

  function openSponsor()        { void safeOpenUrl(SPONSOR_URL); }
  function openRepo()           { void safeOpenUrl(REPO_URL); }
  function openLicense()        { void safeOpenUrl(LICENSE_URL); }
</script>

{#if ui.aboutOpen}
  <div class="backdrop" onclick={onBackdropClick} role="presentation">
    <div
      class="modal"
      bind:this={modalEl}
      role="dialog"
      aria-modal="true"
      aria-labelledby="about-title"
      data-tauri-drag-region="false"
    >
      <button class="close" aria-label={i18n.t("about.close")} onclick={() => ui.closeAbout()}>
        <X size={16} />
      </button>

      <div class="hero">
        <img class="brand-mark" src={appIcon} alt="Agency Agents" width="80" height="80" />
        <h1 id="about-title">Agency Agents</h1>
        <p class="tagline">{i18n.t("app.tagline")}</p>
      </div>

      <dl class="meta">
        <div><dt>{i18n.t("about.version")}</dt><dd class="mono">{appVer ?? "…"}</dd></div>
        <div><dt>{i18n.t("about.license")}</dt><dd><button class="link" onclick={openLicense}>MIT</button></dd></div>
        <div><dt>{i18n.t("about.repo")}</dt><dd><button class="link" onclick={openRepo}>github.com/msitarzewski/agency-agents-app</button></dd></div>
      </dl>

      <button class="donate-cta" onclick={openSponsor} title={i18n.t("about.openSponsors")}>
        <Heart size={16} />
        <span>{i18n.t("about.donate")}</span>
        <ExternalLink size={12} />
      </button>

      <section class="credits">
        <h2><Sparkles size={14} /> {i18n.t("about.builtWith")}</h2>
        <p>{i18n.t("about.credits")}</p>
        <p class="thanks">
          {i18n.t("about.thanks")}
        </p>
      </section>

      <p class="posture text-muted">
        {i18n.t("about.posture")}
      </p>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 200;
    background: color-mix(in oklch, var(--color-surface) 60%, transparent);
    backdrop-filter: blur(6px);
    -webkit-backdrop-filter: blur(6px);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--space-4);
  }
  .modal {
    width: min(440px, 100%);
    max-height: min(85vh, 720px);
    overflow-y: auto;
    background: var(--color-surface-raised);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-lg, 14px);
    box-shadow: 0 24px 60px -12px color-mix(in oklch, black 40%, transparent);
    padding: var(--space-6, 28px);
    position: relative;
  }
  .close {
    position: absolute;
    top: var(--space-3);
    right: var(--space-3);
    color: var(--color-text-muted);
    padding: 4px;
    border-radius: var(--radius-sm);
  }
  .close:hover { background: var(--color-surface-sunken); color: var(--color-text-primary); }

  .hero {
    text-align: center;
    margin-bottom: var(--space-5, 22px);
  }
  .brand-mark {
    display: block;
    width: 80px;
    height: 80px;
    margin: 0 auto var(--space-2);
    /* The PNG already carries the rounded-square shape on transparent corners;
       drop-shadow follows the alpha so the shadow is squircle-shaped, not boxy. */
    filter: drop-shadow(0 6px 16px color-mix(in oklch, black 45%, transparent));
  }
  h1 {
    font-size: var(--text-h1, 1.4rem);
    font-weight: var(--fw-semibold);
    margin: 0;
  }
  .tagline {
    margin-top: 4px;
    color: var(--color-text-secondary);
    font-size: var(--text-body-sm);
  }

  .meta {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-3) var(--space-4);
    background: var(--color-surface-sunken);
    border-radius: var(--radius-md);
    margin-bottom: var(--space-4);
  }
  .meta > div {
    display: grid;
    grid-template-columns: 80px 1fr;
    gap: var(--space-3);
    font-size: var(--text-body-sm);
  }
  dt { color: var(--color-text-muted); }
  dd { color: var(--color-text-primary); margin: 0; min-width: 0; overflow-wrap: anywhere; }
  .mono { font-family: var(--font-mono); }

  .donate-cta {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-2);
    width: 100%;
    padding: var(--space-3);
    background: var(--color-brand);
    color: var(--color-text-inverse);
    border-radius: var(--radius-md);
    font-weight: var(--fw-semibold);
    font-size: var(--text-body);
    cursor: pointer;
    transition: filter 0.12s ease, transform 0.12s ease;
    margin-bottom: var(--space-4);
  }
  .donate-cta:hover { filter: brightness(1.05); transform: translateY(-1px); }

  .credits {
    border-top: 1px solid var(--color-border);
    padding-top: var(--space-4);
    margin-bottom: var(--space-4);
  }
  .credits h2 {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--text-body-sm);
    font-weight: var(--fw-semibold);
    color: var(--color-text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    margin: 0 0 var(--space-2);
  }
  .credits p {
    font-size: var(--text-body-sm);
    color: var(--color-text-secondary);
    margin: 0 0 var(--space-2);
    line-height: var(--lh-normal);
  }
  .thanks { font-style: italic; opacity: 0.85; }

  .link {
    background: transparent;
    color: var(--color-text-link);
    text-decoration: none;
    cursor: pointer;
    padding: 0;
    font: inherit;
  }
  .link:hover { text-decoration: underline; }

  .posture {
    text-align: center;
    font-size: var(--text-caption);
    margin: 0;
  }
</style>
