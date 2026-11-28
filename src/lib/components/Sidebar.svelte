<script lang="ts">
  import { instanceIcon, instanceBanner } from '$lib/instanceImages';
  import { House, Library, Package, Server, Users, Images, Settings, Plus, ChevronRight } from '@lucide/svelte';
  import { app, go, activeAccount, isRunning, openInstanceMenu } from '$lib/state.svelte';
  import { shownName } from '$lib/privacy.svelte';
  import type { View } from '$lib/types';
  import InstanceArt from './InstanceArt.svelte';
  import SkinHead from './SkinHead.svelte';

  const nav: { view: View | 'browse'; label: string; icon: typeof House; soon?: boolean }[] = [
    { view: 'home', label: 'Home', icon: House },
    { view: 'library', label: 'Library', icon: Library },
    { view: 'browse', label: 'Modpacks', icon: Package },
    { view: 'servers', label: 'Servers', icon: Server },
    { view: 'accounts', label: 'Accounts', icon: Users },
    { view: 'screenshots', label: 'Screenshots', icon: Images },
    { view: 'settings', label: 'Settings', icon: Settings }
  ];

  const account = $derived(activeAccount());
  const pinned = $derived(app.instances.slice(0, 5));
</script>

<aside class="sidebar glass">
  <nav>
    {#each nav as item (item.view)}
      {@const Icon = item.icon}
      <button
        class="nav-item"
        class:active={app.view === item.view || (item.view === 'library' && app.view === 'instance')}
        disabled={item.soon}
        onclick={() => !item.soon && go(item.view as View)}
      >
        <Icon size={18} strokeWidth={1.9} />
        <span>{item.label}</span>
        {#if item.soon}<span class="soon">Soon</span>{/if}
      </button>
    {/each}
  </nav>

  <div class="section">
    <div class="section-head">
      <span class="label">Instances</span>
      <button class="add" aria-label="New instance" onclick={() => (app.creating = true)}><Plus size={15} /></button>
    </div>
    <div class="pins">
      {#each pinned as inst (inst.id)}
        <button
          class="pin"
          class:active={app.view === 'instance' && app.selectedId === inst.id}
          onclick={() => go('instance', inst.id)}
          oncontextmenu={(e) => openInstanceMenu(e, inst)}
        >
          <span class="thumb"><InstanceArt loader={inst.loader} image={instanceIcon(inst) ?? instanceBanner(inst)} /></span>
          <span class="pin-text">
            <span class="pin-name">{inst.name}</span>
            <span class="pin-meta">{inst.gameVersion}</span>
          </span>
          {#if isRunning(inst.id)}<span class="live" title="Running"></span>{/if}
        </button>
      {:else}
        <p class="empty">No instances yet</p>
      {/each}
    </div>
  </div>

  <button class="account" onclick={() => go('accounts')}>
    <SkinHead accountId={account?.id ?? null} name={shownName(account?.username ?? '')} size={34} />
    <span class="acc-text">
      <span class="acc-name">{account ? shownName(account.username) : 'Add account'}</span>
      <span class="acc-meta">{account ? (account.kind === 'microsoft' ? 'Microsoft' : 'Offline') : 'Sign in to play'}</span>
    </span>
    <ChevronRight size={16} />
  </button>
</aside>

<style>
  .sidebar {
    width: 236px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    padding: 12px;
    gap: 18px;
    background: color-mix(in srgb, var(--surface) 58%, transparent);
    backdrop-filter: blur(14px);
  }
  nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .nav-item {
    position: relative;
    display: flex;
    align-items: center;
    gap: 12px;
    height: 36px;
    padding: 0 12px;
    border-radius: 7px;
    color: var(--muted);
    font-size: 13.5px;
    font-weight: 520;
    transition: background 0.15s var(--ease), color 0.15s var(--ease);
  }
  .nav-item:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.04);
    color: var(--text);
  }
  .nav-item.active {
    color: var(--text);
    background: var(--surface-2);
  }
  .nav-item.active :global(svg) {
    color: var(--accent);
  }
  /* Summit marker on the right of the active item. */
  .nav-item.active::after {
    content: '';
    position: absolute;
    right: 12px;
    top: 50%;
    margin-top: -3px;
    border-left: 4px solid transparent;
    border-right: 4px solid transparent;
    border-bottom: 6px solid var(--accent);
  }
  .nav-item:disabled {
    cursor: default;
    opacity: 0.55;
  }
  .soon {
    margin-left: auto;
    font-size: 10px;
    font-weight: 650;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    padding: 2px 7px;
    border-radius: 99px;
    background: rgba(255, 255, 255, 0.07);
    color: var(--faint);
  }
  .section {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .section-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 6px 8px 12px;
  }
  .add {
    width: 24px;
    height: 24px;
    border-radius: 7px;
    display: grid;
    place-items: center;
    color: var(--muted);
  }
  .add:hover {
    background: rgba(255, 255, 255, 0.08);
    color: var(--text);
  }
  .pins {
    display: flex;
    flex-direction: column;
    gap: 2px;
    overflow-y: auto;
  }
  .pin {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px;
    border-radius: 7px;
    text-align: left;
    transition: background 0.15s;
  }
  .pin:hover,
  .pin.active {
    background: rgba(255, 255, 255, 0.05);
  }
  .thumb {
    width: 32px;
    height: 32px;
    border-radius: 9px;
    overflow: hidden;
    flex-shrink: 0;
  }
  .pin-text,
  .acc-text {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }
  .pin-name,
  .acc-name {
    font-size: 13px;
    font-weight: 550;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .pin-meta,
  .acc-meta {
    font-size: 11.5px;
    color: var(--faint);
  }
  .live {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--success);
    box-shadow: 0 0 0 3px rgba(52, 211, 153, 0.2);
    animation: pulse 1.6s ease-in-out infinite;
    margin-right: 4px;
  }
  @keyframes pulse {
    50% {
      box-shadow: 0 0 0 6px rgba(52, 211, 153, 0);
    }
  }
  .empty {
    margin: 0;
    padding: 6px 12px;
    font-size: 12.5px;
    color: var(--faint);
  }
  .account {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px;
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    border: 1px solid var(--stroke);
    color: var(--muted);
    text-align: left;
    transition: background 0.2s;
  }
  .account:hover {
    background: rgba(255, 255, 255, 0.07);
  }
  .acc-name {
    color: var(--text);
  }
</style>
