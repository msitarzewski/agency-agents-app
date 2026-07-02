<script lang="ts">
  import { i18n } from "$lib/i18n.svelte";

  interface Props {
    rows?: number;
    label?: string;
  }
  let { rows = 6, label = i18n.t("common.loading") }: Props = $props();
</script>

<div class="loading" role="status" aria-live="polite">
  <span class="sr-only">{label}</span>
  {#each Array.from({ length: rows }) as _, i (i)}
    <div class="skeleton-row"></div>
  {/each}
</div>

<style>
  .loading {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    padding: var(--space-2);
  }
  .skeleton-row {
    height: 30px;
    border-radius: var(--radius-md);
    background: linear-gradient(
      90deg,
      var(--color-surface-sunken) 0%,
      var(--color-surface-raised) 50%,
      var(--color-surface-sunken) 100%
    );
    background-size: 200% 100%;
    animation: shimmer 1.5s ease-in-out infinite;
  }
  @keyframes shimmer {
    0%   { background-position: 200% 0; }
    100% { background-position: -200% 0; }
  }
  @media (prefers-reduced-motion: reduce) {
    .skeleton-row { animation: none; background: var(--color-surface-sunken); }
  }
</style>
