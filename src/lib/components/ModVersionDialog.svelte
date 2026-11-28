<script lang="ts">
  import { t } from '$lib/i18n.svelte';
  // Pick a different version of something already installed: newer, or an
  // older one if the latest breaks the pack. Only versions made for this
  // instance's Minecraft version and loader are listed.
  import { onMount } from 'svelte';
  import { Check, LoaderCircle, ArrowDownUp } from '@lucide/svelte';
  import Modal from './Modal.svelte';
  import { api } from '$lib/api';
  import { ago, toast, errorText } from '$lib/state.svelte';
  import type { ContentKind, ContentVersion, InstalledItem } from '$lib/types';

  let {
    instanceId,
    kind,
    item,
    onpick,
    onclose
  }: {
    instanceId: string;
    kind: ContentKind;
    item: InstalledItem;
    /** Called with the chosen version; the caller installs it. */
    onpick: (versionId: string) => void;
    onclose: () => void;
  } = $props();

  let versions = $state<ContentVersion[] | null>(null);
  let pick = $state('');
  let showAll = $state(false);

  onMount(async () => {
    try {
      versions = await api.contentVersions(instanceId, kind, item.projectId!);
      pick = item.versionId ?? '';
    } catch (e) {
      toast(errorText(e), 'error');
      onclose();
    }
  });

  // Betas and alphas are hidden unless asked for, or one is installed.
  const hasTesting = $derived(!!versions?.some((v) => v.version_type !== 'release'));
  const shown = $derived(
    (versions ?? []).filter((v) => showAll || v.version_type === 'release' || v.id === item.versionId)
  );
  const current = $derived(versions?.find((v) => v.id === item.versionId));
  const newestIndex = $derived(shown.findIndex((v) => v.version_type === 'release'));
</script>

<Modal title={t('Change version')} subtitle={t('{title} · versions for this instance\'s Minecraft and loader', { title: item.title })} width={560} {onclose}>
  {#if versions === null}
    <div class="loading"><LoaderCircle size={20} class="spin" /></div>
  {:else if !versions.length}
    <p class="none">{t('No versions of {title} match this instance\'s Minecraft version and loader.', { title: item.title })}</p>
  {:else}
    {#if hasTesting}
      <label class="testing"><input type="checkbox" bind:checked={showAll} /> {t('Show beta and alpha versions')}</label>
    {/if}
    <div class="list" role="radiogroup" aria-label={t('Versions of {title}', { title: item.title })}>
      {#each shown as v, i (v.id)}
        <button class="ver" class:on={pick === v.id} role="radio" aria-checked={pick === v.id} onclick={() => (pick = v.id)}>
          <span class="dot">{#if pick === v.id}<Check size={12} />{/if}</span>
          <span class="name">
            <b>{v.version_number}</b>
            {#if v.version_type !== 'release'}<em class="tag {v.version_type}">{v.version_type}</em>{/if}
            {#if v.id === item.versionId}<em class="tag now">{t('Installed')}</em>{:else if i === newestIndex}<em class="tag new">{t('Newest')}</em>{/if}
          </span>
          <span class="when">{ago(v.date_published)}</span>
        </button>
      {/each}
    </div>
    {#if !current && item.versionId}
      <p class="hint">{t('The installed version ({x}) isn\'t listed for this instance any more.', { x: item.versionNumber ?? 'unknown' })}</p>
    {/if}
    <div class="actions">
      <button class="btn" onclick={onclose}>{t('Cancel')}</button>
      <button class="btn primary" disabled={!pick || pick === item.versionId} onclick={() => onpick(pick)}>
        <ArrowDownUp size={15} /> {t('Switch to this version')}
      </button>
    </div>
  {/if}
</Modal>

<style>
  .loading {
    display: flex;
    justify-content: center;
    padding: 30px;
  }
  .none,
  .hint {
    margin: 0;
    font-size: 13px;
    color: var(--muted);
  }
  .hint {
    margin-top: 10px;
    font-size: 12px;
  }
  .testing {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 10px;
    font-size: 12.5px;
    color: var(--muted);
    cursor: pointer;
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-height: 340px;
    overflow-y: auto;
    padding: 4px;
    border-radius: 12px;
    border: 1px solid var(--stroke);
    background: rgba(0, 0, 0, 0.18);
  }
  .ver {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 8px 10px;
    border: 1px solid transparent;
    border-radius: 9px;
    background: none;
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .ver:hover {
    background: rgba(255, 255, 255, 0.04);
  }
  .ver.on {
    border-color: color-mix(in srgb, var(--accent) 45%, transparent);
    background: color-mix(in srgb, var(--accent) 10%, transparent);
  }
  .dot {
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    flex-shrink: 0;
    border-radius: 50%;
    border: 1.5px solid var(--stroke);
  }
  .ver.on .dot {
    border-color: var(--accent);
    background: var(--accent);
    color: var(--on-accent, #0b0d12);
  }
  .name {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    overflow: hidden;
  }
  .name b {
    font-size: 13px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .tag {
    flex-shrink: 0;
    padding: 1px 6px;
    border-radius: 6px;
    font-size: 10px;
    font-style: normal;
    font-weight: 650;
    text-transform: capitalize;
    color: var(--muted);
    background: rgba(255, 255, 255, 0.07);
  }
  .tag.beta,
  .tag.alpha {
    color: #fde68a;
    background: rgba(250, 204, 21, 0.12);
  }
  .tag.now {
    color: #a7f3d0;
    background: rgba(16, 185, 129, 0.14);
  }
  .tag.new {
    color: #a5f3fc;
    background: color-mix(in srgb, var(--accent-2) 14%, transparent);
  }
  .when {
    flex-shrink: 0;
    font-size: 11.5px;
    color: var(--faint);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 14px;
  }
</style>
