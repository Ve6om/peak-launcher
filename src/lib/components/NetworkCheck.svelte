<script lang="ts">
  // "Check my connection": which of the services Peak needs can be reached.
  import { Wifi, LoaderCircle, CircleCheck, CircleX } from '@lucide/svelte';
  import { api } from '$lib/api';
  import { toast, errorText } from '$lib/state.svelte';
  import type { NetCheck } from '$lib/types';

  let results = $state<NetCheck[] | null>(null);
  let running = $state(false);

  async function run() {
    running = true;
    try {
      results = await api.networkCheck();
    } catch (e) {
      toast(errorText(e), 'error');
    } finally {
      running = false;
    }
  }

  const failed = $derived(results?.filter((r) => !r.ok) ?? []);
</script>

<div class="check">
  <div class="top">
    <div>
      <b>Check my connection</b>
      <span>See whether Peak can reach Mojang, Microsoft sign-in, Modrinth, CurseForge and the mod loaders. Useful when downloads or signing in fail.</span>
    </div>
    <button class="btn" disabled={running} onclick={run}>
      {#if running}<LoaderCircle size={14} class="spin" /> Checking…{:else}<Wifi size={14} /> {results ? 'Check again' : 'Check'}{/if}
    </button>
  </div>

  {#if results}
    <p class="summary" class:bad={failed.length}>
      {#if !failed.length}
        Everything answered. If something still won't download, the service itself may be having trouble.
      {:else}
        {failed.length} service{failed.length === 1 ? '' : 's'} couldn't be reached. A firewall, antivirus, school or work network, or DNS filter may be blocking {failed.length === 1 ? 'it' : 'them'}{failed.length === results.length ? ', or you may be offline' : ''}. A proxy (above) can help.
      {/if}
    </p>
    <div class="list">
      {#each results as r (r.name)}
        <div class="row" class:bad={!r.ok}>
          {#if r.ok}<CircleCheck size={15} />{:else}<CircleX size={15} />{/if}
          <span class="name">{r.name}<em>{r.usedFor}</em></span>
          <span class="host">{r.host}</span>
          <span class="res">{r.ok ? `${r.ms} ms` : r.error}</span>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .check {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px 0 4px;
  }
  .top {
    display: flex;
    align-items: center;
    gap: 16px;
  }
  .top div {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .top b {
    font-size: 13.5px;
    font-weight: 600;
  }
  .top span {
    font-size: 12.5px;
    color: var(--muted);
  }
  .summary {
    margin: 0;
    font-size: 12.5px;
    color: #86efac;
  }
  .summary.bad {
    color: #fde68a;
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 4px;
    border: 1px solid var(--stroke);
    border-radius: 12px;
    background: rgba(0, 0, 0, 0.16);
  }
  .row {
    display: grid;
    grid-template-columns: 18px minmax(0, 1fr) auto 90px;
    align-items: center;
    gap: 10px;
    min-height: 34px;
    padding: 2px 10px;
    border-radius: 8px;
    font-size: 12.5px;
    color: #86efac;
  }
  .row.bad {
    color: #fca5a5;
    background: rgba(239, 68, 68, 0.06);
  }
  .name {
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .name em {
    margin-left: 8px;
    font-style: normal;
    color: var(--faint);
  }
  .host {
    font-family: var(--font-mono);
    font-size: 11.5px;
    color: var(--faint);
  }
  .res {
    text-align: right;
    color: var(--muted);
  }
  .row.bad .res {
    color: #fca5a5;
  }
</style>
