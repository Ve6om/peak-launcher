<script lang="ts">
  // An instance's console in its own window. It keeps working on its own:
  // it loads what was already printed, then follows the game's output.
  import { onMount } from 'svelte';
  import ConsolePanel from '$lib/components/ConsolePanel.svelte';
  import { api, on, isTauri } from '$lib/api';
  import { applyAccent, errorText } from '$lib/state.svelte';
  import type { LogLine } from '$lib/types';

  let { instanceId }: { instanceId: string } = $props();

  let logs = $state<LogLine[]>([]);
  let running = $state(false);
  let pinned = $state(false);
  let notice = $state<{ kind: 'info' | 'error'; text: string } | null>(null);

  onMount(() => {
    const stops: Array<() => void> = [];
    (async () => {
      const [instances, runningIds, history, settings] = await Promise.all([
        api.instances(),
        api.running(),
        api.gameLogs(instanceId),
        api.settings()
      ]);
      applyAccent(settings.accent);
      const name = instances.find((i) => i.id === instanceId)?.name ?? 'Instance';
      document.title = `${name} - Console`;
      running = runningIds.includes(instanceId);
      logs = history;

      stops.push(
        await on<LogLine & { instanceId: string }>('game-log', (l) => {
          if (l.instanceId !== instanceId) return;
          logs.push({ line: l.line, stream: l.stream });
          if (logs.length > 5000) logs.splice(0, logs.length - 5000);
        }),
        await on<{ instanceId: string }>('game-log-reset', (e) => {
          if (e.instanceId === instanceId) logs = [];
        }),
        await on<{ instanceId: string; state: 'running' | 'stopped'; exitCode: number | null }>('game-state', (s) => {
          if (s.instanceId !== instanceId) return;
          running = s.state === 'running';
          if (running) notice = null;
          else if (s.exitCode && s.exitCode !== 0) notice = { kind: 'error', text: `The game exited with code ${s.exitCode}.` };
          else notice = { kind: 'info', text: 'The game has stopped.' };
        })
      );
    })().catch((e) => (notice = { kind: 'error', text: errorText(e) }));
    return () => stops.forEach((s) => s());
  });

  async function play() {
    notice = null;
    try {
      await api.launch(instanceId);
    } catch (e) {
      notice = { kind: 'error', text: errorText(e) };
    }
  }

  async function togglePin() {
    pinned = !pinned;
    if (isTauri) {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      await getCurrentWindow().setAlwaysOnTop(pinned);
    }
  }

  // Keep the browser's own right-click menu out of the way, except when copying text.
  function oncontextmenu(e: MouseEvent) {
    const t = e.target as HTMLElement | null;
    if (!t?.closest('input') && !window.getSelection()?.toString()) e.preventDefault();
  }
</script>

<svelte:window {oncontextmenu} />

<div class="win">
  <ConsolePanel {instanceId} {logs} {running} {pinned} windowed onclear={() => (logs = [])} onplay={play} onpin={togglePin} />
  {#if notice}
    <div class="notice {notice.kind}">
      <span>{notice.text}</span>
      <button class="btn ghost" onclick={() => (notice = null)}>Dismiss</button>
    </div>
  {/if}
</div>

<style>
  :global(body) {
    background: #07080d;
  }
  .win {
    position: relative;
    height: 100vh;
    overflow: hidden;
  }
  .notice {
    position: absolute;
    left: 50%;
    bottom: 16px;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 6px 6px 14px;
    border-radius: 10px;
    font-size: 12.5px;
    background: var(--surface-2);
    border: 1px solid var(--stroke);
    box-shadow: 0 8px 28px rgba(0, 0, 0, 0.5);
  }
  .notice.error {
    color: #fecaca;
    border-color: rgba(239, 68, 68, 0.45);
  }
</style>
