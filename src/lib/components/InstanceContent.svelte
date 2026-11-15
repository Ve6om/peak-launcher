<script lang="ts">
  import { Plus, FolderOpen, RefreshCw, Trash2, CircleArrowUp, Search, LoaderCircle, Puzzle, Palette, Sun, List } from '@lucide/svelte';
  import ContentBrowser from './ContentBrowser.svelte';
  import ProjectIcon from './ProjectIcon.svelte';
  import Switch from './Switch.svelte';
  import LinkedModsDialog from './LinkedModsDialog.svelte';
  import { api } from '$lib/api';
  import { toast, errorText, isRunning, requestInstall } from '$lib/state.svelte';
  import { CONTENT_KINDS, type ContentKind, type Instance, type InstalledItem, type LinkedMod, type UpdateInfo } from '$lib/types';

  let { inst }: { inst: Instance } = $props();

  let kind = $state<ContentKind>('mod');
  /** Installed list, or the browser for adding more to this instance. */
  let mode = $state<'installed' | 'browse'>('installed');
  let items = $state<InstalledItem[]>([]);
  let updates = $state<Record<string, UpdateInfo>>({});
  let loading = $state(true);
  let checking = $state(false);
  let busy = $state(new Set<string>());
  let filter = $state('');
  const kindIcons = { mod: Puzzle, resourcepack: Palette, shader: Sun } as const;
  const label = $derived(CONTENT_KINDS.find((k) => k.id === kind)!);
  const running = $derived(isRunning(inst.id));

  async function refresh() {
    loading = true;
    try {
      items = await api.contentList(inst.id, kind);
    } catch (e) {
      toast(errorText(e), 'error');
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    kind;
    inst.id;
    updates = {};
    refresh();
  });

  const shown = $derived(
    items.filter((i) => !filter.trim() || i.title.toLowerCase().includes(filter.toLowerCase()) || i.fileName.toLowerCase().includes(filter.toLowerCase()))
  );
  const updateCount = $derived(Object.keys(updates).length);

  async function withBusy(key: string, fn: () => Promise<unknown>) {
    busy = new Set([...busy, key]);
    try {
      await fn();
    } catch (e) {
      toast(errorText(e), 'error');
    } finally {
      busy.delete(key);
      busy = new Set(busy);
    }
  }

  /** Open prompt about mods that depend on (or are needed by) the one being changed. */
  let linkPrompt = $state<null | {
    action: 'disable' | 'remove' | 'enable';
    item: InstalledItem;
    links: LinkedMod[];
    resolve: (choice: 'all' | 'only' | null) => void;
  }>(null);

  /**
   * Mods to change along with `item`, after asking. Returns null if the user
   * cancelled. Only mods have dependencies; other content goes straight through.
   */
  async function withLinked(item: InstalledItem, action: 'disable' | 'remove' | 'enable'): Promise<LinkedMod[] | null> {
    if (kind !== 'mod') return [];
    let links: LinkedMod[] = [];
    try {
      links = await api.contentLinked(inst.id, item.fileName, action);
    } catch {
      return []; // Can't read the mods; don't block the change.
    }
    if (!links.length) return [];
    const choice = await new Promise<'all' | 'only' | null>((resolve) => (linkPrompt = { action, item, links, resolve }));
    linkPrompt = null;
    if (choice === null) return null;
    return choice === 'all' ? links : [];
  }

  async function toggle(item: InstalledItem, enabled: boolean) {
    const links = await withLinked(item, enabled ? 'enable' : 'disable');
    if (links === null) {
      // The switch already moved; put it back.
      items = items.map((x) => ({ ...x }));
      return;
    }
    await withBusy(item.fileName, async () => {
      await api.contentSetEnabled(inst.id, kind, item.fileName, enabled);
      for (const l of links) await api.contentSetEnabled(inst.id, kind, l.fileName, enabled);
    });
    if (links.length) toast(`${enabled ? 'Enabled' : 'Disabled'} ${item.title} and ${links.length} more`);
    await refresh();
  }

  async function remove(item: InstalledItem) {
    const links = await withLinked(item, 'remove');
    if (links === null) return;
    await withBusy(item.fileName, async () => {
      await api.contentRemove(inst.id, kind, item.fileName);
      for (const l of links) await api.contentRemove(inst.id, kind, l.fileName);
    });
    toast(links.length ? `Removed ${item.title} and ${links.length} more` : `Removed ${item.title}`);
    await refresh();
  }

  async function checkUpdates() {
    checking = true;
    try {
      const list = await api.contentUpdates(inst.id, kind);
      updates = Object.fromEntries(list.map((u) => [u.fileName, u]));
      toast(list.length ? `${list.length} update${list.length > 1 ? 's' : ''} available` : 'Everything is up to date', list.length ? 'info' : 'success');
    } catch (e) {
      toast(errorText(e), 'error');
    } finally {
      checking = false;
    }
  }

  async function update(item: InstalledItem) {
    const u = updates[item.fileName];
    if (!u) return;
    let ok = false;
    await withBusy(item.fileName, async () => {
      ok = await requestInstall(inst.id, kind, u.projectId, u.versionId);
    });
    if (ok) delete updates[item.fileName];
    await refresh();
  }

  async function updateAll() {
    for (const item of items.filter((i) => updates[i.fileName])) await update(item);
    toast('Updates installed', 'success');
  }

  const size = (b: number) => (b > 1048576 ? `${(b / 1048576).toFixed(1)} MB` : `${Math.max(1, Math.round(b / 1024))} KB`);
</script>

<div class="content">
  <div class="bar">
    <div class="seg">
      {#each CONTENT_KINDS as k (k.id)}
        {@const Icon = kindIcons[k.id]}
        <button class:active={kind === k.id} onclick={() => (kind = k.id)}><Icon size={14} /> {k.label}</button>
      {/each}
    </div>
    <div class="seg mode">
      <button class:active={mode === 'installed'} onclick={() => (mode = 'installed')}><List size={14} /> Installed ({items.length})</button>
      <button class:active={mode === 'browse'} onclick={() => (mode = 'browse')}><Plus size={14} /> Add {label.label.toLowerCase()}</button>
    </div>
    {#if mode === 'installed'}
      <div class="filter">
        <Search size={14} />
        <input bind:value={filter} placeholder="Filter {label.label.toLowerCase()}" />
      </div>
      <button class="btn" title="Open folder" onclick={() => api.openContentFolder(inst.id, kind)}><FolderOpen size={15} /></button>
      {#if updateCount}
        <button class="btn" onclick={updateAll} disabled={running}><CircleArrowUp size={15} /> Update all ({updateCount})</button>
      {:else}
        <button class="btn" onclick={checkUpdates} disabled={checking || !items.length}>
          {#if checking}<LoaderCircle size={15} class="spin" />{:else}<RefreshCw size={15} />{/if} Check for updates
        </button>
      {/if}
    {/if}
  </div>

  {#if running}
    <p class="note">The game is running. Changes apply the next time you launch.</p>
  {/if}

  {#if mode === 'browse'}
    <ContentBrowser {inst} {kind} onchanged={refresh} />
  {:else if loading && !items.length}
    <div class="empty"><LoaderCircle size={20} class="spin" /></div>
  {:else if !items.length}
    <div class="empty glass">
      <h3>No {label.label.toLowerCase()} yet</h3>
      <p>
        {kind === 'mod' && inst.loader === 'vanilla'
          ? 'This is a vanilla instance. Pick a mod loader in Settings to use mods.'
          : `Find ${label.label.toLowerCase()} on Modrinth or CurseForge, or drop files into the folder.`}
      </p>
      <button class="btn primary" onclick={() => (mode = 'browse')}><Plus size={15} /> Add {label.label.toLowerCase()}</button>
    </div>
  {:else}
    <div class="list glass">
      {#each shown as item (item.fileName)}
        <div class="item" class:disabled={!item.enabled}>
          <ProjectIcon url={item.iconUrl} title={item.title} size={40} />
          <div class="text">
            <b>{item.title}{#if item.projectId?.startsWith('cf:')}<span class="src">CurseForge</span>{/if}</b>
            <span>{item.versionNumber ?? 'Unknown version'} · {item.fileName.replace(/\.disabled$/, '')} · {size(item.size)}</span>
          </div>
          {#if updates[item.fileName]}
            <button class="btn upd" disabled={busy.has(item.fileName)} onclick={() => update(item)}>
              <CircleArrowUp size={14} /> {updates[item.fileName].versionNumber}
            </button>
          {/if}
          <Switch disabled={busy.has(item.fileName)} label="Enable {item.title}" bind:checked={() => item.enabled, (v) => toggle(item, v)} />
          <button class="btn ghost icon del" title="Remove" aria-label="Remove {item.title}" disabled={busy.has(item.fileName)} onclick={() => remove(item)}>
            <Trash2 size={15} />
          </button>
        </div>
      {/each}
    </div>
    <p class="count">{items.length} {items.length === 1 ? label.single : label.label.toLowerCase()} · {items.filter((i) => !i.enabled).length} disabled</p>
  {/if}
</div>

{#if linkPrompt}
  <LinkedModsDialog action={linkPrompt.action} name={linkPrompt.item.title} links={linkPrompt.links} onchoose={linkPrompt.resolve} />
{/if}

<style>
  .content {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .bar .seg button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .mode button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .filter {
    flex: 1;
    min-width: 160px;
    display: flex;
    align-items: center;
    gap: 8px;
    height: 38px;
    padding: 0 12px;
    border-radius: 12px;
    background: rgba(0, 0, 0, 0.2);
    border: 1px solid var(--stroke);
    color: var(--faint);
  }
  .filter input {
    flex: 1;
    min-width: 0;
    background: none;
    border: 0;
    outline: none;
    font-size: 13px;
    color: var(--text);
  }
  .note {
    margin: 0;
    font-size: 12.5px;
    color: #fde68a;
  }
  .list {
    padding: 6px;
    display: flex;
    flex-direction: column;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 8px 8px 10px;
    border-radius: 12px;
    transition: background 0.15s, opacity 0.2s;
  }
  .item:hover {
    background: rgba(255, 255, 255, 0.04);
  }
  .item.disabled {
    opacity: 0.5;
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
  .text span {
    font-size: 11.5px;
    color: var(--faint);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .src {
    margin-left: 8px;
    padding: 1px 6px;
    border-radius: 6px;
    font-size: 10px;
    font-weight: 650;
    letter-spacing: 0.03em;
    color: #fdba74;
    background: rgba(249, 115, 22, 0.12);
    vertical-align: middle;
  }
  .upd {
    height: 30px;
    font-size: 12px;
    color: #a5f3fc;
    border-color: color-mix(in srgb, var(--accent-2) 40%, transparent);
    background: color-mix(in srgb, var(--accent-2) 10%, transparent);
  }
  .del {
    color: var(--faint);
  }
  .del:hover {
    color: #fda4af;
  }
  .count {
    margin: 0;
    font-size: 12px;
    color: var(--faint);
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 44px;
    text-align: center;
    color: var(--muted);
  }
  .empty h3 {
    color: var(--text);
    font-size: 16px;
  }
  .empty p {
    margin: 0 0 6px;
    font-size: 13px;
    max-width: 420px;
  }
</style>
