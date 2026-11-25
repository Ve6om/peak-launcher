<script lang="ts">
  // An instance's own multiplayer list (the one in Minecraft's Multiplayer
  // screen): see who's online, join straight in, add, remove and reorder.
  import { onMount } from 'svelte';
  import { Plus, Play, Trash2, ChevronUp, ChevronDown, Server as ServerIcon, Users, SignalZero, LoaderCircle, UserPlus } from '@lucide/svelte';
  import { api } from '$lib/api';
  import { app, toast, errorText, activeAccount, go, isRunning, isBusy } from '$lib/state.svelte';
  import { motdLines } from '$lib/motd';
  import { playingAt, inviteUrl } from '$lib/invite.svelte';
  import type { Instance, InstanceServer, SavedServer, ServerStatus } from '$lib/types';

  let { inst }: { inst: Instance } = $props();

  let list = $state<InstanceServer[]>([]);
  let loaded = $state(false);
  let status = $state<Record<string, ServerStatus | 'loading'>>({});
  let saved = $state<SavedServer[]>([]);
  let name = $state('');
  let address = $state('');
  let confirmRemove = $state<string | null>(null);
  const running = $derived(isRunning(inst.id));

  onMount(async () => {
    try {
      list = await api.instanceServers(inst.id);
      list.forEach(ping);
    } catch (e) {
      toast(errorText(e), 'error');
    } finally {
      loaded = true;
    }
    saved = await api.serversList().catch(() => []);
  });

  async function ping(s: InstanceServer) {
    status[s.address] = 'loading';
    status[s.address] = await api.serverPing(s.address).catch(
      (e): ServerStatus => ({ online: false, error: errorText(e), version: null, protocol: null, playersOnline: null, playersMax: null, playersSample: [], motd: null, favicon: null, pingMs: null })
    );
  }

  async function add(n = name, a = address) {
    if (!a.trim()) return;
    try {
      list = await api.instanceServerAdd(inst.id, n.trim(), a.trim());
      const added = list.find((s) => s.address.toLowerCase() === a.trim().toLowerCase());
      if (added) ping(added);
      name = '';
      address = '';
    } catch (e) {
      toast(errorText(e), 'error');
    }
  }

  async function remove(s: InstanceServer) {
    if (confirmRemove !== s.address) {
      confirmRemove = s.address;
      setTimeout(() => confirmRemove === s.address && (confirmRemove = null), 3000);
      return;
    }
    try {
      list = await api.instanceServerRemove(inst.id, s.address);
    } catch (e) {
      toast(errorText(e), 'error');
    }
    confirmRemove = null;
  }

  async function move(i: number, by: number) {
    const j = i + by;
    if (j < 0 || j >= list.length) return;
    const order = list.map((s) => s.address);
    [order[i], order[j]] = [order[j], order[i]];
    try {
      list = await api.instanceServersReorder(inst.id, order);
    } catch (e) {
      toast(errorText(e), 'error');
    }
  }

  /** Copy a link friends can open to join this server with this instance's modpack. */
  async function copyInvite(s: InstanceServer) {
    try {
      await navigator.clipboard.writeText(inviteUrl(inst, s));
      toast(`Invite link to ${s.name} copied. Friends open it to join with the same pack.`, 'success');
    } catch {
      toast("Couldn't copy the link", 'error');
    }
  }

  async function join(s: InstanceServer) {
    if (!activeAccount()) {
      toast('Add an account to start playing', 'info');
      go('accounts');
      return;
    }
    app.logs[inst.id] = [];
    playingAt[inst.id] = { address: s.address, name: s.name };
    try {
      await api.instanceServerJoin(inst.id, s.address);
    } catch (e) {
      delete app.progress[inst.id];
      toast(errorText(e), 'error');
    }
  }

  const notInList = $derived(saved.filter((s) => !list.some((l) => l.address.toLowerCase() === s.address.toLowerCase())));
</script>

<div class="wrap">
  <section class="panel glass">
    <h3>Servers in this instance</h3>
    <p class="sub">The same list as Minecraft's Multiplayer screen. Changes here show up in the game, and the top server is first in its list.{running ? ' Close the game to change the list.' : ''}</p>

    {#if !loaded}
      <p class="muted"><LoaderCircle size={14} class="spin" /> Loading…</p>
    {:else if !list.length}
      <p class="muted">No servers yet. Add one below, or join one from the Servers page and Peak adds it here for you.</p>
    {:else}
      <div class="list">
        {#each list as s, i (s.address)}
          {@const st = status[s.address]}
          {@const ready = st && st !== 'loading' ? st : null}
          <div class="row">
            <div class="order">
              <button class="btn ghost icon tiny" aria-label="Move up" title="Move up" disabled={i === 0 || running} onclick={() => move(i, -1)}><ChevronUp size={14} /></button>
              <button class="btn ghost icon tiny" aria-label="Move down" title="Move down" disabled={i === list.length - 1 || running} onclick={() => move(i, 1)}><ChevronDown size={14} /></button>
            </div>
            <span class="icon">{#if ready?.favicon}<img src={ready.favicon} alt="" />{:else}<ServerIcon size={20} />{/if}</span>
            <div class="text">
              <b>{s.name || s.address}</b>
              <span class="addr">{s.address}</span>
              {#if ready?.online}
                <span class="motd">
                  {#each motdLines(ready.motd).slice(0, 1) as line, l (l)}{#each line as seg, j (j)}<span style={seg.style || undefined}>{seg.text}</span>{/each}{/each}
                </span>
              {/if}
            </div>
            <span class="state">
              {#if st === 'loading' || !st}<LoaderCircle size={13} class="spin" />
              {:else if ready?.online}<Users size={13} /> {ready.playersOnline}/{ready.playersMax}
              {:else}<SignalZero size={13} /> Offline{/if}
            </span>
            <button class="btn ghost icon" title="Copy an invite link for friends" aria-label="Copy an invite link to {s.name}" onclick={() => copyInvite(s)}><UserPlus size={15} /></button>
            <button class="btn primary" onclick={() => join(s)} disabled={running || isBusy(inst.id)}><Play size={14} /> Join</button>
            <button class="btn ghost icon del" class:confirm={confirmRemove === s.address} title={confirmRemove === s.address ? 'Click again to remove' : 'Remove'} aria-label="Remove {s.name}" disabled={running} onclick={() => remove(s)}>
              <Trash2 size={15} />
            </button>
          </div>
        {/each}
      </div>
    {/if}
  </section>

  <section class="panel glass">
    <h3>Add a server</h3>
    <form
      class="add"
      onsubmit={(e) => {
        e.preventDefault();
        add();
      }}
    >
      <input class="input" bind:value={name} placeholder="Name (optional)" maxlength="40" />
      <input class="input" bind:value={address} placeholder="play.example.com" spellcheck="false" />
      <button class="btn primary" type="submit" disabled={!address.trim() || running}><Plus size={15} /> Add</button>
    </form>
    {#if notInList.length}
      <div class="from-saved">
        <span class="label">From My servers</span>
        <div class="chips">
          {#each notInList as s (s.id)}
            <button class="btn" disabled={running} onclick={() => add(s.name, s.address)}><Plus size={13} /> {s.name}</button>
          {/each}
        </div>
      </div>
    {/if}
  </section>
</div>

<style>
  .wrap {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding-bottom: 40px;
  }
  .muted {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--faint);
    font-size: 13px;
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-top: 12px;
  }
  .row {
    display: grid;
    grid-template-columns: auto 40px minmax(0, 1fr) auto auto auto;
    align-items: center;
    gap: 12px;
    padding: 8px 10px;
    border-radius: var(--radius-sm);
    background: rgba(255, 255, 255, 0.025);
    border: 1px solid var(--stroke);
  }
  .order {
    display: flex;
    flex-direction: column;
  }
  .tiny {
    width: 24px;
    height: 20px;
  }
  .icon {
    width: 40px;
    height: 40px;
    display: grid;
    place-items: center;
    border-radius: 8px;
    overflow: hidden;
    background: rgba(255, 255, 255, 0.04);
    color: var(--faint);
  }
  .icon img {
    width: 100%;
    height: 100%;
    image-rendering: pixelated;
  }
  .text {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .addr {
    font-family: var(--font-mono);
    font-size: 11.5px;
    color: var(--faint);
  }
  .motd {
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .state {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 12.5px;
    color: var(--muted);
  }
  .del.confirm {
    color: #fecaca;
    background: rgba(239, 68, 68, 0.18);
  }
  .add {
    display: flex;
    gap: 8px;
    margin-top: 10px;
  }
  .add .input:first-child {
    max-width: 200px;
  }
  .from-saved {
    margin-top: 14px;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 6px;
  }
</style>
