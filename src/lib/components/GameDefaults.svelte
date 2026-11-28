<script lang="ts">
  import { t } from '$lib/i18n.svelte';
  // The default keybinds and game settings: what they are, editing them in
  // Peak, copying them from an instance, or clearing them. With `instance`,
  // copying takes that instance's settings; without, a picker of instances.
  import { onMount } from 'svelte';
  import { instanceIcon } from '$lib/instanceImages';
  import Select from './Select.svelte';
  import { Copy, Trash2, Keyboard, Monitor, Pencil, RotateCcw } from '@lucide/svelte';
  import DefaultsEditor from './DefaultsEditor.svelte';
  import { api } from '$lib/api';
  import { app, ago, toast, errorText } from '$lib/state.svelte';
  import type { GameDefaults, Instance } from '$lib/types';

  let { instance = null }: { instance?: Instance | null } = $props();

  let info = $state<GameDefaults | null>(null);
  let loaded = $state(false);
  let from = $state('');
  let busy = $state(false);
  let confirm = $state<'copy' | 'clear' | 'keys' | null>(null);
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
  function sure(what: 'copy' | 'clear' | 'keys') {
    if ((!info && what !== 'keys') || confirm === what) {
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
      if (info.skippedKeybinds) {
        toast(t('Copied {from}\'s settings as the defaults. Its keybinds were left out: Minecraft 1.12 and older stores keys in a way newer versions can\'t read.', { from: info.from }), 'success');
      } else {
        toast(t('Copied {from}\'s keybinds and settings as the defaults', { from: info.from }), 'success');
      }
    } catch (e) {
      toast(errorText(e), 'error');
    } finally {
      busy = false;
    }
  }

  /** Put this instance's keys back to Minecraft's own. */
  async function resetKeys() {
    if (!instance || !sure('keys')) return;
    busy = true;
    try {
      const n = await api.instanceResetKeybinds(instance.id);
      const usesDefaults = instance.shared?.keybinds && info && info.keybinds > 0;
      toast(
        n === 0
          ? t('This instance has no keybinds saved yet')
          : usesDefaults
            ? t('Keys reset. Your default keybinds still go in when it starts; switch off \'Use default keybinds\' to keep Minecraft\'s own.')
            : t('Keys reset to Minecraft\'s own. Mods\' keys go back to theirs too.'),
        n === 0 ? 'info' : 'success'
      );
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
      toast(t('Defaults cleared'));
    } catch (e) {
      toast(errorText(e), 'error');
    }
  }

  const origin = $derived(
    !info ? '' : info.from ? info.edited ? t('Copied from {from}, changed in Peak', { from: info.from }) : t('Copied from {from}', { from: info.from }) : t('Set in Peak')
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
        <span><Keyboard size={13} /> {(info.keybinds === 1 ? t('{keybinds} keybind', { keybinds: info.keybinds }) : t('{keybinds} keybinds', { keybinds: info.keybinds }))}</span>
        <span><Monitor size={13} /> {(info.settings === 1 ? t('{settings} setting', { settings: info.settings }) : t('{settings} settings', { settings: info.settings }))}</span>
        {#if info.fov}<span>{t('FOV {fov}', { fov: info.fov })}</span>{/if}
        {#if info.renderDistance}<span>{t('Render distance {renderDistance}', { renderDistance: info.renderDistance })}</span>{/if}
        {#if info.guiScale}<span>{t('GUI scale {x}', { x: info.guiScale === '0' ? t('Auto') : info.guiScale })}</span>{/if}
      </div>
    {:else}
      <p class="none">{t('No defaults yet. Set them here, or copy them from an instance you\'ve already set up in-game.')}</p>
    {/if}
    <div class="actions">
      <button class="btn primary" onclick={() => (editing = true)}><Pencil size={14} /> {info ? t('Edit…') : t('Set defaults…')}</button>
      {#if !instance}
        <span class="or">{t('or copy from')}</span>
        <Select
          bind:value={from}
          ariaLabel={t('Instance to copy from')}
          style="width: auto; min-width: 160px;"
          options={app.instances.map((i) => ({ value: i.id, label: i.name, icon: instanceIcon(i), loader: i.loader }))}
        />
      {/if}
      <button class="btn" disabled={busy || (!instance && !from)} onclick={copy}>
        <Copy size={14} />
        {confirm === 'copy' ? t('Click again to replace') : instance ? t('Copy from this instance') : t('Copy')}
      </button>
      {#if info}
        <button class="btn ghost danger" title={t('Remove the defaults (instances keep what they have now)')} onclick={clear}>
          <Trash2 size={14} /> {confirm === 'clear' ? t('Click again to clear') : instance ? t('Clear defaults') : t('Clear')}
        </button>
      {/if}
      {#if instance}
        <button class="btn ghost" title={t('Put this instance\'s keys back to Minecraft\'s own')} disabled={busy} onclick={resetKeys}>
          <RotateCcw size={14} /> {confirm === 'keys' ? t('Click again to reset keys') : t('Reset keybinds')}
        </button>
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
