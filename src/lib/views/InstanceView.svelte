<script lang="ts">
  import { ArrowLeft, FolderOpen, Trash2, Terminal, LayoutGrid, Wrench, Clock, Calendar, Cpu, SlidersHorizontal, Layers, PackageOpen, SquareArrowOutUpRight, Server } from '@lucide/svelte';
  import InstanceSettings from '$lib/components/InstanceSettings.svelte';
  import InstanceContent from '$lib/components/InstanceContent.svelte';
  import ConsolePanel from '$lib/components/ConsolePanel.svelte';
  import { Puzzle } from '@lucide/svelte';
  import { loaderLabel, packVersion } from '$lib/types';
  import InstanceArt from '$lib/components/InstanceArt.svelte';
  import PlayButton from '$lib/components/PlayButton.svelte';
  import Progress from '$lib/components/Progress.svelte';
  import { api } from '$lib/api';
  import { app, go, selected, playtime, ago, isBusy, isRunning, toast, repairInstance, openInstanceMenu } from '$lib/state.svelte';

  const inst = $derived(selected());

  const logs = $derived(inst ? (app.logs[inst.id] ?? []) : []);

  // Files still waiting for a manual download (saved with the instance).
  let manualPending = $state(0);
  $effect(() => {
    const id = inst?.id;
    app.manualFor; // re-check when the dialog closes
    app.blocked[id ?? ''];
    if (!id) return;
    api.manualList(id).then((l) => (manualPending = l.filter((f) => !f.done).length)).catch(() => (manualPending = 0));
  });

  // Jump to the console when the game starts (only on the transition).
  let wasRunning = false;
  $effect(() => {
    const r = !!inst && isRunning(inst.id);
    if (r && !wasRunning && app.settings?.openConsole !== false) app.instanceTab = 'console';
    wasRunning = r;
  });
</script>

{#if inst}
  <div class="page">
    <button class="back btn ghost" onclick={() => go('library')}><ArrowLeft size={16} /> Library</button>

    <section class="banner glass rise" oncontextmenu={(e) => openInstanceMenu(e, inst)} aria-label={inst.name}>
      <div class="art"><InstanceArt seed={inst.id} image={inst.pack?.bannerUrl} icon={inst.pack?.iconUrl} /></div>
      <div class="shade"></div>
      <div class="content">
        <div class="title">
          <h1>{inst.name}</h1>
          <div class="chips">
            <span class="chip">Minecraft {inst.gameVersion}</span>
            <span class="chip">{loaderLabel(inst.loader)}{inst.loaderVersion ? ` ${inst.loaderVersion}` : ''}</span>
            {#if inst.pack}<span class="chip">Modpack {packVersion(inst.pack)}</span>{/if}
            {#if isRunning(inst.id)}<span class="chip live">● Playing</span>{/if}
          </div>
        </div>
        <div class="actions">
          <PlayButton {inst} />
          <button class="btn lg icon-lg" title="Open folder" aria-label="Open folder" onclick={() => api.openFolder(inst.id)}>
            <FolderOpen size={18} />
          </button>
        </div>
      </div>
      {#if isBusy(inst.id) && app.progress[inst.id]}
        <div class="progress"><Progress p={app.progress[inst.id]} /></div>
      {/if}
    </section>

    <div class="tabs">
      <button class:active={app.instanceTab === 'overview'} onclick={() => (app.instanceTab = 'overview')}><LayoutGrid size={15} /> Overview</button>
      <button class:active={app.instanceTab === 'content'} onclick={() => (app.instanceTab = 'content')}><Puzzle size={15} /> Content</button>
      <button class:active={app.instanceTab === 'settings'} onclick={() => (app.instanceTab = 'settings')}><SlidersHorizontal size={15} /> Settings</button>
      <button class:active={app.instanceTab === 'console'} onclick={() => (app.instanceTab = 'console')}>
        <Terminal size={15} /> Console {#if logs.length}<span class="count">{logs.length}</span>{/if}
      </button>
    </div>

    {#if app.instanceTab === 'overview'}
      <div class="overview">
        {#if manualPending}
          <div class="panel glass blocked">
            <h3>{manualPending} file{manualPending > 1 ? 's' : ''} still need a manual download</h3>
            <p>Their authors don't allow launchers to download them. Peak will move each one into place as soon as you download it.</p>
            <div class="blocked-actions">
              <button class="btn primary" onclick={() => (app.manualFor = inst.id)}>Download them</button>
            </div>
          </div>
        {/if}
        <div class="stats">
          <div class="stat glass"><Clock size={17} /><div><b>{playtime(inst.playtimeSecs)}</b><span>Playtime</span></div></div>
          <div class="stat glass"><Calendar size={17} /><div><b>{ago(inst.lastPlayed)}</b><span>Last played</span></div></div>
          <div class="stat glass">
            <Cpu size={17} /><div><b>{((inst.memoryMb ?? app.settings?.memoryMb ?? 4096) / 1024).toFixed(1)} GB</b><span>Memory{inst.memoryMb ? '' : ' (default)'}</span></div>
          </div>
        </div>

        <div class="panel glass about">
          <div class="about-row"><Layers size={16} /><span>Mod loader</span><b>{loaderLabel(inst.loader)}{inst.loaderVersion ? ` ${inst.loaderVersion}` : ''}</b></div>
          {#if inst.pack}
            <div class="about-row"><Layers size={16} /><span>Modpack</span><b>{inst.pack.name} · {packVersion(inst.pack)}</b></div>
            {#if inst.pack.description}<p class="notes">{inst.pack.description}</p>{/if}
          {/if}
          {#if inst.notes}<p class="notes">{inst.notes}</p>{/if}
          <button class="btn" onclick={() => (app.instanceTab = 'settings')}><SlidersHorizontal size={15} /> Edit settings</button>
        </div>

        <div class="panel glass">
          <h3>Manage</h3>
          <div class="manage">
            <button class="btn" disabled={isBusy(inst.id) || isRunning(inst.id)} onclick={() => repairInstance(inst)}><Wrench size={16} /> Verify & repair files</button>
            <button class="btn" onclick={() => api.openFolder(inst.id)}><FolderOpen size={16} /> Open folder</button>
            <button class="btn" onclick={() => (app.exporting = inst)}><PackageOpen size={16} /> Export as modpack</button>
            <button class="btn" onclick={() => (app.serverPack = inst)}><Server size={16} /> Export as server pack</button>
            <button class="btn danger" disabled={isRunning(inst.id)} onclick={() => (app.deleting = inst)}><Trash2 size={16} /> Delete instance</button>
          </div>
        </div>
      </div>
    {:else if app.instanceTab === 'content'}
      <InstanceContent {inst} />
    {:else if app.instanceTab === 'settings'}
      <InstanceSettings {inst} />
    {:else}
      {#if app.poppedOut[inst.id]}
        <div class="popped glass">
          <SquareArrowOutUpRight size={22} />
          <b>The console is open in its own window</b>
          <div class="popped-actions">
            <button class="btn" onclick={() => api.openConsoleWindow(inst.id)}>Show window</button>
            <button class="btn ghost" onclick={() => api.closeConsoleWindow(inst.id)}>Bring it back here</button>
          </div>
        </div>
      {:else}
        <ConsolePanel instanceId={inst.id} {logs} running={isRunning(inst.id)} onclear={() => (app.logs[inst.id] = [])} />
      {/if}
    {/if}
  </div>

{:else}
  <p class="missing">This instance no longer exists.</p>
{/if}

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: 18px;
  }
  .back {
    align-self: flex-start;
    margin-left: -10px;
    color: var(--muted);
  }
  .banner {
    position: relative;
    min-height: 230px;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    padding: 0;
  }
  .art {
    position: absolute;
    inset: 0;
  }
  .shade {
    position: absolute;
    inset: 0;
    background: linear-gradient(0deg, rgba(7, 8, 13, 0.92) 0%, rgba(7, 8, 13, 0.35) 55%, rgba(7, 8, 13, 0.1) 100%);
  }
  .content {
    position: relative;
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 20px;
    padding: 28px;
    flex-wrap: wrap;
  }
  h1 {
    font-size: 34px;
    font-weight: 760;
  }
  .chips {
    display: flex;
    gap: 6px;
    margin-top: 10px;
  }
  .chips .chip {
    background: rgba(255, 255, 255, 0.1);
    color: var(--text);
  }
  .chip.live {
    background: rgba(16, 185, 129, 0.25) !important;
    color: #a7f3d0 !important;
  }
  .actions {
    display: flex;
    gap: 10px;
  }
  .icon-lg {
    width: 48px;
    padding: 0;
  }
  .progress {
    position: relative;
    padding: 0 28px 24px;
    max-width: 480px;
  }
  .tabs {
    display: flex;
    gap: 4px;
    border-bottom: 1px solid var(--stroke);
  }
  .tabs button {
    position: relative;
    display: flex;
    align-items: center;
    gap: 7px;
    height: 40px;
    padding: 0 14px;
    font-size: 13.5px;
    font-weight: 560;
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
  .overview {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .stats {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 14px;
  }
  .stat {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 16px 18px;
    color: var(--accent-2);
  }
  .stat div {
    display: flex;
    flex-direction: column;
  }
  .stat b {
    color: var(--text);
    font-size: 16px;
    font-weight: 650;
  }
  .stat span {
    font-size: 12px;
    color: var(--faint);
  }
  .panel {
    padding: 20px 22px;
  }
  .panel h3 {
    font-size: 15px;
    font-weight: 650;
    margin-bottom: 8px;
  }
  .blocked {
    border-color: rgba(245, 158, 11, 0.35);
    background: rgba(245, 158, 11, 0.07);
  }
  .blocked h3 {
    color: #fde68a;
  }
  .blocked p {
    margin: 6px 0 12px;
    font-size: 13px;
    color: var(--muted);
  }
  .blocked-actions {
    display: flex;
    gap: 6px;
    margin-top: 12px;
  }
  .about {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 12px;
  }
  .about-row {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 13px;
    color: var(--muted);
  }
  .about-row b {
    color: var(--text);
    font-weight: 600;
  }
  .notes {
    margin: 0;
    white-space: pre-wrap;
    font-size: 13px;
    color: var(--muted);
    line-height: 1.55;
    user-select: text;
  }
  .manage {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }
  .popped {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    min-height: 220px;
    color: var(--muted);
    text-align: center;
  }
  .popped b {
    color: var(--text);
    font-size: 14px;
  }
  .popped-actions {
    display: flex;
    gap: 8px;
    margin-top: 4px;
  }
  .missing {
    color: var(--muted);
  }
</style>
