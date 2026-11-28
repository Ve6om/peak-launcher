<script lang="ts">
  import { t } from '$lib/i18n.svelte';
  // Which Java build automatic Java downloads: Mojang's (the default),
  // Temurin or GraalVM. Each says what it's good for.
  import { Check } from '@lucide/svelte';
  import type { JavaVendor } from '$lib/types';

  let {
    value = $bindable('mojang'),
    disabled = false
  }: { value?: JavaVendor; disabled?: boolean } = $props();

  const vendors: { id: JavaVendor; name: string; desc: string }[] = [
    { id: 'mojang', name: "Mojang's Java", desc: t('The Java Minecraft is tested with. The safest choice.') },
    { id: 'temurin', name: 'Eclipse Temurin', desc: t('A popular standard build of Java. Runs much like Mojang’s.') },
    {
      id: 'graalvm',
      name: 'GraalVM',
      desc: t('Often runs Minecraft a little faster, modded or not. Takes about 600 MB of space per Java version, and now and then a mod doesn’t like it. Not for Minecraft before 1.17.')
    }
  ];
</script>

<div class="grid" class:disabled role="radiogroup" aria-label={t('Java build')}>
  {#each vendors as v (v.id)}
    <button class="opt" class:active={v.id === value} role="radio" aria-checked={v.id === value} {disabled} onclick={() => (value = v.id)}>
      <span class="name">{#if v.id === value}<Check size={14} />{/if}{v.name}</span>
      <span class="desc">{v.desc}</span>
    </button>
  {/each}
</div>

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 8px;
    width: 100%;
  }
  @media (max-width: 900px) {
    .grid {
      grid-template-columns: 1fr;
    }
  }
  .disabled {
    opacity: 0.55;
  }
  .opt {
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
  .opt:hover:not(:disabled) {
    border-color: var(--stroke-strong);
  }
  .opt.active {
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
    font-size: 12px;
    color: var(--muted);
    line-height: 1.4;
  }
</style>
