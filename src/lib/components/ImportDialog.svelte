<script lang="ts">
  // Import instances from Prism / MultiMC, ATLauncher, CurseForge, the
  // Modrinth App and the official launcher. Files are copied, never moved.
  import { onMount } from 'svelte';
  import { LoaderCircle, FolderSearch, Check, Clock, SearchX } from '@lucide/svelte';
  import Modal from './Modal.svelte';
  import { api } from '$lib/api';
  import { app, playtime, ago, refreshInstances, toast, errorText, go } from '$lib/state.svelte';
  import { loaderLabel, type ImportCandidate } from '$lib/types';

  let candidates = $state<ImportCandidate[]>([]);
  let picked = $state(new Set<string>());
  let loading = $state(true);
  let importing = $state(false);

  onMount(async () => {
    try {
      candidates = await api.importDetect();
    } catch (e) {
      toast(errorText(e), 'error');
    } finally {
      loading = false;
    }
  });

  // Group by launcher, keeping the detected order inside each group.
  const groups = $derived.by(() => {
    const map = new Map<string, ImportCandidate[]>();
    for (const c of candidates) {
      if (!map.has(c.sourceLabel)) map.set(c.sourceLabel, []);
      map.get(c.sourceLabel)!.push(c);
    }
    return [...map.entries()];
  });

  function toggle(c: ImportCandidate) {
    const next = new Set(picked);
    if (next.has(c.path)) next.delete(c.path);
    else next.add(c.path);
    picked = next;
  }

  async function chooseFolder() {
    const path = await api.pickFolder('Choose an instance folder from another launcher');
    if (!path) return;
    try {
      const c = await api.importInspect(path);
      if (!candidates.some((x) => x.path === c.path)) candidates = [c, ...candidates];
      picked = new Set([...picked, c.path]);
    } catch (e) {
      toast(errorText(e), 'error');
    }
  }

  async function run() {
    const list = candidates.filter((c) => picked.has(c.path));
    if (!list.length) return;
    importing = true;
    try {
      const made = await api.importInstances(list);
      await refreshInstances();
      toast(`Importing ${made.length} instance${made.length > 1 ? 's' : ''}. Files copy in the background.`, 'success');
      app.importing = false;
      if (made.length === 1) go('instance', made[0].id);
      else go('library');
    } catch (e) {
      toast(errorText(e), 'error');
    } finally {
      importing = false;
    }
  }
</script>

<Modal
  title="Import from other launchers"
  subtitle="Peak copies worlds, mods and settings. Your other launchers are left untouched."
  width={640}
  onclose={() => (app.importing = false)}
>
  <div class="body">
    {#if loading}
      <div class="center"><LoaderCircle size={22} class="spin" /><span>Looking for instances…</span></div>
    {:else if !candidates.length}
      <div class="center">
        <SearchX size={26} />
        <span>No other launchers found in their usual folders.</span>
        <small>Using MultiMC, or a launcher installed somewhere else? Pick its instance folder below.</small>
      </div>
    {:else}
      {#each groups as [label, list] (label)}
        <div class="group">
          <span class="label">{label}</span>
          {#each list as c (c.path)}
            <button class="cand" class:on={picked.has(c.path)} onclick={() => toggle(c)} title={c.path}>
              <span class="box">{#if picked.has(c.path)}<Check size={13} strokeWidth={3} />{/if}</span>
              <span class="text">
                <b>{c.name}</b>
                <span class="meta">
                  Minecraft {c.gameVersion} · {loaderLabel(c.loader)}{c.loaderVersion ? ` ${c.loaderVersion.replace(`${c.gameVersion}-`, '')}` : ''}
                  {#if c.playtimeSecs}<span class="dot"></span><Clock size={11} /> {playtime(c.playtimeSecs)}{/if}
                  {#if c.lastPlayed}<span class="dot"></span>played {ago(c.lastPlayed).toLowerCase()}{/if}
                </span>
              </span>
              {#if c.alreadyImported}<span class="chip">Imported before</span>{/if}
            </button>
          {/each}
        </div>
      {/each}
    {/if}
  </div>

  <footer>
    <button class="btn ghost" onclick={chooseFolder}><FolderSearch size={16} /> Choose a folder…</button>
    <span class="spacer"></span>
    <button class="btn ghost" onclick={() => (app.importing = false)}>Cancel</button>
    <button class="btn primary" disabled={!picked.size || importing} onclick={run}>
      {#if importing}<LoaderCircle size={16} class="spin" />{/if}
      Import {picked.size || ''}
    </button>
  </footer>
</Modal>

<style>
  .body {
    display: flex;
    flex-direction: column;
    gap: 16px;
    max-height: 52vh;
    min-height: 160px;
    overflow-y: auto;
    margin-right: -8px;
    padding-right: 8px;
  }
  .center {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    min-height: 160px;
    color: var(--muted);
    font-size: 13.5px;
    text-align: center;
  }
  .center small {
    color: var(--faint);
    font-size: 12.5px;
    max-width: 380px;
  }
  .group {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .group .label {
    margin: 0 0 4px 4px;
  }
  .cand {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    border-radius: 12px;
    border: 1px solid transparent;
    text-align: left;
    transition: background 0.15s, border-color 0.15s;
  }
  .cand:hover {
    background: rgba(255, 255, 255, 0.05);
  }
  .cand.on {
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    border-color: color-mix(in srgb, var(--accent) 35%, transparent);
  }
  .box {
    width: 20px;
    height: 20px;
    flex-shrink: 0;
    display: grid;
    place-items: center;
    border-radius: 6px;
    border: 1.5px solid var(--stroke-strong);
    color: var(--on-accent);
  }
  .on .box {
    background: var(--gradient);
    border-color: transparent;
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .text b {
    font-size: 13.5px;
    font-weight: 600;
  }
  .meta {
    display: flex;
    align-items: center;
    gap: 5px;
    flex-wrap: wrap;
    font-size: 12px;
    color: var(--faint);
  }
  .dot {
    width: 3px;
    height: 3px;
    border-radius: 50%;
    background: currentColor;
    margin: 0 2px;
  }
  footer {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 18px;
  }
  .spacer {
    flex: 1;
  }
</style>
