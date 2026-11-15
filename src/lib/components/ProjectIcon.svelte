<script lang="ts">
  import { hues } from '$lib/state.svelte';
  let { url = null, title, size = 48 }: { url?: string | null; title: string; size?: number } = $props();
  let failed = $state(false);
  const pair = $derived(hues(title));
</script>

<div class="icon" style:width="{size}px" style:height="{size}px" style:border-radius="{Math.round(size * 0.26)}px">
  {#if url && !failed}
    <img src={url} alt="" loading="lazy" onerror={() => (failed = true)} />
  {:else}
    <span style:background="hsl({pair[0]} 18% 19%)" style:color="hsl({pair[0]} 45% 78%)" style:font-size="{size * 0.42}px">
      {title.trim()[0]?.toUpperCase() ?? '?'}
    </span>
  {/if}
</div>

<style>
  .icon {
    flex-shrink: 0;
    overflow: hidden;
    background: rgba(255, 255, 255, 0.05);
    box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.08) inset;
  }
  img,
  span {
    width: 100%;
    height: 100%;
    display: grid;
    place-items: center;
    object-fit: cover;
  }
  span {
    font-family: var(--font-display);
    font-weight: 700;
  }
</style>
