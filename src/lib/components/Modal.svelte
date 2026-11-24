<script lang="ts">
  import type { Snippet } from 'svelte';
  import { fade, scale } from 'svelte/transition';
  import { X } from '@lucide/svelte';

  let {
    title,
    subtitle = '',
    width = 520,
    onclose,
    children
  }: { title: string; subtitle?: string; width?: number; onclose: () => void; children: Snippet } = $props();
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onclose()} />

<div class="backdrop" transition:fade={{ duration: 180 }} onclick={onclose} role="presentation">
  <div
    class="modal glass"
    style:width="{width}px"
    transition:scale={{ start: 0.96, duration: 220 }}
    onclick={(e) => e.stopPropagation()}
    role="dialog"
    aria-modal="true"
    aria-label={title}
    tabindex="-1"
    onkeydown={() => {}}
  >
    <header>
      <div>
        <h2>{title}</h2>
        {#if subtitle}<p>{subtitle}</p>{/if}
      </div>
      <button class="btn ghost icon" aria-label="Close" onclick={onclose}><X size={18} /></button>
    </header>
    <!-- Scrolls when the window is too short for the whole dialog. -->
    <div class="body">
      {@render children()}
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 100;
    display: grid;
    place-items: center;
    background: rgba(4, 5, 10, 0.55);
    backdrop-filter: blur(6px);
  }
  :global(html.linux-fx) .backdrop,
  :global(html.light-fx) .backdrop {
    background: rgba(4, 5, 10, 0.7);
  }
  .modal {
    max-width: calc(100vw - 48px);
    max-height: calc(100vh - 80px);
    display: flex;
    flex-direction: column;
    padding: 22px;
    background: rgba(20, 22, 34, 0.82);
    border-color: var(--stroke-strong);
    box-shadow: var(--shadow), 0 40px 80px -20px rgba(0, 0, 0, 0.7);
  }
  .body {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    overscroll-behavior: contain;
    /* Room for focus rings and the scrollbar without clipping. */
    margin: -4px -10px -4px -4px;
    padding: 4px 10px 4px 4px;
  }
  header {
    flex-shrink: 0;
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    margin-bottom: 18px;
  }
  h2 {
    font-size: 19px;
    font-weight: 680;
  }
  p {
    margin: 4px 0 0;
    font-size: 13px;
    color: var(--muted);
  }
</style>
