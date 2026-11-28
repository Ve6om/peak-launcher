<script lang="ts">
  import { t } from '$lib/i18n.svelte';
  // Asks what to do with mods that depend on (or are needed by) the one the
  // user is disabling, removing or enabling.
  import Modal from './Modal.svelte';
  import type { LinkedMod } from '$lib/types';

  let {
    action,
    name,
    links,
    onchoose
  }: {
    action: 'disable' | 'remove' | 'enable';
    name: string;
    links: LinkedMod[];
    onchoose: (choice: 'all' | 'only' | null) => void;
  } = $props();

  const n = $derived(links.length);
  const text = $derived(
    {
      disable: {
        title: t('Other mods need {name}', { name }),
        intro: n === 1 ? t("1 mod won't load without it. Disable it too?") : t("{n} mods won't load without it. Disable them too?", { n }),
        all: t('Disable all {x}', { x: n + 1 }),
        only: t('Only disable {name}', { name })
      },
      remove: {
        title: t('Other mods need {name}', { name }),
        intro: n === 1 ? t("1 mod won't load without it. Remove it too?") : t("{n} mods won't load without it. Remove them too?", { n }),
        all: t('Remove all {x}', { x: n + 1 }),
        only: t('Only remove {name}', { name })
      },
      enable: {
        title: t('{name} needs disabled mods', { name }),
        intro: n === 1 ? t("It won't load without this mod. Turn it back on too?") : t("It won't load without these {n} mods. Turn them back on too?", { n }),
        all: t('Enable all {x}', { x: n + 1 }),
        only: t('Only enable {name}', { name })
      }
    }[action]
  );
</script>

<Modal title={text.title} width={480} onclose={() => onchoose(null)}>
  <p class="intro">{text.intro}</p>
  <ul>
    {#each links as l (l.fileName)}
      <li><b>{l.name}</b><span>{l.reason}</span></li>
    {/each}
  </ul>
  {#if action !== 'enable'}
    <p class="note">{t('If you keep them, the game will probably refuse to start until {name} is back.', { name })}</p>
  {/if}
  <footer>
    <button class="btn ghost" onclick={() => onchoose(null)}>{t('Cancel')}</button>
    <button class="btn" onclick={() => onchoose('only')}>{text.only}</button>
    <button class="btn primary" class:danger={action === 'remove'} onclick={() => onchoose('all')}>{text.all}</button>
  </footer>
</Modal>

<style>
  .intro {
    margin: 0 0 12px;
    color: var(--muted);
    font-size: 13.5px;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 4px;
    max-height: 260px;
    overflow-y: auto;
    border-radius: var(--radius-sm);
    border: 1px solid var(--stroke);
    background: #0e1013;
  }
  li {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    padding: 8px 10px;
    font-size: 13px;
  }
  li + li {
    border-top: 1px solid var(--stroke);
  }
  li span {
    color: var(--faint);
    font-size: 12px;
    text-align: right;
  }
  .note {
    margin: 12px 0 0;
    font-size: 12.5px;
    color: var(--faint);
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 18px;
    flex-wrap: wrap;
  }
  .danger {
    background: #e5484d;
    border-color: #e5484d;
    color: white;
  }
</style>
