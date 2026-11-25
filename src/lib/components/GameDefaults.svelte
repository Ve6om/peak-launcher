<script lang="ts">
  // The default keybinds and game settings: what they are, editing them in
  // Peak, copying them from an instance, or clearing them. With `instance`,
  // copying takes that instance's settings; without, a picker of instances.
  import { onMount } from 'svelte';
  import { instanceIcon } from '$lib/instanceImages';
  import Select from './Select.svelte';
  import { Copy, Trash2, Keyboard, Monitor, Pencil } from '@lucide/svelte';
  import DefaultsEditor from './DefaultsEditor.svelte';
  import { api } from '$lib/api';
  import { app, ago, toast, errorText } from '$lib/state.svelte';
  import type { GameDefaults, Instance } from '$lib/types';

  let { instance = null }: { instance?: Instance | null } = $props();

  let info = $state<GameDefaults | null>(null);
  let loaded = $state(false);
  let from = $state('');
  let busy = $state(false);
  let confirm = $state<'copy' | 'clear' | null>(null);
  let editing = $state(false);

  onMount(async () => {
    try {
      info = await api.gameDefaults();
    } catch {
      info = null;
    }
    loaded = true;
    from = instance?.id ?? app.instances[0]?.id ?? '';
  });

  /** Ask once more before replacing or clearing defaults that exist. */
  function sure(what: 'copy' | 'clear') {
    if (!info || confirm === what) {
      confirm = null;
      return true;
    }
    confirm = what;
    setTimeout(() => confirm === what && (confirm = null), 3000);
    return false;
  }

  async function copy() {
    const id = instance?.id ?? from;
    if (!id || !sure('copy')) return;
    busy = true;
    try {
      info = await api.gameDefaultsSave(id);
      toast(`Copied ${info.from}'s keybinds and settings as the defaults`, 'success');
    } catch (e) {
      toast(errorText(e), 'error');
    } finally {
      busy = false;
    }
  }

  async function clear() {
    if (!sure('clear')) return;
    try {
      await api.gameDefaultsClear();
      info = null;
      toast('Defaults cleared');
    } catch (e) {
      toast(errorText(e), 'error');
    }
  }

  const origin = $derived(
    !info ? '' : info.from ? `Copied from ${info.from}${info.edited ? ', changed in Peak' : ''}` : 'Set in Peak'
  );
</script>

{#if loaded}
  <div class="card">
    {#if info}
      <div class="what">
        <b>{origin}</b>
        {#if info.savedAt}<span>{ago(info.savedAt)}</span>{/if}
      </div>
      <div class="facts">
        <span><Keyboard size={13} /> {info.keybinds} keybind{info.keybinds === 1 ? '' : 's'}</span>
        <span><Monitor size={13} /> {info.settings} setting{info.settings === 1 ? '' : 's'}</span>
        {#if info.fov}<span>FOV {info.fov}</span>{/if}
        {#if info.renderDistance}<span>Render distance {info.renderDistance}</span>{/if}
        {#if info.guiScale}<span>GUI scale {info.guiScale === '0' ? 'Auto' : info.guiScale}</span>{/if}
      </div>
    {:else}
      <p class="none">No defaults yet. Set them here, or copy them from an instance you've already set up in-game.</p>
    {/if}
    <div class="actions">
      <button class="btn primary" onclick={() => (editing = true)}><Pencil size={14} /> {info ? 'Edit…' : 'Set defaults…'}</button>
      {#if !instance}
        <span class="or">or copy from</span>
        <Select
          bind:value={from}
          ariaLabel="Instance to copy from"
          style="width: auto; min-width: 160px;"
          options={app.instances.map((i) => ({ value: i.id, label: i.name, icon: instanceIcon(i), loader: i.loader }))}
        />
      {/if}
      <button class="btn" disabled={busy || (!instance && !from)} onclick={copy}>
        <Copy size={14} />
        {confirm === 'copy' ? 'Click again to replace' : instance ? 'Copy from this instance' : 'Copy'}
      </button>
      {#if info && !instance}
        <button class="btn ghost danger" onclick={clear}><Trash2 size={14} /> {confirm === 'clear' ? 'Click again to clear' : 'Clear'}</button>
      {/if}
    </div>
  </div>
{/if}

{#if editing}
  <DefaultsEditor onsaved={(i) => (info = i.keybinds || i.settings ? i : null)} onclose={() => (editing = false)} />
{/if}

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px;
    margin-bottom: 8px;
    border: 1px solid var(--stroke);
    border-radius: var(--radius-sm);
    background: rgba(0, 0, 0, 0.16);
  }
  .what {
    display: flex;
    align-items: baseline;
    gap: 10px;
  }
  .what b {
    font-size: 13.5px;
    font-weight: 600;
  }
  .what span {
    font-size: 12px;
    color: var(--faint);
  }
  .facts {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .facts span {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 3px 8px;
    border-radius: 7px;
    font-size: 12px;
    color: var(--muted);
    background: rgba(255, 255, 255, 0.05);
  }
  .none {
    margin: 0;
    font-size: 12.5px;
    line-height: 1.45;
    color: var(--muted);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin-top: 2px;
  }
  .or {
    font-size: 12px;
    color: var(--faint);
  }
  .danger:hover {
    color: #fda4af;
  }
</style>
