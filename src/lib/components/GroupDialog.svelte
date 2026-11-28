<script lang="ts">
  import { t } from '$lib/i18n.svelte';
  // File an instance under a Library group: an existing one, a new one, or none.
  import { FolderTree, Check, X } from '@lucide/svelte';
  import Modal from './Modal.svelte';
  import { api } from '$lib/api';
  import { app, toast, errorText, refreshInstances } from '$lib/state.svelte';
  import type { Instance } from '$lib/types';

  let { inst, onclose }: { inst: Instance; onclose: () => void } = $props();

  const groups = $derived([...new Set(app.instances.map((i) => i.group).filter((g): g is string => !!g))].sort((a, b) => a.localeCompare(b)));
  let name = $state('');
  let busy = $state(false);

  async function set(group: string | null) {
    busy = true;
    try {
      await api.instanceSetGroup(inst.id, group);
      await refreshInstances();
      toast(group ? t('{name} is in {group}', { name: inst.name, group }) : t('{name} is no longer in a group', { name: inst.name }), 'success');
      onclose();
    } catch (e) {
      toast(errorText(e), 'error');
    } finally {
      busy = false;
    }
  }
</script>

<Modal title={inst.group ? t('Change group') : t('Add to group')} subtitle={inst.name} width={420} {onclose}>
  {#if groups.length}
    <div class="groups">
      {#each groups as g (g)}
        <button class="group" class:current={inst.group === g} disabled={busy} onclick={() => set(g)}>
          <FolderTree size={15} />
          <span>{g}</span>
          <em>{app.instances.filter((i) => i.group === g).length}</em>
          {#if inst.group === g}<Check size={15} />{/if}
        </button>
      {/each}
    </div>
  {:else}
    <p class="hint">{t('Groups sort the Library into sections, like Modded, Vanilla or Servers. Name the first one:')}</p>
  {/if}

  <form
    class="new"
    onsubmit={(e) => {
      e.preventDefault();
      if (name.trim()) set(name.trim());
    }}
  >
    <input class="input" bind:value={name} placeholder={t('New group')} maxlength="40" />
    <button class="btn primary" type="submit" disabled={busy || !name.trim()}>{t('Add')}</button>
  </form>

  {#if inst.group}
    <div class="actions">
      <button class="btn ghost" disabled={busy} onclick={() => set(null)}><X size={14} /> {t('Take out of {group}', { group: inst.group })}</button>
    </div>
  {/if}
</Modal>

<style>
  .groups {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-bottom: 12px;
  }
  .group {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 40px;
    padding: 0 12px;
    border-radius: 10px;
    border: 1px solid var(--stroke);
    background: rgba(255, 255, 255, 0.025);
    font-size: 13.5px;
    text-align: left;
    color: var(--muted);
  }
  .group:hover {
    background: rgba(255, 255, 255, 0.06);
    color: var(--text);
  }
  .group.current {
    border-color: color-mix(in srgb, var(--accent) 45%, transparent);
    color: var(--accent);
  }
  .group span {
    flex: 1;
    color: var(--text);
  }
  .group em {
    font-style: normal;
    font-size: 12px;
    color: var(--faint);
  }
  .hint {
    margin: 0 0 10px;
    font-size: 13px;
    color: var(--muted);
  }
  .new {
    display: flex;
    gap: 8px;
  }
  .new .input {
    flex: 1;
  }
  .actions {
    display: flex;
    justify-content: flex-start;
    margin-top: 12px;
  }
</style>
