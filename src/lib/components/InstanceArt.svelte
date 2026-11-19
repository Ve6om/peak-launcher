<script lang="ts">
  // Instance cover art. Modpacks show their own artwork (a wide banner, or
  // their icon); everything else shows its mod loader's logo (the grass block
  // for vanilla).
  import { loaderIcon, PIXEL_ART } from '$lib/loaders';

  let {
    loader = 'vanilla',
    letter = '',
    rounded = true,
    image = null,
    icon = null
  }: { loader?: string; letter?: string; rounded?: boolean; image?: string | null; icon?: string | null } = $props();

  let imageFailed = $state(false);
  let iconFailed = $state(false);
  const showImage = $derived(!!image && !imageFailed);
  const showIcon = $derived(!showImage && !!icon && !iconFailed);
</script>

<div class="art" class:rounded>
  {#if showImage}
    <img class="cover" src={image} alt="" loading="lazy" draggable="false" onerror={() => (imageFailed = true)} />
  {:else if showIcon}
    <img class="cover filled" src={icon} alt="" loading="lazy" draggable="false" onerror={() => (iconFailed = true)} />
  {:else}
    <div class="mark">
      <img src={loaderIcon(loader)} alt="" draggable="false" class:pixel={PIXEL_ART.has(loader)} />
    </div>
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
    container-type: size;
  }
  .art.rounded {
    border-radius: inherit;
  }
  .cover {
    position: absolute;
    inset: 0;
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
    object-position: center 35%;
    transition: transform 0.6s var(--ease);
  }
  .mark {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
  }
  .mark img {
    /* Most of the box's shorter side, so it fills cards, banners and thumbnails alike. */
    width: 78cqmin;
    height: 78cqmin;
    max-width: 180px;
    max-height: 180px;
    object-fit: contain;
    filter: drop-shadow(0 6px 18px rgba(0, 0, 0, 0.45));
    transition: transform 0.6s var(--ease);
  }
  /* Small thumbnails (sidebar): nearly edge to edge, no shadow. */
  @container (max-height: 64px) {
    .mark img {
      width: 100cqmin;
      height: 100cqmin;
      filter: none;
    }
  }
  .pixel {
    image-rendering: pixelated;
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
