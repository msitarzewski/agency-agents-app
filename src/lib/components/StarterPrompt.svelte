<script lang="ts">
  /**
   * StarterPrompt — a copy-to-clipboard prompt chip. The reusable unit behind
   * the Playbook, the empty-state nudges, and the Team detail examples. Renders
   * a label + the prompt text with a Copy button that flips to "Copied".
   */
  import { onDestroy } from "svelte";
  import CopyIcon from "@lucide/svelte/icons/copy";
  import CheckIcon from "@lucide/svelte/icons/check";
  import { toast } from "$lib/stores/toast.svelte";
  import { i18n } from "$lib/stores/i18n.svelte";

  interface Props {
    /** The prompt text that gets copied. */
    template: string;
    /** Optional short title above the prompt. */
    label?: string;
    /** Optional one-liner under the label. */
    description?: string;
  }
  let { template, label, description }: Props = $props();

  let copied = $state(false);
  let resetTimer: ReturnType<typeof setTimeout> | undefined;

  async function copy() {
    try {
      await navigator.clipboard.writeText(template);
      copied = true;
      toast.success(i18n.t("common.promptCopied"));
      clearTimeout(resetTimer);
      resetTimer = setTimeout(() => (copied = false), 1600);
    } catch {
      toast.error(i18n.t("common.copyFailed"), i18n.t("common.clipboardUnavailable"));
    }
  }

  onDestroy(() => clearTimeout(resetTimer));
</script>

<div class="sp">
  <div class="sp-head">
    {#if label || description}
      <div class="sp-id">
        {#if label}<span class="sp-label">{label}</span>{/if}
        {#if description}<span class="sp-desc">{description}</span>{/if}
      </div>
    {/if}
    <button
      class="sp-copy"
      class:done={copied}
      onclick={copy}
      aria-live="polite"
      aria-label={copied ? i18n.t("starter.copiedAria") : label ? i18n.t("starter.copyLabelAria", { label }) : i18n.t("starter.copyAria")}
    >
      {#if copied}<CheckIcon size={13} /> {i18n.t("common.copied")}{:else}<CopyIcon size={13} /> {i18n.t("common.copy")}{/if}
    </button>
  </div>
  <p class="sp-template">{template}</p>
</div>

<style>
  .sp {
    border: 1px solid var(--color-border); border-radius: var(--radius-md);
    background: var(--color-surface-sunken); padding: var(--space-3);
    display: flex; flex-direction: column; gap: var(--space-2);
  }
  .sp-head { display: flex; align-items: flex-start; gap: var(--space-3); }
  .sp-id { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 1px; }
  .sp-label { font-size: var(--text-body-sm); font-weight: var(--fw-semibold); color: var(--color-text-primary); }
  .sp-desc { font-size: var(--text-caption); color: var(--color-text-muted); }
  .sp-copy {
    flex: none; display: inline-flex; align-items: center; gap: 5px;
    height: 26px; padding: 0 10px; border-radius: var(--radius-sm);
    border: 1px solid var(--color-border); background: var(--color-surface-raised);
    color: var(--color-text-secondary); font-size: var(--text-caption); cursor: pointer;
    margin-left: auto;
  }
  .sp-copy:hover { color: var(--color-text-primary); border-color: var(--color-brand); }
  .sp-copy.done { color: var(--color-success); border-color: var(--color-success); }
  .sp-template {
    font-family: var(--font-mono, ui-monospace, monospace);
    font-size: var(--text-caption); line-height: var(--lh-normal);
    color: var(--color-text-secondary); white-space: pre-wrap; word-break: break-word; margin: 0;
  }
</style>
