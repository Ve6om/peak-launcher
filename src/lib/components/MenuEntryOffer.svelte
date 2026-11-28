<script lang="ts">
  import { t } from '$lib/i18n.svelte';
  // Shown once when Peak runs as a Linux AppImage without a menu entry (or
  // after the AppImage was moved): offer to add Peak to the applications menu.
  import { onMount } from 'svelte';
  import { fly } from 'svelte/transition';
  import { LayoutGrid, LoaderCircle, X } from '@lucide/svelte';
  import { api } from '$lib/api';
  import { app, toast, errorText } from '$lib/state.svelte';
  import { menu, loadMenuStatus, addToMenu } from '$lib/menuEntry.svelte';

  let hidden = $state(false);

  onMount(loadMenuStatus);

  const show = $derived(!hidden && !!app.settings && !app.settings.appimageMenuDismissed && !!menu.status?.isAppImage && !menu.status.upToDate);
  const moved = $derived(!!menu.status?.installed && !menu.status.upToDate);

  async function add() {
    try {
      await addToMenu();
      toast(t('Peak is in your applications menu. You can pin it from there.'), 'success');
      hidden = true;
    } catch (e) {
      toast(errorText(e), 'error');
    }
  }

  async function dontAsk() {
    hidden = true;
    if (!app.settings) return;
    const next = { ...$state.snapshot(app.settings), appimageMenuDismissed: true };
    try {
      app.settings = await api.saveSettings(next);
    } catch {
      // Not saved; it'll just ask again next time.
    }
  }
</script>

{#if show}
  <div class="offer glass" transition:fly={{ y: 16, duration: 220 }} role="dialog" aria-label={t('Add Peak to your applications menu')}>
    <span class="icon"><LayoutGrid size={18} /></span>
    <div class="text">
      <b>{moved ? t('Peak was moved') : t('Add Peak to your applications menu?')}</b>
      <span>
        {moved
          ? t('Your menu entry still points at the old location. Update it so it opens this copy.')
          : t('Open Peak like any other app, without finding the AppImage file. You can remove it again in Settings.')}
      </span>
    </div>
    <div class="actions">
      <button class="btn ghost" onclick={dontAsk}>{t('Don\'t ask again')}</button>
      <button class="btn primary" onclick={add} disabled={menu.busy}>
        {#if menu.busy}<LoaderCircle size={15} class="spin" />{/if}
        {moved ? t('Update entry') : t('Add to menu')}
      </button>
    </div>
    <button class="close btn ghost icon" aria-label={t('Not now')} title={t('Not now')} onclick={() => (hidden = true)}><X size={15} /></button>
  </div>
{/if}

<style>
  .offer {
    position: fixed;
    right: 22px;
    bottom: 22px;
    z-index: 120;
    width: min(460px, calc(100vw - 44px));
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 10px 12px;
    padding: 14px 16px;
    box-shadow: 0 14px 40px rgba(0, 0, 0, 0.5);
  }
  .icon {
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    border-radius: 10px;
    background: color-mix(in srgb, var(--accent) 16%, transparent);
    color: var(--accent);
  }
  .text {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding-right: 22px;
  }
  .text b {
    font-size: 14px;
  }
  .text span {
    font-size: 12.5px;
    line-height: 1.45;
    color: var(--muted);
  }
  .actions {
    grid-column: 1 / -1;
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .close {
    position: absolute;
    top: 8px;
    right: 8px;
  }
</style>
