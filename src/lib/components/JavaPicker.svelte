<script lang="ts">
  // Choose a Java executable: browse detected installs or type a path, and
  // check that it works.
  import { Search, LoaderCircle, Check } from '@lucide/svelte';
  import { api } from '$lib/api';
  import { toast, errorText } from '$lib/state.svelte';
  import type { DetectedJava } from '$lib/types';

  let { value = $bindable('') }: { value: string } = $props();
  let found = $state<DetectedJava[] | null>(null);
  let scanning = $state(false);
  let tested = $state<DetectedJava | null>(null);

  async function scan() {
    scanning = true;
    try {
      found = await api.detectJava();
      if (!found.length) toast('No Java installations found', 'info');
    } catch (e) {
      toast(errorText(e), 'error');
    } finally {
      scanning = false;
    }
  }

  async function test() {
    try {
      tested = await api.testJava(value);
      toast(`Java ${tested.version} works`, 'success');
    } catch (e) {
      tested = null;
      toast(errorText(e), 'error');
    }
  }
</script>

<div class="picker">
  <div class="line">
    <input class="input mono" bind:value placeholder="Path to java / javaw executable" />
    <button class="btn" onclick={test} disabled={!value.trim()} title="Test this Java"><Check size={15} /> Test</button>
    <button class="btn" onclick={scan} disabled={scanning} title="Find installed Java">
      {#if scanning}<LoaderCircle size={15} class="spin" />{:else}<Search size={15} />{/if} Detect
    </button>
  </div>
  {#if found?.length}
    <div class="found">
      {#each found as j (j.path)}
        <button class="opt" class:active={value === j.path} onclick={() => (value = j.path)}>
          <span class="chip">Java {j.major}</span>
          <span class="path">{j.path}</span>
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .picker {
    display: flex;
    flex-direction: column;
    gap: 8px;
    width: 100%;
  }
  .line {
    display: flex;
    gap: 8px;
  }
  .mono {
    font-family: 'JetBrains Mono', 'Cascadia Code', Consolas, monospace;
    font-size: 12px;
  }
  .found {
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-height: 170px;
    overflow-y: auto;
    padding: 4px;
    border-radius: 12px;
    background: rgba(0, 0, 0, 0.2);
    border: 1px solid var(--stroke);
  }
  .opt {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 9px;
    border-radius: 9px;
    text-align: left;
    font-size: 12px;
    color: var(--muted);
  }
  .opt:hover,
  .opt.active {
    background: rgba(255, 255, 255, 0.06);
    color: var(--text);
  }
  .opt.active .chip {
    color: var(--on-accent);
    background: var(--gradient);
    border-color: transparent;
  }
  .path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
