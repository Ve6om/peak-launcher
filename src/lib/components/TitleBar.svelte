<script lang="ts">
  import { Minus, Square, X, Gamepad2, EyeOff } from '@lucide/svelte';
  import { scale } from 'svelte/transition';
  import { isTauri } from '$lib/api';
  import { app, go, stop, requestClose } from '$lib/state.svelte';
  import Logo from './Logo.svelte';
  import { streaming, setStreamerMode } from '$lib/privacy.svelte';

  async function win() {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    return getCurrentWindow();
  }
  const minimize = async () => isTauri && (await win()).minimize();
  const maximize = async () => isTauri && (await win()).toggleMaximize();
  // Brand wordmark (static/wordmark.png); text until the file exists.
  let wordmark = $state(true);

  // Running games menu.
  let open = $state(false);
  const running = $derived(app.instances.filter((i) => app.running.includes(i.id)));
  $effect(() => {
    if (!running.length) open = false;
  });

  // Lets the page blur private fields (see .private in app.css).
  $effect(() => {
    document.documentElement.classList.toggle('streaming', streaming());
  });

  // Ctrl+Shift+S turns streamer mode on or off from anywhere.
  function keys(e: KeyboardEvent) {
    if (e.key === 'Escape') open = false;
    if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === 's' && app.settings) {
      e.preventDefault();
      setStreamerMode(!streaming());
    }
  }

  async function stopAll() {
    for (const inst of running) await stop(inst);
    open = false;
  }
</script>

<svelte:window onkeydown={keys} />

<header class="titlebar" data-tauri-drag-region>
  <div class="brand" data-tauri-drag-region>
    <Logo size={18} />
    {#if wordmark}
      <img class="wordmark" src="/wordmark.png" alt="Peak" data-tauri-drag-region onerror={() => (wordmark = false)} />
    {:else}
      <span data-tauri-drag-region>Peak <b>Launcher</b></span>
    {/if}
  </div>

  <div class="right">
    {#if streaming()}
      <button class="streamer" onclick={() => setStreamerMode(false)} title="Names, server addresses and folder paths are hidden. Click to turn off (Ctrl+Shift+S).">
        <EyeOff size={13} /> Streamer mode
      </button>
    {/if}
    {#if running.length}
      <div class="games">
        <button class="playing" class:on={open} onclick={() => (open = !open)} title="Running games">
          <span class="dot"></span>
          {running.length === 1 ? 'Playing' : `${running.length} playing`}
        </button>
        {#if open}
          <button class="catch" aria-label="Close menu" onclick={() => (open = false)}></button>
          <div class="menu glass" transition:scale={{ start: 0.96, duration: 120 }}>
            <span class="label">Running</span>
            {#each running as inst (inst.id)}
              <div class="game">
                <Gamepad2 size={15} />
                <button class="name" onclick={() => { open = false; go('instance', inst.id, 'console'); }}>{inst.name}</button>
                <button class="btn stop" onclick={() => stop(inst)}><Square size={11} fill="currentColor" /> Stop</button>
              </div>
            {/each}
            {#if running.length > 1}
              <button class="btn stop all" onclick={stopAll}>Stop all</button>
            {/if}
          </div>
        {/if}
      </div>
    {/if}

    <div class="controls">
      <button aria-label="Minimize" onclick={minimize}><Minus size={15} /></button>
      <button aria-label="Maximize" onclick={maximize}><Square size={12} /></button>
      <button aria-label="Close" class="close" onclick={requestClose}><X size={16} /></button>
    </div>
  </div>
</header>

<style>
  .titlebar {
    position: relative;
    z-index: 50;
    height: 40px;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding-left: 16px;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 9px;
    font-size: 13px;
    color: var(--muted);
    letter-spacing: 0.01em;
  }
  .wordmark {
    height: 13px;
    width: auto;
    pointer-events: none;
  }
  .brand b {
    font-weight: 600;
    color: var(--text);
  }
  .right {
    display: flex;
    align-items: center;
    height: 100%;
    gap: 10px;
  }
  .games {
    position: relative;
  }
  .streamer {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 26px;
    padding: 0 10px;
    border-radius: 6px;
    font-size: 12px;
    font-weight: 600;
    color: #ddd6fe;
    background: rgba(139, 92, 246, 0.14);
    border: 1px solid rgba(167, 139, 250, 0.32);
  }
  .streamer:hover {
    background: rgba(139, 92, 246, 0.24);
  }
  .playing {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 26px;
    padding: 0 10px;
    border-radius: 6px;
    font-size: 12px;
    font-weight: 600;
    color: #a7f3d0;
    background: rgba(16, 185, 129, 0.12);
    border: 1px solid rgba(52, 211, 153, 0.3);
  }
  .playing:hover,
  .playing.on {
    background: rgba(16, 185, 129, 0.2);
  }
  /* A solid dot with a soft ring growing out of it. The dot itself never
     changes, so it can't flicker on graphics drivers that render small
     opacity animations unevenly; the ring is a GPU-composited transform. */
  .dot {
    position: relative;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--success);
  }
  .dot::after {
    content: '';
    position: absolute;
    inset: 0;
    border-radius: 50%;
    background: var(--success);
    will-change: transform, opacity;
    animation: ring 2.4s cubic-bezier(0.2, 0.6, 0.3, 1) infinite;
  }
  @keyframes ring {
    0% {
      transform: scale(1);
      opacity: 0.5;
    }
    70%,
    100% {
      transform: scale(2.6);
      opacity: 0;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .dot::after {
      animation: none;
      opacity: 0;
    }
  }
  .catch {
    position: fixed;
    inset: 0;
    z-index: 60;
    cursor: default;
  }
  .menu {
    position: absolute;
    right: 0;
    top: 32px;
    z-index: 61;
    width: 280px;
    padding: 10px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    background: var(--surface);
    border-color: var(--stroke-strong);
    box-shadow: 0 18px 40px -10px rgba(0, 0, 0, 0.7);
    transform-origin: top right;
  }
  .menu .label {
    margin: 0 0 2px 2px;
  }
  .game {
    display: flex;
    align-items: center;
    gap: 9px;
    color: var(--success);
  }
  .name {
    flex: 1;
    min-width: 0;
    text-align: left;
    font-size: 13px;
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .name:hover {
    text-decoration: underline;
  }
  .stop {
    height: 28px;
    padding: 0 10px;
    font-size: 12px;
    color: #ff9a9d;
  }
  .stop:hover {
    background: rgba(242, 85, 90, 0.12);
    border-color: rgba(242, 85, 90, 0.35);
  }
  .stop.all {
    margin-top: 4px;
  }
  .controls {
    display: flex;
    height: 100%;
  }
  .controls button {
    width: 46px;
    height: 100%;
    display: grid;
    place-items: center;
    color: var(--muted);
    transition: background 0.15s, color 0.15s;
  }
  .controls button:hover {
    background: rgba(255, 255, 255, 0.07);
    color: var(--text);
  }
  .controls .close:hover {
    background: #e81123;
    color: white;
  }
</style>
