<script lang="ts">
  // The Optimize button's dialog: what Peak suggests for this instance and
  // why. Nothing changes until Apply.
  import { onMount } from 'svelte';
  import { Sparkles, Check } from '@lucide/svelte';
  import Modal from './Modal.svelte';
  import Switch from './Switch.svelte';
  import { api } from '$lib/api';
  import { app, errorText, refreshInstances, toast } from '$lib/state.svelte';
  import type { Instance, TuningSuggestion } from '$lib/types';
  import { javaFolderName } from '$lib/javaNames';

  let {
    inst,
    onapply,
    onclose
  }: { inst: Instance; onapply?: (s: TuningSuggestion, useGraal: boolean) => void; onclose: () => void } = $props();

  // Opened from the overview or the right-click menu: save straight away.
  let saving = $state(false);
  async function applyHere(t: TuningSuggestion, useGraal: boolean) {
    saving = true;
    try {
      const next: Instance = { ...$state.snapshot(inst), memoryMb: t.memoryMb, jvmPreset: t.jvmPreset };
      if (useGraal) {
        next.javaRuntime = 'graalvm';
        next.javaPath = null;
      }
      await api.updateInstance(next);
      await refreshInstances();
      toast(`Optimized ${inst.name}`, 'success');
      onclose();
    } catch (e) {
      toast(errorText(e), 'error');
    } finally {
      saving = false;
    }
  }
  function apply() {
    if (!s) return;
    const graal = s.graalvm && useGraal && !alreadyGraal;
    if (onapply) onapply(s, graal);
    else applyHere(s, graal);
  }

  let s = $state<TuningSuggestion | null>(null);
  let error = $state('');
  let useGraal = $state(true);

  onMount(async () => {
    try {
      s = await api.suggestTuning(inst.id);
      useGraal = s.graalvm;
    } catch (e) {
      error = errorText(e);
    }
  });

  const presetName = (id: string) => app.info?.jvmPresets.find((p) => p.id === id)?.name ?? id;
  const gb = (mb: number) => `${+(mb / 1024).toFixed(1)} GB`;
  const g = $derived(app.settings);
  const alreadyGraal = $derived(!inst.javaPath && (inst.javaRuntime === 'graalvm' || (!inst.javaRuntime && g?.useBundledJava !== false && g?.javaVendor === 'graalvm')));

  // What the instance uses now (its own setting, else the global one).
  const memNow = $derived(inst.memoryMb ?? g?.memoryMb ?? 4096);
  const presetNow = $derived(inst.jvmPreset ?? g?.jvmPreset ?? 'none');
  const BUILD: Record<string, string> = { mojang: "Mojang's Java", temurin: 'Temurin', graalvm: 'GraalVM' };
  const javaNow = $derived.by(() => {
    if (inst.javaPath) return javaFolderName(inst.javaPath);
    if (inst.javaRuntime) return BUILD[inst.javaRuntime] ?? inst.javaRuntime;
    if (g && !g.useBundledJava && g.javaPath) return javaFolderName(g.javaPath);
    return BUILD[g?.javaVendor ?? 'mojang'] ?? "Mojang's Java";
  });

  const memOk = $derived(!!s && memNow === s.memoryMb);
  const presetOk = $derived(!!s && presetNow === s.jvmPreset);
  const javaOk = $derived(!!s && (!s.graalvm || alreadyGraal));
  const switchingJava = $derived(!!s && s.graalvm && !alreadyGraal && useGraal);
  const allGood = $derived(memOk && presetOk && javaOk);
</script>

<Modal title="Optimize {inst.name}" subtitle="Memory, Java flags and Java picked for this instance and this computer" width={540} {onclose}>
  {#if error}
    <p class="error">{error}</p>
  {:else if !s}
    <p class="muted">Looking at the instance…</p>
  {:else}
    {#if allGood}
      <p class="done"><Check size={16} /> Already optimized. {inst.name} uses what Peak would pick.</p>
    {/if}

    <div class="picks">
      {@render pick('Memory', memOk, gb(memNow), gb(s.memoryMb))}
      {@render pick('Flags', presetOk, presetName(presetNow), presetName(s.jvmPreset))}
      {@render pick('Java', !switchingJava, javaNow, 'GraalVM')}
    </div>

    <ul class="why">
      {#each s.reasons as r, i (i)}<li>{r}</li>{/each}
      {#if !s.graalvm}<li>Java {s.javaMajor} game: GraalVM isn't made for it, so Java stays as it is.</li>{/if}
    </ul>

    {#if s.graalvm && !alreadyGraal}
      <label class="graal">
        <span>
          <b>Switch to GraalVM</b>
          <small>Downloaded the next time you play (about 600 MB). If the game crashes or acts strangely, set Java build back to Mojang's Java in this instance's settings.</small>
        </span>
        <Switch bind:checked={useGraal} label="Switch to GraalVM" />
      </label>
    {/if}

    <p class="note">
      These are good starting points, not exact numbers: Modrinth and CurseForge don't say how much memory a pack needs, so Peak estimates it from {s.mods} mod{s.mods === 1 ? '' : 's'}{s.shaders ? ', shaders' : ''}, Java {s.javaMajor} and the {gb(s.totalMemoryMb)} of memory in this computer. If the game runs out of memory or stutters, adjust it by hand in the instance's settings.
    </p>

    <div class="actions">
      {#if allGood}
        <button class="btn primary" onclick={onclose}>Close</button>
      {:else}
        <button class="btn ghost" onclick={onclose}>Cancel</button>
        <button class="btn primary" disabled={saving} onclick={apply}><Sparkles size={15} /> Apply</button>
      {/if}
    </div>
  {/if}
</Modal>

<!-- One setting: a tick when it's already right, else what it is now and what it becomes. -->
{#snippet pick(label: string, ok: boolean, now: string, next: string)}
  <div class="pick" class:ok>
    <span class="k">{label}</span>
    {#if ok}
      <span class="v"><Check size={15} /> {now}</span>
      <span class="sub">Already set</span>
    {:else}
      <span class="v">{next}</span>
      <span class="sub">Currently {now}</span>
    {/if}
  </div>
{/snippet}

<style>
  .picks {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 8px;
  }
  .pick {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 10px 12px;
    border: 1px solid var(--stroke);
    border-radius: var(--radius-sm);
    background: rgba(255, 255, 255, 0.025);
  }
  .pick.ok {
    border-color: color-mix(in srgb, #34d399 35%, transparent);
  }
  .pick.ok .v {
    color: #6ee7b7;
  }
  .v {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  .sub {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 11.5px;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .done {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 0 12px;
    padding: 9px 12px;
    border-radius: var(--radius-sm);
    background: color-mix(in srgb, #34d399 12%, transparent);
    border: 1px solid color-mix(in srgb, #34d399 35%, transparent);
    color: #a7f3d0;
    font-size: 13px;
    font-weight: 600;
  }
  .k {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--muted);
  }
  .v {
    font-size: 15px;
    font-weight: 650;
    color: var(--accent);
  }
  .why {
    margin: 14px 0;
    padding-left: 18px;
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 13px;
    color: var(--text);
  }
  .graal {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    padding: 10px 12px;
    border: 1px solid var(--stroke);
    border-radius: var(--radius-sm);
    margin-bottom: 12px;
  }
  .graal span {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 13px;
  }
  .graal small {
    color: var(--muted);
    font-size: 12px;
    line-height: 1.4;
  }
  .muted {
    color: var(--muted);
    font-size: 13px;
  }
  .note {
    margin: 0;
    padding: 9px 12px;
    border-radius: var(--radius-sm);
    background: rgba(255, 255, 255, 0.035);
    font-size: 12px;
    line-height: 1.45;
    color: var(--muted);
  }
  .error {
    color: var(--danger, #ff6464);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 14px;
  }
</style>
