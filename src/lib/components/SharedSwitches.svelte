<script lang="ts">
  // Switches for using the default keybinds and game settings, or for
  // sharing packs. Used for one instance, and in Settings for new instances.
  import Row from './Row.svelte';
  import Switch from './Switch.svelte';
  import type { SharedParts } from '$lib/types';

  let { value = $bindable(), show }: { value: SharedParts; show: 'defaults' | 'packs' } = $props();

  const all: Record<'defaults' | 'packs', { key: keyof SharedParts; title: string; desc: string }[]> = {
    defaults: [
      { key: 'keybinds', title: 'Use default keybinds', desc: "Controls, including mods' keys" },
      { key: 'options', title: 'Use default game settings', desc: 'Video, sound, GUI scale, FOV and the rest of Options' }
    ],
    packs: [
      { key: 'resourcepacks', title: 'Share resource packs', desc: 'Added in one, there in all, without taking extra space' },
      { key: 'shaderpacks', title: 'Share shader packs', desc: 'Same, for instances with Iris or OptiFine' }
    ]
  };
  const parts = $derived(all[show]);
</script>

<div class="rows">
  {#each parts as p (p.key)}
    <Row title={p.title} desc={p.desc}>
      <Switch bind:checked={() => value[p.key], (v) => (value = { ...value, [p.key]: v })} label={p.title} />
    </Row>
  {/each}
</div>
