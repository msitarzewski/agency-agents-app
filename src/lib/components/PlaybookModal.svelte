<script lang="ts">
  /**
   * PlaybookModal — "how to get real work out of your agents." The flagship
   * surface for the Playbook content (practices + copyable starter prompts).
   * Opened from the title-bar ? button and the command palette.
   */
  import Modal from "./Modal.svelte";
  import Button from "./Button.svelte";
  import StarterPrompt from "./StarterPrompt.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { i18n } from "$lib/i18n.svelte";
  import { playbookPractices, starterPrompts } from "$lib/data/playbook";

  const practices = $derived(playbookPractices(i18n.locale));
  const prompts = $derived(starterPrompts(i18n.locale));
</script>

<Modal open={ui.playbookOpen} title={i18n.t("playbook.title")} size="wide" onClose={() => ui.closePlaybook()}>
  <div class="pb">
    <p class="intro">
      {i18n.t("playbook.intro")}
    </p>

    <ol class="practices">
      {#each practices as p, i (p.title)}
        <li>
          <span class="num">{i + 1}</span>
          <div class="p-body">
            <span class="p-title">{p.title}</span>
            <p>{p.body}</p>
          </div>
        </li>
      {/each}
    </ol>

    <h2 class="sec">{i18n.t("playbook.starterPrompts")}</h2>
    <p class="sec-sub">{i18n.t("playbook.starterSub")}</p>
    <div class="starters">
      {#each prompts as s (s.id)}
        <StarterPrompt label={s.label} description={s.description} template={s.template} />
      {/each}
    </div>
  </div>

  {#snippet actions()}
    <Button variant="primary" onclick={() => ui.closePlaybook()}>{i18n.t("playbook.gotIt")}</Button>
  {/snippet}
</Modal>

<style>
  .pb { max-height: 62vh; overflow-y: auto; padding-right: var(--space-1); }
  .intro { font-size: var(--text-body); color: var(--color-text-secondary); line-height: var(--lh-normal); margin-bottom: var(--space-4); }

  .practices { list-style: none; margin: 0 0 var(--space-5); padding: 0; display: flex; flex-direction: column; gap: var(--space-3); }
  .practices li { display: flex; gap: var(--space-3); }
  .num {
    flex: none; display: inline-flex; align-items: center; justify-content: center;
    width: 24px; height: 24px; border-radius: 999px;
    background: var(--color-brand); color: var(--color-text-inverse);
    font-size: var(--text-caption); font-weight: var(--fw-bold);
  }
  .p-body { flex: 1; min-width: 0; }
  .p-body .p-title { display: block; font-size: var(--text-body); font-weight: var(--fw-semibold); color: var(--color-text-primary); margin-bottom: 2px; }
  .p-body p { font-size: var(--text-body-sm); color: var(--color-text-secondary); line-height: var(--lh-normal); }

  .sec { font-size: var(--text-body-sm); font-weight: var(--fw-semibold); color: var(--color-text-muted); text-transform: uppercase; letter-spacing: 0.04em; margin-bottom: 2px; }
  .sec-sub { font-size: var(--text-body-sm); color: var(--color-text-secondary); margin-bottom: var(--space-3); }
  .starters { display: flex; flex-direction: column; gap: var(--space-2); }
</style>
