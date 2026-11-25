<script lang="ts">
  // Browse modpacks from Modrinth, CurseForge, Feed The Beast and Technic.
  // Installing one creates a new instance. (Mods, resource packs and shaders
  // are added from each instance's Content tab.)
  import { untrack } from 'svelte';
  import Select from '$lib/components/Select.svelte';
  import { Search, Download, Heart, Check, LoaderCircle, SearchX, FileUp, KeyRound, ExternalLink } from '@lucide/svelte';
  import ProjectIcon from '$lib/components/ProjectIcon.svelte';
  import ProjectModal from '$lib/components/ProjectModal.svelte';
  import SourceLogo from '$lib/components/SourceLogo.svelte';
  import { api } from '$lib/api';
  import { app, go, compact, ago, errorText, installModpack, importModpack } from '$lib/state.svelte';
  import type { SearchHit, Source } from '$lib/types';

  const SOURCES: { id: Source; label: string; note: string }[] = [
    { id: 'modrinth', label: 'Modrinth', note: 'Open-source modpacks' },
    { id: 'curseforge', label: 'CurseForge', note: 'The largest library' },
    { id: 'ftb', label: 'FTB', note: 'Official Feed The Beast packs' },
    { id: 'technic', label: 'Technic', note: 'Classics like Tekkit' }
  ];

  let source = $state<Source>(untrack(() => app.browse.source));
  let query = $state('');
  /** Technic has no categories; these are searches by name (see technic.rs). */
  const TECHNIC_CATEGORIES = [
    { id: '', label: 'Trending' },
    { id: 'tech', label: 'Tech' },
    { id: 'magic', label: 'Magic' },
    { id: 'adventure', label: 'Adventure' },
    { id: 'rpg', label: 'RPG' },
    { id: 'skyblock', label: 'Skyblock' },
    { id: 'pixelmon', label: 'Pixelmon' },
    { id: 'horror', label: 'Horror' },
    { id: 'survival', label: 'Survival' },
    { id: '1.12.2', label: '1.12.2' },
    { id: '1.7.10', label: '1.7.10' }
  ];
  let technicCategory = $state('');
  const technicBrowsing = $derived(source === 'technic' && !query.trim());
  let sort = $state('relevance');
  let hits = $state<SearchHit[]>([]);
  let total = $state(0);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let installing = $state(new Set<string>());
  let open = $state<string | null>(null);

  const needsKey = $derived(source === 'curseforge' && !app.info?.curseforgeBuiltIn && !app.settings?.curseforgeApiKey?.trim());
  /** Packs that already have an instance, with that instance's id. */
  const installedPacks = $derived(new Map(app.instances.filter((i) => i.pack?.projectId).map((i) => [i.pack!.projectId!, i.id])));

  $effect(() => {
    app.browse = { ...untrack(() => app.browse), source, kind: 'modpack' };
  });

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
      const category = source === 'technic' && technicCategory ? technicCategory : null;
      const res = await api.modpackSearch(source, query.trim(), sort, reset ? 0 : hits.length, category);
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
    source;
    sort;
    query;
    technicCategory;
    needsKey;
    clearTimeout(timer);
    timer = setTimeout(() => load(true), 250);
    return () => clearTimeout(timer);
  });

  async function install(hit: SearchHit) {
    installing = new Set([...installing, hit.project_id]);
    await installModpack({ projectId: hit.project_id });
    installing.delete(hit.project_id);
    installing = new Set(installing);
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

<div class="page">
  <header>
    <div>
      <h1>Modpacks</h1>
      <p>Install a modpack and Peak sets up a ready-to-play instance. Add single mods from an instance's Content tab.</p>
    </div>
    <button class="btn" onclick={importModpack}><FileUp size={15} /> Import modpack file</button>
  </header>

  <div class="sources">
    {#each SOURCES as s (s.id)}
      <button class="src glass" class:on={source === s.id} onclick={() => (source = s.id)}>
        <SourceLogo source={s.id} size={36} />
        <span class="src-text">
          <b>{s.label}</b>
          <span>{s.note}</span>
        </span>
      </button>
    {/each}
  </div>

  <div class="bar">
    <div class="search glass">
      <Search size={15} />
      <input bind:value={query} placeholder="Search {SOURCES.find((s) => s.id === source)?.label} modpacks…" />
      {#if loading}<LoaderCircle size={15} class="spin" />{/if}
    </div>
    <Select
      bind:value={sort}
      ariaLabel="Sort"
      style="width: 180px; height: 40px; flex-shrink: 0;"
      options={[
        { value: 'relevance', label: source === 'ftb' && !query ? 'Featured' : 'Relevance' },
        { value: 'downloads', label: 'Most downloaded' },
        { value: 'follows', label: source === 'ftb' || source === 'technic' ? 'Most played' : 'Most followed' },
        ...(source !== 'technic'
          ? [
              { value: 'updated', label: 'Recently updated' },
              { value: 'newest', label: 'Newest' }
            ]
          : [])
      ]}
    />
  </div>

  {#if technicBrowsing}
    <div class="cats" role="tablist" aria-label="Technic categories">
      {#each TECHNIC_CATEGORIES as c (c.id)}
        <button role="tab" aria-selected={technicCategory === c.id} class:on={technicCategory === c.id} onclick={() => (technicCategory = c.id)}>{c.label}</button>
      {/each}
    </div>
    <p class="cats-note">
      {#if technicCategory}
        Technic doesn't sort packs into categories, so these are found by name and description. A few may not quite fit. Search to find any other Technic pack.
      {:else}
        Technic only shares its 20 trending packs with launchers. Pick a category above or search to find any other Technic pack.
      {/if}
    </p>
  {/if}

  {#if needsKey}
    <div class="empty glass">
      <KeyRound size={30} />
      <h3>CurseForge needs an API key</h3>
      <p>CurseForge only allows apps with their own key. Get a free one at console.curseforge.com, then paste it into Settings → Integrations.</p>
      <div class="empty-actions">
        <button class="btn" onclick={() => api.openUrl('https://console.curseforge.com/')}><ExternalLink size={15} /> Get a key</button>
        <button class="btn primary" onclick={() => go('settings')}>Open Settings</button>
      </div>
    </div>
  {:else if error}
    <div class="empty glass">
      <SearchX size={30} />
      <h3>Can't load modpacks</h3>
      <p>{error}</p>
    </div>
  {:else}
    <div class="meta-line">{#if total}{compact(total)} modpacks{/if}</div>
    <div class="list">
      {#each hits as hit, i (hit.project_id)}
        {@const have = installedPacks.get(hit.project_id)}
        <div class="row glass rise" style:animation-delay="{Math.min(i, 12) * 25}ms">
          <button class="open" aria-label="Details for {hit.title}" onclick={() => (open = hit.project_id)}></button>
          <ProjectIcon url={hit.icon_url} title={hit.title} size={64} />
          <div class="info">
            <div class="title-line"><b>{hit.title}</b><span class="by">by {hit.author}</span></div>
            <p>{hit.description}</p>
            <div class="tags">
              <span class="stat"><Download size={12} /> {compact(hit.downloads)}</span>
              <span class="stat"><Heart size={12} /> {compact(hit.follows)}</span>
              <span class="stat">Updated {ago(hit.date_modified).toLowerCase()}</span>
              {#each hit.categories.filter((c) => !['fabric', 'forge', 'quilt', 'neoforge', 'FTB'].includes(c)).slice(0, 3) as c (c)}
                <span class="chip">{c}</span>
              {/each}
            </div>
          </div>
          <div class="action">
            {#if have}
              <button class="btn" onclick={() => go('instance', have)}><Check size={15} /> Open</button>
            {/if}
            <button class="btn primary" disabled={installing.has(hit.project_id)} onclick={() => install(hit)}>
              {#if installing.has(hit.project_id)}<LoaderCircle size={15} class="spin" />{:else}<Download size={15} />{/if}
              {have ? 'Install again' : 'Install'}
            </button>
          </div>
        </div>
      {:else}
        {#if !loading}
          <div class="empty glass"><SearchX size={30} /><h3>No modpacks found</h3><p>Try a different search.</p></div>
        {/if}
      {/each}
      <div bind:this={sentinel} class="sentinel">{#if loading && hits.length}<LoaderCircle size={18} class="spin" />{/if}</div>
    </div>
  {/if}
</div>

{#if open}
  <ProjectModal projectId={open} instanceId="" kind="modpack" onclose={() => (open = null)} oninstalled={() => {}} />
{/if}

<style>
  .cats {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: -4px;
  }
  .cats button {
    height: 30px;
    padding: 0 12px;
    border-radius: 99px;
    border: 1px solid var(--stroke);
    background: rgba(255, 255, 255, 0.03);
    color: var(--muted);
    font: inherit;
    font-size: 12.5px;
    cursor: pointer;
    transition: background 0.15s, color 0.15s, border-color 0.15s;
  }
  .cats button:hover {
    color: var(--text);
    background: rgba(255, 255, 255, 0.06);
  }
  .cats button.on {
    color: var(--accent);
    border-color: color-mix(in srgb, var(--accent) 45%, transparent);
    background: color-mix(in srgb, var(--accent) 12%, transparent);
  }
  .cats-note {
    margin: 0;
    font-size: 12px;
    color: var(--faint);
  }
  .page {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    gap: 16px;
    flex-wrap: wrap;
  }
  h1 {
    font-size: 28px;
    font-weight: 720;
  }
  header p {
    margin: 6px 0 0;
    color: var(--muted);
    font-size: 13.5px;
    max-width: 560px;
  }
  .sources {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 10px;
  }
  .src {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 14px;
    text-align: left;
    transition: border-color 0.2s, background 0.2s, transform 0.2s var(--ease);
  }
  .src-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .src b {
    font-size: 14.5px;
  }
  .src-text span {
    font-size: 11.5px;
    color: var(--faint);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .src:hover {
    border-color: var(--stroke-strong);
  }
  .src.on {
    border-color: color-mix(in srgb, var(--accent) 55%, transparent);
    background: linear-gradient(135deg, color-mix(in srgb, var(--accent) 18%, transparent), color-mix(in srgb, var(--accent-2) 8%, transparent));
  }
  .bar {
    position: sticky;
    top: -18px;
    z-index: 4;
    display: flex;
    gap: 10px;
    align-items: center;
    padding: 6px 0;
  }
  .search {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 8px;
    height: 40px;
    padding: 0 12px;
    border-radius: 12px;
    color: var(--faint);
    background: rgba(14, 16, 26, 0.7);
  }
  .search input {
    flex: 1;
    background: none;
    border: 0;
    outline: none;
    font-size: 13.5px;
    color: var(--text);
  }
  .meta-line {
    min-height: 16px;
    font-size: 12px;
    color: var(--faint);
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
    gap: 16px;
    padding: 14px 16px;
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
    font-size: 15.5px;
    font-weight: 650;
  }
  .by {
    font-size: 12px;
    color: var(--faint);
  }
  .info p {
    margin: 4px 0 8px;
    font-size: 13px;
    color: var(--muted);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .tags {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 12px;
  }
  .stat {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 12px;
    color: var(--faint);
  }
  .action {
    position: relative;
    z-index: 1;
    flex-shrink: 0;
    display: flex;
    gap: 8px;
  }
  .sentinel {
    display: grid;
    place-items: center;
    height: 40px;
    color: var(--muted);
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 48px;
    color: var(--faint);
    text-align: center;
  }
  .empty h3 {
    color: var(--text);
    font-size: 16px;
    margin-top: 4px;
  }
  .empty p {
    margin: 0 0 6px;
    font-size: 13px;
    max-width: 420px;
  }
  .empty-actions {
    display: flex;
    gap: 8px;
  }
</style>
