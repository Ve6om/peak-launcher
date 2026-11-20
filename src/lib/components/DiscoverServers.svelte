<script lang="ts">
  // Discover servers: browse and filter the public list, see live status,
  // vote once a day, and join in one click (modded servers install their
  // pack first).
  import { onMount } from 'svelte';
  import {
    Search,
    Users,
    ThumbsUp,
    Play,
    Star,
    BadgeCheck,
    Server as ServerIcon,
    SignalZero,
    LoaderCircle,
    Copy,
    ExternalLink,
    Bookmark,
    Package,
    Globe,
    Flag
  } from '@lucide/svelte';
  import Modal from './Modal.svelte';
  import LoaderIcon from './LoaderIcon.svelte';
  import PackPicker from './PackPicker.svelte';
  import { api } from '$lib/api';
  import { app, toast, errorText, isBusy, isRunning } from '$lib/state.svelte';
  import { motdLines, plain } from '$lib/motd';
  import { pendingJoins, watchPendingJoins, vanillaChoices, packInstance, playListing, serverLink } from '$lib/discover.svelte';
  import { loaderLabel, type Listing, type ServerStatus } from '$lib/types';

  let listings = $state<Listing[]>([]);
  let status = $state<Record<string, ServerStatus | 'loading'>>({});
  let loaded = $state(false);
  let loadError = $state<string | null>(null);
  let open = $state<Listing | null>(null);
  /** Vanilla listings: the instance picked to join with ('new' makes one). */
  let picked = $state<Record<string, string>>({});

  // ---- filters ----
  let q = $state('');
  let kind = $state<'all' | 'vanilla' | 'modded'>('all');
  let tag = $state('');
  let packFilter = $state('');
  let version = $state('');
  let onlineOnly = $state(false);
  let sort = $state<'votes' | 'players' | 'name'>('votes');

  onMount(async () => {
    watchPendingJoins();
    try {
      listings = await api.discoverList();
      listings.forEach(ping);
    } catch (e) {
      loadError = errorText(e);
    } finally {
      loaded = true;
    }
  });

  // Open the server a peak:// link points to, once the list has loaded.
  $effect(() => {
    const id = serverLink.id;
    if (!id || !loaded) return;
    serverLink.id = null;
    const found = listings.find((l) => l.id === id);
    if (found) open = found;
    else toast('That server isn’t on the list any more.', 'info');
  });

  async function ping(l: Listing) {
    status[l.id] = 'loading';
    status[l.id] = await api.serverPing(l.address).catch(
      (e): ServerStatus => ({ online: false, error: errorText(e), version: null, protocol: null, playersOnline: null, playersMax: null, playersSample: [], motd: null, favicon: null, pingMs: null })
    );
  }

  const live = (l: Listing) => {
    const s = status[l.id];
    return s && s !== 'loading' ? s : null;
  };

  const allTags = $derived([...new Set(listings.flatMap((l) => l.tags))].sort());
  // Modpacks on the list, most-used first, with an icon and a server count.
  const allPacks = $derived.by(() => {
    const byPack = new Map<string, { id: string; name: string; icon: string | null; count: number }>();
    for (const l of listings) {
      if (!l.pack) continue;
      const o = byPack.get(l.pack.projectId) ?? { id: l.pack.projectId, name: l.pack.name, icon: l.pack.iconUrl, count: 0 };
      o.count++;
      byPack.set(l.pack.projectId, o);
    }
    return [...byPack.values()].sort((a, b) => b.count - a.count || a.name.localeCompare(b.name));
  });
  const allVersions = $derived([...new Set(listings.map((l) => l.gameVersion))].sort().reverse());

  const shown = $derived.by(() => {
    const needle = q.trim().toLowerCase();
    const list = listings.filter((l) => {
      if (kind === 'vanilla' && l.pack) return false;
      if (kind === 'modded' && !l.pack) return false;
      if (tag && !l.tags.includes(tag)) return false;
      if (packFilter && l.pack?.projectId !== packFilter) return false;
      if (version && l.gameVersion !== version) return false;
      if (onlineOnly && !live(l)?.online) return false;
      if (needle) {
        const hay = [l.name, l.tagline, l.address, l.pack?.name ?? '', ...l.tags].join(' ').toLowerCase();
        if (!hay.includes(needle)) return false;
      }
      return true;
    });
    const players = (l: Listing) => live(l)?.playersOnline ?? -1;
    return list.sort((a, b) => (sort === 'name' ? a.name.localeCompare(b.name) : sort === 'players' ? players(b) - players(a) : b.votes - a.votes));
  });
  const featured = $derived(shown.filter((l) => l.featured));
  const filtering = $derived(!!(q.trim() || kind !== 'all' || tag || packFilter || version || onlineOnly));

  function chosen(l: Listing): string {
    if (picked[l.id]) return picked[l.id];
    const best = vanillaChoices(l)[0];
    return best && best.gameVersion === l.gameVersion ? best.id : 'new';
  }

  /** Votes are made on the server's page on the website, where they're checked. */
  function vote(l: Listing) {
    if (!l.pageUrl) return toast('This server has no page to vote on yet.', 'info');
    api.openUrl(`${l.pageUrl}#vote`);
  }

  async function save(l: Listing) {
    try {
      const list = await api.serversList();
      if (list.some((s) => s.address.toLowerCase() === l.address.toLowerCase())) {
        toast(`${l.name} is already in My servers`, 'info');
        return;
      }
      await api.serverSave({ id: '', name: l.name, address: l.address, instanceId: null });
      toast(`Added ${l.name} to My servers`, 'success');
    } catch (e) {
      toast(errorText(e), 'error');
    }
  }

  async function copy(address: string) {
    await navigator.clipboard.writeText(address).catch(() => {});
    toast('Address copied');
  }

  function playLabel(l: Listing): string {
    const inst = packInstance(l);
    if (inst && pendingJoins[inst.id]) return 'Installing…';
    if (l.pack && !inst) return 'Install & play';
    return 'Play';
  }

  const busyFor = (l: Listing) => {
    const inst = l.pack ? packInstance(l) : app.instances.find((i) => i.id === chosen(l));
    return !!inst && (isRunning(inst.id) || isBusy(inst.id) || !!pendingJoins[inst.id]);
  };

  function bars(ms: number | null): number {
    if (ms == null) return 1;
    return ms < 80 ? 4 : ms < 150 ? 3 : ms < 300 ? 2 : 1;
  }

  const compact = (n: number) => (n >= 1000 ? `${(n / 1000).toFixed(n >= 10000 ? 0 : 1)}k` : String(n));
</script>

<div class="discover">

  <div class="filters glass">
    <label class="search">
      <Search size={15} />
      <input bind:value={q} placeholder="Search servers, tags or modpacks" spellcheck="false" />
    </label>
    <div class="seg">
      <button class:active={kind === 'all'} onclick={() => (kind = 'all')}>All</button>
      <button class:active={kind === 'vanilla'} onclick={() => (kind = 'vanilla')}>Vanilla</button>
      <button class:active={kind === 'modded'} onclick={() => (kind = 'modded')}>Modpacks</button>
    </div>
    <select class="input small" bind:value={tag} aria-label="Tag">
      <option value="">Any type</option>
      {#each allTags as t (t)}<option value={t}>{t}</option>{/each}
    </select>
    <PackPicker options={allPacks} bind:value={packFilter} />
    <select class="input small" bind:value={version} aria-label="Minecraft version">
      <option value="">Any version</option>
      {#each allVersions as v (v)}<option value={v}>{v}</option>{/each}
    </select>
    <label class="check"><input type="checkbox" bind:checked={onlineOnly} /> Online now</label>
    <select class="input small" bind:value={sort} aria-label="Sort">
      <option value="votes">Most votes</option>
      <option value="players">Most players</option>
      <option value="name">Name</option>
    </select>
  </div>

  {#if !loaded}
    <div class="empty"><LoaderCircle size={18} class="spin" /> Loading servers…</div>
  {:else if loadError}
    <div class="empty error">{loadError}</div>
  {:else}
    {#if featured.length && !filtering}
      <span class="label">Featured</span>
      <div class="featured">
        {#each featured as l (l.id)}
          {@const st = live(l)}
          <button class="feat glass" class:with-banner={!!l.bannerUrl} onclick={() => (open = l)}>
            {#if l.bannerUrl}<img class="feat-banner" src={l.bannerUrl} alt="" loading="lazy" />{/if}
            <div class="feat-top">
              <span class="icon sm">
                {#if st?.favicon}<img src={st.favicon} alt="" />{:else if l.pack?.iconUrl}<img src={l.pack.iconUrl} alt="" />{:else}<ServerIcon size={20} />{/if}
              </span>
              <div class="feat-name">
                <b>{l.name}</b>
                <span>{l.tagline}</span>
              </div>
              <Star size={14} class="star" />
            </div>
            <div class="feat-meta">
              {#if st?.online}<span><Users size={12} /> {st.playersOnline}</span>{:else if status[l.id] === 'loading'}<span><LoaderCircle size={12} class="spin" /></span>{:else}<span class="faint">Offline</span>{/if}
              <span><ThumbsUp size={12} /> {compact(l.votes)}</span>
              {#if l.pack}<span class="packname">{l.pack.name}</span>{:else}<span>{l.gameVersion}</span>{/if}
            </div>
          </button>
        {/each}
      </div>
    {/if}

    <span class="label">{filtering ? `${shown.length} server${shown.length === 1 ? '' : 's'}` : 'All servers'}</span>
    <div class="list">
      {#each shown as l, i (l.id)}
        {@const st = live(l)}
        <article class="row glass rise" style:animation-delay="{i * 30}ms">
          <button class="row-main" onclick={() => (open = l)}>
            <span class="rank">#{listings.slice().sort((a, b) => b.votes - a.votes).findIndex((x) => x.id === l.id) + 1}</span>
            <span class="icon">
              {#if st?.favicon}<img src={st.favicon} alt="" />{:else if l.pack?.iconUrl}<img src={l.pack.iconUrl} alt="" />{:else}<ServerIcon size={24} />{/if}
            </span>
            <div class="text">
              <div class="name">
                <b>{l.name}</b>
                {#if l.verified}<span title="Owner verified"><BadgeCheck size={14} class="verified" /></span>{/if}
              </div>
              <span class="tagline">{l.tagline}</span>
              <div class="tags">
                {#if l.pack}
                  <span class="chip pack"><LoaderIcon loader={l.loader} size={12} /> {l.pack.name}</span>
                {:else}
                  <span class="chip">Vanilla {l.gameVersion}</span>
                {/if}
                {#each l.tags.slice(0, 3) as t (t)}<span class="chip">{t}</span>{/each}
              </div>
            </div>
          </button>

          <div class="stats">
            {#if st?.online}
              <span class="players"><Users size={13} /> {st.playersOnline ?? '?'}<span class="faint">/{st.playersMax ?? '?'}</span></span>
              <span class="ping"><span class="bars" data-bars={bars(st.pingMs)}><i></i><i></i><i></i><i></i></span>{st.pingMs ?? ''} ms</span>
            {:else if status[l.id] === 'loading' || !status[l.id]}
              <span class="faint"><LoaderCircle size={13} class="spin" /> Checking…</span>
            {:else}
              <span class="faint"><SignalZero size={13} /> Offline</span>
            {/if}
          </div>

          <button class="votes" onclick={() => vote(l)} title="Vote for this server (opens its page on peaklauncher.com)">
            <ThumbsUp size={15} />
            <span>{compact(l.votes)}</span>
          </button>

          <div class="play">
            {#if !l.pack}
              <select class="input small" bind:value={() => chosen(l), (v) => (picked[l.id] = v)} aria-label="Instance to join with">
                {#each vanillaChoices(l) as x (x.id)}<option value={x.id}>{x.name} · {x.gameVersion}</option>{/each}
                <option value="new">New {l.gameVersion} instance</option>
              </select>
            {/if}
            <button class="btn primary" onclick={() => playListing(l, l.pack ? null : chosen(l))} disabled={busyFor(l)}>
              <Play size={15} /> {playLabel(l)}
            </button>
          </div>
        </article>
      {:else}
        <div class="empty">No servers match those filters.</div>
      {/each}
    </div>
  {/if}
</div>

{#if open}
  {@const l = open}
  {@const st = live(l)}
  <Modal title={l.name} subtitle={l.tagline} width={640} onclose={() => (open = null)}>
    <div class="detail">
      {#if l.bannerUrl}<img class="d-banner" src={l.bannerUrl} alt="{l.name} banner" />{/if}
      <div class="d-top">
        <span class="icon lg">
          {#if st?.favicon}<img src={st.favicon} alt="" />{:else if l.pack?.iconUrl}<img src={l.pack.iconUrl} alt="" />{:else}<ServerIcon size={30} />{/if}
        </span>
        <div class="d-status">
          <button class="address" onclick={() => copy(l.address)} title="Copy address">{l.address} <Copy size={11} /></button>
          {#if st?.online}
            <div class="motd">
              {#each motdLines(st.motd) as line, n (n)}
                <div>{#each line as seg, j (j)}<span style={seg.style || undefined}>{seg.text}</span>{/each}</div>
              {/each}
            </div>
            <div class="d-live">
              <span><Users size={13} /> {st.playersOnline}/{st.playersMax} online</span>
              {#if st.pingMs != null}<span>{st.pingMs} ms</span>{/if}
              {#if st.version}<span>{plain(st.version)}</span>{/if}
            </div>
          {:else}
            <span class="faint">{status[l.id] === 'loading' ? 'Checking…' : `Offline · ${st?.error ?? "couldn't reach it"}`}</span>
          {/if}
        </div>
      </div>

      <p class="desc">{l.description}</p>

      <div class="facts">
        <div><span class="label">Version</span><b>{l.gameVersion}</b></div>
        <div><span class="label">Loader</span><b>{loaderLabel(l.loader)}</b></div>
        <div><span class="label">Region</span><b>{l.region}</b></div>
        <div><span class="label">Language</span><b>{l.language}</b></div>
      </div>

      {#if l.pack}
        <div class="pack-card">
          {#if l.pack.iconUrl}<img src={l.pack.iconUrl} alt="" />{:else}<Package size={22} />{/if}
          <div>
            <b>{l.pack.name}</b>
            <span>
              {packInstance(l)
                ? `Installed as "${packInstance(l)!.name}". Play joins with it.`
                : 'Play installs this modpack first, then joins the server. No extra steps.'}
            </span>
          </div>
        </div>
      {/if}

      <div class="tags">{#each l.tags as t (t)}<span class="chip">{t}</span>{/each}</div>

      <div class="d-actions">
        <button class="btn" onclick={() => vote(l)} title="Opens this server's page on peaklauncher.com">
          <ThumbsUp size={15} /> Vote · {compact(l.votes)}
        </button>
        <button class="btn" onclick={() => save(l)}><Bookmark size={15} /> Save to My servers</button>
        {#if l.website}<button class="btn ghost" onclick={() => api.openUrl(l.website!)}><Globe size={15} /> Website</button>{/if}
        {#if l.discord}<button class="btn ghost" onclick={() => api.openUrl(l.discord!)}><ExternalLink size={15} /> Discord</button>{/if}
        <button class="btn ghost" onclick={() => toast('Thanks. Reports will go to Peak\'s moderators once the list is live.', 'info')}><Flag size={15} /> Report</button>
        <span class="spacer"></span>
        {#if !l.pack}
          <select class="input small" bind:value={() => chosen(l), (v) => (picked[l.id] = v)} aria-label="Instance to join with">
            {#each vanillaChoices(l) as x (x.id)}<option value={x.id}>{x.name} · {x.gameVersion}</option>{/each}
            <option value="new">New {l.gameVersion} instance</option>
          </select>
        {/if}
        <button class="btn primary" onclick={() => (playListing(l, l.pack ? null : chosen(l)), (open = null))} disabled={busyFor(l)}>
          <Play size={15} /> {playLabel(l)}
        </button>
      </div>
    </div>
  </Modal>
{/if}

<style>
  .discover {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .filters {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    align-items: center;
    padding: 10px;
  }
  .search {
    flex: 1;
    min-width: 200px;
    display: flex;
    align-items: center;
    gap: 8px;
    height: 36px;
    padding: 0 12px;
    border-radius: var(--radius-sm);
    background: #0e1013;
    border: 1px solid var(--stroke);
    color: var(--faint);
  }
  .search input {
    flex: 1;
    min-width: 0;
    border: 0;
    outline: none;
    background: none;
    user-select: text;
  }
  .input.small {
    width: auto;
    height: 36px;
    padding: 0 10px;
    font-size: 13px;
  }
  /* Leave room for the dropdown arrow. */
  select.input.small {
    padding-right: 32px;
    background-position: right 10px center;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
    color: var(--muted);
    padding: 0 4px;
  }
  .label {
    margin-top: 4px;
  }
  .featured {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: 10px;
  }
  .feat {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px;
    text-align: left;
    border-color: color-mix(in srgb, var(--accent) 30%, transparent);
    background: linear-gradient(135deg, color-mix(in srgb, var(--accent) 10%, transparent), var(--glass));
    transition: border-color 0.15s;
  }
  .feat:hover {
    border-color: color-mix(in srgb, var(--accent) 55%, transparent);
  }
  .feat.with-banner {
    padding-top: 0;
    overflow: hidden;
  }
  /* Banners keep their own shape (468 x 60 as well as wider ones). */
  .feat-banner {
    display: block;
    width: calc(100% + 28px);
    height: auto;
    max-height: 90px;
    margin: 0 -14px;
    object-fit: contain;
    background: rgba(0, 0, 0, 0.35);
  }
  .d-banner {
    display: block;
    width: auto;
    max-width: 100%;
    height: auto;
    max-height: 110px;
    border-radius: var(--radius-sm);
  }
  .feat-top {
    display: flex;
    gap: 10px;
    align-items: center;
  }
  .feat-top :global(.star) {
    margin-left: auto;
    color: #fbbf24;
    flex-shrink: 0;
  }
  .feat-name {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .feat-name span {
    font-size: 12px;
    color: var(--faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .feat-meta {
    display: flex;
    gap: 12px;
    font-size: 12px;
    color: var(--muted);
    min-width: 0;
  }
  .feat-meta span {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    white-space: nowrap;
    flex-shrink: 0;
  }
  /* Only the pack name gets cut short. */
  .feat-meta span.packname {
    flex-shrink: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    display: block;
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 120px 64px auto;
    align-items: center;
    gap: 14px;
    padding: 10px 14px 10px 10px;
  }
  .row-main {
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
    text-align: left;
  }
  .rank {
    width: 30px;
    flex-shrink: 0;
    text-align: right;
    font-size: 12px;
    font-weight: 650;
    color: var(--faint);
    font-variant-numeric: tabular-nums;
  }
  .icon {
    width: 48px;
    height: 48px;
    flex-shrink: 0;
    display: grid;
    place-items: center;
    border-radius: 10px;
    overflow: hidden;
    background: rgba(255, 255, 255, 0.04);
    color: var(--faint);
  }
  .icon.sm {
    width: 36px;
    height: 36px;
  }
  .icon.lg {
    width: 72px;
    height: 72px;
  }
  .icon img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    image-rendering: pixelated;
  }
  .text {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }
  .name {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .name b {
    font-size: 15px;
  }
  .name :global(.verified) {
    color: var(--accent);
  }
  .tagline {
    font-size: 12.5px;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tags {
    display: flex;
    gap: 5px;
    flex-wrap: wrap;
    margin-top: 2px;
  }
  .chip.pack {
    color: var(--accent-2);
    background: color-mix(in srgb, var(--accent) 10%, transparent);
    border-color: color-mix(in srgb, var(--accent) 28%, transparent);
  }
  .stats {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12.5px;
  }
  .stats span {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  .faint {
    color: var(--faint);
  }
  .bars {
    display: inline-flex;
    align-items: flex-end;
    gap: 1.5px;
    height: 11px;
  }
  .bars i {
    width: 3px;
    background: var(--faint);
    border-radius: 1px;
  }
  .bars i:nth-child(1) {
    height: 30%;
  }
  .bars i:nth-child(2) {
    height: 55%;
  }
  .bars i:nth-child(3) {
    height: 78%;
  }
  .bars i:nth-child(4) {
    height: 100%;
  }
  .bars[data-bars='4'] i,
  .bars[data-bars='3'] i:nth-child(-n + 3),
  .bars[data-bars='2'] i:nth-child(-n + 2),
  .bars[data-bars='1'] i:nth-child(1) {
    background: var(--success);
  }
  .votes {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    padding: 6px 0;
    border-radius: 10px;
    border: 1px solid var(--stroke);
    background: rgba(255, 255, 255, 0.03);
    font-size: 12px;
    font-weight: 650;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  .votes:hover:not(:disabled) {
    color: var(--accent);
    border-color: color-mix(in srgb, var(--accent) 45%, transparent);
  }
  .play {
    display: flex;
    gap: 8px;
    align-items: center;
    justify-content: flex-end;
  }
  .play select {
    max-width: 190px;
  }
  .empty {
    display: flex;
    gap: 8px;
    align-items: center;
    justify-content: center;
    padding: 40px;
    color: var(--faint);
    font-size: 13px;
  }
  .empty.error {
    color: #ff9a9d;
  }

  /* ---- detail ---- */
  .detail {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .d-top {
    display: flex;
    gap: 14px;
    align-items: flex-start;
  }
  .d-status {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  .address {
    align-self: flex-start;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--muted);
    padding: 3px 8px;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.05);
  }
  .motd {
    font-family: var(--font-mono);
    font-size: 12.5px;
    line-height: 1.5;
    padding: 8px 10px;
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.3);
    white-space: pre-wrap;
  }
  .d-live {
    display: flex;
    gap: 14px;
    font-size: 12.5px;
    color: var(--muted);
  }
  .d-live span {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  .desc {
    margin: 0;
    font-size: 13.5px;
    line-height: 1.6;
    color: var(--muted);
  }
  .facts {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 10px;
  }
  .facts div {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .facts b {
    font-size: 13px;
  }
  .pack-card {
    display: flex;
    gap: 12px;
    align-items: center;
    padding: 12px;
    border-radius: var(--radius-sm);
    background: color-mix(in srgb, var(--accent) 7%, transparent);
    border: 1px solid color-mix(in srgb, var(--accent) 22%, transparent);
    color: var(--accent);
  }
  .pack-card img {
    width: 40px;
    height: 40px;
    border-radius: 8px;
  }
  .pack-card div {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .pack-card b {
    color: var(--text);
  }
  .pack-card span {
    font-size: 12.5px;
    color: var(--muted);
  }
  .d-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    align-items: center;
    padding-top: 6px;
    border-top: 1px solid var(--stroke);
  }
  .spacer {
    flex: 1;
  }
</style>
