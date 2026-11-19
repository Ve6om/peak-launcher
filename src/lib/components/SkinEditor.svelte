<script lang="ts">
  // Change an account's skin, arm style and cape through Minecraft's own
  // profile API. Changes show on the 3D model first; Apply sends them.
  import { onMount } from 'svelte';
  import { Upload, LoaderCircle, RotateCcw, Check, X, History } from '@lucide/svelte';
  import Modal from './Modal.svelte';
  import SkinViewer from './SkinViewer.svelte';
  import SkinHead from './SkinHead.svelte';
  import { api } from '$lib/api';
  import { toast, errorText } from '$lib/state.svelte';
  import { skins } from '$lib/skins.svelte';
  import type { AccountInfo, PastSkin, Wardrobe } from '$lib/types';

  let { account, onclose }: { account: AccountInfo; onclose: () => void } = $props();

  let wardrobe = $state<Wardrobe | null>(null);
  let loading = $state(true);
  let saving = $state(false);
  let error = $state<string | null>(null);

  // What's being edited.
  let filePath = $state<string | null>(null);
  let filePreview = $state<string | null>(null);
  let model = $state<'classic' | 'slim'>('classic');
  let capeId = $state<string | null>(null);
  let confirmReset = $state(false);
  // Skins this account wore before (Peak keeps the history; Mojang doesn't).
  let history = $state<PastSkin[]>([]);
  let savedId = $state<string | null>(null);

  const currentCape = $derived(wardrobe?.capes.find((c) => c.active)?.id ?? null);
  const preview = $derived(filePreview ?? wardrobe?.skin?.png ?? null);
  const previewCape = $derived(wardrobe?.capes.find((c) => c.id === capeId)?.png ?? null);
  const changed = $derived(!!filePath || !!savedId || (wardrobe?.skin && model !== wardrobe.skin.model) || capeId !== currentCape);

  function take(w: Wardrobe) {
    wardrobe = w;
    model = w.skin?.model ?? 'classic';
    capeId = w.capes.find((c) => c.active)?.id ?? null;
    filePath = null;
    filePreview = null;
    savedId = null;
    // Heads and the preview elsewhere update straight away.
    skins[account.id] = w.skin;
  }

  async function loadHistory() {
    try {
      history = await api.skinHistory(account.id);
    } catch {
      history = [];
    }
  }

  function pickSaved(h: PastSkin) {
    if (h.png === wardrobe?.skin?.png) {
      // The one in use: back to no pending change.
      savedId = null;
      filePath = null;
      filePreview = null;
      model = wardrobe.skin.model;
      return;
    }
    savedId = h.id;
    filePath = null;
    filePreview = h.png;
    model = h.model;
  }

  async function forget(h: PastSkin) {
    try {
      await api.skinHistoryRemove(account.id, h.id);
      history = history.filter((x) => x.id !== h.id);
      if (savedId === h.id) pickSaved({ ...h, png: wardrobe?.skin?.png ?? '' });
    } catch (e) {
      toast(errorText(e), 'error');
    }
  }

  function when(iso: string): string {
    const days = Math.floor((Date.now() - new Date(iso).getTime()) / 86400000);
    if (days < 1) return 'Today';
    if (days === 1) return 'Yesterday';
    if (days < 30) return `${days} days ago`;
    return new Date(iso).toLocaleDateString();
  }

  onMount(async () => {
    try {
      take(await api.skinWardrobe(account.id));
      loadHistory();
    } catch (e) {
      error = errorText(e);
    } finally {
      loading = false;
    }
  });

  async function choose() {
    const path = await api.pickSkinFile();
    if (!path) return;
    try {
      filePreview = await api.skinReadFile(path);
      filePath = path;
      savedId = null;
    } catch (e) {
      toast(errorText(e), 'error');
    }
  }

  async function apply() {
    if (!wardrobe) return;
    saving = true;
    try {
      let w = wardrobe;
      if (filePath) w = await api.skinUpload(account.id, filePath, model);
      else if (savedId) w = await api.skinUseSaved(account.id, savedId, model);
      else if (w.skin && model !== w.skin.model) w = await api.skinSetModel(account.id, model);
      if (capeId !== (w.capes.find((c) => c.active)?.id ?? null)) w = await api.skinSetCape(account.id, capeId);
      take(w);
      loadHistory();
      toast('Skin updated. It shows in game the next time you join a world.', 'success');
      onclose();
    } catch (e) {
      toast(errorText(e), 'error');
    } finally {
      saving = false;
    }
  }

  async function reset() {
    if (!confirmReset) {
      confirmReset = true;
      setTimeout(() => (confirmReset = false), 3000);
      return;
    }
    saving = true;
    try {
      take(await api.skinReset(account.id));
      loadHistory();
      toast("Back to Minecraft's default skin", 'success');
    } catch (e) {
      toast(errorText(e), 'error');
    } finally {
      saving = false;
      confirmReset = false;
    }
  }
</script>

<Modal title="Change skin" subtitle="For {account.username}. Changes show here first; Apply sends them to Minecraft." width={640} onclose={() => !saving && onclose()}>
  {#if loading}
    <div class="loading"><LoaderCircle size={20} class="spin" /> Loading your skin…</div>
  {:else if error}
    <p class="error">{error}</p>
  {:else if wardrobe}
    <div class="layout">
      <div class="stage">
        {#if preview}
          <SkinViewer png={preview} {model} cape={previewCape} width={220} height={290} />
        {:else}
          <p class="muted">This account uses Minecraft's default skin. Choose a file to upload your own.</p>
        {/if}
      </div>

      <div class="controls">
        <div class="field">
          <span class="label">Skin</span>
          <button class="btn" onclick={choose}><Upload size={15} /> {filePath ? 'Choose a different file…' : 'Choose a skin file…'}</button>
          <span class="hint">{filePath ? filePath.split(/[\\/]/).pop() : 'A 64x64 PNG, like the ones from skin sites or your own.'}</span>
        </div>

        <div class="field">
          <span class="label">Arms</span>
          <div class="seg">
            <button class:active={model === 'classic'} onclick={() => (model = 'classic')} disabled={!preview}>Classic (Steve)</button>
            <button class:active={model === 'slim'} onclick={() => (model = 'slim')} disabled={!preview}>Slim (Alex)</button>
          </div>
        </div>

        {#if wardrobe.capes.length}
          <div class="field">
            <span class="label">Cape</span>
            <div class="capes">
              <button class="cape" class:active={capeId === null} onclick={() => (capeId = null)}>
                <span class="none">None</span>
              </button>
              {#each wardrobe.capes as c (c.id)}
                <button class="cape" class:active={capeId === c.id} onclick={() => (capeId = c.id)} title={c.name}>
                  {#if c.png}<span class="cape-art" style:background-image="url({c.png})"></span>{/if}
                  <span class="cape-name">{c.name}</span>
                </button>
              {/each}
            </div>
          </div>
        {/if}

        {#if wardrobe.skin}
          <button class="btn ghost reset" class:confirm={confirmReset} onclick={reset} disabled={saving}>
            <RotateCcw size={14} /> {confirmReset ? 'Click again to reset' : 'Reset to default skin'}
          </button>
        {/if}
      </div>
    </div>

    {#if history.length}
      <div class="history">
        <span class="label"><History size={13} /> Your recent skins</span>
        <div class="past">
          {#each history as h (h.id)}
            {@const inUse = h.png === wardrobe.skin?.png}
            <div class="past-item" class:selected={savedId === h.id || (inUse && !savedId && !filePath)}>
              <button class="past-pick" onclick={() => pickSaved(h)} title={inUse ? 'In use now' : `Worn ${when(h.usedAt).toLowerCase()}. Click to preview, then Apply.`}>
                <SkinHead accountId={null} name={account.username} png={h.png} size={40} />
                <span>{inUse ? 'In use' : when(h.usedAt)}</span>
              </button>
              {#if !inUse}
                <button class="past-forget" aria-label="Forget this skin" title="Forget this skin" onclick={() => forget(h)}><X size={11} /></button>
              {/if}
            </div>
          {/each}
        </div>
      </div>
    {/if}

    <footer>
      <button class="btn ghost" onclick={onclose} disabled={saving}>Cancel</button>
      <button class="btn primary" onclick={apply} disabled={!changed || saving}>
        {#if saving}<LoaderCircle size={16} class="spin" /> Saving…{:else}<Check size={16} /> Apply{/if}
      </button>
    </footer>
  {/if}
</Modal>

<style>
  .loading {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 30px 4px;
    color: var(--muted);
  }
  .error {
    color: #ff9a9d;
    font-size: 13px;
  }
  .layout {
    display: flex;
    gap: 22px;
    align-items: stretch;
  }
  .stage {
    width: 220px;
    height: 290px;
    flex-shrink: 0;
    display: grid;
    place-items: center;
    border-radius: var(--radius);
    background: radial-gradient(circle at 50% 45%, color-mix(in srgb, var(--accent) 14%, transparent), transparent 65%), rgba(0, 0, 0, 0.2);
  }
  .muted {
    margin: 0;
    padding: 0 18px;
    text-align: center;
    font-size: 12.5px;
    color: var(--muted);
  }
  .controls {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 7px;
    align-items: flex-start;
  }
  .hint {
    font-size: 12px;
    color: var(--faint);
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .capes {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .cape {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    width: 64px;
    padding: 6px 4px;
    border-radius: 10px;
    border: 1px solid var(--stroke);
    background: rgba(255, 255, 255, 0.03);
    color: var(--muted);
    font: inherit;
    font-size: 11px;
    cursor: pointer;
  }
  .cape.active {
    border-color: color-mix(in srgb, var(--accent) 60%, transparent);
    color: var(--text);
    background: color-mix(in srgb, var(--accent) 10%, transparent);
  }
  .cape-art {
    width: 30px;
    height: 48px;
    /* The cape's back panel sits at x 1-10, y 1-16 of its 64x32 texture. */
    background-size: 640% 300%;
    background-position: 1.85% 3.2%;
    image-rendering: pixelated;
  }
  .none {
    height: 48px;
    display: grid;
    place-items: center;
  }
  .cape-name {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .reset {
    align-self: flex-start;
    margin-top: auto;
    color: var(--muted);
  }
  .reset.confirm {
    color: #fecaca;
    background: rgba(239, 68, 68, 0.18);
  }
  .history {
    margin-top: 18px;
    padding-top: 14px;
    border-top: 1px solid var(--stroke);
  }
  .history .label {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .past {
    display: flex;
    gap: 8px;
    overflow-x: auto;
    padding: 8px 2px 4px;
  }
  .past-item {
    position: relative;
    flex-shrink: 0;
  }
  .past-pick {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 5px;
    width: 72px;
    padding: 8px 4px 6px;
    border-radius: 10px;
    border: 1px solid var(--stroke);
    background: rgba(255, 255, 255, 0.03);
    color: var(--muted);
    font: inherit;
    font-size: 11px;
    cursor: pointer;
  }
  .past-pick span {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .past-item.selected .past-pick {
    border-color: color-mix(in srgb, var(--accent) 60%, transparent);
    color: var(--text);
    background: color-mix(in srgb, var(--accent) 10%, transparent);
  }
  .past-forget {
    position: absolute;
    top: -5px;
    right: -5px;
    display: none;
    place-items: center;
    width: 18px;
    height: 18px;
    padding: 0;
    border-radius: 50%;
    border: 1px solid var(--stroke);
    background: var(--surface-2);
    color: var(--muted);
    cursor: pointer;
  }
  .past-item:hover .past-forget,
  .past-forget:focus-visible {
    display: grid;
  }
  .past-forget:hover {
    color: #fecaca;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 18px;
  }
</style>
