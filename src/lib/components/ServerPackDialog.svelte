<script lang="ts">
  // Build a server pack: review which mods are client-only, then write a zip
  // with the server mods, configs and start scripts for the instance's loader.
  import { onMount } from 'svelte';
  import { LoaderCircle, Monitor, Server, CircleHelp, FolderOpen, PackageCheck, Search } from '@lucide/svelte';
  import Modal from './Modal.svelte';
  import Switch from './Switch.svelte';
  import { api } from '$lib/api';
  import { app, toast, errorText } from '$lib/state.svelte';
  import { loaderLabel, type Instance, type ModSide, type ServerPackReport } from '$lib/types';

  let { inst }: { inst: Instance } = $props();

  let mods = $state<ModSide[]>([]);
  /** File names the user has set to client-only. */
  let client = $state(new Set<string>());
  let keepClient = $state(true);
  let filter = $state('');
  let loading = $state(true);
  let error = $state<string | null>(null);
  let exporting = $state(false);
  let report = $state<ServerPackReport | null>(null);

  onMount(async () => {
    try {
      mods = await api.serverPackAnalyze(inst.id);
      client = new Set(mods.filter((m) => m.clientOnly).map((m) => m.fileName));
    } catch (e) {
      error = errorText(e);
    } finally {
      loading = false;
    }
  });

  const clientCount = $derived(client.size);
  const unsure = $derived(mods.filter((m) => !m.sure).length);
  const shown = $derived(
    mods.filter((m) => !filter.trim() || m.name.toLowerCase().includes(filter.toLowerCase()) || m.fileName.toLowerCase().includes(filter.toLowerCase()))
  );

  function toggle(m: ModSide, isClient: boolean) {
    const next = new Set(client);
    if (isClient) next.add(m.fileName);
    else next.delete(m.fileName);
    client = next;
  }

  /** The reason shown, updated when the user overrides Peak's guess. */
  function reason(m: ModSide) {
    return client.has(m.fileName) === m.clientOnly ? m.reason : 'Changed by you';
  }

  async function exportPack() {
    const safe = inst.name.replace(/[\\/:*?"<>|]+/g, '').trim() || 'server';
    const path = await api.pickSavePath(`${safe}-server.zip`, 'zip', 'Server pack');
    if (!path) return;
    exporting = true;
    try {
      report = await api.serverPackExport(inst.id, { path, clientFiles: [...client], keepClient });
    } catch (e) {
      toast(errorText(e), 'error');
    } finally {
      exporting = false;
    }
  }

  const size = (b: number) => (b > 1e9 ? `${(b / 1e9).toFixed(1)} GB` : `${Math.max(1, Math.round(b / 1e6))} MB`);
  const close = () => !exporting && (app.serverPack = null);
</script>

<Modal title="Export {inst.name} as a server pack" subtitle="Minecraft {inst.gameVersion} · {loaderLabel(inst.loader)}" width={640} onclose={close}>
  {#if report}
    <div class="done">
      <PackageCheck size={30} />
      <b>Server pack ready</b>
      <p>
        {report.serverMods} mods for the server{#if report.clientMods}, {report.clientMods} client-only {keepClient ? 'in mods/CLIENTSIDE' : 'left out'}{/if}
        · {size(report.sizeBytes)} · needs Java {report.javaMajor}
      </p>
      <p class="hint">Unzip it on the server and run start.bat (Windows) or start.sh (Linux). The README inside explains the first start and the EULA.</p>
      <footer>
        <button class="btn" onclick={() => api.revealFile(report!.path)}><FolderOpen size={15} /> Show file</button>
        <button class="btn primary" onclick={() => (app.serverPack = null)}>Done</button>
      </footer>
    </div>
  {:else if loading}
    <div class="loading"><LoaderCircle size={22} class="spin" /> Checking which mods belong on the server…</div>
  {:else if error}
    <p class="error">{error}</p>
  {:else}
    <p class="intro">
      <b>{clientCount}</b> of {mods.length} mods are set to client-only{#if unsure}. <b>{unsure}</b> are best guesses (marked
      <CircleHelp size={13} />), so check those{/if}. Switch any mod to the right side before exporting.
    </p>

    <label class="search">
      <Search size={14} />
      <input placeholder="Filter mods" bind:value={filter} spellcheck="false" />
    </label>

    <div class="list">
      {#each shown as m (m.fileName)}
        {@const isClient = client.has(m.fileName)}
        <div class="row" class:client={isClient}>
          <span class="side" title={isClient ? 'Client only' : 'Server'}>
            {#if isClient}<Monitor size={15} />{:else}<Server size={15} />{/if}
          </span>
          <div class="text">
            <b>{m.name}{#if !m.sure}<span class="guess" title="Best guess, worth checking"><CircleHelp size={13} /></span>{/if}</b>
            <span>{reason(m)} · {m.fileName}</span>
          </div>
          <div class="seg small">
            <button class:active={!isClient} onclick={() => toggle(m, false)}>Server</button>
            <button class:active={isClient} onclick={() => toggle(m, true)}>Client only</button>
          </div>
        </div>
      {:else}
        <p class="empty">{mods.length ? 'No mods match.' : 'This instance has no mods.'}</p>
      {/each}
    </div>

    <div class="option">
      <div>
        <b>Keep client-only mods in mods/CLIENTSIDE</b>
        <span>The server ignores that folder, so they're easy to move back if one turns out to be needed.</span>
      </div>
      <Switch bind:checked={keepClient} label="Keep client-only mods in mods/CLIENTSIDE" />
    </div>

    <p class="hint">Includes configs, the {loaderLabel(inst.loader)} server setup, start scripts and a README. Worlds, resource packs and shaders are left out.</p>

    <footer>
      <button class="btn ghost" disabled={exporting} onclick={close}>Cancel</button>
      <button class="btn primary" disabled={exporting} onclick={exportPack}>
        {#if exporting}<LoaderCircle size={16} class="spin" /> Building…{:else}<PackageCheck size={16} /> Export server pack{/if}
      </button>
    </footer>
  {/if}
</Modal>

<style>
  .loading {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 30px 4px;
    color: var(--muted);
  }
  .intro {
    margin: 0 0 12px;
    color: var(--muted);
    font-size: 13.5px;
  }
  .intro :global(svg) {
    vertical-align: -2px;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 34px;
    padding: 0 12px;
    margin-bottom: 8px;
    border-radius: var(--radius-sm);
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid var(--stroke);
    color: var(--faint);
  }
  .search input {
    flex: 1;
    background: none;
    border: none;
    outline: none;
    color: var(--text);
    font: inherit;
    font-size: 13px;
  }
  .list {
    max-height: 340px;
    overflow-y: auto;
    border-radius: var(--radius-sm);
    border: 1px solid var(--stroke);
    background: #0e1013;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
  }
  .row + .row {
    border-top: 1px solid var(--stroke);
  }
  .side {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.05);
    color: var(--muted);
    flex-shrink: 0;
  }
  .client .side {
    background: color-mix(in srgb, var(--accent) 16%, transparent);
    color: var(--accent);
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .text b {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 13px;
  }
  .text span {
    font-size: 11.5px;
    color: var(--faint);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .guess {
    color: #fcd34d;
    display: inline-flex;
  }
  .seg.small button {
    font-size: 11.5px;
    padding: 0 8px;
    height: 24px;
  }
  .option {
    display: flex;
    align-items: center;
    gap: 14px;
    margin-top: 14px;
  }
  .option div {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .option b {
    font-size: 13px;
  }
  .option span,
  .hint {
    font-size: 12px;
    color: var(--faint);
  }
  .hint {
    margin: 12px 0 0;
  }
  .empty,
  .error {
    padding: 14px;
    margin: 0;
    color: var(--muted);
    font-size: 13px;
  }
  .error {
    color: #ff9a9d;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 16px;
  }
  .done {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    text-align: center;
    padding: 8px 0 0;
  }
  .done :global(svg) {
    color: var(--accent);
  }
  .done p {
    margin: 0;
    color: var(--muted);
    font-size: 13px;
  }
  .done footer {
    align-self: stretch;
  }
</style>
