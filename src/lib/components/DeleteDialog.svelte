<script lang="ts">
  import { t } from '$lib/i18n.svelte';
  import { Trash2 } from '@lucide/svelte';
  import Modal from './Modal.svelte';
  import { app, deleteInstance } from '$lib/state.svelte';

  let busy = $state(false);
  async function confirm() {
    if (!app.deleting) return;
    busy = true;
    await deleteInstance(app.deleting);
    busy = false;
  }
</script>

{#if app.deleting}
  <Modal
    title={t('Delete {name}?', { name: app.deleting.name })}
    subtitle={t('Worlds, mods and settings in this instance will be permanently removed.')}
    width={440}
    onclose={() => (app.deleting = null)}
  >
    <div class="actions">
      <button class="btn ghost" onclick={() => (app.deleting = null)}>{t('Cancel')}</button>
      <button class="btn danger" disabled={busy} onclick={confirm}><Trash2 size={16} /> {t('Delete forever')}</button>
    </div>
  </Modal>
{/if}

<style>
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
