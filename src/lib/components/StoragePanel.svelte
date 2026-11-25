<script lang="ts">
  // What takes up space in Peak's folder, and clearing what's safe to clear.
  import { onMount } from 'svelte';
  import { LoaderCircle, Trash2, FolderOpen, RefreshCw } from '@lucide/svelte';
  import { api } from '$lib/api';
  import { toast, errorText } from '$lib/state.svelte';
  import type { StorageUsage } from '$lib/types';

  let usage = $state<StorageUsage | null>(null);
  let loading = $state(false);
  let cleaning = $state<string | null>(null);

  async function load() {
    loading = true;
    try {
      usage = await api.storageUsage();
    } catch (e) {
      toast(errorText(e), 'error');
    } finally {
      loading = false;
    }
  }
  onMount(load);

  async function clean(id: string, name: string) {
    cleaning = id;
    try {
      const freed = await api.storageClean(id);
      toast(`${name}: freed ${fmt(freed)}`, 'success');
      await load();
    } catch (e) {
      toast(errorText(e), 'error');
    } finally {
      cleaning = null;
    }
  }

  function fmt(b: number) {
    if (b >= 1024 ** 3) return `${(b / 1024 ** 3).toFixed(1)} GB`;
    if (b >= 1024 ** 2) return `${Math.round(b / 1024 ** 2)} MB`;
    if (b > 0) return `${Math.max(1, Math.round(b / 1024))} KB`;
    return 'Nothing';
  }

  const shownParts = $derived((usage?.parts ?? []).filter((p) => p.bytes > 0).sort((a, b) => b.bytes - a.bytes));
  const colors = ['var(--accent)', 'var(--accent-2)', '#a78bfa', '#f472b6', '#fbbf24', '#34d399', '#94a3b8'];
  const freeable = $derived((usage?.cleanable ?? []).reduce((n, c) => n + c.bytes, 0));
</script>

{#if !usage}
  <div class="loading"><LoaderCircle size={20} class="spin" /> Adding up Peak's files…</div>
{:else}
  <div class="total">
    <div>
      <b>{fmt(usage.total)}</b>
      <span>used by Peak{#if freeable > 0} · {fmt(freeable)} can be cleared{/if}</span>
    </div>
    <button class="btn ghost icon" title="Count again" aria-label="Count again" disabled={loading} onclick={load}>
      {#if loading}<LoaderCircle size={15} class="spin" />{:else}<RefreshCw size={15} />{/if}
    </button>
    <button class="btn ghost icon" title="Open Peak's folder" aria-label="Open Peak's folder" onclick={() => api.openDataFolder()}>
      <FolderOpen size={15} />
    </button>
  </div>

  <div class="bar" role="img" aria-label="Space used by each part">
    {#each shownParts as p, i (p.id)}
      <span style:flex-grow={p.bytes} style:background={colors[i % colors.length]} title="{p.name}: {fmt(p.bytes)}"></span>
    {/each}
  </div>
  <ul class="legend">
    {#each shownParts as p, i (p.id)}
      <li><i style:background={colors[i % colors.length]}></i>{p.name}<em>{fmt(p.bytes)}</em></li>
    {/each}
  </ul>

  <h4>Safe to clear</h4>
  <div class="list">
    {#each usage.cleanable as c (c.id)}
      <div class="row">
        <div class="text">
          <b>{c.name}</b>
          <span>{c.desc}</span>
        </div>
        <em class="size">{fmt(c.bytes)}</em>
        <button class="btn" disabled={c.bytes === 0 || cleaning !== null} onclick={() => clean(c.id, c.name)}>
          {#if cleaning === c.id}<LoaderCircle size={14} class="spin" />{:else}<Trash2 size={14} />{/if} Clear
        </button>
        {#if c.more}
          {@const m = c.more}
          <button class="btn" title="Every log and crash report, not just old ones: {fmt(m.bytes)}" disabled={m.bytes === 0 || cleaning !== null} onclick={() => clean(m.id, 'All logs and crash reports')}>
            {#if cleaning === m.id}<LoaderCircle size={14} class="spin" />{:else}<Trash2 size={14} />{/if} {m.label} ({fmt(m.bytes)})
          </button>
        {/if}
      </div>
    {/each}
  </div>
  <p class="hint">Downloaded Java builds can be removed under Java & memory. Instances are deleted from their own menu.</p>

  {#if usage.instances.length}
    <h4>Instances</h4>
    <div class="list">
      {#each usage.instances as inst (inst.id)}
        <div class="row small">
          <span class="iname">{inst.name}</span>
          <span class="meter"><span style:width="{(inst.bytes / usage.instances[0].bytes) * 100}%"></span></span>
          <em class="size">{fmt(inst.bytes)}</em>
        </div>
      {/each}
    </div>
  {/if}
{/if}

<style>
  .loading {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 20px 0;
    font-size: 13px;
    color: var(--muted);
  }
  .total {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 12px;
  }
  .total div {
    flex: 1;
    display: flex;
    align-items: baseline;
    gap: 10px;
  }
  .total b {
    font-size: 22px;
    font-weight: 700;
  }
  .total span {
    font-size: 13px;
    color: var(--muted);
  }
  .bar {
    display: flex;
    gap: 2px;
    height: 12px;
    border-radius: 6px;
    overflow: hidden;
    background: rgba(255, 255, 255, 0.05);
  }
  .bar span {
    flex-basis: 0;
    min-width: 3px;
  }
  .legend {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: 6px 16px;
    margin: 12px 0 0;
    padding: 0;
    list-style: none;
    font-size: 12.5px;
    color: var(--muted);
  }
  .legend li {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .legend i {
    width: 10px;
    height: 10px;
    border-radius: 3px;
    flex-shrink: 0;
  }
  .legend em {
    margin-left: auto;
    font-style: normal;
    color: var(--text);
  }
  h4 {
    margin: 22px 0 8px;
    font-size: 13px;
    font-weight: 650;
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 10px;
    border-radius: 10px;
    background: rgba(0, 0, 0, 0.16);
  }
  .row.small {
    padding: 6px 10px;
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .text b {
    font-size: 13px;
    font-weight: 600;
  }
  .text span {
    font-size: 11.5px;
    color: var(--faint);
  }
  .size {
    min-width: 64px;
    text-align: right;
    font-style: normal;
    font-size: 12.5px;
    color: var(--muted);
  }
  .iname {
    width: 34%;
    font-size: 13px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .meter {
    flex: 1;
    height: 6px;
    border-radius: 3px;
    background: rgba(255, 255, 255, 0.06);
    overflow: hidden;
  }
  .meter span {
    display: block;
    height: 100%;
    border-radius: 3px;
    background: var(--accent);
  }
  .hint {
    margin: 8px 0 0;
    font-size: 12px;
    color: var(--faint);
  }
</style>
