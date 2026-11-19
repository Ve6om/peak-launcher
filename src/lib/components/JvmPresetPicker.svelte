<script lang="ts">
  // Pick a Java flag preset. Each card says what it's for; the exact flags
  // are one click away for anyone curious.
  import { Check } from '@lucide/svelte';
  import { app } from '$lib/state.svelte';

  let { value = $bindable('none'), disabled = false }: { value?: string; disabled?: boolean } = $props();

  const presets = $derived(app.info?.jvmPresets ?? []);
  let showFlags = $state(false);
  const current = $derived(presets.find((p) => p.id === value));
</script>

<div class="picker" class:disabled>
  <div class="grid" role="radiogroup" aria-label="Java flag preset">
    {#each presets as p (p.id)}
      <button
        class="preset"
        class:active={p.id === value}
        role="radio"
        aria-checked={p.id === value}
        {disabled}
        onclick={() => (value = p.id)}
      >
        <span class="name">{#if p.id === value}<Check size={14} />{/if}{p.name}</span>
        <span class="desc">{p.description}</span>
      </button>
    {/each}
  </div>
  {#if current?.flags}
    <button class="toggle" onclick={() => (showFlags = !showFlags)}>{showFlags ? 'Hide' : 'Show'} the flags</button>
    {#if showFlags}<code class="flags">{current.flags}</code>{/if}
  {/if}
</div>

<style>
  .picker {
    display: flex;
    flex-direction: column;
    gap: 8px;
    width: 100%;
  }
  .disabled {
    opacity: 0.55;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px;
  }
  .preset {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 10px 12px;
    text-align: left;
    border-radius: var(--radius-sm);
    border: 1px solid var(--stroke);
    background: rgba(255, 255, 255, 0.025);
    color: var(--text);
    font: inherit;
    cursor: pointer;
  }
  .preset:hover:not(:disabled) {
    border-color: var(--stroke-strong);
  }
  .preset.active {
    border-color: color-mix(in srgb, var(--accent) 55%, transparent);
    background: color-mix(in srgb, var(--accent) 10%, transparent);
  }
  .name {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
    font-weight: 650;
  }
  .active .name {
    color: var(--accent);
  }
  .desc {
    font-size: 11.5px;
    line-height: 1.45;
    color: var(--muted);
  }
  .toggle {
    align-self: flex-start;
    padding: 0;
    border: none;
    background: none;
    font: inherit;
    font-size: 12px;
    color: var(--accent);
    cursor: pointer;
  }
  .flags {
    display: block;
    padding: 8px 10px;
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.3);
    font-size: 11.5px;
    line-height: 1.5;
    color: var(--muted);
    white-space: normal;
    word-break: break-word;
    user-select: text;
  }
</style>
