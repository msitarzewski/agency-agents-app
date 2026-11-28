<script lang="ts">
  import appIcon from "$lib/assets/app-icon.png";
  import appIconDark from "$lib/assets/app-icon-dark.png";

  let { size }: { size: number } = $props();
</script>

<span aria-hidden="true">
  <img class="light" src={appIcon} alt="" width={size} height={size} />
  <img class="dark" src={appIconDark} alt="" width={size} height={size} />
</span>

<style>
  span { display: inline-flex; flex: none; vertical-align: middle; }
  img { display: block; }
  .dark { display: none; }
  :global([data-theme="dark"]) .light { display: none; }
  :global([data-theme="dark"]) .dark { display: block; }
</style>
