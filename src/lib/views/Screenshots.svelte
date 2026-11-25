<script lang="ts">
  // Every screenshot from every instance, grouped by play session, with
  // previews, the player's own tags, and copy / save / export, without
  // digging through instance folders.
  import { onMount } from 'svelte';
  import { instanceIcon } from '$lib/instanceImages';
  import Select from '$lib/components/Select.svelte';
  import { fade } from 'svelte/transition';
  import { Camera, Copy, Download, FolderOpen, Trash2, X, ChevronLeft, ChevronRight, RefreshCw, Check, Tag, Archive } from '@lucide/svelte';
  import { api, isTauri } from '$lib/api';
  import { app, toast, errorText } from '$lib/state.svelte';
  import type { Screenshot } from '$lib/types';

  let shots = $state<Screenshot[] | null>(null);
  let instanceFilter = $state<string>('all');
  let tagFilter = $state<string | null>(null);
  let selected = $state<Record<string, boolean>>({});
  let open = $state<number | null>(null);
  let busy = $state(false);

  let toUrl: ((p: string) => string) | null = $state(null);
  const src = (p: string) => (toUrl ? toUrl(p) : p);

  async function load() {
    try {
      shots = await api.screenshots();
    } catch (e) {
      shots = [];
      toast(errorText(e), 'error');
    }
  }
  onMount(async () => {
    if (isTauri) toUrl = (await import('@tauri-apps/api/core')).convertFileSrc;
    load();
  });

  const nameOf = (id: string) => app.instances.find((i) => i.id === id)?.name ?? 'Deleted instance';
  const counts = $derived.by(() => {
    const c: Record<string, number> = {};
    for (const s of shots ?? []) c[s.instanceId] = (c[s.instanceId] ?? 0) + 1;
    return c;
  });
  const allTags = $derived([...new Set((shots ?? []).flatMap((s) => s.tags))].sort());
  const visible = $derived(
    (shots ?? []).filter((s) => (instanceFilter === 'all' || s.instanceId === instanceFilter) && (!tagFilter || s.tags.includes(tagFilter)))
  );

  // A session: screenshots from one instance with no more than 30 minutes
  // between them (shots are newest first).
  const SESSION_GAP_MS = 30 * 60 * 1000;
  const sessions = $derived.by(() => {
    const out: { key: string; instanceId: string; start: Date; items: { shot: Screenshot; index: number }[] }[] = [];
    visible.forEach((shot, index) => {
      const t = new Date(shot.takenAt);
      const last = out.at(-1);
      const prev = last?.items.at(-1)?.shot;
      if (last && prev && last.instanceId === shot.instanceId && +new Date(prev.takenAt) - +t <= SESSION_GAP_MS) {
        last.items.push({ shot, index });
        last.start = t;
      } else {
        out.push({ key: shot.key, instanceId: shot.instanceId, start: t, items: [{ shot, index }] });
      }
    });
    return out;
  });

  const day = (d: Date) => d.toLocaleDateString([], { weekday: 'short', day: 'numeric', month: 'short', year: d.getFullYear() === new Date().getFullYear() ? undefined : 'numeric' });
  const time = (d: Date) => d.toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });
  const size = (b: number) => (b >= 1e6 ? `${(b / 1e6).toFixed(1)} MB` : `${Math.max(1, Math.round(b / 1e3))} KB`);

  // ---- previews, made when a card scrolls into view ----
  let previews = $state<Record<string, string>>({});
  const asked = new Set<string>();
  function preview(node: HTMLElement, shot: Screenshot) {
    const io = new IntersectionObserver(
      (entries) => {
        if (!entries.some((e) => e.isIntersecting) || asked.has(shot.key)) return;
        asked.add(shot.key);
        io.disconnect();
        api
          .screenshotPreview(shot.instanceId, shot.file)
          .then((p) => (previews[shot.key] = src(p)))
          .catch(() => (previews[shot.key] = src(shot.path)));
      },
      { rootMargin: '300px' }
    );
    io.observe(node);
    return { destroy: () => io.disconnect() };
  }

  // ---- selection ----
  const selectedShots = $derived(visible.filter((s) => selected[s.key]));
  const selecting = $derived(selectedShots.length > 0);
  function toggle(s: Screenshot) {
    selected[s.key] = !selected[s.key];
  }
  const pair = (s: Screenshot): [string, string] => [s.instanceId, s.file];

  // ---- actions ----
  async function copy(s: Screenshot) {
    try {
      await api.screenshotCopy(s.instanceId, s.file);
      toast('Copied the picture', 'success');
    } catch (e) {
      toast(errorText(e), 'error');
    }
  }

  async function saveAs(s: Screenshot) {
    const ext = s.file.split('.').pop()?.toLowerCase() ?? 'png';
    const to = await api.pickSavePath(s.file, ext, 'Image');
    if (!to) return;
    try {
      await api.screenshotExport([pair(s)], to);
      toast('Saved', 'success');
    } catch (e) {
      toast(errorText(e), 'error');
    }
  }

  async function exportZip(list: Screenshot[]) {
    if (!list.length) return;
    const to = await api.pickSavePath('screenshots.zip', 'zip', 'Zip file');
    if (!to) return;
    busy = true;
    try {
      const n = await api.screenshotExport(list.map(pair), to);
      toast(`Saved ${n} screenshot${n === 1 ? '' : 's'} to a zip`, 'success');
      selected = {};
    } catch (e) {
      toast(errorText(e), 'error');
    } finally {
      busy = false;
    }
  }

  let confirmTrash = $state(false);
  async function trash(list: Screenshot[]) {
    if (!list.length) return;
    if (!confirmTrash) {
      confirmTrash = true;
      setTimeout(() => (confirmTrash = false), 4000);
      return;
    }
    confirmTrash = false;
    busy = true;
    try {
      const n = await api.screenshotTrash(list.map(pair));
      const gone = new Set(list.map((s) => s.key));
      shots = (shots ?? []).filter((s) => !gone.has(s.key));
      selected = {};
      if (open !== null && open >= visible.length) open = visible.length ? visible.length - 1 : null;
      toast(`Moved ${n} screenshot${n === 1 ? '' : 's'} to the ${app.info?.os === 'windows' ? 'Recycle Bin' : 'Trash'}`, 'success');
    } catch (e) {
      toast(errorText(e), 'error');
    } finally {
      busy = false;
    }
  }

  // ---- tags ----
  let tagText = $state('');
  async function setTags(s: Screenshot, tags: string[]) {
    try {
      s.tags = await api.screenshotSetTags(s.key, tags);
    } catch (e) {
      toast(errorText(e), 'error');
    }
  }
  function addTag(s: Screenshot) {
    const t = tagText.trim().toLowerCase();
    tagText = '';
    if (t && !s.tags.includes(t)) setTags(s, [...s.tags, t]);
  }

  // ---- viewer ----
  const current = $derived(open !== null ? visible[open] : null);
  function step(by: number) {
    if (open === null || !visible.length) return;
    open = (open + by + visible.length) % visible.length;
    tagText = '';
    confirmTrash = false;
  }
  function keys(e: KeyboardEvent) {
    if (open === null || (e.target as HTMLElement)?.tagName === 'INPUT') return;
    if (e.key === 'ArrowRight') step(1);
    else if (e.key === 'ArrowLeft') step(-1);
    else if (e.key === 'Escape') open = null;
  }
</script>

<svelte:window onkeydown={keys} />

<div class="page">
  <header>
    <div>
      <h1>Screenshots</h1>
      <p>Every screenshot from your instances, grouped by when you played.</p>
    </div>
    <div class="tools">
      <Select
        bind:value={() => instanceFilter, (v) => (instanceFilter = v ?? 'all')}
        ariaLabel="Instance"
        style="width: auto; min-width: 200px;"
        options={[
          { value: 'all', label: 'All instances', hint: String(shots?.length ?? 0) },
          ...app.instances.filter((i) => counts[i.id]).map((i) => ({ value: i.id, label: i.name, hint: String(counts[i.id]), icon: instanceIcon(i), loader: i.loader }))
        ]}
      />
      <button class="btn" onclick={load} title="Look for new screenshots"><RefreshCw size={15} /> Refresh</button>
    </div>
  </header>

  {#if allTags.length}
    <div class="tags-bar">
      <Tag size={14} />
      <button class="chip" class:active={!tagFilter} onclick={() => (tagFilter = null)}>All</button>
      {#each allTags as t (t)}
        <button class="chip" class:active={tagFilter === t} onclick={() => (tagFilter = tagFilter === t ? null : t)}>{t}</button>
      {/each}
    </div>
  {/if}

  {#if shots === null}
    <p class="muted">Looking for screenshots…</p>
  {:else if !visible.length}
    <div class="empty panel glass">
      <Camera size={28} />
      <h3>{shots.length ? 'Nothing matches' : 'No screenshots yet'}</h3>
      <p>{shots.length ? 'Try another instance or tag.' : 'Press F2 in game to take one. They show up here with their instance.'}</p>
    </div>
  {:else}
    {#each sessions as sess (sess.key)}
      <section class="session">
        <h2>
          <span>{day(sess.start)}</span>
          <span class="dot">·</span>
          <span class="inst">{nameOf(sess.instanceId)}</span>
          <span class="meta">{time(sess.start)} · {sess.items.length} shot{sess.items.length === 1 ? '' : 's'}</span>
        </h2>
        <div class="grid">
          {#each sess.items as { shot, index } (shot.key)}
            <div class="card" class:picked={selected[shot.key]} use:preview={shot}>
              <button class="open" onclick={() => (selecting ? toggle(shot) : (open = index))} aria-label="Open screenshot">
                {#if previews[shot.key]}
                  <img src={previews[shot.key]} alt="" loading="lazy" draggable="false" in:fade={{ duration: 150 }} />
                {/if}
              </button>
              <button class="tick" class:on={selected[shot.key]} onclick={() => toggle(shot)} aria-label="Select">
                <Check size={13} />
              </button>
              {#if shot.tags.length}
                <div class="card-tags">{#each shot.tags.slice(0, 3) as t (t)}<span>{t}</span>{/each}</div>
              {/if}
            </div>
          {/each}
        </div>
      </section>
    {/each}
  {/if}
</div>

{#if selecting}
  <div class="selbar glass" transition:fade={{ duration: 150 }}>
    <span>{selectedShots.length} selected</span>
    <button class="btn ghost" onclick={() => (selected = {})}>Clear</button>
    <button class="btn" disabled={busy} onclick={() => exportZip(selectedShots)}><Archive size={15} /> Save as zip</button>
    <button class="btn" class:danger={confirmTrash} disabled={busy} onclick={() => trash(selectedShots)}>
      <Trash2 size={15} />
      {confirmTrash ? 'Sure? Click again' : app.info?.os === 'windows' ? 'Recycle Bin' : 'Trash'}
    </button>
  </div>
{/if}

{#if current}
  <div class="viewer" transition:fade={{ duration: 160 }} role="dialog" aria-modal="true" aria-label="Screenshot">
    <button class="backdrop" onclick={() => (open = null)} aria-label="Close"></button>
    <button class="nav prev" onclick={() => step(-1)} aria-label="Previous"><ChevronLeft size={26} /></button>
    <figure>
      <img src={src(current.path)} alt={current.file} draggable="false" />
    </figure>
    <button class="nav next" onclick={() => step(1)} aria-label="Next"><ChevronRight size={26} /></button>
    <aside class="info glass">
      <button class="btn ghost icon close" onclick={() => (open = null)} aria-label="Close"><X size={18} /></button>
      <h3>{nameOf(current.instanceId)}</h3>
      <p class="muted">{day(new Date(current.takenAt))}, {time(new Date(current.takenAt))} · {size(current.sizeBytes)}</p>
      <p class="file mono">{current.file}</p>

      <div class="actions">
        <button class="btn" onclick={() => copy(current)}><Copy size={15} /> Copy picture</button>
        <button class="btn" onclick={() => saveAs(current)}><Download size={15} /> Save as…</button>
        <button class="btn" onclick={() => api.revealFile(current.path)}><FolderOpen size={15} /> Show in folder</button>
        <button class="btn" class:danger={confirmTrash} disabled={busy} onclick={() => trash([current])}>
          <Trash2 size={15} />
          {confirmTrash ? 'Sure? Click again' : `Move to ${app.info?.os === 'windows' ? 'Recycle Bin' : 'Trash'}`}
        </button>
      </div>

      <div class="tagger">
        <span class="tags-title">Tags</span>
        <div class="chips">
          {#each current.tags as t (t)}
            <span class="chip on">
              {t}
              <button onclick={() => setTags(current, current.tags.filter((x) => x !== t))} aria-label="Remove tag {t}"><X size={11} /></button>
            </span>
          {/each}
          <input
            class="input tag-input"
            placeholder="Add a tag…"
            maxlength="24"
            bind:value={tagText}
            onkeydown={(e) => e.key === 'Enter' && addTag(current)}
            list="screenshot-tags"
          />
          <datalist id="screenshot-tags">{#each allTags as t (t)}<option value={t}></option>{/each}</datalist>
        </div>
      </div>
      <p class="hint">← → to move between screenshots</p>
    </aside>
  </div>
{/if}

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: 18px;
    padding-bottom: 70px;
  }
  header {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 16px;
    flex-wrap: wrap;
  }
  h1 {
    font-size: 28px;
    font-weight: 720;
  }
  header p {
    margin: 4px 0 0;
    color: var(--muted);
    font-size: 13.5px;
  }
  .tools {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .tools {
    align-items: center;
  }
  .muted {
    color: var(--muted);
    font-size: 13px;
  }
  .tags-bar {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px;
    color: var(--muted);
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 26px;
    padding: 0 10px;
    border-radius: 999px;
    border: 1px solid var(--stroke);
    background: rgba(255, 255, 255, 0.03);
    color: var(--muted);
    font-size: 12px;
  }
  button.chip:hover {
    color: var(--text);
    border-color: var(--stroke-strong);
  }
  .chip.active,
  .chip.on {
    color: var(--accent);
    border-color: color-mix(in srgb, var(--accent) 50%, transparent);
    background: color-mix(in srgb, var(--accent) 10%, transparent);
  }
  .chip.on button {
    display: inline-flex;
    color: inherit;
    opacity: 0.7;
  }
  .chip.on button:hover {
    opacity: 1;
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 48px 20px;
    text-align: center;
    color: var(--muted);
  }
  .empty h3 {
    color: var(--text);
  }
  .empty p {
    margin: 0;
    font-size: 13px;
  }
  .session h2 {
    display: flex;
    align-items: baseline;
    gap: 8px;
    margin: 0 0 10px;
    font-size: 14px;
    font-weight: 650;
  }
  .session .dot {
    color: var(--faint, var(--muted));
  }
  .session .inst {
    color: var(--accent);
  }
  .session .meta {
    margin-left: auto;
    font-size: 12px;
    font-weight: 500;
    color: var(--muted);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: 10px;
  }
  .card {
    position: relative;
    aspect-ratio: 16 / 9;
    border-radius: var(--radius-sm);
    overflow: hidden;
    border: 1px solid var(--stroke);
    background: rgba(255, 255, 255, 0.03);
  }
  .card.picked {
    border-color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent);
  }
  .open {
    display: block;
    width: 100%;
    height: 100%;
    padding: 0;
    border: 0;
    background: none;
    cursor: zoom-in;
  }
  .open img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
    transition: transform 0.25s;
  }
  .card:hover .open img {
    transform: scale(1.03);
  }
  .tick {
    position: absolute;
    top: 7px;
    left: 7px;
    width: 22px;
    height: 22px;
    display: grid;
    place-items: center;
    border-radius: 6px;
    border: 1px solid rgba(255, 255, 255, 0.5);
    background: rgba(0, 0, 0, 0.45);
    color: transparent;
    opacity: 0;
    transition: opacity 0.15s;
  }
  .card:hover .tick,
  .tick.on {
    opacity: 1;
  }
  .tick.on {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }
  .card-tags {
    position: absolute;
    left: 7px;
    bottom: 7px;
    display: flex;
    gap: 4px;
    pointer-events: none;
  }
  .card-tags span {
    padding: 2px 7px;
    border-radius: 999px;
    background: rgba(0, 0, 0, 0.55);
    font-size: 11px;
    color: #fff;
  }
  .selbar {
    position: fixed;
    left: 50%;
    bottom: 22px;
    transform: translateX(-50%);
    z-index: 30;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px 10px 18px;
    border-radius: var(--radius);
    font-size: 13px;
  }
  .danger {
    color: var(--danger, #ff6464);
  }

  /* Viewer */
  .viewer {
    position: fixed;
    inset: 0;
    z-index: 60;
    display: grid;
    grid-template-columns: 56px 1fr 56px 320px;
    align-items: center;
  }
  .backdrop {
    position: absolute;
    inset: 0;
    border: 0;
    background: rgba(4, 6, 10, 0.86);
    cursor: default;
  }
  figure {
    position: relative;
    margin: 0;
    height: 100%;
    display: grid;
    place-items: center;
    padding: 30px 0;
    pointer-events: none;
  }
  figure img {
    max-width: 100%;
    max-height: 100%;
    border-radius: 8px;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.6);
  }
  .nav {
    position: relative;
    justify-self: center;
    width: 42px;
    height: 42px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    border: 1px solid var(--stroke);
    background: rgba(255, 255, 255, 0.06);
    color: var(--text);
  }
  .nav:hover {
    background: rgba(255, 255, 255, 0.12);
  }
  .info {
    position: relative;
    align-self: stretch;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 22px 20px;
    border-radius: 0;
    border-left: 1px solid var(--stroke);
    overflow-y: auto;
  }
  .info h3 {
    font-size: 16px;
    padding-right: 30px;
  }
  .info .muted {
    margin: 0;
  }
  .close {
    position: absolute;
    top: 14px;
    right: 12px;
  }
  .file {
    margin: 0;
    font-size: 11.5px;
    color: var(--faint, var(--muted));
    word-break: break-all;
  }
  .actions {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-top: 8px;
  }
  .actions .btn {
    justify-content: flex-start;
  }
  .tagger {
    margin-top: 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .tags-title {
    font-size: 12px;
    font-weight: 650;
    color: var(--muted);
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .tag-input {
    flex: 1;
    min-width: 120px;
    height: 28px;
    font-size: 12.5px;
  }
  .hint {
    margin: auto 0 0;
    font-size: 11.5px;
    color: var(--faint, var(--muted));
  }
  @media (max-width: 900px) {
    .viewer {
      grid-template-columns: 44px 1fr 44px;
      grid-template-rows: 1fr auto;
    }
    .info {
      grid-column: 1 / -1;
      max-height: 40vh;
      border-left: 0;
      border-top: 1px solid var(--stroke);
    }
  }
</style>
