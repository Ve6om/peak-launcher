<script lang="ts">
  import { t } from '$lib/i18n.svelte';
  import { onMount } from 'svelte';
  import Select from './Select.svelte';
  import { Search, LoaderCircle, Check } from '@lucide/svelte';
  import Modal from './Modal.svelte';
  import LoaderIcon from './LoaderIcon.svelte';
  import { loaderTint } from '$lib/loaders';
  import { scale } from 'svelte/transition';
  import { api } from '$lib/api';
  import { app, go, refreshInstances, toast, errorText } from '$lib/state.svelte';
  import { LOADERS, loaderLabel, type LoaderId, type LoaderVersion, type ManifestEntry } from '$lib/types';

  let versions = $state<ManifestEntry[]>([]);
  let latest = $state('');
  let loading = $state(true);
  let filter = $state<'release' | 'snapshot' | 'old'>('release');
  let query = $state('');
  let version = $state('');
  let name = $state('');
  let nameTouched = $state(false);
  let saving = $state(false);

  let loader = $state<LoaderId>('vanilla');
  let loaderVersions = $state<LoaderVersion[]>([]);
  let loaderVersion = $state<string | null>(null);
  let loadingLoader = $state(false);
  let showUnstable = $state(false);

  // Fetch loader versions whenever the loader or game version changes.
  let req = 0;
  $effect(() => {
    const l = loader;
    const v = version;
    loaderVersions = [];
    loaderVersion = null;
    if (l === 'vanilla' || !v) return;
    const my = ++req;
    loadingLoader = true;
    api
      .loaderVersions(l, v)
      .then((list) => {
        if (my !== req) return;
        loaderVersions = list;
        loaderVersion = (list.find((x) => x.recommended) ?? list.find((x) => x.stable) ?? list[0])?.id ?? null;
      })
      .catch((e) => my === req && toast(errorText(e), 'error'))
      .finally(() => my === req && (loadingLoader = false));
  });
  const shownLoaderVersions = $derived(
    loaderVersions.filter((v) => showUnstable || v.stable || v.id === loaderVersion)
  );

  onMount(async () => {
    try {
      const m = await api.versions();
      versions = m.versions;
      latest = m.latest.release;
      version = m.latest.release;
    } catch (e) {
      toast(`Couldn't load versions: ${errorText(e)}`, 'error');
    } finally {
      loading = false;
    }
  });

  const shown = $derived(
    versions.filter(
      (v) =>
        (filter === 'old' ? v.type.startsWith('old') : v.type === filter) &&
        v.id.toLowerCase().includes(query.trim().toLowerCase())
    )
  );

  $effect(() => {
    if (!nameTouched) name = version ? (loader === 'vanilla' ? t('Minecraft {version}', { version }) : `${loaderLabel(loader)} ${version}`) : '';
  });

  async function create() {
    if (!name.trim() || !version) return;
    saving = true;
    try {
      const inst = await api.createInstance(name.trim(), version, loader, loader === 'vanilla' ? null : loaderVersion);
      await refreshInstances();
      app.creating = false;
      go('instance', inst.id);
      toast(t('Created {name}', { name: inst.name }), 'success');
      // Start downloading right away so Play is instant later.
      api.install(inst.id).catch((e) => toast(errorText(e), 'error'));
    } catch (e) {
      toast(errorText(e), 'error');
    } finally {
      saving = false;
    }
  }

</script>

<Modal title={t('New instance')} subtitle={t('Pick a version. Everything else is handled for you.')} width={640} onclose={() => (app.creating = false)}>
  <div class="layout">
    <div class="preview">
      <div class="art" style:--tint={loaderTint(loader)}>
        {#key loader}
          <div class="mark" in:scale={{ start: 0.8, duration: 200 }}>
            <LoaderIcon {loader} size={88} />
          </div>
        {/key}
      </div>
      <label class="field">
        <span class="label">{t('Name')}</span>
        <input class="input" bind:value={name} oninput={() => (nameTouched = true)} placeholder={t('My world')} maxlength="48" />
      </label>
      <div class="field">
        <span class="label">{t('Mod loader')}</span>
        <div class="loaders">
          {#each LOADERS as l (l.id)}
            <button class="loader" class:active={l.id === loader} onclick={() => (loader = l.id)}>
              <LoaderIcon loader={l.id} size={16} />
              {l.label}
            </button>
          {/each}
        </div>
      </div>
      {#if loader !== 'vanilla'}
        <div class="field">
          <span class="label">{t('{loaderLabel} version', { loaderLabel: loaderLabel(loader) })}</span>
          {#if loadingLoader}
            <div class="loader-status"><LoaderCircle size={14} class="spin" /> {t('Loading versions…')}</div>
          {:else if !loaderVersions.length}
            <div class="loader-status warn">{t('Not available for Minecraft {version}', { version })}</div>
          {:else}
            <Select
              bind:value={loaderVersion}
              ariaLabel={t('Loader version')}
              options={shownLoaderVersions.map((v) => ({ value: v.id, label: v.id, hint: v.recommended ? 'recommended' : v.stable ? undefined : 'beta' }))}
            />
            {#if loaderVersions.some((v) => !v.stable)}
              <label class="unstable"><input type="checkbox" bind:checked={showUnstable} /> {t('Show beta versions')}</label>
            {/if}
          {/if}
        </div>
      {/if}
    </div>

    <div class="picker">
      <div class="tabs">
        <button class:active={filter === 'release'} onclick={() => (filter = 'release')}>{t('Releases')}</button>
        <button class:active={filter === 'snapshot'} onclick={() => (filter = 'snapshot')}>{t('Snapshots')}</button>
        <button class:active={filter === 'old'} onclick={() => (filter = 'old')}>{t('Classic')}</button>
      </div>
      <div class="search">
        <Search size={15} />
        <input bind:value={query} placeholder={t('Search versions')} />
      </div>
      <div class="list">
        {#if loading}
          <div class="center"><LoaderCircle size={20} class="spin" /></div>
        {:else}
          {#each shown as v (v.id)}
            <button class="ver" class:active={version === v.id} onclick={() => (version = v.id)}>
              <span>{v.id}</span>
              {#if v.id === latest}<span class="chip">{t('Latest')}</span>{/if}
              {#if version === v.id}<Check size={15} />{/if}
            </button>
          {:else}
            <div class="center muted">{t('No versions match')}</div>
          {/each}
        {/if}
      </div>
    </div>
  </div>

  <footer>
    <button class="btn ghost" onclick={() => (app.creating = false)}>{t('Cancel')}</button>
    <button class="btn primary" disabled={!version || !name.trim() || saving || (loader !== 'vanilla' && !loaderVersion)} onclick={create}>
      {#if saving}<LoaderCircle size={16} class="spin" />{/if} {t('Create instance')}
    </button>
  </footer>
</Modal>

<style>
  .layout {
    display: grid;
    grid-template-columns: 240px 1fr;
    gap: 20px;
    min-height: 0;
  }
  .preview {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .art {
    position: relative;
    aspect-ratio: 16 / 10;
    border-radius: 14px;
    overflow: hidden;
    box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.1) inset;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .loaders {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .art {
    display: grid;
    place-items: center;
    /* A soft glow in the loader's own colour behind its logo. */
    background:
      radial-gradient(circle at 50% 50%, color-mix(in srgb, var(--tint) 34%, transparent), transparent 62%),
      linear-gradient(160deg, color-mix(in srgb, var(--tint) 14%, #101216), #0b0c0f);
    transition: background 0.3s;
  }
  .mark {
    grid-area: 1 / 1;
    filter: drop-shadow(0 8px 22px rgba(0, 0, 0, 0.45));
  }
  .mark :global(img) {
    border-radius: 14px;
  }
  .loader {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 11px 0 7px;
    border-radius: 99px;
    font-size: 12.5px;
    font-weight: 550;
    border: 1px solid var(--stroke);
    color: var(--muted);
  }
  .loader.active {
    color: var(--on-accent);
    border-color: transparent;
    background: var(--gradient);
  }
  .loader:not(.active):hover {
    color: var(--text);
    border-color: var(--stroke-strong);
  }
  .loader-status {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 40px;
    font-size: 12.5px;
    color: var(--muted);
  }
  .loader-status.warn {
    color: #fcd34d;
  }
  .unstable {
    display: flex;
    align-items: center;
    gap: 7px;
    font-size: 12px;
    color: var(--muted);
  }
  .unstable input {
    accent-color: var(--accent);
  }
  .picker {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-height: 0;
  }
  .tabs {
    display: flex;
    gap: 4px;
    padding: 4px;
    border-radius: 12px;
    background: rgba(0, 0, 0, 0.25);
    border: 1px solid var(--stroke);
  }
  .tabs button {
    flex: 1;
    height: 30px;
    border-radius: 9px;
    font-size: 12.5px;
    font-weight: 560;
    color: var(--muted);
    transition: background 0.2s, color 0.2s;
  }
  .tabs button.active {
    background: rgba(255, 255, 255, 0.09);
    color: var(--text);
  }
  .search {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 36px;
    padding: 0 12px;
    border-radius: 11px;
    background: rgba(0, 0, 0, 0.2);
    border: 1px solid var(--stroke);
    color: var(--faint);
  }
  .search input {
    flex: 1;
    background: none;
    border: 0;
    outline: none;
    font-size: 13px;
  }
  .list {
    height: 280px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding-right: 4px;
  }
  .ver {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 36px;
    padding: 0 12px;
    border-radius: 10px;
    font-size: 13.5px;
    font-variant-numeric: tabular-nums;
    text-align: left;
    color: var(--muted);
  }
  .ver span:first-child {
    flex: 1;
  }
  .ver:hover {
    background: rgba(255, 255, 255, 0.05);
    color: var(--text);
  }
  .ver.active {
    color: var(--text);
    background: color-mix(in srgb, var(--accent) 18%, transparent);
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--accent) 35%, transparent) inset;
  }
  .ver.active :global(svg) {
    color: var(--accent-2);
  }
  .center {
    display: grid;
    place-items: center;
    height: 100%;
    color: var(--muted);
  }
  .muted {
    font-size: 13px;
    color: var(--faint);
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 20px;
  }
</style>
