<script lang="ts">
  // A mod loader's logo (or Peak's grass block for vanilla).
  let { loader, size = 18 }: { loader: string; size?: number } = $props();

  const files: Record<string, string> = {
    vanilla: '/loaders/vanilla.svg',
    fabric: '/loaders/fabric.png',
    quilt: '/loaders/quilt.png',
    neoforge: '/loaders/neoforge.png',
    forge: '/loaders/forge.png'
  };
  // Pixel-art logos stay sharp instead of going blurry when scaled.
  const pixelArt = new Set(['fabric', 'quilt', 'neoforge']);
  const src = $derived(files[loader]);
</script>

{#if src}
  <img {src} alt="" width={size} height={size} draggable="false" class:pixel={pixelArt.has(loader)} />
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
