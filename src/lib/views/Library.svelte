<script lang="ts">
  import { Plus, Search, Layers, Package, FileUp, FolderInput, ChevronDown } from '@lucide/svelte';
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

  /** Groups in name order, then instances not in one. One section, without a
   *  heading, when nothing is grouped. */
  const sections = $derived.by(() => {
    const names = [...new Set(shown.map((i) => i.group).filter((g): g is string => !!g))].sort((a, b) => a.localeCompare(b));
    if (!names.length) return [{ name: null as string | null, items: shown }];
    const out = names.map((name) => ({ name: name as string | null, items: shown.filter((i) => i.group === name) }));
    const rest = shown.filter((i) => !i.group);
    if (rest.length) out.push({ name: 'Not in a group', items: rest });
    return out;
  });

  // Collapsed groups are remembered on this computer.
  const KEY = 'peak.collapsedGroups';
  let collapsed = $state<string[]>(read());
  function read(): string[] {
    try {
      return JSON.parse(localStorage.getItem(KEY) ?? '[]');
    } catch {
      return [];
    }
  }
  function toggle(name: string) {
    collapsed = collapsed.includes(name) ? collapsed.filter((n) => n !== name) : [...collapsed, name];
    try {
      localStorage.setItem(KEY, JSON.stringify(collapsed));
    } catch {
      // Not remembered; fine.
    }
  }
  /** Searching opens every group, so matches aren't hidden. */
  const isOpen = (name: string | null) => !name || !!query.trim() || !collapsed.includes(name);
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
    {#each sections as sec (sec.name ?? '')}
      <section class="section">
        {#if sec.name}
          <button class="heading" aria-expanded={isOpen(sec.name)} onclick={() => toggle(sec.name!)}>
            <span class="chev" class:shut={!isOpen(sec.name)}><ChevronDown size={16} /></span>
            <span class="title">{sec.name}</span>
            <span class="count">{sec.items.length}</span>
          </button>
        {/if}
        {#if isOpen(sec.name)}
          <div class="grid">
            {#each sec.items as inst, i (inst.id)}
              <InstanceCard {inst} index={i} />
            {/each}
          </div>
        {/if}
      </section>
    {/each}
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
  .section {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .heading {
    display: flex;
    align-items: center;
    gap: 8px;
    align-self: flex-start;
    padding: 4px 8px 4px 2px;
    border-radius: 8px;
    color: var(--muted);
  }
  .heading:hover {
    color: var(--text);
  }
  .heading .title {
    font-size: 15px;
    font-weight: 650;
    color: var(--text);
  }
  .heading .count {
    padding: 1px 7px;
    border-radius: 7px;
    font-size: 11.5px;
    background: rgba(255, 255, 255, 0.07);
  }
  .chev {
    display: grid;
    transition: transform 0.18s;
  }
  .chev.shut {
    transform: rotate(-90deg);
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
