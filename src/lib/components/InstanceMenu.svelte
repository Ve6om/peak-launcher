<script lang="ts">
  // Right-click menu for an instance (cards, sidebar, hero).
  import { tick } from 'svelte';
  import { scale } from 'svelte/transition';
  import { Play, Square, ExternalLink, SlidersHorizontal, FolderOpen, Copy, Wrench, Trash2, PackageOpen, Server, Sparkles, ArrowDownUp, MonitorPlay, FolderTree } from '@lucide/svelte';
  import { api } from '$lib/api';
  import type { Instance } from '$lib/types';
  import { packUpdates } from '$lib/packUpdates.svelte';
  import { app, go, play, stop, isRunning, isBusy, duplicateInstance, repairInstance, createShortcut } from '$lib/state.svelte';

  const inst = $derived(app.instances.find((i) => i.id === app.menu?.id) ?? null);
  let el = $state<HTMLDivElement>();
  let pos = $state({ x: 0, y: 0 });

  // Keep the menu inside the window.
  $effect(() => {
    const m = app.menu;
    if (!m) return;
    pos = { x: m.x, y: m.y };
    tick().then(() => {
      if (!el) return;
      // offset sizes ignore the opening scale animation
      const w = el.offsetWidth;
      const h = el.offsetHeight;
      pos = {
        x: Math.max(8, Math.min(m.x, window.innerWidth - w - 8)),
        y: Math.max(8, Math.min(m.y, window.innerHeight - h - 8))
      };
      el.querySelector<HTMLButtonElement>('button:not(:disabled)')?.focus();
    });
  });

  const close = () => (app.menu = null);
  // Capture the instance before closing: closing clears `inst`.
  const run = (fn: (i: Instance) => void) => () => {
    const target = inst;
    close();
    if (target) fn(target);
  };

  function onkey(e: KeyboardEvent) {
    if (!app.menu) return;
    if (e.key === 'Escape') close();
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      const items = [...(el?.querySelectorAll<HTMLButtonElement>('button:not(:disabled)') ?? [])];
      const i = items.indexOf(document.activeElement as HTMLButtonElement);
      items[(i + (e.key === 'ArrowDown' ? 1 : -1) + items.length) % items.length]?.focus();
    }
  }
</script>

<svelte:window onkeydown={onkey} onresize={close} onblur={close} />

{#if app.menu && inst}
  {@const running = isRunning(inst.id)}
  {@const busy = isBusy(inst.id)}
  {@const hasUpdate = packUpdates.list.some((u) => u.instanceId === inst.id)}
  <div class="catcher" role="presentation" onpointerdown={close} oncontextmenu={(e) => { e.preventDefault(); close(); }}></div>
  <div
    class="menu glass"
    bind:this={el}
    style:left="{pos.x}px"
    style:top="{pos.y}px"
    role="menu"
    tabindex="-1"
    transition:scale={{ start: 0.95, duration: 120 }}
  >
    <div class="title">{inst.name}</div>
    {#if running}
      <button role="menuitem" onclick={run((i) => stop(i))}><Square size={15} /> Stop</button>
    {:else}
      <button role="menuitem" class="accent" disabled={busy} onclick={run((i) => play(i))}><Play size={15} /> Play</button>
    {/if}
    <button role="menuitem" onclick={run((i) => go('instance', i.id))}><ExternalLink size={15} /> Open</button>
    <button role="menuitem" onclick={run((i) => go('instance', i.id, 'settings'))}><SlidersHorizontal size={15} /> Edit settings</button>
    <button role="menuitem" onclick={run((i) => (app.optimizing = i))}><Sparkles size={15} /> Optimize…</button>
    {#if inst.pack?.projectId}
      <button role="menuitem" disabled={busy || running} onclick={run((i) => (app.packVersionFor = i))}>
        <ArrowDownUp size={15} /> {hasUpdate ? 'Update modpack…' : 'Change modpack version…'}
      </button>
    {/if}
    <button role="menuitem" onclick={run((i) => api.openFolder(i.id))}><FolderOpen size={15} /> Open folder</button>
    <div class="sep"></div>
    <button role="menuitem" onclick={run((i) => duplicateInstance(i))}><Copy size={15} /> Duplicate</button>
    <button role="menuitem" onclick={run((i) => createShortcut(i))}><MonitorPlay size={15} /> Desktop shortcut</button>
    <button role="menuitem" onclick={run((i) => (app.grouping = i))}><FolderTree size={15} /> {inst.group ? 'Change group…' : 'Add to group…'}</button>
    <button role="menuitem" onclick={run((i) => (app.exporting = i))}><PackageOpen size={15} /> Export as modpack…</button>
    <button role="menuitem" onclick={run((i) => (app.serverPack = i))}><Server size={15} /> Export as server pack…</button>
    <button role="menuitem" disabled={busy || running} onclick={run((i) => repairInstance(i))}><Wrench size={15} /> Verify & repair</button>
    <div class="sep"></div>
    <button role="menuitem" class="danger" disabled={running} onclick={run((i) => (app.deleting = i))}><Trash2 size={15} /> Delete…</button>
  </div>
{/if}

<style>
  .catcher {
    position: fixed;
    inset: 0;
    z-index: 150;
  }
  .menu {
    position: fixed;
    z-index: 151;
    min-width: 210px;
    padding: 6px;
    display: flex;
    flex-direction: column;
    background: rgba(20, 22, 34, 0.88);
    border-color: var(--stroke-strong);
    border-radius: 14px;
    box-shadow: var(--shadow), 0 20px 50px -10px rgba(0, 0, 0, 0.7);
    transform-origin: top left;
  }
  .title {
    padding: 6px 10px 8px;
    font-size: 11.5px;
    font-weight: 650;
    color: var(--faint);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 260px;
  }
  button {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 34px;
    padding: 0 10px;
    border-radius: 9px;
    font-size: 13px;
    color: var(--text);
    text-align: left;
    outline: none;
  }
  button :global(svg) {
    color: var(--muted);
  }
  button:hover:not(:disabled),
  button:focus-visible {
    background: rgba(255, 255, 255, 0.07);
  }
  button:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .accent :global(svg) {
    color: var(--accent-2);
  }
  .danger,
  .danger :global(svg) {
    color: #fda4af;
  }
  .danger:hover:not(:disabled),
  .danger:focus-visible {
    background: rgba(244, 63, 94, 0.14);
  }
  .sep {
    height: 1px;
    margin: 5px 6px;
    background: var(--stroke);
  }
</style>
