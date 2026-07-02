<script lang="ts">
  import type { Snippet } from "svelte";
  import Modal from "./Modal.svelte";
  import Button from "./Button.svelte";
  import { i18n } from "$lib/stores/i18n.svelte";

  interface Props {
    open: boolean;
    title: string;
    confirmLabel?: string;
    cancelLabel?: string;
    /** "danger" for true destructive ops, "primary" for additive ones like Restore */
    confirmVariant?: "danger" | "primary";
    confirmDisabled?: boolean;
    onConfirm: () => void;
    onCancel: () => void;
    children?: Snippet;
  }

  let {
    open,
    title,
    confirmLabel,
    cancelLabel,
    confirmVariant = "danger",
    confirmDisabled = false,
    onConfirm,
    onCancel,
    children,
  }: Props = $props();

  const confirmText = $derived(confirmLabel ?? i18n.t("common.confirm"));
  const cancelText = $derived(cancelLabel ?? i18n.t("common.cancel"));
</script>

<Modal {open} {title} defaultFocus="cancel" onClose={onCancel}>
  {#if children}
    {@render children()}
  {/if}
  {#snippet actions()}
    <Button variant="secondary" modalAction="cancel" onclick={onCancel}>{cancelText}</Button>
    <Button variant={confirmVariant} modalAction="confirm" disabled={confirmDisabled} onclick={onConfirm}>{confirmText}</Button>
  {/snippet}
</Modal>
