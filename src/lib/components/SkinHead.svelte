<script lang="ts">
  // A player's face from their skin: the head's front plus the hat layer on
  // top, kept pixel-sharp. Falls back to the first letter of the name.
  import { skins, loadSkin } from '$lib/skins.svelte';

  let {
    accountId,
    name,
    size = 34,
    png: given = null
  }: { accountId: string | null; name: string; size?: number; /** A skin image to use instead of the account's. */ png?: string | null } = $props();

  let canvas = $state<HTMLCanvasElement>();
  let drawn = $state(false);

  const skin = $derived(accountId ? skins[accountId] : null);

  // Load whenever the account changes (e.g. switching the active account).
  $effect(() => {
    if (accountId && !given) loadSkin(accountId);
  });

  $effect(() => {
    const png = given ?? skin?.png;
    drawn = false;
    if (!png || !canvas) return;
    const img = new Image();
    img.onload = () => {
      const ctx = canvas?.getContext('2d');
      if (!ctx) return;
      ctx.imageSmoothingEnabled = false;
      ctx.clearRect(0, 0, 8, 8);
      ctx.drawImage(img, 8, 8, 8, 8, 0, 0, 8, 8); // face
      ctx.drawImage(img, 40, 8, 8, 8, 0, 0, 8, 8); // hat layer
      drawn = true;
    };
    img.src = png;
  });
</script>

<span class="head" style:width="{size}px" style:height="{size}px" style:font-size="{Math.round(size * 0.42)}px">
  <canvas bind:this={canvas} width="8" height="8" class:show={drawn}></canvas>
  {#if !drawn}<span class="letter">{name ? name[0].toUpperCase() : '?'}</span>{/if}
</span>

<style>
  .head {
    position: relative;
    display: inline-grid;
    place-items: center;
    flex-shrink: 0;
    border-radius: 9px;
    overflow: hidden;
    background: var(--accent);
    color: var(--on-accent);
    font-weight: 750;
  }
  canvas {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    image-rendering: pixelated;
    opacity: 0;
  }
  canvas.show {
    opacity: 1;
  }
  .letter {
    position: relative;
  }
</style>
