<script lang="ts">
  // One labelled setting row. With `override`, a "Custom" switch decides
  // whether the instance uses its own value or the global default.
  import type { Snippet } from 'svelte';
  import Switch from './Switch.svelte';

  let {
    title,
    desc = '',
    override = undefined,
    onoverride,
    globalHint = '',
    wide = false,
    children
  }: {
    title: string;
    desc?: string;
    override?: boolean;
    onoverride?: (on: boolean) => void;
    globalHint?: string;
    wide?: boolean;
    children: Snippet;
  } = $props();

  let on = $state(false);
  $effect(() => {
    on = !!override;
  });
</script>

<div class="row" class:wide>
  <div class="text">
    <b>{title}</b>
    {#if desc}<small>{desc}</small>{/if}
    {#if override !== undefined}
      <span class="ovr">
        <Switch bind:checked={() => on, (v) => { on = v; onoverride?.(v); }} label="Customize {title}" />
        <span>{on ? 'Custom for this instance' : `Using global${globalHint ? `: ${globalHint}` : ''}`}</span>
      </span>
    {/if}
  </div>
  <div class="control" class:muted={override !== undefined && !on}>
    {@render children()}
  </div>
</div>

<style>
  .row {
    display: grid;
    grid-template-columns: 1fr minmax(260px, 380px);
    align-items: center;
    gap: 24px;
    padding: 15px 0;
    border-top: 1px solid var(--stroke);
  }
  .row.wide {
    grid-template-columns: 1fr;
    gap: 10px;
  }
  :global(.rows) > .row:first-child,
  :global(h3) + .row {
    border-top: 0;
  }
  .text {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }
  b {
    font-size: 13.5px;
    font-weight: 580;
  }
  small {
    font-size: 12px;
    color: var(--faint);
    line-height: 1.45;
  }
  .ovr {
    display: flex;
    align-items: center;
    gap: 9px;
    margin-top: 6px;
    font-size: 12px;
    color: var(--muted);
  }
  .ovr :global(.switch) {
    transform: scale(0.8);
    transform-origin: left center;
    margin-right: -6px;
  }
  .control {
    display: flex;
    align-items: center;
    gap: 10px;
    justify-content: flex-end;
    min-width: 0;
    transition: opacity 0.2s;
  }
  .wide .control {
    flex-direction: column;
    align-items: stretch;
  }
  .control.muted {
    opacity: 0.4;
    pointer-events: none;
  }
</style>
