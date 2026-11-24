<script lang="ts">
  // App background. Two layers you can mix in Settings:
  //  - aurora:   slowly drifting violet / cyan / pink glow (matches the art)
  //  - contours: a quiet topographic map, every fifth line a little stronger
  import { app } from '$lib/state.svelte';
  import { view } from '$lib/visibility.svelte';

  const style = $derived(app.bgPreview ?? app.settings?.background ?? 'aurora');
  const aurora = $derived(style === 'aurora' || style === 'both');
  const contoursOn = $derived(style === 'contours' || style === 'both');
  // Lighter effects: the glow stays still and panels lose their blur.
  const light = $derived(app.lightPreview ?? app.settings?.lightEffects ?? false);
  const moving = $derived(!light && (app.glowPreview ?? app.settings?.movingGlow ?? true));
  // The glow only moves while you can see Peak and are using it; otherwise it
  // sits still and costs nothing.
  const paused = $derived(!moving || !view.shown || !view.focused);
  // Linux's web view redoes a 90px blur whenever anything near it repaints, so
  // there the glow is drawn as gradients shaped exactly like the blurred
  // circles instead. Same picture, a fraction of the work.
  const baked = $derived(app.info?.os === 'linux' || light);
  $effect(() => {
    document.documentElement.classList.toggle('light-fx', light);
  });
  $effect(() => {
    document.documentElement.classList.toggle('linux-fx', app.info?.os === 'linux');
  });

  type Peak = { x: number; y: number; rings: number; step: number; seed: number };
  const peaks: Peak[] = [
    { x: 1260, y: 170, rings: 22, step: 26, seed: 1.3 },
    { x: 240, y: 880, rings: 16, step: 30, seed: 4.1 },
    { x: 760, y: 560, rings: 9, step: 22, seed: 2.7 }
  ];

  function ring(p: Peak, k: number) {
    const base = p.step * (k + 1);
    const pts: string[] = [];
    const n = 96;
    for (let i = 0; i <= n; i++) {
      const t = (i / n) * Math.PI * 2;
      const wobble =
        1 +
        0.13 * Math.sin(3 * t + p.seed + k * 0.11) +
        0.07 * Math.sin(5 * t + p.seed * 2 - k * 0.07) +
        0.04 * Math.sin(9 * t + p.seed * 3);
      const r = base * wobble;
      pts.push(`${(p.x + Math.cos(t) * r * 1.25).toFixed(1)},${(p.y + Math.sin(t) * r * 0.85).toFixed(1)}`);
    }
    return 'M' + pts.join('L') + 'Z';
  }

  const contours = peaks.flatMap((p) => Array.from({ length: p.rings }, (_, k) => ({ d: ring(p, k), index: (k + 1) % 5 === 0 })));
</script>

<div class="bg" class:paused class:baked aria-hidden="true">
  {#if aurora}
    <div class="blob violet"></div>
    <div class="blob cyan"></div>
    <div class="blob pink"></div>
    <div class="blob teal"></div>
  {/if}
  {#if contoursOn}
    <svg viewBox="0 0 1600 1000" preserveAspectRatio="xMidYMid slice" class:over-aurora={aurora}>
      {#each contours as c, i (i)}
        <path d={c.d} class:index={c.index} />
      {/each}
    </svg>
  {/if}
  {#if !aurora}
    <div class="glow"></div>
  {/if}
  <div class="vignette"></div>
</div>

<style>
  .bg {
    position: fixed;
    inset: 0;
    z-index: 0;
    overflow: hidden;
    background: radial-gradient(1200px 800px at 70% -10%, #13152a 0%, transparent 60%), var(--bg);
  }

  /* Aurora: fixed colours so it stays colourful whatever the accent is. */
  .blob {
    position: absolute;
    border-radius: 50%;
    filter: blur(90px);
    will-change: transform;
  }
  .violet {
    --r: 320px;
    --c: #8b5cf6;
    width: 640px;
    height: 640px;
    left: -160px;
    top: -220px;
    background: #8b5cf6;
    opacity: 0.42;
    animation: drift1 26s ease-in-out infinite alternate;
  }
  .cyan {
    --r: 290px;
    --c: #22d3ee;
    width: 580px;
    height: 580px;
    right: -180px;
    top: 18%;
    background: #22d3ee;
    opacity: 0.26;
    animation: drift2 32s ease-in-out infinite alternate;
  }
  .pink {
    --r: 260px;
    --c: #ec4899;
    width: 520px;
    height: 520px;
    left: 30%;
    bottom: -320px;
    background: #ec4899;
    opacity: 0.17;
    animation: drift1 38s ease-in-out infinite alternate-reverse;
  }
  .teal {
    --r: 210px;
    --c: #10b981;
    width: 420px;
    height: 420px;
    left: 18%;
    top: 35%;
    background: #10b981;
    opacity: 0.1;
    animation: drift2 44s ease-in-out infinite alternate;
  }
  .paused .blob {
    animation-play-state: paused;
    will-change: auto;
  }
  /* A circle blurred by 90px, drawn directly: a gradient following the blur's
     falloff (the normal curve), spreading 3 blur-widths (270px) past the edge. */
  .baked .blob {
    filter: none;
    background: none;
  }
  .baked .blob::before {
    content: '';
    position: absolute;
    inset: -270px;
    border-radius: 50%;
    background: radial-gradient(
      circle closest-side,
      var(--c) 0,
      color-mix(in srgb, var(--c) 97.7%, transparent) calc(var(--r) - 180px),
      color-mix(in srgb, var(--c) 93.3%, transparent) calc(var(--r) - 135px),
      color-mix(in srgb, var(--c) 84.1%, transparent) calc(var(--r) - 90px),
      color-mix(in srgb, var(--c) 69.1%, transparent) calc(var(--r) - 45px),
      color-mix(in srgb, var(--c) 50%, transparent) var(--r),
      color-mix(in srgb, var(--c) 30.9%, transparent) calc(var(--r) + 45px),
      color-mix(in srgb, var(--c) 15.9%, transparent) calc(var(--r) + 90px),
      color-mix(in srgb, var(--c) 6.7%, transparent) calc(var(--r) + 135px),
      color-mix(in srgb, var(--c) 2.3%, transparent) calc(var(--r) + 180px),
      color-mix(in srgb, var(--c) 0.6%, transparent) calc(var(--r) + 225px),
      transparent calc(var(--r) + 270px)
    );
  }
  @keyframes drift1 {
    to {
      transform: translate(140px, 90px) scale(1.12);
    }
  }
  @keyframes drift2 {
    to {
      transform: translate(-120px, -70px) scale(0.92);
    }
  }

  svg {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
  }
  path {
    fill: none;
    stroke: rgba(255, 255, 255, 0.035);
    stroke-width: 1;
    vector-effect: non-scaling-stroke;
  }
  path.index {
    stroke: rgba(255, 255, 255, 0.07);
  }
  /* Slightly brighter lines so they read over the glow. */
  .over-aurora path {
    stroke: rgba(255, 255, 255, 0.045);
  }
  .over-aurora path.index {
    stroke: rgba(255, 255, 255, 0.09);
  }

  .glow {
    position: absolute;
    right: -10%;
    top: -25%;
    width: 70%;
    height: 80%;
    background: radial-gradient(closest-side, color-mix(in srgb, var(--accent) 9%, transparent), transparent);
  }
  .vignette {
    position: absolute;
    inset: 0;
    background: radial-gradient(ellipse at 60% 30%, transparent 45%, rgba(0, 0, 0, 0.5));
  }
  @media (prefers-reduced-motion: reduce) {
    .blob {
      animation: none;
    }
  }
</style>
