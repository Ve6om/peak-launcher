<script lang="ts">
  // "Download this pack yourself": open its CurseForge page, then Peak
  // spots the zip in Downloads and installs it.
  import { ExternalLink, FolderOpen, LoaderCircle, CheckCircle2, CircleAlert } from '@lucide/svelte';
  import { untrack } from 'svelte';
  import Modal from './Modal.svelte';
  import { api } from '$lib/api';
  import { manualPack, installDownloaded } from '$lib/manualPack.svelte';

  const cur = $derived(manualPack.current);
  let opened = $state(false);
  let installing = $state(false);
  let failed = $state(false);

  // Look for the file every couple of seconds while the dialog is open
  // (a quick look at one folder; nothing is read until it's there).
  // Only `cur` restarts this: everything else is read untracked, or setting
  // `installing` would restart the watcher and install the pack again.
  $effect(() => {
    const c = cur;
    return untrack(() => {
      opened = false;
      installing = false;
      failed = false;
      if (!c) return;
      let stop = false;
      const tick = async () => {
        if (stop || installing) return;
        const path = await api.findDownload(c.info.fileName, c.info.size).catch(() => null);
        if (stop || installing || !path) return;
        stop = true;
        clearInterval(timer);
        installing = true;
        if (!(await installDownloaded(path, true))) {
          installing = false;
          failed = true;
        }
      };
      const timer = setInterval(tick, 2000);
      tick();
      return () => {
        stop = true;
        clearInterval(timer);
      };
    });
  });

  function download() {
    if (!cur) return;
    opened = true;
    api.openUrl(cur.info.downloadPage);
  }

  async function pick() {
    const path = await api.pickModpackFile();
    if (!path) return;
    installing = true;
    failed = false;
    if (!(await installDownloaded(path))) {
      installing = false;
      failed = true;
    }
  }

  const mb = (n: number) => `${(n / 1024 / 1024).toFixed(n > 100 * 1024 * 1024 ? 0 : 1)} MB`;
</script>

{#if cur}
  <Modal title="Download {cur.info.name} from CurseForge" subtitle={cur.info.version} width={520} onclose={() => !installing && (manualPack.current = null)}>
    <div class="body">
      {#if cur.info.iconUrl}<img src={cur.info.iconUrl} alt="" />{/if}
      <p>
        The pack's author doesn't allow launchers to download it, so download it from CurseForge yourself. Keep this
        open: as soon as <b>{cur.info.fileName}</b> ({mb(cur.info.size)}) is in your Downloads folder, Peak installs it.
      </p>
    </div>
    <div class="status">
      {#if installing}
        <CheckCircle2 size={16} /> Found it. Installing…
      {:else if failed}
        <CircleAlert size={16} /> That file didn't install. Download it again, or pick the file yourself.
      {:else if opened}
        <LoaderCircle size={16} class="spin" /> Waiting for the download to finish…
      {:else}
        <LoaderCircle size={16} class="spin" /> Watching your Downloads folder…
      {/if}
    </div>
    <footer>
      <button class="btn ghost" onclick={() => (manualPack.current = null)} disabled={installing}>Cancel</button>
      <button class="btn" onclick={pick} disabled={installing}><FolderOpen size={15} /> I already have the file</button>
      <button class="btn primary" onclick={download} disabled={installing}><ExternalLink size={15} /> Download from CurseForge</button>
    </footer>
  </Modal>
{/if}

<style>
  .body {
    display: flex;
    gap: 14px;
    align-items: flex-start;
  }
  img {
    width: 48px;
    height: 48px;
    border-radius: 10px;
    flex-shrink: 0;
  }
  p {
    margin: 0;
    font-size: 13.5px;
    line-height: 1.6;
    color: var(--muted);
  }
  b {
    color: var(--text);
    word-break: break-all;
  }
  .status {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 14px;
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    background: rgba(255, 255, 255, 0.04);
    font-size: 13px;
    color: var(--muted);
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 18px;
  }
</style>
