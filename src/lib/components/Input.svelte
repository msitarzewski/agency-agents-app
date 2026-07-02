<script lang="ts">
  import type { Snippet } from "svelte";
  import Search from "@lucide/svelte/icons/search";
  import X from "@lucide/svelte/icons/x";
  import { i18n } from "$lib/i18n.svelte";

  type Size = "sm" | "md";
  type Variant = "default" | "search";

  interface Props {
    value: string;
    placeholder?: string;
    type?: string;
    size?: Size;
    variant?: Variant;
    disabled?: boolean;
    invalid?: boolean;
    ariaLabel?: string;
    onInput?: (v: string) => void;
    onKeydown?: (e: KeyboardEvent) => void;
    leading?: Snippet;
    trailing?: Snippet;
  }

  let {
    value = $bindable(),
    placeholder,
    type = "text",
    size = "md",
    variant = "default",
    disabled = false,
    invalid = false,
    ariaLabel,
    onInput,
    onKeydown,
    leading,
    trailing,
  }: Props = $props();

  function handleInput(e: Event) {
    const t = e.target as HTMLInputElement;
    value = t.value;
    onInput?.(t.value);
  }

  function clear() {
    value = "";
    onInput?.("");
  }
</script>

<div class="wrap size-{size}" class:invalid class:disabled>
  {#if variant === "search"}
    <span class="ico leading" aria-hidden="true"><Search size={14} /></span>
  {:else if leading}
    <span class="ico leading" aria-hidden="true">{@render leading()}</span>
  {/if}

  <input
    {type}
    {placeholder}
    {disabled}
    {value}
    aria-label={ariaLabel}
    oninput={handleInput}
    onkeydown={onKeydown}
  />

  {#if variant === "search" && value}
    <button type="button" class="ico trailing clear" aria-label={i18n.t("common.clear")} onclick={clear}>
      <X size={14} />
    </button>
  {:else if trailing}
    <span class="ico trailing" aria-hidden="true">{@render trailing()}</span>
  {/if}
</div>

<style>
  .wrap {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    border: 1px solid var(--color-border);
    background: var(--color-surface-raised);
    border-radius: var(--radius-md);
    padding: 0 var(--space-3);
    transition:
      border-color var(--motion-duration-fast) var(--motion-ease-out),
      box-shadow var(--motion-duration-fast) var(--motion-ease-out);
    min-width: 0;
  }
  .wrap:hover { border-color: var(--color-border-strong); }
  .wrap:focus-within {
    border-color: var(--color-border-focus);
    box-shadow: var(--shadow-focus-ring);
  }
  .size-sm { height: 26px; font-size: var(--text-body-sm); padding: 0 var(--space-2); }
  .size-md { height: 30px; font-size: var(--text-body); padding: 0 var(--space-3); }
  .invalid { border-color: var(--color-danger); }
  .disabled { background: var(--color-surface-sunken); }

  input {
    flex: 1;
    min-width: 0;
    background: transparent;
    color: var(--color-text-primary);
    line-height: 1;
  }
  input::placeholder { color: var(--color-text-muted); }
  input:disabled { color: var(--color-text-muted); }

  .ico {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--color-text-muted);
    flex: none;
  }
  .clear { cursor: pointer; border-radius: var(--radius-sm); padding: 2px; }
  .clear:hover { color: var(--color-text-primary); background: var(--color-surface-sunken); }
</style>
