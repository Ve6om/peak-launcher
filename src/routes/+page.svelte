<script lang="ts">
  import { onMount } from 'svelte';
  import { fade } from 'svelte/transition';
  import Background from '$lib/components/Background.svelte';
  import TitleBar from '$lib/components/TitleBar.svelte';
  import Sidebar from '$lib/components/Sidebar.svelte';
  import Toasts from '$lib/components/Toasts.svelte';
  import CreateInstance from '$lib/components/CreateInstance.svelte';
  import InstanceMenu from '$lib/components/InstanceMenu.svelte';
  import DeleteDialog from '$lib/components/DeleteDialog.svelte';
  import ImportDialog from '$lib/components/ImportDialog.svelte';
  import ExportDialog from '$lib/components/ExportDialog.svelte';
  import DependencyDialog from '$lib/components/DependencyDialog.svelte';
  import ManualDownloads from '$lib/components/ManualDownloads.svelte';
  import CloseDialog from '$lib/components/CloseDialog.svelte';
  import Logo from '$lib/components/Logo.svelte';
  import Home from '$lib/views/Home.svelte';
  import Library from '$lib/views/Library.svelte';
  import InstanceView from '$lib/views/InstanceView.svelte';
  import Accounts from '$lib/views/Accounts.svelte';
  import Browse from '$lib/views/Browse.svelte';
  import Settings from '$lib/views/Settings.svelte';
  import { app, init, toast, errorText, pending } from '$lib/state.svelte';

  onMount(() => {
    init().catch((e) => toast(`Startup failed: ${errorText(e)}`, 'error'));
  });

  // Hide the browser's own right-click menu (Back, Refresh, Inspect…) so the
  // launcher feels like a native app. Text fields keep it for copy/paste, and
  // in development builds Shift+right-click still opens it for Inspect.
  function blockNativeMenu(e: MouseEvent) {
    const t = e.target as HTMLElement | null;
    const editable = t?.closest('input, textarea, [contenteditable="true"]');
    const selecting = !!window.getSelection()?.toString();
    const devInspect = app.info?.devBuild && e.shiftKey;
    if (!editable && !selecting && !devInspect) e.preventDefault();
  }

  let main = $state<HTMLElement>();
  // Scroll back to the top when switching pages.
  $effect(() => {
    app.view;
    app.selectedId;
    main?.scrollTo({ top: 0 });
  });
</script>

<svelte:window oncontextmenu={blockNativeMenu} />

<Background />

<div class="shell">
  <TitleBar />
  {#if app.ready}
    <div class="body" in:fade={{ duration: 300 }}>
      <Sidebar />
      <main bind:this={main}>
        {#key app.view + (app.view === 'instance' ? app.selectedId : '')}
          <div class="view" in:fade={{ duration: 220 }}>
            {#if app.view === 'home'}
              <Home />
            {:else if app.view === 'library'}
              <Library />
            {:else if app.view === 'instance'}
              <InstanceView />
            {:else if app.view === 'browse'}
              <Browse />
            {:else if app.view === 'accounts'}
              <Accounts />
            {:else if app.view === 'settings'}
              <Settings />
            {/if}
          </div>
        {/key}
      </main>
    </div>
  {:else}
    <div class="splash"><Logo size={56} /></div>
  {/if}
</div>

{#if app.creating}
  <CreateInstance />
{/if}
<InstanceMenu />
<DeleteDialog />
{#if app.importing}
  <ImportDialog />
{/if}
{#if app.exporting}
  <ExportDialog inst={app.exporting} />
{/if}
{#if pending.plan}
  <DependencyDialog />
{/if}
{#if app.closePrompt}
  <CloseDialog />
{/if}
{#if app.manualFor}
  {#key app.manualFor}
    <ManualDownloads instanceId={app.manualFor} />
  {/key}
{/if}
<Toasts />

<style>
  .shell {
    position: relative;
    z-index: 1;
    height: 100vh;
    display: flex;
    flex-direction: column;
  }
  .body {
    flex: 1;
    min-height: 0;
    display: flex;
    gap: 12px;
    padding: 0 12px 12px;
  }
  main {
    flex: 1;
    min-width: 0;
    overflow-y: auto;
    padding: 18px 26px 40px;
    border-radius: var(--radius);
  }
  .view {
    max-width: 1280px;
    margin: 0 auto;
  }
  .splash {
    flex: 1;
    display: grid;
    place-items: center;
    animation: breathe 1.6s ease-in-out infinite;
  }
  @keyframes breathe {
    50% {
      opacity: 0.5;
      transform: scale(0.96);
    }
  }
</style>
