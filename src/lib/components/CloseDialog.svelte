<script lang="ts">
  // Shown when closing the launcher while a game is still running.
  import { Gamepad2, Square } from '@lucide/svelte';
  import Modal from './Modal.svelte';
  import { app, quitApp } from '$lib/state.svelte';

  const running = $derived(app.instances.filter((i) => app.running.includes(i.id)));
  let busy = $state(false);

  async function quit(stopGames: boolean) {
    busy = true;
    await quitApp(stopGames);
    busy = false;
  }

  // If the game closes on its own meanwhile, there's nothing left to ask.
  $effect(() => {
    if (!running.length && app.closePrompt && !busy) quitApp(false);
  });
</script>

<Modal
  title="Minecraft is still running"
  subtitle="Close the launcher and keep playing, or stop the game too."
  width={460}
  onclose={() => (app.closePrompt = false)}
>
  <div class="games">
    {#each running as inst (inst.id)}
      <div class="game"><Gamepad2 size={15} /> {inst.name}</div>
    {/each}
  </div>
  <p class="note">If you keep playing, the game's console output and playtime for this session won't be recorded.</p>
  <footer>
    <button class="btn ghost" onclick={() => (app.closePrompt = false)}>Cancel</button>
    <button class="btn" disabled={busy} onclick={() => quit(false)}>Quit, keep playing</button>
    <button class="btn danger" disabled={busy} onclick={() => quit(true)}>
      <Square size={12} fill="currentColor" /> Stop {running.length > 1 ? 'games' : 'game'} & quit
    </button>
  </footer>
</Modal>

<style>
  .games {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .game {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 9px 12px;
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    border: 1px solid var(--stroke);
    font-size: 13.5px;
    color: var(--text);
  }
  .game :global(svg) {
    color: var(--success);
  }
  .note {
    margin: 12px 0 0;
    font-size: 12px;
    color: var(--faint);
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 18px;
  }
</style>
