<script lang="ts">
  // Export an instance as a Modrinth .mrpack or CurseForge .zip.
  import { onMount } from 'svelte';
  import { LoaderCircle, Check, Folder, FileText, TriangleAlert, FolderOpen, PackageCheck } from '@lucide/svelte';
  import Modal from './Modal.svelte';
  import { api } from '$lib/api';
  import { app, activeAccount, toast, errorText } from '$lib/state.svelte';
  import type { ExportEntry, ExportReport, Instance } from '$lib/types';

  let { inst }: { inst: Instance } = $props();

  let format = $state<'modrinth' | 'curseforge'>('modrinth');
  let name = $state('');
  let version = $state('1.0.0');
  let author = $state('');
  let summary = $state('');
  let entries = $state<ExportEntry[]>([]);
  let include = $state(new Set<string>());
  let loading = $state(true);
  let exporting = $state(false);
  let report = $state<ExportReport | null>(null);

  const hasCfKey = $derived(!!app.info?.curseforgeBuiltIn || !!app.settings?.curseforgeApiKey?.trim());

  onMount(async () => {
    name = inst.pack?.name ?? inst.name;
    author = activeAccount()?.username ?? '';
    try {
      entries = await api.exportEntries(inst.id);
      include = new Set(entries.filter((e) => e.suggested).map((e) => e.name));
    } catch (e) {
      toast(errorText(e), 'error');
    } finally {
      loading = false;
    }
  });

  function toggle(n: string) {
    const next = new Set(include);
    if (next.has(n)) next.delete(n);
    else next.add(n);
    include = next;
  }

  const safe = (s: string) => s.replace(/[<>:"/\\|?*]+/g, '').trim() || 'modpack';

  async function run() {
    const ext = format === 'modrinth' ? 'mrpack' : 'zip';
    const path = await api.pickSavePath(
      `${safe(name)}-${safe(version)}.${ext}`,
      ext,
      format === 'modrinth' ? 'Modrinth modpack' : 'CurseForge modpack'
    );
    if (!path) return;
    exporting = true;
    try {
      report = await api.exportPack(inst.id, {
        format,
        name: name.trim(),
        version: version.trim(),
        author: author.trim(),
        summary: summary.trim(),
        include: [...include],
        path
      });
    } catch (e) {
      toast(errorText(e), 'error');
    } finally {
      exporting = false;
    }
  }

  const size = (b: number) => (b > 1048576 ? `${(b / 1048576).toFixed(1)} MB` : `${Math.max(1, Math.round(b / 1024))} KB`);
</script>

<Modal title="Export {inst.name}" subtitle="Share this instance as a modpack others can install." width={600} onclose={() => (app.exporting = null)}>
  {#if report}
    <div class="done">
      <div class="done-icon"><PackageCheck size={26} /></div>
      <h3>Modpack exported</h3>
      <p class="path">{report.path}</p>
      <ul class="stats">
        <li><b>{report.linked}</b> file{report.linked === 1 ? '' : 's'} linked to {format === 'modrinth' ? 'Modrinth' : 'CurseForge'} (downloaded on install)</li>
        <li><b>{report.overrideFiles}</b> config and extra file{report.overrideFiles === 1 ? '' : 's'} included</li>
        <li>Pack size <b>{size(report.sizeBytes)}</b></li>
      </ul>
      {#if report.bundled.length}
        <div class="warn">
          <TriangleAlert size={16} />
          <div>
            <b>{report.bundled.length} file{report.bundled.length > 1 ? 's were' : ' was'} bundled inside the pack</b>
            <p>
              {format === 'modrinth' ? "They aren't on Modrinth" : "They couldn't be matched on CurseForge"}, so the files themselves are in the
              pack. Make sure you're allowed to share them{format === 'curseforge' && !hasCfKey ? ', or add a CurseForge API key so Peak can link mods that are on CurseForge' : ''}.
            </p>
            <div class="bundled">{#each report.bundled as b (b)}<code>{b}</code>{/each}</div>
          </div>
        </div>
      {/if}
      <div class="actions">
        <button class="btn" onclick={() => report && api.revealFile(report.path)}><FolderOpen size={15} /> Show in folder</button>
        <button class="btn primary" onclick={() => (app.exporting = null)}>Done</button>
      </div>
    </div>
  {:else}
    <div class="form">
      <div class="formats">
        <button class="fmt" class:on={format === 'modrinth'} onclick={() => (format = 'modrinth')}>
          <b>Modrinth</b><span>.mrpack · works in Peak, Modrinth App, Prism</span>
        </button>
        <button class="fmt" class:on={format === 'curseforge'} onclick={() => (format = 'curseforge')}>
          <b>CurseForge</b><span>.zip · works in the CurseForge app and most launchers</span>
        </button>
      </div>
      {#if format === 'curseforge' && !hasCfKey}
        <p class="hint">Without a CurseForge API key, only mods you installed from CurseForge can be linked; the rest get bundled.</p>
      {/if}

      <div class="grid">
        <label class="field"><span class="label">Name</span><input class="input" bind:value={name} maxlength="64" /></label>
        <label class="field"><span class="label">Version</span><input class="input" bind:value={version} maxlength="32" /></label>
        <label class="field"><span class="label">Author</span><input class="input" bind:value={author} maxlength="64" /></label>
        {#if format === 'modrinth'}
          <label class="field"><span class="label">Summary</span><input class="input" bind:value={summary} placeholder="Optional" maxlength="200" /></label>
        {/if}
      </div>

      <div class="field">
        <span class="label">Also include</span>
        <p class="hint">Mods, resource packs and shaders are always included. Pick any configs or extras the pack needs.</p>
        {#if loading}
          <div class="center"><LoaderCircle size={18} class="spin" /></div>
        {:else if !entries.length}
          <p class="hint">Nothing else in this instance yet.</p>
        {:else}
          <div class="entries">
            {#each entries as e (e.name)}
              <button class="entry" class:on={include.has(e.name)} onclick={() => toggle(e.name)}>
                <span class="box">{#if include.has(e.name)}<Check size={12} strokeWidth={3} />{/if}</span>
                {#if e.isDir}<Folder size={14} />{:else}<FileText size={14} />{/if}
                <span>{e.name}</span>
                {#if e.name === 'saves'}<span class="tag">worlds</span>{/if}
              </button>
            {/each}
          </div>
        {/if}
      </div>
    </div>
    <footer>
      <button class="btn ghost" onclick={() => (app.exporting = null)}>Cancel</button>
      <button class="btn primary" disabled={exporting || !name.trim() || !version.trim()} onclick={run}>
        {#if exporting}<LoaderCircle size={16} class="spin" /> Exporting…{:else}Export {format === 'modrinth' ? '.mrpack' : '.zip'}{/if}
      </button>
    </footer>
  {/if}
</Modal>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .formats {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }
  .fmt {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 12px 14px;
    border-radius: 14px;
    text-align: left;
    border: 1px solid var(--stroke);
    background: rgba(255, 255, 255, 0.03);
    transition: border-color 0.2s, background 0.2s;
  }
  .fmt b {
    font-size: 14px;
  }
  .fmt span {
    font-size: 12px;
    color: var(--faint);
  }
  .fmt.on {
    border-color: color-mix(in srgb, var(--accent) 55%, transparent);
    background: color-mix(in srgb, var(--accent) 12%, transparent);
  }
  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 7px;
  }
  .hint {
    margin: 0;
    font-size: 12px;
    color: var(--faint);
  }
  .entries {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    max-height: 160px;
    overflow-y: auto;
  }
  .entry {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 32px;
    padding: 0 11px 0 8px;
    border-radius: 10px;
    font-size: 12.5px;
    color: var(--muted);
    border: 1px solid var(--stroke);
  }
  .entry.on {
    color: var(--text);
    border-color: color-mix(in srgb, var(--accent) 45%, transparent);
    background: color-mix(in srgb, var(--accent) 10%, transparent);
  }
  .box {
    width: 16px;
    height: 16px;
    display: grid;
    place-items: center;
    border-radius: 5px;
    border: 1.5px solid var(--stroke-strong);
    color: var(--on-accent);
  }
  .on .box {
    background: var(--gradient);
    border-color: transparent;
  }
  .tag {
    font-size: 10.5px;
    color: var(--faint);
  }
  .center {
    display: grid;
    place-items: center;
    height: 50px;
    color: var(--muted);
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 20px;
  }
  .done {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    text-align: center;
  }
  .done-icon {
    width: 52px;
    height: 52px;
    border-radius: 16px;
    display: grid;
    place-items: center;
    color: var(--on-accent);
    background: var(--gradient);
  }
  .done h3 {
    font-size: 17px;
  }
  .path {
    margin: 0;
    font-size: 12px;
    color: var(--faint);
    word-break: break-all;
    user-select: text;
  }
  .stats {
    list-style: none;
    margin: 4px 0;
    padding: 0;
    font-size: 13px;
    color: var(--muted);
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .stats b {
    color: var(--text);
  }
  .warn {
    display: flex;
    gap: 10px;
    text-align: left;
    padding: 12px 14px;
    border-radius: 12px;
    color: #fde68a;
    background: rgba(245, 158, 11, 0.08);
    border: 1px solid rgba(245, 158, 11, 0.25);
  }
  .warn p {
    margin: 4px 0 8px;
    font-size: 12.5px;
    color: var(--muted);
  }
  .bundled {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .bundled code {
    font-size: 11px;
    padding: 1px 6px;
    border-radius: 5px;
    background: rgba(255, 255, 255, 0.07);
    color: var(--muted);
  }
  .actions {
    display: flex;
    gap: 8px;
    margin-top: 8px;
  }
</style>
