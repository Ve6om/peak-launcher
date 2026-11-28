<script lang="ts">
  import { t } from '$lib/i18n.svelte';
  import { Play, Clock, LoaderCircle } from '@lucide/svelte';
  import InstanceArt from './InstanceArt.svelte';
  import { instanceIcon, instanceBanner } from '$lib/instanceImages';
  import { versionStatus } from '$lib/packUpdates.svelte';
  import { app, go, play, ago, isRunning, isBusy, openInstanceMenu } from '$lib/state.svelte';
  import { loaderLabel, packVersion, type Instance } from '$lib/types';

  let { inst, index = 0 }: { inst: Instance; index?: number } = $props();
  const running = $derived(isRunning(inst.id));
  const busy = $derived(isBusy(inst.id));
  const p = $derived(app.progress[inst.id]);
</script>

<div class="card glass rise" style:animation-delay="{index * 40}ms" oncontextmenu={(e) => openInstanceMenu(e, inst)} role="group" aria-label={inst.name}>
  <button class="open" aria-label={t('Open {name}', { name: inst.name })} onclick={() => go('instance', inst.id)}></button>
  <div class="art">
    <InstanceArt loader={inst.loader} image={instanceBanner(inst)} icon={instanceIcon(inst)} prefer="logo" />
    <div class="badges">
      <span class="chip dark"><span class="peak">▲</span> {inst.gameVersion}</span>
      {#if running}<span class="chip live">{t('● Playing')}</span>{/if}
    </div>
    {#if !running}
      <button class="play" aria-label={t('Play {name}', { name: inst.name })} disabled={busy} onclick={() => play(inst)}>
        {#if busy}<LoaderCircle size={20} class="spin" />{:else}<Play size={20} fill="currentColor" />{/if}
      </button>
    {/if}
    {#if busy && p?.total}
      <div class="mini-progress"><div style:width="{(p.done / p.total) * 100}%"></div></div>
    {/if}
  </div>
  <div class="body">
    <h3>{inst.name}</h3>
    <div class="meta">
      <span>{inst.pack ? `${packVersion(inst.pack)}` : loaderLabel(inst.loader)}</span>
      {#if versionStatus(inst)}
        <span class="vstatus" class:outdated={versionStatus(inst) === 'outdated'}>{versionStatus(inst) === 'outdated' ? t('Outdated') : t('Latest')}</span>
      {/if}
      <span class="dot"></span>
      <Clock size={12} />
      <span>{ago(inst.lastPlayed)}</span>
    </div>
  </div>
</div>

<style>
  .vstatus {
    padding: 0 6px;
    border-radius: 5px;
    font-size: 10.5px;
    font-weight: 650;
    color: #86efac;
    background: rgba(16, 185, 129, 0.12);
  }
  .vstatus.outdated {
    color: #fcd34d;
    background: rgba(251, 191, 36, 0.13);
  }
  .card {
    position: relative;
    overflow: hidden;
    padding: 0;
    transition: transform 0.3s var(--ease), border-color 0.3s var(--ease), box-shadow 0.3s var(--ease);
  }
  .card:hover {
    transform: translateY(-3px);
    border-color: var(--stroke-strong);
  }
  .open {
    position: absolute;
    inset: 0;
    z-index: 1;
  }
  .art {
    position: relative;
    aspect-ratio: 16 / 9;
    overflow: hidden;
  }
  .art :global(svg),
  .art :global(img) {
    transition: transform 0.6s var(--ease);
  }
  .card:hover .art :global(svg),
  .card:hover .art :global(img) {
    transform: scale(1.05);
  }
  .badges {
    position: absolute;
    top: 10px;
    left: 10px;
    display: flex;
    gap: 6px;
  }
  .chip.dark {
    background: rgba(0, 0, 0, 0.4);
    backdrop-filter: blur(8px);
    color: white;
    border-color: rgba(255, 255, 255, 0.15);
  }
  .peak {
    font-size: 8px;
    color: var(--accent);
  }
  .chip.live {
    background: rgba(16, 185, 129, 0.25);
    border-color: rgba(52, 211, 153, 0.4);
    color: #a7f3d0;
    backdrop-filter: blur(8px);
  }
  .play {
    position: absolute;
    z-index: 2;
    right: 12px;
    bottom: 12px;
    width: 46px;
    height: 46px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    color: var(--on-accent);
    background: var(--gradient);
    opacity: 0;
    transform: translateY(8px) scale(0.9);
    transition: opacity 0.25s var(--ease), transform 0.25s var(--ease);
  }
  .card:hover .play,
  .play:disabled {
    opacity: 1;
    transform: none;
  }
  .play:hover {
    filter: brightness(1.1);
  }
  .play :global(svg) {
    margin-left: 2px;
  }
  .mini-progress {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 3px;
    background: rgba(0, 0, 0, 0.4);
  }
  .mini-progress div {
    height: 100%;
    background: var(--gradient);
    transition: width 0.3s;
  }
  .body {
    padding: 13px 15px 15px;
  }
  h3 {
    font-size: 14.5px;
    font-weight: 620;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .meta {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 5px;
    font-size: 12px;
    color: var(--faint);
  }
  .dot {
    width: 3px;
    height: 3px;
    border-radius: 50%;
    background: currentColor;
  }
</style>
