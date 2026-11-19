<script lang="ts">
  // A 3D model of a skin that turns slowly and can be dragged around.
  // skinview3d (and three.js) load only when this first appears.
  import { onMount } from 'svelte';

  let { png, model = 'classic', width = 220, height = 300 }: { png: string; model?: string; width?: number; height?: number } = $props();

  let canvas = $state<HTMLCanvasElement>();
  let failed = $state(false);
  // The skinview3d viewer, kept loosely typed so the library stays a lazy import.
  let viewer: any = null;

  onMount(() => {
    let disposed = false;
    import('skinview3d')
      .then((sv) => {
        if (disposed || !canvas) return;
        viewer = new sv.SkinViewer({ canvas, width, height });
        viewer.zoom = 0.82;
        viewer.fov = 40;
        viewer.autoRotate = true;
        viewer.autoRotateSpeed = 0.6;
        viewer.controls.enableZoom = false;
        viewer.controls.enablePan = false;
        viewer.animation = new sv.IdleAnimation();
        viewer.background = null;
        viewer.loadSkin(png, { model: model === 'slim' ? 'slim' : 'default' });
      })
      .catch(() => (failed = true));
    return () => {
      disposed = true;
      viewer?.dispose();
      viewer = null;
    };
  });

  // A different skin or arm model: reload it into the same viewer.
  $effect(() => {
    const src = png;
    const arms = model;
    viewer?.loadSkin(src, { model: arms === 'slim' ? 'slim' : 'default' });
  });
</script>

{#if failed}
  <img class="flat" src={png} alt="Skin" />
{:else}
  <canvas bind:this={canvas} {width} {height} aria-label="3D view of the skin. Drag to turn it."></canvas>
{/if}

<style>
  canvas {
    display: block;
    cursor: grab;
    touch-action: none;
  }
  canvas:active {
    cursor: grabbing;
  }
  .flat {
    width: 128px;
    image-rendering: pixelated;
  }
</style>
