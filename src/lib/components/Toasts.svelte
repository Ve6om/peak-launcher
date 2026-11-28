<script lang="ts">
  import { t } from '$lib/i18n.svelte';
  import { fly } from 'svelte/transition';
  import { CircleCheck, CircleAlert, Info, Angry } from '@lucide/svelte';
  import { app } from '$lib/state.svelte';
  import { rage } from '$lib/rage';
</script>

<div class="toasts">
  {#each app.toasts as item (item.id)}
    <div class="toast glass {item.kind}" transition:fly={{ y: 16, duration: 250 }}>
      {#if item.kind === 'success'}<CircleCheck size={17} />{:else if item.kind === 'error'}<CircleAlert size={17} />{:else}<Info size={17} />{/if}
      <span>{item.text}</span>
      {#if item.rage}
        <button class="rage" title={t('Let it out: shake Peak for five seconds')} onclick={rage}><Angry size={14} /> {t('Rage')}</button>
      {/if}
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    right: 20px;
    bottom: 20px;
    z-index: 200;
    display: flex;
    flex-direction: column;
    gap: 8px;
    align-items: flex-end;
    pointer-events: none;
  }
  .toast {
    display: flex;
    align-items: center;
    gap: 10px;
    max-width: min(380px, calc(100vw - 40px));
    padding: 12px 16px;
    /* Long links and file names wrap instead of running off the window. */
    overflow-wrap: anywhere;
    font-size: 13px;
    background: rgba(20, 22, 34, 0.8);
    pointer-events: auto;
  }
  .success :global(svg) {
    color: var(--success);
  }
  .error :global(svg) {
    color: var(--danger);
  }
  .info :global(svg) {
    color: var(--accent-2);
  }
  .toast :global(svg) {
    flex-shrink: 0;
  }
  .rage {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    flex-shrink: 0;
    padding: 4px 9px;
    border: 1px solid color-mix(in srgb, var(--danger) 45%, transparent);
    border-radius: 99px;
    background: color-mix(in srgb, var(--danger) 16%, transparent);
    color: var(--text);
    font: inherit;
    font-size: 12px;
    font-weight: 650;
    cursor: pointer;
  }
  .rage:hover {
    background: color-mix(in srgb, var(--danger) 30%, transparent);
  }
  .rage :global(svg) {
    color: var(--danger);
  }
</style>
