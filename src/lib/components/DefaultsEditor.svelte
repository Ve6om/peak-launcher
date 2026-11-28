<script lang="ts">
  import { t } from '$lib/i18n.svelte';
  // Edit the default keybinds and game settings inside Peak. Each setting
  // can be left unset, so instances keep their own value for it.
  import { onMount } from 'svelte';
  import Select from './Select.svelte';
  import { LoaderCircle, X, Undo2 } from '@lucide/svelte';
  import Modal from './Modal.svelte';
  import Switch from './Switch.svelte';
  import { api } from '$lib/api';
  import { toast, errorText } from '$lib/state.svelte';
  import { GAME_OPTIONS, KEYBINDS, keyFromEvent, keyFromMouse, keyLabel, modKeyLabel, UNBOUND, type GameOption } from '$lib/gameOptions';
  import type { GameDefaults } from '$lib/types';

  let { onsaved, onclose }: { onsaved: (info: GameDefaults) => void; onclose: () => void } = $props();

  /** The defaults being edited, as key -> value, in file order. */
  let lines = $state<[string, string][] | null>(null);
  let saving = $state(false);
  let tab = $state<string>('Video');
  /** The keybind waiting for a key press. */
  let capturing = $state<string | null>(null);

  onMount(async () => {
    try {
      lines = await api.gameDefaultsLines();
    } catch (e) {
      toast(errorText(e), 'error');
      onclose();
    }
  });

  const known = new Set([...GAME_OPTIONS.map((o) => o.key), ...KEYBINDS.map((k) => k.key), 'version']);
  const get = (key: string) => lines?.find(([k]) => k === key)?.[1];
  function set(key: string, value: string | null) {
    if (!lines) return;
    const i = lines.findIndex(([k]) => k === key);
    if (value === null) {
      if (i >= 0) lines.splice(i, 1);
    } else if (i >= 0) lines[i] = [key, value];
    else lines.push([key, value]);
  }

  const modKeys = $derived((lines ?? []).filter(([k]) => k.startsWith('key_') && !known.has(k)));
  const otherLines = $derived((lines ?? []).filter(([k]) => !k.startsWith('key_') && !known.has(k)));
  const tabs = $derived(['Video', 'Sound', 'Controls', 'Keybinds', 'Chat & other', ...(otherLines.length ? ['More'] : [])]);
  const setCount = (group: string) =>
    group === 'Keybinds'
      ? KEYBINDS.filter((k) => get(k.key) !== undefined).length + modKeys.length
      : group === 'More'
        ? otherLines.length
        : GAME_OPTIONS.filter((o) => o.group === group && get(o.key) !== undefined).length;

  function numberOf(o: GameOption, raw: string | undefined) {
    if (o.control.kind !== 'range') return 0;
    const s = raw ?? o.initial;
    const n = o.control.fromFile ? o.control.fromFile(s) : parseFloat(s);
    return Number.isFinite(n) ? Math.min(o.control.max, Math.max(o.control.min, n)) : o.control.min;
  }
  function writeNumber(o: GameOption, v: number) {
    if (o.control.kind !== 'range') return;
    set(o.key, o.control.toFile ? o.control.toFile(v) : String(v));
  }

  function capture(e: KeyboardEvent) {
    if (!capturing) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.key === 'Escape') {
      capturing = null;
      return;
    }
    const name = keyFromEvent(e);
    if (name) {
      set(capturing, name);
      capturing = null;
    }
  }
  /** Set after a mouse button is bound, so the same click doesn't start listening again. */
  let skipClick = false;
  function captureMouse(e: MouseEvent, key: string) {
    if (capturing !== key) return;
    e.preventDefault();
    set(key, keyFromMouse(e.button));
    capturing = null;
    skipClick = true;
  }
  function keyClicked(key: string) {
    if (skipClick) {
      skipClick = false;
      return;
    }
    capturing = capturing === key ? null : key;
  }

  async function save() {
    if (!lines) return;
    saving = true;
    try {
      const info = await api.gameDefaultsSet($state.snapshot(lines));
      toast(t('Default game settings saved'), 'success');
      onsaved(info);
      onclose();
    } catch (e) {
      toast(errorText(e), 'error');
    } finally {
      saving = false;
    }
  }
</script>

<svelte:window onkeydowncapture={capture} />

<Modal title={t('Edit default game settings')} subtitle={t('Switch on a setting to give it a default. Ones left off stay as each instance has them.')} width={720} onclose={() => (capturing ? (capturing = null) : onclose())}>
  {#if !lines}
    <div class="loading"><LoaderCircle size={20} class="spin" /></div>
  {:else}
    <div class="seg tabs">
      {#each tabs as tb (tb)}
        <button class:active={tab === tb} onclick={() => (tab = tb)}>{t(tb)}{#if setCount(tb)}<em>{setCount(tb)}</em>{/if}</button>
      {/each}
    </div>

    <div class="list">
      {#if tab === 'Keybinds'}
        {#each KEYBINDS as k (k.key)}
          {@render keyRow(k.key, k.label, k.initial)}
        {/each}
        {#if modKeys.length}
          <h4>{t('From mods')}</h4>
          {#each modKeys as [key] (key)}
            {@render keyRow(key, modKeyLabel(key), UNBOUND)}
          {/each}
        {/if}
        <p class="hint">{t('Default keybinds are left out for Minecraft 1.12 and older, which stores keys differently.')}</p>
      {:else if tab === 'More'}
        <p class="hint first">{t('Copied from an instance; Peak has no friendly name for these. Remove any you don\'t want as defaults.')}</p>
        {#each otherLines as [key, value] (key)}
          <div class="row">
            <span class="name mono">{key}</span>
            <span class="raw mono">{value}</span>
            <button class="btn ghost icon" title={t('Remove')} aria-label={t('Remove {key}', { key })} onclick={() => set(key, null)}><X size={14} /></button>
          </div>
        {/each}
      {:else}
        {#each GAME_OPTIONS.filter((o) => o.group === tab) as o (o.key)}
          {@const raw = get(o.key)}
          {@const on = raw !== undefined}
          <div class="row" class:off={!on}>
            <Switch bind:checked={() => on, (v) => set(o.key, v ? o.initial : null)} label={t('Set a default for {label}', { label: o.label })} />
            <span class="name">{o.label}</span>
            <span class="ctl">
              {#if o.control.kind === 'range'}
                {@const n = numberOf(o, raw)}
                <input type="range" min={o.control.min} max={o.control.max} step={o.control.step} value={n} disabled={!on} oninput={(e) => writeNumber(o, +e.currentTarget.value)} />
                <span class="val">{o.control.show(n)}</span>
              {:else if o.control.kind === 'toggle'}
                <Switch disabled={!on} bind:checked={() => (raw ?? o.initial) === 'true', (v) => set(o.key, v ? 'true' : 'false')} label={o.label} />
              {:else}
                <Select
                  disabled={!on}
                  value={raw ?? o.initial}
                  onchange={(v) => v !== null && set(o.key, v)}
                  ariaLabel={o.label}
                  style="width: 180px;"
                  options={o.control.options.map((c) => ({ value: c.value, label: c.label }))}
                />
              {/if}
            </span>
          </div>
        {/each}
      {/if}
    </div>

    <div class="actions">
      <button class="btn ghost" onclick={onclose}>{t('Cancel')}</button>
      <button class="btn primary" disabled={saving} onclick={save}>{t('Save defaults')}</button>
    </div>
  {/if}
</Modal>

{#snippet keyRow(key: string, label: string, initial: string)}
  {@const raw = get(key)}
  {@const on = raw !== undefined}
  <div class="row" class:off={!on}>
    <Switch bind:checked={() => on, (v) => set(key, v ? initial : null)} label={t('Set a default for {label}', { label })} />
    <span class="name">{label}</span>
    <span class="ctl">
      <button
        class="btn keycap"
        class:listening={capturing === key}
        disabled={!on}
        onclick={() => keyClicked(key)}
        onmousedown={(e) => captureMouse(e, key)}
        onauxclick={() => (skipClick = false)}
        oncontextmenu={(e) => e.preventDefault()}
      >
        {capturing === key ? t('Press a key or click…') : keyLabel(raw ?? initial)}
      </button>
      {#if on && raw !== UNBOUND}
        <button class="btn ghost icon" title={t('Unbind')} aria-label={t('Unbind {label}', { label })} onclick={() => set(key, UNBOUND)}><X size={14} /></button>
      {:else if on && initial !== UNBOUND}
        <button class="btn ghost icon" title={t('Back to Minecraft\'s key')} aria-label={t('Reset {label}', { label })} onclick={() => set(key, initial)}><Undo2 size={14} /></button>
      {:else}
        <span class="spacer"></span>
      {/if}
    </span>
  </div>
{/snippet}

<style>
  .loading {
    display: flex;
    justify-content: center;
    padding: 40px;
  }
  .tabs {
    display: flex;
    flex-wrap: wrap;
    margin-bottom: 10px;
  }
  .tabs em {
    margin-left: 6px;
    padding: 0 6px;
    border-radius: 6px;
    font-size: 10.5px;
    font-style: normal;
    font-weight: 650;
    background: color-mix(in srgb, var(--accent) 18%, transparent);
    color: var(--accent);
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-height: 52vh;
    overflow-y: auto;
    padding: 4px;
    border: 1px solid var(--stroke);
    border-radius: 12px;
    background: rgba(0, 0, 0, 0.16);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 42px;
    padding: 4px 8px;
    border-radius: 9px;
  }
  .row:hover {
    background: rgba(255, 255, 255, 0.03);
  }
  .name {
    flex: 1;
    min-width: 0;
    font-size: 13px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .row.off .name {
    color: var(--muted);
  }
  .row.off .ctl {
    opacity: 0.45;
  }
  .ctl {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
    width: 300px;
    flex-shrink: 0;
  }
  .ctl input[type='range'] {
    flex: 1;
    accent-color: var(--accent);
  }
  .val {
    width: 104px;
    font-size: 12px;
    text-align: right;
    color: var(--muted);
    white-space: nowrap;
  }
  .keycap {
    min-width: 170px;
    height: 32px;
    justify-content: center;
    font-size: 12.5px;
  }
  .keycap.listening {
    border-color: var(--accent);
    color: var(--accent);
  }
  .spacer {
    width: 32px;
  }
  .raw {
    max-width: 260px;
    font-size: 12px;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .mono {
    font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
  }
  h4 {
    margin: 12px 8px 4px;
    font-size: 12px;
    font-weight: 650;
    color: var(--muted);
  }
  .hint {
    margin: 8px;
    font-size: 12px;
    color: var(--faint);
  }
  .hint.first {
    margin-top: 4px;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 14px;
  }
</style>
