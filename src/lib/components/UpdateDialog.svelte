<script lang="ts">
  import { Download, LoaderCircle, TriangleAlert } from '@lucide/svelte';
  import Modal from './Modal.svelte';
  import Markdown from './Markdown.svelte';
  import { updates, installUpdate, snooze } from '$lib/updates.svelte';
  import { app } from '$lib/state.svelte';

  const u = $derived(updates.available!);
  const date = $derived(u.date ? new Date(u.date).toLocaleDateString(undefined, { year: 'numeric', month: 'long', day: 'numeric' }) : null);
  const pct = $derived(updates.progress === null ? null : Math.round(updates.progress * 100));
</script>

<Modal
  title="Peak {u.version} is available"
  subtitle="You have {u.currentVersion}.{date ? ` Released ${date}.` : ''}"
  width={520}
  onclose={() => !updates.installing && snooze()}
>
  <div class="notes">
    <span class="label">What's new</span>
    <div class="body"><Markdown source={u.notes} /></div>
  </div>

  {#if app.running.length}
    <p class="warn"><TriangleAlert size={14} /> Peak restarts to finish updating. Your running game keeps going.</p>
  {/if}

  {#if updates.installing}
    <div class="progress">
      <div class="row">
        <span>{pct === 100 ? 'Installing…' : 'Downloading update…'}</span>
        <span>{pct === null ? '' : `${pct}%`}</span>
      </div>
      <div class="track" class:indeterminate={pct === null}><div class="bar" style:width="{pct ?? 35}%"></div></div>
    </div>
  {/if}

  {#if updates.error}
    <p class="error">Couldn't install the update: {updates.error}</p>
  {/if}

  <footer>
    <button class="btn ghost" disabled={updates.installing} onclick={snooze}>Later</button>
    <button class="btn primary" disabled={updates.installing} onclick={installUpdate}>
      {#if updates.installing}<LoaderCircle size={16} class="spin" /> Updating…{:else}<Download size={16} /> Update now{/if}
    </button>
  </footer>
</Modal>

<style>
  .notes {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .body {
    max-height: 40vh;
    overflow-y: auto;
    padding: 12px 14px;
    border-radius: var(--radius-sm);
    background: #0e1013;
    border: 1px solid var(--stroke);
  }
  .body :global(h3) {
    font-size: 13.5px;
    margin: 10px 0 4px;
  }
  .body :global(h3:first-child) {
    margin-top: 0;
  }
  .body :global(ul) {
    margin: 4px 0;
  }
  .warn {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 12px 0 0;
    font-size: 12.5px;
    color: #fde68a;
  }
  .error {
    margin: 12px 0 0;
    font-size: 12.5px;
    color: #ff9a9d;
  }
  .progress {
    display: flex;
    flex-direction: column;
    gap: 7px;
    margin-top: 14px;
  }
  .row {
    display: flex;
    justify-content: space-between;
    font-size: 12px;
    color: var(--muted);
  }
  .track {
    height: 6px;
    border-radius: 99px;
    background: rgba(255, 255, 255, 0.08);
    overflow: hidden;
  }
  .bar {
    height: 100%;
    background: var(--accent);
    transition: width 0.2s;
  }
  .indeterminate .bar {
    animation: slide 1.2s ease-in-out infinite;
  }
  @keyframes slide {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(300%);
    }
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 18px;
  }
</style>
