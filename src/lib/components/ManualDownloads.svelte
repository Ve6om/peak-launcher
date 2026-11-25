<script lang="ts">
  // Files a modpack author doesn't allow launchers to download. The user opens
  // each page and downloads it; Peak watches the Downloads folder, moves each
  // file into the instance as it appears and removes it from Downloads.
  import { onMount } from 'svelte';
  import { ExternalLink, Check, LoaderCircle, FolderOpen, Download } from '@lucide/svelte';
  import Modal from './Modal.svelte';
  import { api } from '$lib/api';
  import { app, toast, errorText } from '$lib/state.svelte';
  import type { ManualFile } from '$lib/types';

  let { instanceId }: { instanceId: string } = $props();

  let files = $state<ManualFile[]>([]);
  let loading = $state(true);
  const inst = $derived(app.instances.find((i) => i.id === instanceId));
  const remaining = $derived(files.filter((f) => !f.done).length);
  const where: Record<string, string> = { mods: 'Mods', resourcepacks: 'Resource packs', shaderpacks: 'Shaders', datapacks: 'Data packs' };

  async function scan() {
    try {
      const next = await api.manualScan(instanceId);
      if (next.length) files = next;
      else if (files.length) files = files.map((f) => ({ ...f, done: true }));
    } catch (e) {
      toast(errorText(e), 'error');
    }
  }

  onMount(() => {
    let alive = true;
    (async () => {
      files = await api.manualList(instanceId).catch(() => []);
      loading = false;
      await scan();
    })();
    // Watch the Downloads folder while the dialog is open.
    const timer = setInterval(() => alive && scan(), 1500);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  });

  // Everything found: close and celebrate.
  $effect(() => {
    if (!loading && files.length && remaining === 0) {
      toast(`All files added to ${inst?.name ?? 'the instance'}`, 'success');
      delete app.blocked[instanceId];
      app.manualFor = null;
    }
  });

  async function openAll() {
    for (const f of files.filter((f) => !f.done)) {
      await api.openUrl(f.url);
      await new Promise((r) => setTimeout(r, 350)); // let the browser keep up
    }
  }

  async function skip() {
    app.manualFor = null;
  }

  async function dismiss() {
    await api.manualDismiss(instanceId);
    delete app.blocked[instanceId];
    app.manualFor = null;
  }
</script>

<Modal
  title="{remaining} file{remaining === 1 ? ' needs' : 's need'} a manual download"
  subtitle="Their authors don't allow launchers to download them. Download each one in your browser and Peak will move it into {inst?.name ?? 'the instance'} for you."
  width={600}
  onclose={skip}
>
  <ol class="steps">
    <li>Open each page below and press <b>Download</b> on CurseForge.</li>
    <li>Leave the file in your <b>Downloads</b> folder. Peak picks it up automatically.</li>
  </ol>

  <div class="list">
    {#if loading}
      <div class="center"><LoaderCircle size={20} class="spin" /></div>
    {:else}
      {#each files as f (f.fileName)}
        <div class="row" class:done={f.done}>
          <span class="status">
            {#if f.done}<Check size={14} strokeWidth={3} />{:else}<LoaderCircle size={14} class="spin" />{/if}
          </span>
          <div class="text">
            <b>{f.name}</b>
            <span class="mono">{f.fileName}</span>
          </div>
          <span class="chip">{where[f.folder] ?? f.folder}</span>
          {#if f.done}
            <span class="added">Added</span>
          {:else}
            <button class="btn" onclick={() => api.openUrl(f.url)}><ExternalLink size={14} /> Open</button>
          {/if}
        </div>
      {/each}
    {/if}
  </div>

  <p class="watch"><Download size={13} /> Watching your Downloads folder. Files are moved into the instance and removed from Downloads.</p>

  <footer>
    <button class="btn ghost" onclick={() => api.openDownloadsFolder()}><FolderOpen size={15} /> Open Downloads</button>
    <span class="spacer"></span>
    <button class="btn ghost" onclick={dismiss} title="Stop asking about these files">Ignore</button>
    <button class="btn" onclick={skip}>Later</button>
    <button class="btn primary" disabled={!remaining} onclick={openAll}><ExternalLink size={15} /> Open all ({remaining})</button>
  </footer>
</Modal>

<style>
  .steps {
    margin: 0 0 14px;
    padding-left: 18px;
    font-size: 13px;
    color: var(--muted);
    line-height: 1.6;
  }
  .steps b {
    color: var(--text);
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-height: 46vh;
    overflow-y: auto;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--stroke);
    background: var(--surface-2);
    transition: opacity 0.2s, border-color 0.2s;
  }
  .row.done {
    border-color: color-mix(in srgb, var(--success) 35%, transparent);
  }
  .status {
    width: 22px;
    height: 22px;
    flex-shrink: 0;
    display: grid;
    place-items: center;
    border-radius: 6px;
    color: var(--faint);
    background: rgba(255, 255, 255, 0.05);
  }
  .done .status {
    color: #07160d;
    background: var(--success);
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
  .text .mono {
    font-size: 11.5px;
    color: var(--faint);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .added {
    font-size: 12.5px;
    font-weight: 600;
    color: var(--success);
    padding: 0 6px;
  }
  .watch {
    display: flex;
    align-items: center;
    gap: 7px;
    margin: 12px 0 0;
    font-size: 12px;
    color: var(--faint);
  }
  .center {
    display: grid;
    place-items: center;
    height: 80px;
    color: var(--muted);
  }
  footer {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 16px;
  }
  .spacer {
    flex: 1;
  }
</style>
