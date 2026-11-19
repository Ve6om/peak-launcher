<script lang="ts">
  // Shown after start-up when a modpack the player installed has a new
  // version: update it in place, skip this version, or stop asking.
  import { onMount } from 'svelte';
  import { fly } from 'svelte/transition';
  import { ArrowUpCircle, LoaderCircle, X } from '@lucide/svelte';
  import { api } from '$lib/api';
  import { app, refreshInstances, toast, errorText, go } from '$lib/state.svelte';
  import { packUpdates, checkPackUpdates } from '$lib/packUpdates.svelte';

  // Ones closed with "Not now" stay quiet until Peak is opened again.
  let later = $state<string[]>([]);
  let busy = $state(false);

  onMount(() => {
    // Give start-up a moment before asking modpack sites for versions.
    const t = setTimeout(checkPackUpdates, 4000);
    return () => clearTimeout(t);
  });

  const offer = $derived(packUpdates.list.find((u) => !later.includes(u.instanceId)) ?? null);
  const remaining = $derived(packUpdates.list.filter((u) => !later.includes(u.instanceId)).length);
  const show = $derived(!!offer && app.settings?.modpackUpdateCheck !== false);

  function drop(id: string) {
    packUpdates.list = packUpdates.list.filter((u) => u.instanceId !== id);
  }

  async function update() {
    if (!offer) return;
    if (app.running.includes(offer.instanceId)) {
      toast(`Close ${offer.instanceName} first, then update it.`, 'info');
      return;
    }
    busy = true;
    const u = offer;
    try {
      await api.modpackChangeVersion(u.instanceId, u.latestId);
      drop(u.instanceId);
      await refreshInstances();
      toast(`Updating ${u.packName} to ${u.latest}. Your old mods folder was backed up.`, 'success');
    } catch (e) {
      toast(errorText(e), 'error');
    } finally {
      busy = false;
    }
  }

  async function skip() {
    if (!offer) return;
    const u = offer;
    drop(u.instanceId);
    await api.modpackSkipUpdate(u.instanceId, u.latestId).catch(() => {});
  }

  async function mute() {
    if (!offer) return;
    const u = offer;
    drop(u.instanceId);
    await api.modpackMuteUpdates(u.instanceId, true).catch(() => {});
    await refreshInstances();
    toast(`Peak won't ask about ${u.packName} updates again. You can turn this back on in the instance's settings.`, 'info');
  }
</script>

{#if show && offer}
  {#key offer.instanceId}
    <div class="offer glass" in:fly={{ y: 16, duration: 220 }} role="dialog" aria-label="Modpack update available">
      <span class="icon">
        {#if offer.iconUrl}<img src={offer.iconUrl} alt="" />{:else}<ArrowUpCircle size={18} />{/if}
      </span>
      <div class="text">
        <b>{offer.packName} has an update</b>
        <span>
          <button class="link" onclick={() => go('instance', offer!.instanceId, 'settings')}>{offer.instanceName}</button>:
          {offer.current} → <strong>{offer.latest}</strong>. Your worlds and settings stay; the old mods folder is backed up.
          {#if remaining > 1}<em>{remaining - 1} more {remaining - 1 === 1 ? 'pack has' : 'packs have'} updates.</em>{/if}
        </span>
      </div>
      <div class="actions">
        <button class="btn ghost" onclick={mute} disabled={busy}>Don't ask again</button>
        <button class="btn ghost" onclick={skip} disabled={busy}>Skip this version</button>
        <button class="btn primary" onclick={update} disabled={busy}>
          {#if busy}<LoaderCircle size={15} class="spin" />{/if} Update
        </button>
      </div>
      <button class="close btn ghost icon" aria-label="Not now" title="Not now" onclick={() => (later = [...later, offer!.instanceId])}>
        <X size={15} />
      </button>
    </div>
  {/key}
{/if}

<style>
  .offer {
    position: fixed;
    right: 22px;
    bottom: 22px;
    z-index: 119;
    width: min(500px, calc(100vw - 44px));
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 10px 12px;
    padding: 14px 16px;
    box-shadow: 0 14px 40px rgba(0, 0, 0, 0.5);
  }
  .icon {
    display: grid;
    place-items: center;
    width: 38px;
    height: 38px;
    border-radius: 10px;
    overflow: hidden;
    background: color-mix(in srgb, var(--accent) 16%, transparent);
    color: var(--accent);
  }
  .icon img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .text {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding-right: 22px;
    min-width: 0;
  }
  .text b {
    font-size: 14px;
  }
  .text span {
    font-size: 12.5px;
    line-height: 1.5;
    color: var(--muted);
  }
  .text strong {
    color: var(--text);
    font-weight: 600;
  }
  .text em {
    display: block;
    font-style: normal;
    color: var(--faint);
  }
  .link {
    color: var(--accent);
    font-size: inherit;
  }
  .link:hover {
    text-decoration: underline;
  }
  .actions {
    grid-column: 1 / -1;
    display: flex;
    justify-content: flex-end;
    gap: 6px;
    flex-wrap: wrap;
  }
  .close {
    position: absolute;
    top: 8px;
    right: 8px;
  }
</style>
