<script lang="ts">
  import { t } from '$lib/i18n.svelte';
  // Logo for a content source. Modrinth and CurseForge use their official
  // brand marks (Simple Icons); FTB and Technic use their site icons.
  import { siModrinth, siCurseforge } from 'simple-icons';
  import type { Source } from '$lib/types';

  let { source, size = 28 }: { source: Source; size?: number } = $props();

  const svg = { modrinth: siModrinth, curseforge: siCurseforge } as const;
  const img = { ftb: '/sources/ftb.png', technic: '/sources/technic.png' } as const;
</script>

<span class="logo" style:width="{size}px" style:height="{size}px" style:border-radius="{Math.round(size * 0.28)}px">
  {#if source === 'modrinth' || source === 'curseforge'}
    {@const icon = svg[source]}
    <svg viewBox="0 0 24 24" width={size * 0.62} height={size * 0.62} aria-label={icon.title} role="img">
      <path d={icon.path} fill="#{icon.hex}" />
    </svg>
  {:else}
    <img src={img[source]} alt={source === 'ftb' ? t('Feed The Beast') : t('Technic')} width={size * 0.78} height={size * 0.78} />
  {/if}
</span>

<style>
  .logo {
    display: inline-grid;
    place-items: center;
    flex-shrink: 0;
    background: rgba(255, 255, 255, 0.06);
    box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.08) inset;
  }
  img {
    object-fit: contain;
    image-rendering: auto;
  }
</style>
