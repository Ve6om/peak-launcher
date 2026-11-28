<script lang="ts">
  import { t } from '$lib/i18n.svelte';
  // Switches for using the default keybinds and game settings, or for
  // sharing packs. Used for one instance, and in Settings for new instances.
  import Row from './Row.svelte';
  import Switch from './Switch.svelte';
  import type { SharedParts } from '$lib/types';

  let { value = $bindable(), show }: { value: SharedParts; show: 'defaults' | 'packs' } = $props();

  const all: Record<'defaults' | 'packs', { key: keyof SharedParts; title: string; desc: string }[]> = {
    defaults: [
      { key: 'keybinds', title: t('Use default keybinds'), desc: t('Controls, including mods\' keys') },
      { key: 'options', title: t('Use default game settings'), desc: t('Video, sound, GUI scale, FOV and the rest of Options') }
    ],
    packs: [
      { key: 'resourcepacks', title: t('Share resource packs'), desc: t('Added in one, there in all, without taking extra space') },
      { key: 'shaderpacks', title: t('Share shader packs'), desc: t('Same, for instances with Iris or OptiFine') }
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
