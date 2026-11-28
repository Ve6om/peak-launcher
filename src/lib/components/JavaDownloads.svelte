<script lang="ts">
  import { t } from '$lib/i18n.svelte';
  // Every Java Peak has downloaded, how much space each takes, and a way to
  // remove the ones you don't need. Peak downloads one again if a game needs it.
  import { onMount } from 'svelte';
  import { Trash2 } from '@lucide/svelte';
  import { api } from '$lib/api';
  import { toast, errorText } from '$lib/state.svelte';
  import type { InstalledJava } from '$lib/types';

  let list = $state<InstalledJava[] | null>(null);
  let busy = $state<string | null>(null);
  let confirm = $state<string | null>(null);

  async function load() {
    try {
      list = await api.installedJava();
    } catch {
      list = [];
    }
  }
  onMount(load);

  const total = $derived((list ?? []).reduce((n, j) => n + j.sizeBytes, 0));
  const size = (b: number) => (b >= 1e9 ? t('{x} GB', { x: (b / 1e9).toFixed(1) }) : t('{n} MB', { n: Math.max(1, Math.round(b / 1e6)) }));

  async function remove(j: InstalledJava) {
    if (confirm !== j.id) {
      confirm = j.id;
      setTimeout(() => confirm === j.id && (confirm = null), 4000);
      return;
    }
    confirm = null;
    busy = j.id;
    try {
      await api.removeJava(j.id);
      toast(t('Removed {label}', { label: j.label }), 'success');
      await load();
    } catch (e) {
      toast(errorText(e), 'error');
    } finally {
      busy = null;
    }
  }
</script>

<div class="java-list">
  {#if list === null}
    <p class="empty">{t('Looking…')}</p>
  {:else if !list.length}
    <p class="empty">{t('No Java downloaded yet. Peak gets the right one the first time you play.')}</p>
  {:else}
    {#each list as j (j.id)}
      <div class="item">
        <span class="vendor">{j.vendor}</span>
        <span class="jname">{j.label}</span>
        <span class="size">{size(j.sizeBytes)}</span>
        <button class="btn ghost small" class:danger={confirm === j.id} disabled={busy === j.id} onclick={() => remove(j)} title={t('Remove')}>
          <Trash2 size={14} />
          {confirm === j.id ? t('Sure?') : t('Remove')}
        </button>
      </div>
    {/each}
    <p class="total">{t('{size} in total. Removed ones download again the next time a game needs them.', { size: size(total) })}</p>
  {/if}
</div>

<style>
  .java-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    width: 100%;
  }
  .item {
    display: grid;
    grid-template-columns: 80px 1fr auto auto;
    align-items: center;
    gap: 12px;
    padding: 8px 10px;
    border: 1px solid var(--stroke);
    border-radius: var(--radius-sm);
    background: rgba(255, 255, 255, 0.025);
    font-size: 13px;
  }
  .vendor {
    font-size: 11px;
    font-weight: 650;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--faint, var(--muted));
  }
  .size {
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  .small {
    height: 28px;
    padding: 0 10px;
    font-size: 12px;
    gap: 6px;
  }
  .danger {
    color: var(--danger, #ff6464);
  }
  .empty,
  .total {
    margin: 2px 0 0;
    font-size: 12px;
    color: var(--muted);
  }
</style>
