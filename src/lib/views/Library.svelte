<script lang="ts">
  import { Plus, Search, Layers, Package, FileUp, FolderInput } from '@lucide/svelte';
  import InstanceCard from '$lib/components/InstanceCard.svelte';
  import { app, browseFor, importModpack } from '$lib/state.svelte';

  let query = $state('');
  let sort = $state<'recent' | 'name' | 'version'>('recent');

  const shown = $derived.by(() => {
    const q = query.trim().toLowerCase();
    const list = app.instances.filter((i) => !q || i.name.toLowerCase().includes(q) || i.gameVersion.includes(q));
    if (sort === 'name') return [...list].sort((a, b) => a.name.localeCompare(b.name));
    if (sort === 'version') return [...list].sort((a, b) => b.gameVersion.localeCompare(a.gameVersion, undefined, { numeric: true }));
    return list;
  });
</script>

<div class="library">
  <header>
    <div>
      <h1>Library</h1>
      <p>{app.instances.length} instance{app.instances.length === 1 ? '' : 's'}</p>
    </div>
    <div class="tools">
      <div class="search glass">
        <Search size={15} />
        <input bind:value={query} placeholder="Search instances" />
      </div>
      <div class="seg glass">
        <button class:active={sort === 'recent'} onclick={() => (sort = 'recent')}>Recent</button>
        <button class:active={sort === 'name'} onclick={() => (sort = 'name')}>Name</button>
        <button class:active={sort === 'version'} onclick={() => (sort = 'version')}>Version</button>
      </div>
      <button class="btn icon" title="Import a modpack file (.mrpack or CurseForge .zip)" aria-label="Import modpack file" onclick={importModpack}><FileUp size={16} /></button>
      <button class="btn" onclick={() => (app.importing = true)}><FolderInput size={16} /> Import</button>
      <button class="btn" onclick={() => browseFor(null, 'modpack')}><Package size={16} /> Modpacks</button>
      <button class="btn primary" onclick={() => (app.creating = true)}><Plus size={17} /> New instance</button>
    </div>
  </header>

  {#if shown.length}
    <div class="grid">
      {#each shown as inst, i (inst.id)}
        <InstanceCard {inst} index={i} />
      {/each}
    </div>
  {:else}
    <div class="empty glass">
      <Layers size={30} />
      <h3>{query ? 'No matches' : 'No instances yet'}</h3>
      <p>{query ? 'Try a different search.' : 'Create one to get started.'}</p>
    </div>
  {/if}
</div>

<style>
  .library {
    display: flex;
    flex-direction: column;
    gap: 22px;
  }
  header {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 16px;
    flex-wrap: wrap;
  }
  h1 {
    font-size: 28px;
    font-weight: 720;
  }
  header p {
    margin: 4px 0 0;
    color: var(--muted);
    font-size: 13.5px;
  }
  .tools {
    display: flex;
    gap: 10px;
    align-items: center;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 38px;
    width: 240px;
    padding: 0 12px;
    border-radius: 12px;
    color: var(--faint);
  }
  .search input {
    flex: 1;
    min-width: 0;
    background: none;
    border: 0;
    outline: none;
    font-size: 13px;
  }
  .seg {
    display: flex;
    padding: 3px;
    border-radius: 12px;
    gap: 2px;
  }
  .seg button {
    height: 30px;
    padding: 0 12px;
    border-radius: 9px;
    font-size: 12.5px;
    font-weight: 560;
    color: var(--muted);
  }
  .seg button.active {
    background: rgba(255, 255, 255, 0.09);
    color: var(--text);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: 16px;
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 60px;
    color: var(--faint);
  }
  .empty h3 {
    color: var(--text);
    margin-top: 6px;
    font-size: 16px;
  }
  .empty p {
    margin: 0;
    font-size: 13px;
  }
</style>
