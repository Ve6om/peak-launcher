<script lang="ts">
  // A mod loader's logo (or Peak's grass block for vanilla).
  import { LOADER_ICONS, PIXEL_ART } from '$lib/loaders';

  let { loader, size = 18 }: { loader: string; size?: number } = $props();

  const src = $derived(LOADER_ICONS[loader]);
</script>

{#if src}
  <img {src} alt="" width={size} height={size} draggable="false" class:pixel={PIXEL_ART.has(loader)} />
{/if}

<style>
  img {
    display: block;
    flex-shrink: 0;
    object-fit: contain;
  }
  .pixel {
    image-rendering: pixelated;
  }
</style>
