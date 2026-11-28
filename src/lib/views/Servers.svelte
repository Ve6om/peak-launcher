<script lang="ts">
  // Saved servers with live status, and a Join button that launches an
  // instance straight into the server.
  import { onMount } from 'svelte';
  import { instanceIcon } from '$lib/instanceImages';
  import Select from '$lib/components/Select.svelte';
  import { Plus, RefreshCw, FolderInput, Server as ServerIcon, Play, Pencil, Trash2, Users, Copy, LoaderCircle, SignalZero, Bookmark, Compass, Star, ChevronUp, ChevronDown } from '@lucide/svelte';
  import Modal from '$lib/components/Modal.svelte';
  import LoaderIcon from '$lib/components/LoaderIcon.svelte';
  import DiscoverServers from '$lib/components/DiscoverServers.svelte';
  import { serverLink } from '$lib/discover.svelte';
  import { playingAt } from '$lib/invite.svelte';
  import { api } from '$lib/api';
  import { app, go, toast, errorText, activeAccount, isRunning, isBusy } from '$lib/state.svelte';
  import { shownAddress } from '$lib/privacy.svelte';
  import { motdLines, plain } from '$lib/motd';
  import type { SavedServer, ServerStatus } from '$lib/types';

  let tab = $state<'mine' | 'discover'>(serverLink.id ? 'discover' : 'mine');
  // A peak:// link while this page is already open.
  $effect(() => {
    if (serverLink.id) tab = 'discover';
  });
  let servers = $state<SavedServer[]>([]);
  let status = $state<Record<string, ServerStatus | 'loading'>>({});
  let loaded = $state(false);
  /** Instance picked per server, before joining. */
  let picked = $state<Record<string, string>>({});
  let editing = $state<SavedServer | null>(null);
  let importing = $state(false);
  let confirmDelete = $state<string | null>(null);

  onMount(async () => {
    try {
      servers = await api.serversList();
    } catch (e) {
      toast(errorText(e), 'error');
    }
    loaded = true;
    refreshAll();
  });

  async function ping(s: SavedServer) {
    status[s.id] = 'loading';
    status[s.id] = await api.serverPing(s.address).catch(
      (e): ServerStatus => ({ online: false, error: errorText(e), version: null, protocol: null, playersOnline: null, playersMax: null, playersSample: [], motd: null, favicon: null, pingMs: null })
    );
  }

  function refreshAll() {
    servers.forEach(ping);
  }

  /** The instance to offer first: last used for this server, else the most recently played. */
  function defaultInstance(s: SavedServer): string {
    if (s.instanceId && app.instances.some((i) => i.id === s.instanceId)) return s.instanceId;
    return [...app.instances].sort((a, b) => (b.lastPlayed ?? '').localeCompare(a.lastPlayed ?? ''))[0]?.id ?? '';
  }
  const chosen = (s: SavedServer) => picked[s.id] ?? defaultInstance(s);

  async function join(s: SavedServer) {
    const inst = app.instances.find((i) => i.id === chosen(s));
    if (!inst) {
      toast('Create an instance first, then join from here.', 'info');
      return;
    }
    if (!activeAccount()) {
      toast('Add an account to start playing', 'info');
      go('accounts');
      return;
    }
    if (isRunning(inst.id)) {
      toast(`${inst.name} is already running.`, 'info');
      return;
    }
    app.logs[inst.id] = [];
    playingAt[inst.id] = { address: s.address, name: s.name };
    try {
      await api.serverJoin(s.id, inst.id);
      s.instanceId = inst.id;
    } catch (e) {
      delete app.progress[inst.id];
      toast(errorText(e), 'error');
    }
  }

  async function saveServer() {
    if (!editing) return;
    try {
      const isNew = !editing.id;
      servers = await api.serverSave($state.snapshot(editing));
      const saved = isNew ? servers[servers.length - 1] : servers.find((s) => s.id === editing!.id);
      editing = null;
      if (saved) ping(saved);
    } catch (e) {
      toast(errorText(e), 'error');
    }
  }

  async function remove(s: SavedServer) {
    if (confirmDelete !== s.id) {
      confirmDelete = s.id;
      setTimeout(() => confirmDelete === s.id && (confirmDelete = null), 3000);
      return;
    }
    servers = await api.serverDelete(s.id);
    delete status[s.id];
    confirmDelete = null;
  }

  // Starred servers first, then the player's own order.
  const ordered = $derived([...servers].sort((a, b) => Number(!!b.favorite) - Number(!!a.favorite)));

  async function toggleFavorite(s: SavedServer) {
    try {
      servers = await api.serverSave({ ...$state.snapshot(s), favorite: !s.favorite });
    } catch (e) {
      toast(errorText(e), 'error');
    }
  }

  /** Move a server up or down (within starred, or within the rest). */
  async function move(s: SavedServer, by: number) {
    const list = ordered.slice();
    const i = list.findIndex((x) => x.id === s.id);
    const j = i + by;
    if (j < 0 || j >= list.length || !!list[j].favorite !== !!s.favorite) return;
    [list[i], list[j]] = [list[j], list[i]];
    try {
      servers = await api.serversReorder(list.map((x) => x.id));
    } catch (e) {
      toast(errorText(e), 'error');
    }
  }

  async function copyAddress(address: string) {
    await navigator.clipboard.writeText(address).catch(() => {});
    toast('Address copied');
  }

  /** 0-4 bars, like the game's server list. */
  function bars(ms: number | null): number {
    if (ms == null) return 1;
    return ms < 80 ? 4 : ms < 150 ? 3 : ms < 300 ? 2 : 1;
  }

  // ---- import from an instance's own server list ----
  let importFrom = $state('');
  let found = $state<{ name: string; address: string; take: boolean; exists: boolean }[]>([]);
  let finding = $state(false);

  async function loadImport() {
    if (!importFrom) return;
    finding = true;
    try {
      const have = new Set(servers.map((s) => s.address.toLowerCase()));
      found = (await api.serversFromInstance(importFrom)).map(([name, address]) => {
        const exists = have.has(address.toLowerCase());
        return { name, address, take: !exists, exists };
      });
    } catch (e) {
      toast(errorText(e), 'error');
      found = [];
    } finally {
      finding = false;
    }
  }

  async function doImport() {
    const chosenOnes = found.filter((f) => f.take && !f.exists);
    for (const f of chosenOnes) {
      servers = await api.serverSave({ id: '', name: f.name, address: f.address, instanceId: importFrom });
    }
    importing = false;
    found = [];
    toast(chosenOnes.length === 1 ? 'Added 1 server' : `Added ${chosenOnes.length} servers`, 'success');
    refreshAll();
  }

  function openImport() {
    importing = true;
    importFrom = app.instances[0]?.id ?? '';
    found = [];
    loadImport();
  }
</script>

<div class="page">
  <header>
    <div>
      <h1>Servers</h1>
      <p>{tab === 'mine' ? "Save the servers you play on, see who's online, and join with any instance." : 'Find new servers to play on. Modded servers install their pack for you.'}</p>
    </div>
    {#if tab === 'mine'}<div class="tools">
      <button class="btn" onclick={refreshAll} disabled={!servers.length}><RefreshCw size={15} /> Refresh</button>
      <button class="btn" onclick={openImport} disabled={!app.instances.length}><FolderInput size={15} /> Import from instance</button>
      <button class="btn primary" onclick={() => (editing = { id: '', name: '', address: '', instanceId: null })}><Plus size={16} /> Add server</button>
    </div>{/if}
  </header>

  <div class="tabs" role="tablist">
    <button role="tab" aria-selected={tab === 'mine'} class:active={tab === 'mine'} onclick={() => (tab = 'mine')}>
      <Bookmark size={15} /> My servers {#if servers.length}<span class="count">{servers.length}</span>{/if}
    </button>
    <button role="tab" aria-selected={tab === 'discover'} class:active={tab === 'discover'} onclick={() => (tab = 'discover')}>
      <Compass size={15} /> Discover <span class="new">New</span>
    </button>
  </div>

  {#if tab === 'discover'}
    <DiscoverServers />
  {:else}
  <div class="list">
    {#each ordered as s, i (s.id)}
      {@const st = status[s.id]}
      {@const ready = st && st !== 'loading' ? st : null}
      {@const inst = app.instances.find((x) => x.id === chosen(s))}
      <article class="server glass rise" style:animation-delay="{i * 40}ms">
        <div class="icon">
          {#if ready?.favicon}
            <img src={ready.favicon} alt="" />
          {:else}
            <ServerIcon size={26} />
          {/if}
        </div>

        <div class="main">
          <div class="top">
            <b>{s.name}</b>
            <button class="address" title="Copy address" onclick={() => copyAddress(s.address)}>{shownAddress(s.address)} <Copy size={11} /></button>
          </div>
          {#if st === 'loading' || !st}
            <div class="motd muted"><LoaderCircle size={13} class="spin" /> Checking…</div>
          {:else if !st.online}
            <div class="motd offline"><SignalZero size={14} /> Offline · {st.error ?? "Couldn't reach it"}</div>
          {:else}
            <div class="motd">
              {#each motdLines(st.motd) as line, l (l)}
                <div class="line">{#each line as seg, j (j)}<span style={seg.style || undefined}>{seg.text}</span>{/each}</div>
              {:else}
                <div class="line muted">No message</div>
              {/each}
            </div>
          {/if}
        </div>

        <div class="meta">
          {#if ready?.online}
            <span class="players" title={ready.playersSample.length ? ready.playersSample.join(', ') : 'Players online'}>
              <Users size={13} /> {ready.playersOnline ?? '?'}<span class="faint">/{ready.playersMax ?? '?'}</span>
            </span>
            <span class="ping" title={ready.pingMs != null ? `${ready.pingMs} ms` : 'Ping unknown'}>
              <span class="bars" data-bars={bars(ready.pingMs)}><i></i><i></i><i></i><i></i></span>
              {ready.pingMs != null ? `${ready.pingMs} ms` : ''}
            </span>
            {#if ready.version}<span class="version" title={plain(ready.version)}>{plain(ready.version)}</span>{/if}
          {/if}
        </div>

        <div class="actions">
          <label class="pick" title="Instance to join with">
            {#if inst}<LoaderIcon loader={inst.loader} size={16} />{/if}
            <Select
              bare
              bind:value={() => chosen(s), (v) => v && (picked[s.id] = v)}
              disabled={!app.instances.length}
              placeholder="No instances yet"
              ariaLabel="Instance to join with"
              style="max-width: 170px;"
              options={app.instances.map((x) => ({ value: x.id, label: x.name, hint: x.gameVersion, icon: instanceIcon(x), loader: x.loader }))}
            />
          </label>
          <button class="btn primary" onclick={() => join(s)} disabled={!inst || isRunning(inst.id) || isBusy(inst.id)}>
            <Play size={15} /> Join
          </button>
          <button class="btn ghost icon star" class:on={s.favorite} title={s.favorite ? 'Unstar' : 'Star (keeps it at the top)'} aria-label={s.favorite ? `Unstar ${s.name}` : `Star ${s.name}`} onclick={() => toggleFavorite(s)}>
            <Star size={15} fill={s.favorite ? 'currentColor' : 'none'} />
          </button>
          <span class="order">
            <button class="btn ghost icon tiny" title="Move up" aria-label="Move {s.name} up" disabled={i === 0 || !!ordered[i - 1]?.favorite !== !!s.favorite} onclick={() => move(s, -1)}><ChevronUp size={13} /></button>
            <button class="btn ghost icon tiny" title="Move down" aria-label="Move {s.name} down" disabled={i === ordered.length - 1 || !!ordered[i + 1]?.favorite !== !!s.favorite} onclick={() => move(s, 1)}><ChevronDown size={13} /></button>
          </span>
          <button class="btn ghost icon" title="Edit" aria-label="Edit {s.name}" onclick={() => (editing = { ...s })}><Pencil size={15} /></button>
          <button class="btn ghost icon del" class:confirm={confirmDelete === s.id} title={confirmDelete === s.id ? 'Click again to remove' : 'Remove'} aria-label="Remove {s.name}" onclick={() => remove(s)}>
            <Trash2 size={15} />
          </button>
        </div>
      </article>
    {:else}
      {#if loaded}
        <div class="empty glass">
          <ServerIcon size={30} />
          <h3>No servers yet</h3>
          <p>Add a server by its address, or import the list from one of your instances.</p>
          <div class="empty-actions">
            <button class="btn primary" onclick={() => (editing = { id: '', name: '', address: '', instanceId: null })}><Plus size={16} /> Add server</button>
            {#if app.instances.length}<button class="btn" onclick={openImport}><FolderInput size={15} /> Import from instance</button>{/if}
          </div>
        </div>
      {/if}
    {/each}
  </div>
  {/if}
</div>

{#if editing}
  <Modal title={editing.id ? 'Edit server' : 'Add a server'} subtitle="The address is what you'd type in Minecraft's multiplayer screen." width={460} onclose={() => (editing = null)}>
    <form
      class="form"
      onsubmit={(e) => {
        e.preventDefault();
        saveServer();
      }}
    >
      <label class="field">
        <span class="label">Address</span>
        <!-- svelte-ignore a11y_autofocus -->
        <input class="input private" bind:value={editing.address} placeholder="play.example.com" autofocus spellcheck="false" />
      </label>
      <label class="field">
        <span class="label">Name</span>
        <input class="input" bind:value={editing.name} placeholder="Optional, shown in the list" maxlength="48" />
      </label>
      <footer>
        <button type="button" class="btn ghost" onclick={() => (editing = null)}>Cancel</button>
        <button type="submit" class="btn primary" disabled={!editing.address.trim()}>{editing.id ? 'Save' : 'Add server'}</button>
      </footer>
    </form>
  </Modal>
{/if}

{#if importing}
  <Modal title="Import servers" subtitle="Copy the servers saved in an instance's multiplayer list." width={500} onclose={() => (importing = false)}>
    <label class="field">
      <span class="label">From instance</span>
      <Select
        bind:value={importFrom}
        onchange={loadImport}
        ariaLabel="Instance to import from"
        options={app.instances.map((x) => ({ value: x.id, label: x.name, hint: x.gameVersion, icon: instanceIcon(x), loader: x.loader }))}
      />
    </label>
    <div class="found">
      {#if finding}
        <p class="muted"><LoaderCircle size={14} class="spin" /> Reading its server list…</p>
      {:else}
        {#each found as f (f.address)}
          <label class="found-row" class:dim={f.exists}>
            <input type="checkbox" bind:checked={f.take} disabled={f.exists} />
            <span><b>{f.name}</b><small>{shownAddress(f.address)}{f.exists ? ' · already added' : ''}</small></span>
          </label>
        {:else}
          <p class="muted">This instance has no saved servers.</p>
        {/each}
      {/if}
    </div>
    <footer>
      <button class="btn ghost" onclick={() => (importing = false)}>Cancel</button>
      <button class="btn primary" onclick={doImport} disabled={!found.some((f) => f.take && !f.exists)}>
        Import {found.filter((f) => f.take && !f.exists).length || ''}
      </button>
    </footer>
  </Modal>
{/if}

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: 18px;
  }
  .star.on {
    color: #fbbf24;
  }
  .order {
    display: inline-flex;
    flex-direction: column;
  }
  .order .tiny {
    width: 26px;
    height: 18px;
  }
  .tabs {
    display: flex;
    gap: 4px;
    margin-top: -6px;
    border-bottom: 1px solid var(--stroke);
  }
  .tabs button {
    position: relative;
    display: flex;
    align-items: center;
    gap: 7px;
    height: 42px;
    padding: 0 16px;
    font-size: 14px;
    font-weight: 600;
    color: var(--muted);
  }
  .tabs button:hover {
    color: var(--text);
  }
  .tabs button.active {
    color: var(--text);
  }
  .tabs button.active::after {
    content: '';
    position: absolute;
    left: 10px;
    right: 10px;
    bottom: -1px;
    height: 2px;
    border-radius: 2px;
    background: var(--gradient);
  }
  .count {
    font-size: 11px;
    padding: 1px 7px;
    border-radius: 99px;
    background: rgba(255, 255, 255, 0.08);
    color: var(--muted);
  }
  .new {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    padding: 2px 6px;
    border-radius: 5px;
    color: var(--on-accent);
    background: var(--accent);
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
    gap: 8px;
    flex-wrap: wrap;
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .server {
    display: grid;
    grid-template-columns: 56px minmax(0, 1fr) auto;
    grid-template-areas:
      'icon main meta'
      'icon main actions';
    column-gap: 16px;
    row-gap: 8px;
    align-items: center;
    padding: 14px 16px;
  }
  .icon {
    grid-area: icon;
    width: 56px;
    height: 56px;
    display: grid;
    place-items: center;
    border-radius: 12px;
    overflow: hidden;
    background: rgba(255, 255, 255, 0.05);
    color: var(--faint);
    align-self: start;
  }
  .icon img {
    width: 100%;
    height: 100%;
    image-rendering: pixelated;
  }
  .main {
    grid-area: main;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .top {
    display: flex;
    align-items: baseline;
    gap: 10px;
    min-width: 0;
  }
  .top b {
    font-size: 15px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .address {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 0;
    border: none;
    background: none;
    font: inherit;
    font-size: 12px;
    color: var(--faint);
    cursor: pointer;
    white-space: nowrap;
  }
  .address:hover {
    color: var(--text);
  }
  .motd {
    font-family: 'JetBrains Mono', Consolas, monospace;
    font-size: 12.5px;
    line-height: 1.5;
    color: #aaaaaa;
    padding: 6px 10px;
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.28);
    min-height: 28px;
  }
  .motd .line {
    white-space: pre;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .motd.muted,
  .motd.offline {
    display: flex;
    align-items: center;
    gap: 8px;
    font-family: inherit;
  }
  .motd.offline {
    color: #ff9a9d;
  }
  .muted {
    color: var(--muted);
  }
  .meta {
    grid-area: meta;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 12px;
    font-size: 12.5px;
    color: var(--muted);
    white-space: nowrap;
  }
  .players {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    color: var(--text);
    font-weight: 600;
  }
  .faint {
    color: var(--faint);
    font-weight: 400;
  }
  .ping {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .bars {
    display: inline-flex;
    align-items: flex-end;
    gap: 2px;
    height: 12px;
  }
  .bars i {
    width: 3px;
    border-radius: 1px;
    background: rgba(255, 255, 255, 0.15);
  }
  .bars i:nth-child(1) {
    height: 4px;
  }
  .bars i:nth-child(2) {
    height: 6px;
  }
  .bars i:nth-child(3) {
    height: 9px;
  }
  .bars i:nth-child(4) {
    height: 12px;
  }
  .bars[data-bars='4'] i,
  .bars[data-bars='3'] i:nth-child(-n + 3),
  .bars[data-bars='2'] i:nth-child(-n + 2) {
    background: #4ade80;
  }
  .bars[data-bars='1'] i:nth-child(1) {
    background: #fbbf24;
  }
  .version {
    max-width: 150px;
    overflow: hidden;
    text-overflow: ellipsis;
    padding: 2px 8px;
    border-radius: 99px;
    border: 1px solid var(--stroke);
    font-size: 11.5px;
  }
  .actions {
    grid-area: actions;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 6px;
  }
  .pick {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 34px;
    padding: 0 6px 0 10px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--stroke);
    background: rgba(255, 255, 255, 0.03);
  }
  .del.confirm {
    color: #fecaca;
    background: rgba(239, 68, 68, 0.2);
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 46px 20px;
    text-align: center;
    color: var(--faint);
  }
  .empty h3 {
    margin: 4px 0 0;
    color: var(--text);
  }
  .empty p {
    margin: 0;
    font-size: 13px;
    color: var(--muted);
  }
  .empty-actions {
    display: flex;
    gap: 8px;
    margin-top: 10px;
  }
  .form,
  .field {
    display: flex;
    flex-direction: column;
  }
  .form {
    gap: 14px;
  }
  .field {
    gap: 7px;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 16px;
  }
  .found {
    margin-top: 12px;
    max-height: 280px;
    overflow-y: auto;
    border-radius: var(--radius-sm);
    border: 1px solid var(--stroke);
    background: #0e1013;
  }
  .found p {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    padding: 14px;
    font-size: 13px;
  }
  .found-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 12px;
    cursor: pointer;
  }
  .found-row + .found-row {
    border-top: 1px solid var(--stroke);
  }
  .found-row.dim {
    opacity: 0.55;
    cursor: default;
  }
  .found-row span {
    display: flex;
    flex-direction: column;
  }
  .found-row b {
    font-size: 13px;
  }
  .found-row small {
    font-size: 11.5px;
    color: var(--faint);
  }
  @media (max-width: 1100px) {
    .server {
      grid-template-columns: 56px minmax(0, 1fr);
      grid-template-areas:
        'icon main'
        'icon meta'
        'icon actions';
    }
    .meta,
    .actions {
      justify-content: flex-start;
    }
  }
</style>
