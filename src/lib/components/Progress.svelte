<script lang="ts">
  import type { InstallProgress } from '$lib/types';
  import { t } from '$lib/i18n.svelte';
  let { p }: { p: InstallProgress } = $props();
  const pct = $derived(p.total ? Math.round((p.done / p.total) * 100) : 0);
</script>

<div class="progress">
  <div class="row">
    <span>{t(p.stage)}</span>
    <span class="num">{p.total ? `${p.done.toLocaleString()} / ${p.total.toLocaleString()}` : ''}</span>
  </div>
  <div class="track" class:indeterminate={!p.total}>
    <div class="bar" style:width="{p.total ? pct : 35}%"></div>
  </div>
</div>

<style>
  .progress {
    display: flex;
    flex-direction: column;
    gap: 7px;
    width: 100%;
  }
  .row {
    display: flex;
    justify-content: space-between;
    font-size: 12px;
    color: var(--muted);
  }
  .num {
    font-variant-numeric: tabular-nums;
    color: var(--faint);
  }
  .track {
    height: 6px;
    border-radius: 99px;
    background: rgba(255, 255, 255, 0.08);
    overflow: hidden;
  }
  .bar {
    height: 100%;
    border-radius: inherit;
    background: var(--gradient);
    transition: width 0.3s var(--ease);
  }
  .indeterminate .bar {
    animation: slide 1.2s ease-in-out infinite;
  }
  @keyframes slide {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(300%);
    }
  }
</style>
