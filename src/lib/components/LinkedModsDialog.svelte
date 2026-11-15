<script lang="ts">
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
  const mods = $derived(n === 1 ? 'mod' : 'mods');
  const text = $derived(
    {
      disable: {
        title: `Other mods need ${name}`,
        intro: `${n} ${mods} won't load without it. Disable ${n === 1 ? 'it' : 'them'} too?`,
        all: `Disable all ${n + 1}`,
        only: `Only disable ${name}`
      },
      remove: {
        title: `Other mods need ${name}`,
        intro: `${n} ${mods} won't load without it. Remove ${n === 1 ? 'it' : 'them'} too?`,
        all: `Remove all ${n + 1}`,
        only: `Only remove ${name}`
      },
      enable: {
        title: `${name} needs disabled mods`,
        intro: `It won't load without ${n === 1 ? 'this mod' : `these ${n} mods`}. Turn ${n === 1 ? 'it' : 'them'} back on too?`,
        all: `Enable all ${n + 1}`,
        only: `Only enable ${name}`
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
    <p class="note">If you keep them, the game will probably refuse to start until {name} is back.</p>
  {/if}
  <footer>
    <button class="btn ghost" onclick={() => onchoose(null)}>Cancel</button>
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
