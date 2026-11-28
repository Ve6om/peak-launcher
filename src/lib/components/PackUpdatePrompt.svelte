<script lang="ts">
  import { t, th } from '$lib/i18n.svelte';
  // Asked when the player presses Play on a modpack that has a newer version:
  // update first, play this version, skip this version, or stop asking.
  import { ArrowUpCircle, Play, LoaderCircle } from '@lucide/svelte';
  import Modal from './Modal.svelte';
  import { api } from '$lib/api';
  import { refreshInstances, toast, errorText, go } from '$lib/state.svelte';
  import { packUpdates } from '$lib/packUpdates.svelte';
  import { changePackVersion } from '$lib/manualPack.svelte';

  const p = $derived(packUpdates.prompt);
  let busy = $state(false);

  function finish(play: boolean) {
    const resolve = packUpdates.prompt?.resolve;
    packUpdates.prompt = null;
    resolve?.(play);
  }

  function mark(change: Partial<{ skipped: boolean; muted: boolean }>) {
    if (!p) return;
    packUpdates.list = packUpdates.list.map((u) => (u.instanceId === p.inst.id ? { ...u, ...change } : u));
  }

  async function update() {
    if (!p) return;
    busy = true;
    const { inst, update } = p;
    try {
      const started = await changePackVersion(inst, update.latestId, () => {
        packUpdates.list = packUpdates.list.filter((u) => u.instanceId !== inst.id);
      });
      finish(false);
      // Null: CurseForge wants the file downloaded by hand; that dialog takes over.
      if (!started) return;
      packUpdates.list = packUpdates.list.filter((u) => u.instanceId !== inst.id);
      await refreshInstances();
      go('instance', inst.id);
      toast(t('Updating {packName} to {latest}. Press Play when it\'s done.', { packName: update.packName, latest: update.latest }), 'success');
    } catch (e) {
      toast(errorText(e), 'error');
    } finally {
      busy = false;
    }
  }

  async function skip() {
    if (!p) return;
    await api.modpackSkipUpdate(p.inst.id, p.update.latestId).catch(() => {});
    mark({ skipped: true });
    finish(true);
  }

  async function mute() {
    if (!p) return;
    await api.modpackMuteUpdates(p.inst.id, true).catch(() => {});
    mark({ muted: true });
    await refreshInstances();
    finish(true);
  }
</script>

{#if p}
  <Modal title={t('{packName} has an update', { packName: p.update.packName })} subtitle={p.inst.name} width={500} onclose={() => !busy && finish(false)}>
    <div class="body">
      {#if p.update.iconUrl}<img src={p.update.iconUrl} alt="" />{:else}<span class="ic"><ArrowUpCircle size={22} /></span>{/if}
      <p>
        {@html th('You have <b>{current}</b>; <b>{latest}</b> is out. Update before playing? Your worlds and settings stay, and the old mods folder is backed up. If you play on a server, check which version it runs.', { current: p.update.current, latest: p.update.latest })}
      </p>
    </div>
    <footer>
      <button class="btn ghost" onclick={mute} disabled={busy}>{t('Don\'t ask again')}</button>
      <button class="btn ghost" onclick={skip} disabled={busy}>{t('Skip this version')}</button>
      <span class="spacer"></span>
      <button class="btn" onclick={() => finish(true)} disabled={busy}><Play size={14} /> {t('Play anyway')}</button>
      <button class="btn primary" onclick={update} disabled={busy}>
        {#if busy}<LoaderCircle size={15} class="spin" />{:else}<ArrowUpCircle size={15} />{/if} {t('Update')}
      </button>
    </footer>
  </Modal>
{/if}

<style>
  .body {
    display: flex;
    gap: 14px;
    align-items: flex-start;
  }
  img,
  .ic {
    width: 48px;
    height: 48px;
    flex-shrink: 0;
    border-radius: 10px;
  }
  .ic {
    display: grid;
    place-items: center;
    background: color-mix(in srgb, var(--accent) 15%, transparent);
    color: var(--accent);
  }
  p {
    margin: 0;
    font-size: 13.5px;
    line-height: 1.6;
    color: var(--muted);
  }
  p :global(b) {
    color: var(--text);
  }
  footer {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
    margin-top: 18px;
  }
  .spacer {
    flex: 1;
  }
</style>
