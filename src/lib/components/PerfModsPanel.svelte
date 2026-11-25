<script lang="ts">
  // Optional extra in the Optimize dialog: add well-known performance mods
  // that fit this instance, and take them out again with Undo.
  import { ChevronDown, LoaderCircle, Undo2, Plus, Check } from '@lucide/svelte';
  import { slide } from 'svelte/transition';
  import { api } from '$lib/api';
  import { toast, errorText, isRunning } from '$lib/state.svelte';
  import type { Instance, PerfMods } from '$lib/types';

  let { inst }: { inst: Instance } = $props();

  let open = $state(false);
  let info = $state<PerfMods | null>(null);
  let picked = $state(new Set<string>());
  let busy = $state(false);
  const vanilla = $derived(inst.loader === 'vanilla');
  const running = $derived(isRunning(inst.id));
  const addable = $derived(info?.mods.filter((m) => m.status === 'add') ?? []);

  async function load() {
    try {
      info = await api.perfMods(inst.id);
      picked = new Set();
    } catch (e) {
      toast(errorText(e), 'error');
    }
  }

  function toggleOpen() {
    open = !open;
    if (open && !info && !vanilla) load();
  }

  function flip(id: string) {
    const next = new Set(picked);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    picked = next;
  }

  async function add() {
    busy = true;
    try {
      const names = await api.perfModsAdd(inst.id, [...picked]);
      toast(names.length > 1 ? `Added ${names[0]} and ${names.length - 1} more` : `Added ${names[0]}`, 'success');
      await load();
    } catch (e) {
      toast(errorText(e), 'error');
    } finally {
      busy = false;
    }
  }

  async function undo() {
    busy = true;
    try {
      const n = await api.perfModsUndo(inst.id);
      toast(`Took out ${n} performance mod${n === 1 ? '' : 's'}`, 'success');
      await load();
    } catch (e) {
      toast(errorText(e), 'error');
    } finally {
      busy = false;
    }
  }
</script>

<div class="extra">
  <button class="head" aria-expanded={open} onclick={toggleOpen}>
    <span>
      <b>Extra: performance mods</b>
      <small>Optional. Adds well-known mods that make the game run faster.</small>
    </span>
    <ChevronDown size={16} class={open ? 'turned' : ''} />
  </button>

  {#if open}
    <div class="body" transition:slide={{ duration: 160 }}>
      {#if vanilla}
        <p class="muted">This is a vanilla instance. Pick Fabric, Quilt, Forge or NeoForge in its settings to use mods.</p>
      {:else if !info}
        <p class="muted"><LoaderCircle size={14} class="spin" /> Checking which ones fit {inst.name}…</p>
      {:else}
        <p class="warn">
          These are popular and widely used, but a pack's own mods sometimes don't get along with them: crashes, missing visuals, or a mod behaving strangely. If anything seems off after adding them, press Undo.
        </p>
        <ul class="mods">
          {#each info.mods as m (m.projectId)}
            <li class:off={m.status !== 'add'}>
              {#if m.status === 'add'}
                <label>
                  <input type="checkbox" checked={picked.has(m.projectId)} onchange={() => flip(m.projectId)} />
                  <span><b>{m.title}</b><small>{m.what}</small></span>
                </label>
              {:else}
                <span class="row">
                  {#if m.status === 'installed'}<Check size={14} />{:else}<i class="slot"></i>{/if}
                  <span><b>{m.title}</b><small>{m.status !== 'installed' ? m.note : info.added.includes(m.title) ? 'Added by Optimize (Undo takes it out)' : 'Already in this instance'}</small></span>
                </span>
              {/if}
            </li>
          {/each}
        </ul>
        {#if !addable.length}
          <p class="muted">Nothing to add: every one that fits is already in {inst.name}.</p>
        {/if}
        <div class="actions">
          {#if info.added.length}
            <button class="btn" disabled={busy || running} title="Remove {info.added.join(', ')}" onclick={undo}>
              <Undo2 size={14} /> Undo ({info.added.length})
            </button>
          {/if}
          {#if addable.length}
            <button class="btn" disabled={busy} onclick={() => (picked = picked.size === addable.length ? new Set() : new Set(addable.map((m) => m.projectId)))}>
              {picked.size === addable.length ? 'Select none' : 'Select all'}
            </button>
            <button class="btn primary" disabled={busy || running || !picked.size} onclick={add}>
              {#if busy}<LoaderCircle size={14} class="spin" />{:else}<Plus size={14} />{/if} Add {picked.size || ''}
            </button>
          {/if}
        </div>
        {#if running}<p class="muted">Close the game to add or remove mods.</p>{/if}
      {/if}
    </div>
  {/if}
</div>

<style>
  .extra {
    margin-top: 12px;
    border: 1px solid var(--stroke);
    border-radius: var(--radius-sm);
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    width: 100%;
    padding: 10px 12px;
    border: 0;
    background: none;
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .head span,
  li span {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .head b,
  li b {
    font-size: 13px;
    font-weight: 600;
  }
  .head small,
  li small {
    font-size: 12px;
    color: var(--muted);
  }
  .head :global(.turned) {
    transform: rotate(180deg);
  }
  .head :global(svg) {
    transition: transform 0.15s;
    flex-shrink: 0;
  }
  .body {
    padding: 0 12px 12px;
  }
  .warn {
    margin: 0 0 10px;
    padding: 8px 10px;
    border-radius: 8px;
    font-size: 12px;
    line-height: 1.45;
    color: #fde68a;
    background: rgba(250, 204, 21, 0.08);
    border: 1px solid rgba(250, 204, 21, 0.22);
  }
  .mods {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  li label,
  li .row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 4px;
    border-radius: 8px;
    flex-direction: row;
  }
  li label {
    cursor: pointer;
  }
  li label:hover {
    background: rgba(255, 255, 255, 0.04);
  }
  li.off {
    opacity: 0.6;
  }
  li .row :global(svg) {
    color: #6ee7b7;
    flex-shrink: 0;
  }
  li input,
  .slot {
    flex-shrink: 0;
    width: 14px;
    margin: 0;
  }
  .muted {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 8px 0 0;
    font-size: 12.5px;
    color: var(--muted);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 10px;
  }
</style>
