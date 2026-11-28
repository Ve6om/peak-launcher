<script lang="ts">
  import { t } from '$lib/i18n.svelte';
  // A modpack filter that stays usable when there are many packs: search box,
  // each pack's icon, and how many servers run it.
  import { Package, ChevronDown } from '@lucide/svelte';

  type Option = { id: string; name: string; icon: string | null; count: number };
  let { options, value = $bindable('') }: { options: Option[]; value?: string } = $props();

  let open = $state(false);
  let q = $state('');
  let active = $state(0);
  let root = $state<HTMLDivElement>();
  let search = $state<HTMLInputElement>();

  const all = $derived<Option[]>([{ id: '', name: t('Any modpack'), icon: null, count: options.reduce((n, o) => n + o.count, 0) }, ...options]);
  const shown = $derived(all.filter((o) => !q.trim() || o.name.toLowerCase().includes(q.trim().toLowerCase())));
  const current = $derived(all.find((o) => o.id === value) ?? all[0]);

  function toggle() {
    open = !open;
    if (open) {
      q = '';
      active = Math.max(0, all.findIndex((o) => o.id === value));
      queueMicrotask(() => search?.focus());
    }
  }

  function pick(o: Option) {
    value = o.id;
    open = false;
  }

  function key(e: KeyboardEvent) {
    if (e.key === 'ArrowDown') active = Math.min(shown.length - 1, active + 1);
    else if (e.key === 'ArrowUp') active = Math.max(0, active - 1);
    else if (e.key === 'Enter' && shown[active]) pick(shown[active]);
    else if (e.key === 'Escape') open = false;
    else return;
    e.preventDefault();
  }
</script>

<svelte:window onclick={(e) => open && root && !root.contains(e.target as Node) && (open = false)} />

<div class="picker" bind:this={root}>
  <button type="button" class="input trigger" aria-haspopup="listbox" aria-expanded={open} onclick={toggle}>
    {#if current.id}
      {#if current.icon}<img src={current.icon} alt="" />{:else}<Package size={15} />{/if}
    {/if}
    <span class="name">{current.name}</span>
    <ChevronDown size={14} class="chev" />
  </button>
  {#if open}
    <div class="pop glass">
      <input class="input" bind:this={search} bind:value={q} oninput={() => (active = 0)} onkeydown={key} placeholder={t('Search modpacks')} aria-label={t('Search modpacks')} />
      <ul role="listbox" aria-label={t('Modpacks')}>
        {#each shown as o, i (o.id)}
          <li role="option" aria-selected={o.id === value} class:active={i === active} onmousedown={(e) => e.preventDefault()} onclick={() => pick(o)} onkeydown={() => {}} onmouseenter={() => (active = i)}>
            {#if o.icon}<img src={o.icon} alt="" />{:else}<span class="noicon"><Package size={13} /></span>{/if}
            <span class="name">{o.name}</span>
            {#if o.count}<span class="count">{o.count}</span>{/if}
          </li>
        {:else}
          <li class="none">{t('No modpacks match')}</li>
        {/each}
      </ul>
    </div>
  {/if}
</div>

<style>
  .picker {
    position: relative;
  }
  .trigger {
    display: flex;
    align-items: center;
    gap: 8px;
    width: auto;
    min-width: 170px;
    max-width: 230px;
    height: 36px;
    padding: 0 10px;
    font-size: 13px;
    text-align: left;
    cursor: pointer;
  }
  .trigger :global(.chev) {
    margin-left: auto;
    flex-shrink: 0;
    color: var(--muted);
  }
  img,
  .noicon {
    width: 20px;
    height: 20px;
    flex-shrink: 0;
    border-radius: 5px;
    object-fit: cover;
  }
  .noicon {
    display: grid;
    place-items: center;
    background: rgba(255, 255, 255, 0.06);
    color: var(--faint);
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .pop {
    position: absolute;
    z-index: 40;
    top: calc(100% + 6px);
    left: 0;
    width: 300px;
    padding: 8px;
    background: var(--surface);
    box-shadow: 0 16px 40px rgba(0, 0, 0, 0.55);
  }
  .pop .input {
    height: 34px;
    margin-bottom: 6px;
    font-size: 13px;
  }
  ul {
    max-height: 280px;
    overflow-y: auto;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 8px;
    border-radius: 6px;
    font-size: 13px;
    cursor: pointer;
  }
  li.active {
    background: rgba(255, 255, 255, 0.06);
  }
  li[aria-selected='true'] {
    color: var(--accent);
  }
  .count {
    font-size: 11.5px;
    color: var(--faint);
  }
  .none {
    color: var(--faint);
    cursor: default;
  }
</style>
