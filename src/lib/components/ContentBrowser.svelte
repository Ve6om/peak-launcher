<script lang="ts">
  // Search Modrinth / CurseForge for mods, resource packs or shaders that fit
  // one instance, and install into it.
  import { Search, Download, Heart, Check, LoaderCircle, SearchX, KeyRound, ExternalLink, SlidersHorizontal } from '@lucide/svelte';
  import Select from './Select.svelte';
  import ProjectIcon from './ProjectIcon.svelte';
  import ProjectModal from './ProjectModal.svelte';
  import SourceLogo from './SourceLogo.svelte';
  import { api } from '$lib/api';
  import { app, go, compact, ago, errorText, requestInstall } from '$lib/state.svelte';
  import { CONTENT_KINDS, loaderLabel, sideLabel, type ContentKind, type Instance, type SideFilter, type SearchHit, type Source } from '$lib/types';

  let { inst, kind, onchanged }: { inst: Instance; kind: ContentKind; onchanged: () => void } = $props();

  let source = $state<Source>('modrinth');
  let query = $state('');
  let sort = $state('relevance');
  // Which side a mod runs on. Only Modrinth says, so it only filters there.
  let side = $state<SideFilter>('any');
  const sideFilters = $derived(kind === 'mod' && source === 'modrinth');
  let hits = $state<SearchHit[]>([]);
  let total = $state(0);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let installed = $state(new Set<string>());
  let installing = $state(new Set<string>());
  let open = $state<string | null>(null);

  const label = $derived(CONTENT_KINDS.find((k) => k.id === kind)!);
  const needsKey = $derived(source === 'curseforge' && !app.info?.curseforgeBuiltIn && !app.settings?.curseforgeApiKey?.trim());

  /** Squashed mod ids and titles of what's installed, to recognise a mod
   *  installed from the other site (or by hand). */
  let installedKeys = $state(new Set<string>());
  const squash = (s: string) => s.toLowerCase().replace(/\([^)]*\)|\[[^\]]*\]/g, '').replace(/[^a-z0-9]/g, '');
  const isInstalled = (hit: SearchHit) =>
    installed.has(hit.project_id) || [hit.slug, hit.title].some((s) => s && installedKeys.has(squash(s)));

  async function refreshInstalled() {
    try {
      const [list, ids] = await Promise.all([api.contentList(inst.id, kind), kind === 'mod' ? api.contentModKeys(inst.id) : Promise.resolve([])]);
      installed = new Set(list.map((i) => i.projectId).filter(Boolean) as string[]);
      installedKeys = new Set([...ids, ...list.map((i) => squash(i.title))].filter(Boolean));
    } catch {
      installed = new Set();
      installedKeys = new Set();
    }
  }

  let req = 0;
  async function load(reset: boolean) {
    const my = ++req;
    if (needsKey) {
      hits = [];
      total = 0;
      loading = false;
      return;
    }
    loading = true;
    error = null;
    try {
      const res = await api.contentSearch(source, inst.id, kind, query.trim(), sort, reset ? 0 : hits.length, sideFilters ? side : 'any');
      if (my !== req) return;
      hits = reset ? res.hits : [...hits, ...res.hits];
      total = res.total_hits;
    } catch (e) {
      if (my !== req) return;
      hits = [];
      total = 0;
      error = errorText(e);
    } finally {
      if (my === req) loading = false;
    }
  }

  let timer: ReturnType<typeof setTimeout>;
  $effect(() => {
    inst.id;
    kind;
    source;
    sort;
    side;
    query;
    needsKey;
    clearTimeout(timer);
    timer = setTimeout(() => {
      load(true);
      refreshInstalled();
    }, 250);
    return () => clearTimeout(timer);
  });

  async function install(hit: SearchHit) {
    installing = new Set([...installing, hit.project_id]);
    const ok = await requestInstall(inst.id, kind, hit.project_id);
    installing.delete(hit.project_id);
    installing = new Set(installing);
    if (ok) {
      await refreshInstalled();
      onchanged();
    }
  }

  let sentinel = $state<HTMLDivElement>();
  $effect(() => {
    if (!sentinel) return;
    const io = new IntersectionObserver((entries) => {
      if (entries[0].isIntersecting && !loading && hits.length < total) load(false);
    });
    io.observe(sentinel);
    return () => io.disconnect();
  });
</script>

<div class="browser">
  <div class="bar">
    <div class="seg">
      <button class:active={source === 'modrinth'} onclick={() => (source = 'modrinth')}><SourceLogo source="modrinth" size={20} /> Modrinth</button>
      <button class:active={source === 'curseforge'} onclick={() => (source = 'curseforge')}><SourceLogo source="curseforge" size={20} /> CurseForge</button>
    </div>
    <div class="search">
      <Search size={15} />
      <input bind:value={query} placeholder="Search {label.label.toLowerCase()} for {inst.gameVersion}{kind === 'mod' ? ` ${loaderLabel(inst.loader)}` : ''}…" />
      {#if loading}<LoaderCircle size={15} class="spin" />{/if}
    </div>
    {#if kind === 'mod'}
      <Select
        bind:value={side}
        disabled={!sideFilters}
        title={sideFilters ? 'Where the mod runs' : "CurseForge doesn't say which side mods run on"}
        ariaLabel="Runs on"
        style="flex: 0 0 auto; width: auto; min-width: 150px;"
        options={[
          { value: 'any', label: 'Any side' },
          { value: 'client', label: 'Client only' },
          { value: 'server', label: 'Server only' },
          { value: 'both', label: 'Client and server' }
        ]}
      />
    {/if}
    <Select
      bind:value={sort}
      ariaLabel="Sort"
      style="flex: 0 0 auto; width: auto; min-width: 150px;"
      options={[
        { value: 'relevance', label: 'Relevance' },
        { value: 'downloads', label: 'Most downloaded' },
        { value: 'follows', label: 'Most followed' },
        { value: 'updated', label: 'Recently updated' },
        { value: 'newest', label: 'Newest' }
      ]}
    />
  </div>

  {#if needsKey}
    <div class="empty glass">
      <KeyRound size={28} />
      <h3>CurseForge needs an API key</h3>
      <p>Get a free key at console.curseforge.com and paste it into Settings → Integrations.</p>
      <div class="actions">
        <button class="btn" onclick={() => api.openUrl('https://console.curseforge.com/')}><ExternalLink size={15} /> Get a key</button>
        <button class="btn primary" onclick={() => go('settings')}>Open Settings</button>
      </div>
    </div>
  {:else if error}
    <div class="empty glass">
      <SearchX size={28} />
      <h3>Can't show {label.label.toLowerCase()}</h3>
      <p>{error}</p>
      {#if inst.loader === 'vanilla' && kind === 'mod'}
        <button class="btn primary" onclick={() => go('instance', inst.id, 'settings')}><SlidersHorizontal size={15} /> Add a mod loader</button>
      {/if}
    </div>
  {:else}
    <div class="list">
      {#each hits as hit (hit.project_id)}
        <div class="row glass">
          <button class="open" aria-label="Details for {hit.title}" onclick={() => (open = hit.project_id)}></button>
          <ProjectIcon url={hit.icon_url} title={hit.title} size={48} />
          <div class="info">
            <div class="title-line"><b>{hit.title}</b><span class="by">by {hit.author}</span></div>
            <p>{hit.description}</p>
            <div class="tags">
              <span class="stat"><Download size={12} /> {compact(hit.downloads)}</span>
              <span class="stat"><Heart size={12} /> {compact(hit.follows)}</span>
              <span class="stat">Updated {ago(hit.date_modified).toLowerCase()}</span>
              {#if kind === 'mod' && sideLabel(hit)}<span class="side">{sideLabel(hit)}</span>{/if}
            </div>
          </div>
          <div class="action">
            {#if isInstalled(hit)}
              <span class="installed"><Check size={15} /> Installed</span>
            {:else}
              <button class="btn primary" disabled={installing.has(hit.project_id)} onclick={() => install(hit)}>
                {#if installing.has(hit.project_id)}<LoaderCircle size={15} class="spin" />{:else}<Download size={15} />{/if} Install
              </button>
            {/if}
          </div>
        </div>
      {:else}
        {#if !loading}
          <div class="empty glass"><SearchX size={28} /><h3>No results</h3><p>Try a different search.</p></div>
        {/if}
      {/each}
      <div bind:this={sentinel} class="sentinel">{#if loading && hits.length}<LoaderCircle size={18} class="spin" />{/if}</div>
    </div>
  {/if}
</div>

{#if open}
  <ProjectModal
    projectId={open}
    instanceId={inst.id}
    {kind}
    installed={installed.has(open) || hits.some((h) => h.project_id === open && isInstalled(h))}
    onclose={() => (open = null)}
    oninstalled={() => {
      refreshInstalled();
      onchanged();
    }}
  />
{/if}

<style>
  .browser {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .bar {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    align-items: center;
  }
  .bar .seg button {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    padding-left: 6px;
  }
  .search {
    flex: 1 1 220px;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    height: 38px;
    padding: 0 12px;
    border-radius: 12px;
    background: rgba(0, 0, 0, 0.2);
    border: 1px solid var(--stroke);
    color: var(--faint);
  }
  .search input {
    flex: 1;
    min-width: 0;
    background: none;
    border: 0;
    outline: none;
    font-size: 13px;
    color: var(--text);
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .row {
    position: relative;
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 12px 14px;
    transition: border-color 0.2s, background 0.2s;
  }
  .row:hover {
    background: var(--glass-hover);
    border-color: var(--stroke-strong);
  }
  .open {
    position: absolute;
    inset: 0;
    border-radius: inherit;
  }
  .info {
    flex: 1;
    min-width: 0;
    pointer-events: none;
  }
  .title-line {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }
  .title-line b {
    font-size: 14px;
    font-weight: 640;
  }
  .by {
    font-size: 12px;
    color: var(--faint);
  }
  .info p {
    margin: 3px 0 6px;
    font-size: 12.5px;
    color: var(--muted);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .tags {
    display: flex;
    gap: 12px;
  }
  .stat {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 11.5px;
    color: var(--faint);
  }
  .side {
    padding: 1px 7px;
    border-radius: 99px;
    font-size: 11px;
    font-weight: 600;
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 14%, transparent);
  }
  .action {
    position: relative;
    z-index: 1;
    flex-shrink: 0;
  }
  .installed {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 38px;
    padding: 0 14px;
    border-radius: 12px;
    font-size: 13px;
    font-weight: 560;
    color: #a7f3d0;
    background: rgba(16, 185, 129, 0.12);
    border: 1px solid rgba(52, 211, 153, 0.25);
  }
  .sentinel {
    display: grid;
    place-items: center;
    height: 36px;
    color: var(--muted);
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 40px;
    text-align: center;
    color: var(--faint);
  }
  .empty h3 {
    color: var(--text);
    font-size: 15px;
  }
  .empty p {
    margin: 0 0 6px;
    font-size: 13px;
    max-width: 420px;
  }
  .actions {
    display: flex;
    gap: 8px;
  }
</style>
