<script lang="ts">
  import { t } from '$lib/i18n.svelte';
  // Find the broken mod: Peak turns off batches of mods, you play each test,
  // and say whether it still crashed, until one mod is left.
  import { Play, LoaderCircle, Bug, Check, Undo2, Power, Search, TriangleAlert } from '@lucide/svelte';
  import { untrack } from 'svelte';
  import Modal from './Modal.svelte';
  import { api } from '$lib/api';
  import { app, play, isRunning, toast, errorText, refreshInstances } from '$lib/state.svelte';

  let { instanceId, guess, onclose }: { instanceId: string; guess: 'crashed' | 'worked' | null; onclose: () => void } = $props();

  const inst = $derived(app.instances.find((i) => i.id === instanceId));
  const search = $derived(app.modSearches[instanceId]);
  const running = $derived(isRunning(instanceId));
  let busy = $state(false);
  /** The guess applies to the test just played; gone once answered. */
  let lastGuess = $state(untrack(() => guess));

  async function run<T>(f: () => Promise<T>): Promise<T | undefined> {
    busy = true;
    try {
      return await f();
    } catch (e) {
      toast(errorText(e), 'error');
    } finally {
      busy = false;
    }
  }

  async function start() {
    const s = await run(() => api.modSearchStart(instanceId));
    if (s) {
      app.modSearches[instanceId] = s;
      lastGuess = null;
      await refreshInstances();
    }
  }

  async function answer(crashed: boolean) {
    const s = await run(() => api.modSearchAnswer(instanceId, crashed));
    if (s) {
      app.modSearches[instanceId] = s;
      lastGuess = null;
    }
  }

  async function finish(keepOff: boolean) {
    const ok = await run(async () => {
      await api.modSearchFinish(instanceId, keepOff);
      return true;
    });
    if (!ok) return;
    const found = search?.culprits.map((c) => c.name).join(', ');
    delete app.modSearches[instanceId];
    await refreshInstances();
    toast(keepOff && found ? t('Kept {found} off and turned the other mods back on', { found }) : t('Every mod is back the way it was'), 'success');
    onclose();
  }

  function playTest() {
    if (!inst) return;
    lastGuess = null;
    play(inst);
  }

  /** Close the window; a search keeps going until it's stopped. */
  function close() {
    if (search && search.phase !== 'found' && search.phase !== 'notAMod') {
      toast(t('The search is paused. Pick it up from the instance page.'), 'info');
    }
    onclose();
  }
</script>

<Modal title={t('Find the broken mod')} subtitle={inst?.name} width={560} onclose={close}>
  {#if !search}
    <p class="lead">
      {t('Peak turns off a batch of mods, you play, and you tell Peak whether it still crashed. Each test halves the list, so even big packs take only a handful of tests.')}
    </p>
    <ul class="how">
      <li>{t('The first test runs with every mod off, to make sure a mod is to blame at all.')}</li>
      <li>{t('Mods other mods need are only turned off along with those mods, so a test never crashes over a missing dependency.')}</li>
      <li>{t('Stopping at any point puts every mod back exactly as it was.')}</li>
    </ul>
    <p class="warn">
      <TriangleAlert size={14} />
      <span>{t('Don\'t load a world you care about during a test: with mods turned off, the game can delete their blocks and items from it. If the crash only happens in-game, make a new world to test in.')}</span>
    </p>
    <div class="actions">
      <button class="btn ghost" onclick={onclose}>{t('Cancel')}</button>
      <button class="btn primary" disabled={busy || running} onclick={start}>
        {#if busy}<LoaderCircle size={14} class="spin" />{:else}<Search size={14} />{/if} {t('Start')}
      </button>
    </div>
    {#if running}<p class="hint">{t('Close the game first.')}</p>{/if}
  {:else if search.phase === 'found'}
    <div class="found">
      <Bug size={20} />
      <div>
        <b>{search.culprits.length === 1 ? t('Found it') : t('It\'s one of these {length}', { length: search.culprits.length })}</b>
        <span>{search.culprits.map((c) => c.name).join(', ')}</span>
      </div>
    </div>
    {#if search.culprits.length > 1}
      <p class="lead">{t('These mods need each other, so Peak can\'t test them apart.')}</p>
    {/if}
    <p class="lead">
      {search.needsIt.length ? t('Try another version of it or remove it. Keeping it off now lets you play straight away, along with the mods that need it:') : t('Try another version of it or remove it. Keeping it off now lets you play straight away.')}
    </p>
    {#if search.needsIt.length}
      <p class="needs">{search.needsIt.map((n) => n.name).join(', ')}</p>
    {/if}
    <div class="actions">
      <button class="btn ghost" disabled={busy || running} onclick={() => finish(false)}><Undo2 size={14} /> {t('Put everything back')}</button>
      <button class="btn primary" disabled={busy || running} onclick={() => finish(true)}><Power size={14} /> {t('Keep it off, turn the rest on')}</button>
    </div>
  {:else if search.phase === 'notAMod'}
    <p class="lead">
      {t('It still crashed with every mod turned off, so a mod isn\'t the cause. It\'s more likely the Java version, memory, the mod loader, or a config file. The crash report and the console usually say which.')}
    </p>
    <div class="actions">
      <button class="btn primary" disabled={busy || running} onclick={() => finish(false)}><Undo2 size={14} /> {t('Put the mods back')}</button>
    </div>
  {:else}
    <div class="progress">
      <b>{t('Test {test} of about {tests}', { test: search.test, tests: search.tests })}</b>
      <span>
        {search.phase === 'allOff' ? t('Every mod is off ({mods})', { mods: search.mods }) : t('{off} of {mods} mods off · {suspects} still suspected', { off: search.off, mods: search.mods, suspects: search.suspects })}
      </span>
      <div class="bar"><i style="width: {Math.min(100, ((search.test - 1) / Math.max(1, search.tests)) * 100)}%"></i></div>
    </div>

    {#if running}
      <p class="lead"><LoaderCircle size={14} class="spin" /> {t('Testing. Do what made it crash before, then close the game.')}</p>
    {:else if lastGuess}
      <p class="lead">
        {lastGuess === 'crashed' ? t('That looked like a crash.') : t('The game closed normally.')} {t('How did test {test} go?', { test: search.test })}
      </p>
    {:else}
      <p class="lead">
        {search.phase === 'allOff' ? t('Press Play and do what made it crash before (with no mods, just getting to the title screen counts). If it starts fine, get as far as where it used to crash, then close the game.') : t('Press Play and do what made it crash before. If it starts fine, get as far as where it used to crash, then close the game.')}
      </p>
    {/if}

    <div class="actions">
      <button class="btn ghost" disabled={busy || running} onclick={() => finish(false)}><Undo2 size={14} /> {t('Stop')}</button>
      <span class="spacer"></span>
      {#if lastGuess || running}
        <button class="btn" class:primary={lastGuess === 'crashed'} disabled={busy || running} onclick={() => answer(true)}><Bug size={14} /> {t('It crashed')}</button>
        <button class="btn" class:primary={lastGuess === 'worked'} disabled={busy || running} onclick={() => answer(false)}><Check size={14} /> {t('It worked')}</button>
      {:else}
        <button class="btn ghost small" disabled={busy} onclick={() => answer(true)} title={t('Already tried this test')}>{t('It crashed')}</button>
        <button class="btn ghost small" disabled={busy} onclick={() => answer(false)} title={t('Already tried this test')}>{t('It worked')}</button>
        <button class="btn primary" disabled={busy} onclick={playTest}><Play size={14} /> {t('Play test {test}', { test: search.test })}</button>
      {/if}
    </div>
  {/if}
</Modal>

<style>
  .lead {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 0 10px;
    font-size: 13.5px;
    line-height: 1.5;
  }
  .how {
    margin: 0 0 12px;
    padding-left: 18px;
    font-size: 12.5px;
    line-height: 1.6;
    color: var(--muted);
  }
  .warn {
    display: flex;
    gap: 8px;
    margin: 0;
    padding: 10px 12px;
    border-radius: 10px;
    font-size: 12.5px;
    line-height: 1.5;
    color: #fde68a;
    background: rgba(250, 204, 21, 0.08);
    border: 1px solid rgba(250, 204, 21, 0.2);
  }
  .warn :global(svg) {
    flex-shrink: 0;
    margin-top: 2px;
  }
  .hint {
    margin: 8px 0 0;
    font-size: 12px;
    color: var(--faint);
    text-align: right;
  }
  .progress {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-bottom: 14px;
  }
  .progress b {
    font-size: 15px;
    font-weight: 650;
  }
  .progress span {
    font-size: 12.5px;
    color: var(--muted);
  }
  .bar {
    height: 6px;
    margin-top: 6px;
    border-radius: 3px;
    background: rgba(255, 255, 255, 0.07);
    overflow: hidden;
  }
  .bar i {
    display: block;
    height: 100%;
    background: var(--accent);
    transition: width 0.3s;
  }
  .found {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 12px;
    padding: 12px;
    border-radius: 12px;
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    border: 1px solid color-mix(in srgb, var(--accent) 30%, transparent);
    color: var(--accent);
  }
  .found div {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .found b {
    font-size: 15px;
    color: var(--text);
  }
  .found span {
    font-size: 13px;
    color: var(--text);
  }
  .needs {
    margin: 0 0 10px;
    font-size: 12.5px;
    color: var(--muted);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 16px;
  }
  .spacer {
    flex: 1;
  }
  .small {
    font-size: 12px;
  }
</style>
