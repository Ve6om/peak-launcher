<script lang="ts">
  // A modpack instance's pack version: switch to any other version (newer or
  // older) in place, and choose whether Peak mentions updates to this pack.
  import { onMount, untrack } from 'svelte';
  import { ArrowUpCircle, LoaderCircle, Package, RefreshCw } from '@lucide/svelte';
  import Row from './Row.svelte';
  import Switch from './Switch.svelte';
  import { api } from '$lib/api';
  import { app, refreshInstances, toast, errorText, isBusy } from '$lib/state.svelte';
  import { packVersion, type ContentVersion, type Instance } from '$lib/types';
  import { packUpdates } from '$lib/packUpdates.svelte';
  import { changePackVersion } from '$lib/manualPack.svelte';

  let { inst, onchanged, plain = false }: { inst: Instance; onchanged: (i: Instance) => void; plain?: boolean } = $props();

  const pack = $derived(inst.pack!);
  let versions = $state<ContentVersion[]>([]);
  let loading = $state(false);
  let loadError = $state<string | null>(null);
  let selected = $state<string | null>(null);
  let confirming = $state(false);
  let busy = $state(false);
  let prompts = $state(untrack(() => !inst.pack?.noUpdatePrompts));

  const current = $derived(pack.versionId);
  const newest = $derived(versions.find((v) => v.version_type === 'release') ?? versions[0] ?? null);
  const update = $derived(packUpdates.list.find((u) => u.instanceId === inst.id) ?? null);
  const running = $derived(app.running.includes(inst.id));
  const installing = $derived(isBusy(inst.id));
  const choice = $derived(versions.find((v) => v.id === selected) ?? null);
  const older = $derived.by(() => {
    if (!choice || !current) return false;
    const i = versions.findIndex((v) => v.id === choice.id);
    const c = versions.findIndex((v) => v.id === current);
    return c >= 0 && i > c;
  });

  async function load() {
    if (!pack.projectId) return;
    loading = true;
    loadError = null;
    try {
      versions = await api.modpackVersions(pack.projectId);
      selected = current && versions.some((v) => v.id === current) ? current : (newest?.id ?? null);
    } catch (e) {
      loadError = errorText(e);
    } finally {
      loading = false;
    }
  }
  onMount(load);

  function label(v: ContentVersion): string {
    const parts = [v.version_number || v.name];
    if (v.version_type !== 'release') parts.push(`(${v.version_type})`);
    if (v.id === current) parts.push('· installed');
    else if (v.id === newest?.id) parts.push('· latest');
    return parts.join(' ');
  }

  async function change() {
    if (!choice) return;
    if (!confirming) {
      confirming = true;
      setTimeout(() => (confirming = false), 6000);
      return;
    }
    confirming = false;
    busy = true;
    try {
      const picked = choice;
      const changed = await changePackVersion(inst, picked.id, async () => {
        packUpdates.list = packUpdates.list.filter((u) => u.instanceId !== inst.id);
        await refreshInstances();
        onchanged(app.instances.find((i) => i.id === inst.id) ?? inst);
      });
      // Null: CurseForge wants the file downloaded by hand; that dialog takes over.
      if (!changed) return;
      // Going back to an older version on purpose: don't nag about the newest.
      if (newest && choice.id !== newest.id) await api.modpackSkipUpdate(inst.id, newest.id).catch(() => {});
      packUpdates.list = packUpdates.list.filter((u) => u.instanceId !== inst.id);
      await refreshInstances();
      onchanged(app.instances.find((i) => i.id === inst.id) ?? changed);
      toast(`Switching ${pack.name} to ${choice.version_number}. Your old mods folder was backed up.`, 'success');
    } catch (e) {
      toast(errorText(e), 'error');
    } finally {
      busy = false;
    }
  }

  async function setPrompts(on: boolean) {
    try {
      await api.modpackMuteUpdates(inst.id, !on);
      if (!on) packUpdates.list = packUpdates.list.filter((u) => u.instanceId !== inst.id);
      await refreshInstances();
    } catch (e) {
      toast(errorText(e), 'error');
    }
  }
</script>

<section class:panel={!plain} class:glass={!plain}>
  {#if !plain}<h3><Package size={16} /> Modpack</h3>{/if}
  <div class="rows">
    <Row title={pack.name} desc="Installed version: {packVersion(pack) || pack.versionNumber || 'unknown'}">
      {#if update}
        <button class="btn primary" onclick={() => ((selected = update.latestId), change())} disabled={busy || running || installing}>
          <ArrowUpCircle size={15} /> {confirming && selected === update.latestId ? 'Click again to update' : `Update to ${update.latest}`}
        </button>
      {/if}
    </Row>

    {#if !pack.projectId}
      <p class="hint">This pack was installed from a file, so Peak can't fetch its other versions.</p>
    {:else}
      <Row title="Version" desc="Switch to any version of this pack, newer or older, without making a new instance">
        <div class="pick">
          {#if loading}
            <span class="hint"><LoaderCircle size={14} class="spin" /> Loading versions…</span>
          {:else if loadError}
            <span class="hint warn">{loadError}</span>
            <button class="btn ghost icon" title="Try again" aria-label="Try again" onclick={load}><RefreshCw size={14} /></button>
          {:else}
            <select class="input" bind:value={selected} onchange={() => (confirming = false)}>
              {#each versions as v (v.id)}<option value={v.id}>{label(v)}</option>{/each}
            </select>
            <button
              class="btn"
              class:confirm={confirming}
              onclick={change}
              disabled={!choice || choice.id === current || busy || running || installing}
            >
              {#if busy}<LoaderCircle size={15} class="spin" />{/if}
              {confirming ? 'Click again to switch' : older ? 'Switch to older version' : 'Switch version'}
            </button>
          {/if}
        </div>
      </Row>
      {#if confirming}
        <p class="note">
          Your worlds, settings and screenshots stay. The current mods folder is moved to a backup in this instance's
          <code>backups</code> folder first, so any mods you added yourself aren't lost; add them back after the switch.
          {#if older} Going back can break worlds that were opened in a newer version, so back those up first.{/if}
        </p>
      {/if}
      {#if running}<p class="hint">Close the game to change the version.</p>{/if}

      <Row title="Update reminders" desc="Ask me when a new version of this pack comes out">
        <!-- The switch's input fires change before it bubbles here, so `prompts` is already updated. -->
        <div class="switch-wrap" onchange={() => setPrompts(prompts)}>
          <Switch bind:checked={prompts} label="Update reminders" />
        </div>
      </Row>
    {/if}
  </div>
</section>

<style>
  h3 {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .switch-wrap {
    display: contents;
  }
  .pick {
    display: flex;
    gap: 8px;
    align-items: center;
    width: 100%;
  }
  .pick select {
    min-width: 0;
    flex: 1;
  }
  .hint {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12.5px;
    color: var(--faint);
    margin: 0;
  }
  .hint.warn {
    color: #fbbf24;
  }
  .note {
    margin: 0;
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    font-size: 12.5px;
    line-height: 1.5;
    color: var(--muted);
    background: color-mix(in srgb, var(--accent) 7%, transparent);
    border: 1px solid color-mix(in srgb, var(--accent) 20%, transparent);
  }
  code {
    font-size: 11.5px;
    padding: 1px 5px;
    border-radius: 5px;
    background: rgba(255, 255, 255, 0.08);
  }
  .btn.confirm {
    color: var(--on-accent);
    background: var(--accent);
  }
</style>
