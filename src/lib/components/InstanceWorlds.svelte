<script lang="ts">
  // An instance's single-player worlds: play one directly, back it up or
  // restore it, rename, copy or delete it, and manage its data packs.
  import { onMount } from 'svelte';
  import Select from './Select.svelte';
  import Switch from './Switch.svelte';
  import {
    Play,
    Archive,
    ChevronDown,
    Pencil,
    Copy,
    FolderOpen,
    Trash2,
    Sprout,
    RotateCcw,
    Plus,
    LoaderCircle,
    Globe,
    Check,
    X,
    Package
  } from '@lucide/svelte';
  import { api } from '$lib/api';
  import { app, toast, errorText, isRunning, isBusy, activeAccount, go } from '$lib/state.svelte';
  import type { Datapack, Instance, WorldBackup, WorldInfo } from '$lib/types';

  let { inst }: { inst: Instance } = $props();

  let worlds = $state<WorldInfo[]>([]);
  let loaded = $state(false);
  let open = $state<string | null>(null);
  let backups = $state<WorldBackup[]>([]);
  let packs = $state<Datapack[]>([]);
  let renaming = $state<string | null>(null);
  let newName = $state('');
  let busy = $state<string | null>(null);
  /** A destructive button waiting for its second click. */
  let confirm = $state<string | null>(null);
  const running = $derived(isRunning(inst.id));

  async function load() {
    try {
      worlds = await api.worldsList(inst.id);
    } catch (e) {
      toast(errorText(e), 'error');
    } finally {
      loaded = true;
    }
  }
  onMount(load);

  // Reload when the game closes (worlds were played, maybe backed up).
  let wasRunning = false;
  $effect(() => {
    if (wasRunning && !running) setTimeout(load, 1500);
    wasRunning = running;
  });

  async function expand(w: WorldInfo) {
    if (open === w.folder) {
      open = null;
      return;
    }
    open = w.folder;
    renaming = null;
    await loadDetails(w.folder);
  }
  async function loadDetails(folder: string) {
    [backups, packs] = await Promise.all([api.worldBackups(inst.id, folder).catch(() => []), api.worldDatapacks(inst.id, folder).catch(() => [])]);
  }

  /** Run an action with a spinner on `key`, then reload. */
  async function act(key: string, f: () => Promise<unknown>, done?: string) {
    busy = key;
    try {
      await f();
      if (done) toast(done, 'success');
      await load();
      if (open) await loadDetails(open);
    } catch (e) {
      toast(errorText(e), 'error');
    } finally {
      busy = null;
    }
  }

  /** Ask once more before something that can't be undone easily. */
  function sure(key: string) {
    if (confirm === key) {
      confirm = null;
      return true;
    }
    confirm = key;
    setTimeout(() => confirm === key && (confirm = null), 3000);
    return false;
  }

  async function play(w: WorldInfo) {
    if (!activeAccount()) {
      toast('Add an account to start playing', 'info');
      go('accounts');
      return;
    }
    app.logs[inst.id] = [];
    try {
      await api.launchWorld(inst.id, w.folder);
    } catch (e) {
      toast(errorText(e), 'error', { rage: true });
    }
  }

  async function saveName(w: WorldInfo) {
    const name = newName.trim();
    renaming = null;
    if (!name || name === w.name) return;
    await act(`rename:${w.folder}`, () => api.worldRename(inst.id, w.folder, name), `Renamed to ${name}`);
  }

  async function addPacks(w: WorldInfo) {
    const files = await api.pickDatapacks();
    if (!files.length) return;
    await act(`packs:${w.folder}`, async () => {
      for (const f of files) await api.worldAddDatapack(inst.id, w.folder, f);
    }, files.length === 1 ? 'Data pack added' : `${files.length} data packs added`);
  }

  async function copySeed(seed: string) {
    await navigator.clipboard.writeText(seed).catch(() => {});
    toast('Seed copied', 'success');
  }

  // Automatic backups (a Peak-wide setting).
  async function setAuto(patch: Partial<{ worldBackupsAuto: boolean; worldBackupsKeep: number }>) {
    if (!app.settings) return;
    try {
      app.settings = await api.saveSettings({ ...$state.snapshot(app.settings), ...patch });
    } catch (e) {
      toast(errorText(e), 'error');
    }
  }

  const size = (b: number) =>
    b >= 1073741824 ? `${(b / 1073741824).toFixed(1)} GB` : b >= 1048576 ? `${Math.round(b / 1048576)} MB` : `${Math.max(1, Math.round(b / 1024))} KB`;
  function when(ms: number | null) {
    if (!ms) return 'Never played';
    const s = (Date.now() - ms) / 1000;
    if (s < 60) return 'Just now';
    if (s < 3600) return `${Math.floor(s / 60)}m ago`;
    if (s < 86400) return `${Math.floor(s / 3600)}h ago`;
    if (s < 86400 * 30) return `${Math.floor(s / 86400)}d ago`;
    return new Date(ms).toLocaleDateString();
  }
  const date = (ms: number) => new Date(ms).toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' });
  const modeName = (w: WorldInfo) => (w.hardcore ? 'Hardcore' : w.mode ? w.mode[0].toUpperCase() + w.mode.slice(1) : null);
</script>

<div class="wrap">
  <section class="panel glass">
    <h3>Worlds</h3>
    <p class="sub">
      Single-player worlds in this instance, most recently played first.{running ? " The game is open, so worlds can be backed up but not changed until it's closed." : ''}
    </p>

    {#if !loaded}
      <p class="muted"><LoaderCircle size={14} class="spin" /> Loading…</p>
    {:else if !worlds.length}
      <p class="muted"><Globe size={14} /> No worlds yet. Create one in the game and it shows up here.</p>
    {:else}
      <div class="list">
        {#each worlds as w (w.folder)}
          {@const isOpen = open === w.folder}
          <div class="world" class:open={isOpen}>
            <div class="row">
              <span class="icon">{#if w.icon}<img src={w.icon} alt="" />{:else}<Globe size={20} />{/if}</span>
              <div class="text">
                {#if renaming === w.folder}
                  <form
                    class="rename"
                    onsubmit={(e) => {
                      e.preventDefault();
                      saveName(w);
                    }}
                  >
                    <!-- svelte-ignore a11y_autofocus -->
                    <input class="input" bind:value={newName} maxlength="100" autofocus onkeydown={(e) => e.key === 'Escape' && (renaming = null)} />
                    <button class="btn ghost icon" type="submit" aria-label="Save name"><Check size={14} /></button>
                    <button class="btn ghost icon" type="button" aria-label="Cancel" onclick={() => (renaming = null)}><X size={14} /></button>
                  </form>
                {:else}
                  <b>{w.name}</b>
                {/if}
                <span class="meta">
                  {#if modeName(w)}<span class:hardcore={w.hardcore}>{modeName(w)}</span>{/if}
                  {#if w.version}<span>{w.version}</span>{/if}
                  <span>{when(w.lastPlayed)}</span>
                  <span>{size(w.size)}</span>
                  {#if w.backups}<span><Archive size={11} /> {w.backups}</span>{/if}
                </span>
              </div>
              <button class="btn ghost icon" title="Back up now" aria-label="Back up {w.name}" disabled={!!busy} onclick={() => act(`backup:${w.folder}`, () => api.worldBackup(inst.id, w.folder), `Backed up ${w.name}`)}>
                {#if busy === `backup:${w.folder}`}<LoaderCircle size={15} class="spin" />{:else}<Archive size={15} />{/if}
              </button>
              <button class="btn ghost icon chev" class:flip={isOpen} title="More" aria-label="More for {w.name}" aria-expanded={isOpen} onclick={() => expand(w)}><ChevronDown size={16} /></button>
              <button class="btn primary" disabled={running || isBusy(inst.id)} onclick={() => play(w)}><Play size={14} /> Play</button>
            </div>

            {#if isOpen}
              <div class="details">
                <div class="actions">
                  <button
                    class="btn"
                    disabled={running}
                    onclick={() => {
                      renaming = w.folder;
                      newName = w.name;
                    }}><Pencil size={14} /> Rename</button
                  >
                  <button class="btn" disabled={running || !!busy} onclick={() => act(`copy:${w.folder}`, () => api.worldCopy(inst.id, w.folder), `Copied ${w.name}`)}>
                    {#if busy === `copy:${w.folder}`}<LoaderCircle size={14} class="spin" />{:else}<Copy size={14} />{/if} Make a copy
                  </button>
                  <button class="btn" onclick={() => api.worldOpenFolder(inst.id, w.folder).catch((e) => toast(errorText(e), 'error'))}><FolderOpen size={14} /> Open folder</button>
                  {#if w.seed}
                    <button class="btn" title={w.seed} onclick={() => copySeed(w.seed!)}><Sprout size={14} /> Copy seed</button>
                  {/if}
                  <span class="spacer"></span>
                  <button
                    class="btn ghost danger"
                    class:armed={confirm === `delete:${w.folder}`}
                    disabled={running}
                    onclick={() => sure(`delete:${w.folder}`) && act(`delete:${w.folder}`, () => api.worldDelete(inst.id, w.folder), `${w.name} moved to the recycle bin`)}
                  >
                    <Trash2 size={14} /> {confirm === `delete:${w.folder}` ? 'Click again to delete' : 'Delete'}
                  </button>
                </div>

                <h4>Backups</h4>
                {#if backups.length}
                  <div class="sublist">
                    {#each backups as b (b.fileName)}
                      <div class="subrow">
                        <Archive size={14} />
                        <span class="grow">{date(b.created)}{#if b.auto}<em>Automatic</em>{/if}</span>
                        <span class="dim">{size(b.size)}</span>
                        <button
                          class="btn small"
                          class:armed={confirm === `restore:${b.fileName}`}
                          disabled={running || !!busy}
                          title="Put the world back as it was then (it's backed up as it is now first)"
                          onclick={() => sure(`restore:${b.fileName}`) && act(`restore:${b.fileName}`, () => api.worldRestore(inst.id, w.folder, b.fileName), `${w.name} restored`)}
                        >
                          {#if busy === `restore:${b.fileName}`}<LoaderCircle size={13} class="spin" />{:else}<RotateCcw size={13} />{/if}
                          {confirm === `restore:${b.fileName}` ? 'Click again to restore' : 'Restore'}
                        </button>
                        <button
                          class="btn ghost icon small-icon"
                          class:armed={confirm === `delbak:${b.fileName}`}
                          title={confirm === `delbak:${b.fileName}` ? 'Click again to delete this backup' : 'Delete this backup'}
                          aria-label="Delete backup"
                          onclick={() => sure(`delbak:${b.fileName}`) && act('delbak', () => api.worldDeleteBackup(inst.id, w.folder, b.fileName))}
                        >
                          <Trash2 size={13} />
                        </button>
                      </div>
                    {/each}
                  </div>
                {:else}
                  <p class="muted small">No backups of this world yet.</p>
                {/if}

                <h4>Data packs</h4>
                {#if packs.length}
                  <div class="sublist">
                    {#each packs as p (p.fileName)}
                      <div class="subrow">
                        <Package size={14} />
                        <span class="grow mono">{p.fileName}</span>
                        <span class="dim">{size(p.size)}</span>
                        <button
                          class="btn ghost icon small-icon"
                          class:armed={confirm === `pack:${p.fileName}`}
                          title={confirm === `pack:${p.fileName}` ? 'Click again to remove' : 'Remove'}
                          aria-label="Remove {p.fileName}"
                          disabled={running}
                          onclick={() => sure(`pack:${p.fileName}`) && act('pack', () => api.worldRemoveDatapack(inst.id, w.folder, p.fileName))}
                        >
                          <Trash2 size={13} />
                        </button>
                      </div>
                    {/each}
                  </div>
                {:else}
                  <p class="muted small">No data packs in this world.</p>
                {/if}
                <div class="actions">
                  <button class="btn small" disabled={running || !!busy} onclick={() => addPacks(w)}><Plus size={13} /> Add data packs</button>
                  {#if running}<span class="dim">New data packs load the next time the world is opened.</span>{/if}
                </div>
              </div>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  </section>

  {#if app.settings}
    <section class="panel glass">
      <h3>Automatic backups</h3>
      <p class="sub">When the game closes, Peak backs up each world that was played, in every instance. Backups you make by hand are always kept.</p>
      <div class="auto">
        <Switch bind:checked={() => app.settings!.worldBackupsAuto, (v) => setAuto({ worldBackupsAuto: v })} label="Back up worlds when the game closes" />
        <span class="grow">Back up worlds when the game closes</span>
        <span class="dim">Keep</span>
        <Select
          small
          disabled={!app.settings.worldBackupsAuto}
          value={String(app.settings.worldBackupsKeep)}
          onchange={(v) => v && setAuto({ worldBackupsKeep: +v })}
          ariaLabel="Automatic backups to keep per world"
          style="width: 90px;"
          options={[1, 2, 3, 5, 10, 20].map((n) => ({ value: String(n), label: String(n) }))}
        />
        <span class="dim">per world</span>
      </div>
    </section>
  {/if}
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
  .muted.small {
    margin: 2px 0 0;
    font-size: 12.5px;
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-top: 12px;
  }
  .world {
    border-radius: var(--radius-sm);
    background: rgba(255, 255, 255, 0.025);
    border: 1px solid var(--stroke);
  }
  .world.open {
    background: rgba(255, 255, 255, 0.04);
  }
  .row {
    display: grid;
    grid-template-columns: 44px minmax(0, 1fr) auto auto auto;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
  }
  .icon {
    width: 44px;
    height: 44px;
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
    gap: 2px;
    min-width: 0;
  }
  .text b {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 12px;
    font-size: 12px;
    color: var(--muted);
  }
  .meta span {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .meta .hardcore {
    color: #fca5a5;
  }
  .rename {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .rename .input {
    height: 30px;
    max-width: 320px;
  }
  .chev :global(svg) {
    transition: transform 0.2s;
  }
  .chev.flip :global(svg) {
    transform: rotate(180deg);
  }
  .details {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 4px 12px 12px 64px;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .spacer,
  .grow {
    flex: 1;
    min-width: 0;
  }
  h4 {
    margin: 10px 0 2px;
    font-size: 12px;
    font-weight: 650;
    color: var(--muted);
  }
  .sublist {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .subrow {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 34px;
    padding: 2px 6px 2px 8px;
    border-radius: 8px;
    font-size: 12.5px;
    color: var(--muted);
  }
  .subrow:hover {
    background: rgba(255, 255, 255, 0.03);
  }
  .subrow .grow {
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .subrow em {
    margin-left: 8px;
    padding: 1px 6px;
    border-radius: 6px;
    font-style: normal;
    font-size: 10.5px;
    color: var(--muted);
    background: rgba(255, 255, 255, 0.06);
  }
  .mono {
    font-family: var(--font-mono);
    font-size: 12px;
  }
  .dim {
    font-size: 12px;
    color: var(--faint);
  }
  .small {
    height: 28px;
    font-size: 12px;
  }
  .small-icon {
    width: 28px;
    height: 28px;
  }
  .armed {
    color: #fecaca;
    background: rgba(239, 68, 68, 0.18);
  }
  .danger:hover {
    color: #fda4af;
  }
  .auto {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 10px;
    font-size: 13px;
  }
</style>
