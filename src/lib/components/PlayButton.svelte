<script lang="ts">
  import { t } from '$lib/i18n.svelte';
  import { Play, Square, LoaderCircle } from '@lucide/svelte';
  import { app, play, stop, isRunning, isBusy } from '$lib/state.svelte';
  import type { Instance } from '$lib/types';

  let { inst, size = 'lg' }: { inst: Instance; size?: 'lg' | 'md' } = $props();
  const running = $derived(isRunning(inst.id));
  const busy = $derived(isBusy(inst.id));
</script>

{#if running}
  <button class="btn {size} stop" onclick={() => stop(inst)}>
    <Square size={15} fill="currentColor" /> {t('Stop')}
  </button>
{:else if busy}
  <button class="btn primary {size}" disabled>
    <LoaderCircle size={17} class="spin" /> {app.progress[inst.id]?.stage === 'Downloading Java' ? t('Getting Java') : t('Installing')}
  </button>
{:else}
  <button class="btn primary {size}" onclick={() => play(inst)}>
    <Play size={16} fill="currentColor" /> {t('Play')}
  </button>
{/if}

<style>
  .stop {
    background: rgba(244, 63, 94, 0.16);
    border-color: rgba(244, 63, 94, 0.4);
    color: #fecdd3;
  }
  .stop:hover {
    background: rgba(244, 63, 94, 0.26);
    border-color: rgba(244, 63, 94, 0.6);
  }
  .md {
    height: 38px;
  }
</style>
