<script lang="ts">
  // Instance cover art. Modpacks show their own artwork (a wide banner, or
  // their icon over a blurred copy of itself); everything else gets a
  // generated mountain range in colours derived from the instance id.
  import { hues } from '$lib/state.svelte';

  let {
    seed,
    letter = '',
    rounded = true,
    image = null,
    icon = null
  }: { seed: string; letter?: string; rounded?: boolean; image?: string | null; icon?: string | null } = $props();

  let imageFailed = $state(false);
  let iconFailed = $state(false);
  const showImage = $derived(!!image && !imageFailed);
  const showIcon = $derived(!showImage && !!icon && !iconFailed);

  const pair = $derived(hues(seed));
  const uid = $derived('a' + seed.replace(/[^a-z0-9]/gi, ''));
  // Pseudo-random ridge offsets from the seed.
  const r = $derived.by(() => {
    let h = 0;
    for (const c of seed) h = (h * 31 + c.charCodeAt(0)) >>> 0;
    return Array.from({ length: 6 }, (_, i) => ((h >> (i * 4)) & 15) / 15);
  });
</script>

<div class="art" class:rounded>
  {#if showImage}
    <img class="cover" src={image} alt="" loading="lazy" draggable="false" onerror={() => (imageFailed = true)} />
  {:else if showIcon}
    <img class="cover filled" src={icon} alt="" loading="lazy" draggable="false" onerror={() => (iconFailed = true)} />
  {:else}
  <svg viewBox="0 0 400 225" preserveAspectRatio="xMidYMid slice" aria-hidden="true">
    <defs>
      <!-- Dusk-toned: low saturation so every instance sits in the same palette. -->
      <linearGradient id="{uid}-sky" x1="0" y1="0" x2="0.3" y2="1">
        <stop offset="0" stop-color="hsl({pair[0]} 22% 13%)" />
        <stop offset="1" stop-color="hsl({pair[1]} 30% 30%)" />
      </linearGradient>
      <linearGradient id="{uid}-m1" x1="0" y1="0" x2="0" y2="1">
        <stop offset="0" stop-color="hsl({pair[1]} 18% 34%)" />
        <stop offset="1" stop-color="hsl({pair[0]} 18% 15%)" />
      </linearGradient>
      <linearGradient id="{uid}-m2" x1="0" y1="0" x2="0" y2="1">
        <stop offset="0" stop-color="hsl({pair[0]} 16% 16%)" />
        <stop offset="1" stop-color="hsl({pair[0]} 18% 7%)" />
      </linearGradient>
    </defs>
    <rect width="400" height="225" fill="url(#{uid}-sky)" />
    <!-- Faint contour lines in the sky, echoing the app background. -->
    {#each [0, 1, 2, 3, 4] as k (k)}
      <ellipse
        cx={260 + r[0] * 90}
        cy={55 + r[1] * 25}
        rx={34 + k * 26}
        ry={20 + k * 15}
        fill="none"
        stroke="white"
        stroke-opacity="0.06"
      />
    {/each}
    <circle cx={260 + r[0] * 90} cy={55 + r[1] * 25} r="12" fill="hsl({pair[1]} 60% 85%)" opacity="0.55" />
    <path
      d="M0 170 L{60 + r[2] * 40} {95 + r[3] * 20} L{150 + r[4] * 30} 150 L{230 + r[5] * 30} {70 + r[0] * 25} L330 140 L400 {100 + r[1] * 30} L400 225 L0 225 Z"
      fill="url(#{uid}-m1)"
      opacity="0.9"
    />
    <path
      d="M0 200 L{90 + r[3] * 30} {140 + r[4] * 15} L190 185 L{280 + r[2] * 30} {125 + r[5] * 20} L400 190 L400 225 L0 225 Z"
      fill="url(#{uid}-m2)"
    />
  </svg>
  {/if}
  {#if letter}
    <span class="letter">{letter}</span>
  {/if}
</div>

<style>
  .art {
    position: relative;
    width: 100%;
    height: 100%;
    overflow: hidden;
  }
  .art.rounded {
    border-radius: inherit;
  }
  svg {
    display: block;
    width: 100%;
    height: 100%;
  }
  .cover,
  .filled {
    object-position: center 35%;
  }
  .backdrop {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .backdrop {
    filter: blur(22px) saturate(1.2) brightness(0.55);
    transform: scale(1.4);
  }
  .icon {
    position: absolute;
    left: 50%;
    top: 50%;
    width: 46%;
    max-width: 120px;
    aspect-ratio: 1;
    transform: translate(-50%, -50%);
    object-fit: cover;
    border-radius: 18%;
    box-shadow: 0 6px 24px rgba(0, 0, 0, 0.45);
  }
  .letter {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    font-weight: 750;
    font-size: 1.4em;
    color: white;
    text-shadow: 0 2px 10px rgba(0, 0, 0, 0.4);
  }
</style>
