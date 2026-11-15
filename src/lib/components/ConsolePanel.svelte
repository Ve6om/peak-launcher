<script lang="ts">
  import { tick } from 'svelte';
  import { Copy, Check, Eraser, FolderOpen, Search, SquareArrowOutUpRight, Pin, PinOff, Play, OctagonX, WrapText, ArrowDownToLine } from '@lucide/svelte';
  import { api } from '$lib/api';
  import type { LogLine } from '$lib/types';
  import { parseColors, type Segment } from '$lib/ansi';

  let {
    instanceId,
    logs,
    running,
    windowed = false,
    pinned = false,
    onclear,
    onplay,
    onpin
  }: {
    instanceId: string;
    logs: LogLine[];
    running: boolean;
    /** Shown in its own window: fills it and swaps "Pop out" for "Keep on top". */
    windowed?: boolean;
    pinned?: boolean;
    onclear: () => void;
    /** Only offered in the pop-out; the instance page has its own Play button. */
    onplay?: () => void;
    onpin?: () => void;
  } = $props();

  let el = $state<HTMLDivElement>();
  let follow = $state(true);
  let wrap = $state(true);
  let query = $state('');
  let level = $state<'all' | 'warn' | 'error'>('all');
  let copied = $state(false);
  let confirmKill = $state(false);
  let killTimer: ReturnType<typeof setTimeout> | undefined;

  interface Row {
    text: string;
    stream: LogLine['stream'];
    segs: Segment[] | null;
    warn: boolean;
    err: boolean;
  }

  // Parsed lines are cached by their raw text, so new output doesn't re-parse the whole log.
  const cache = new Map<string, Omit<Row, 'stream'>>();
  function row(l: LogLine): Row {
    let r = cache.get(l.line);
    if (!r) {
      const segs = parseColors(l.line);
      const text = segs ? segs.map((x) => x.text).join('') : l.line;
      r = {
        text,
        segs: segs && segs.some((x) => x.style) ? segs : null,
        warn: /\bWARN/.test(text),
        err: /\b(ERROR|FATAL)|Exception/.test(text)
      };
      if (cache.size > 20000) cache.clear();
      cache.set(l.line, r);
    }
    return { ...r, stream: l.stream };
  }

  const rows = $derived(logs.map(row));
  const shown = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return rows.filter((r) => {
      if (level === 'error' && !r.err) return false;
      if (level === 'warn' && !r.warn && !r.err) return false;
      return !q || r.text.toLowerCase().includes(q);
    });
  });
  const counts = $derived({ warn: rows.filter((r) => r.warn).length, error: rows.filter((r) => r.err).length });

  $effect(() => {
    shown.length;
    if (follow && el) tick().then(() => el && (el.scrollTop = el.scrollHeight));
  });

  // Scrolling up pauses auto-scroll; reaching the bottom turns it back on.
  function onscroll() {
    if (!el) return;
    follow = el.scrollHeight - el.scrollTop - el.clientHeight < 24;
  }

  function jumpToEnd() {
    follow = true;
    el?.scrollTo({ top: el.scrollHeight });
  }

  async function copy() {
    await navigator.clipboard.writeText(shown.map((r) => r.text).join('\n'));
    copied = true;
    setTimeout(() => (copied = false), 1500);
  }

  function kill() {
    if (!confirmKill) {
      confirmKill = true;
      clearTimeout(killTimer);
      killTimer = setTimeout(() => (confirmKill = false), 3000);
      return;
    }
    confirmKill = false;
    api.stop(instanceId);
  }
</script>

<div class="console" class:windowed>
  <div class="bar">
    <span class="status" class:on={running}><i></i>{running ? 'Running' : 'Stopped'}</span>

    <label class="search">
      <Search size={14} />
      <input placeholder="Filter output" bind:value={query} spellcheck="false" />
    </label>

    <div class="seg" role="group" aria-label="Show">
      <button class:active={level === 'all'} onclick={() => (level = 'all')}>All</button>
      <button class:active={level === 'warn'} onclick={() => (level = 'warn')}>Warnings{#if counts.warn}<span class="n warn">{counts.warn}</span>{/if}</button>
      <button class:active={level === 'error'} onclick={() => (level = 'error')}>Errors{#if counts.error}<span class="n err">{counts.error}</span>{/if}</button>
    </div>

    <div class="spacer"></div>

    <button class="btn ghost icon" class:on={wrap} title="Wrap long lines" aria-label="Wrap long lines" onclick={() => (wrap = !wrap)}><WrapText size={15} /></button>
    <button class="btn ghost icon" title={copied ? 'Copied' : 'Copy log'} aria-label="Copy log" disabled={!shown.length} onclick={copy}>
      {#if copied}<Check size={15} />{:else}<Copy size={15} />{/if}
    </button>
    <button class="btn ghost icon" title="Clear" aria-label="Clear" disabled={!logs.length} onclick={onclear}><Eraser size={15} /></button>
    <button class="btn ghost icon" title="Open instance folder" aria-label="Open instance folder" onclick={() => api.openFolder(instanceId)}><FolderOpen size={15} /></button>
    {#if windowed}
      <button class="btn ghost icon" class:on={pinned} title={pinned ? 'Stop keeping on top' : 'Keep on top'} aria-label="Keep on top" onclick={onpin}>
        {#if pinned}<PinOff size={15} />{:else}<Pin size={15} />{/if}
      </button>
    {:else}
      <button class="btn ghost icon" title="Open in its own window" aria-label="Pop out" onclick={() => api.openConsoleWindow(instanceId)}><SquareArrowOutUpRight size={15} /></button>
    {/if}

    {#if running}
      <button class="btn kill" class:confirm={confirmKill} onclick={kill}><OctagonX size={15} /> {confirmKill ? 'Click to confirm' : 'Kill'}</button>
    {:else if windowed && onplay}
      <button class="btn primary" onclick={onplay}><Play size={15} /> Play</button>
    {/if}
  </div>

  <div class="log" class:nowrap={!wrap} bind:this={el} {onscroll}>
    {#each shown as l, i (i)}
      <!-- One line on purpose: lines keep their whitespace. -->
      <div class="line {l.stream}" class:warn={l.warn} class:err={l.err}>{#if l.segs}{#each l.segs as seg, j (j)}<span style={seg.style || undefined}>{seg.text}</span>{/each}{:else}{l.text}{/if}</div>
    {:else}
      <div class="empty">
        {#if logs.length}No lines match.{:else if running}Waiting for output…{:else}Game output will appear here when you press Play.{/if}
      </div>
    {/each}
  </div>

  {#if !follow && shown.length}
    <button class="btn end" onclick={jumpToEnd}><ArrowDownToLine size={14} /> Jump to latest</button>
  {/if}
</div>

<style>
  .console {
    position: relative;
    display: flex;
    flex-direction: column;
    height: calc(100vh - 470px);
    min-height: 280px;
    border-radius: var(--radius);
    border: 1px solid var(--stroke);
    overflow: hidden;
    background: rgba(5, 6, 10, 0.6);
    backdrop-filter: blur(14px);
  }
  .console.windowed {
    height: 100vh;
    min-height: 0;
    border: none;
    border-radius: 0;
    background: #07080d;
    backdrop-filter: none;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 8px 6px 12px;
    border-bottom: 1px solid var(--stroke);
    flex-wrap: wrap;
  }
  .status {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    font-size: 12px;
    color: var(--faint);
    margin-right: 4px;
    min-width: 70px;
  }
  .status i {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--faint);
  }
  .status.on {
    color: #86efac;
  }
  .status.on i {
    background: #4ade80;
    box-shadow: 0 0 8px #4ade80;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 30px;
    padding: 0 10px;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid var(--stroke);
    color: var(--faint);
    width: 190px;
  }
  .search input {
    flex: 1;
    min-width: 0;
    background: none;
    border: none;
    outline: none;
    color: var(--text);
    font: inherit;
    font-size: 12.5px;
  }
  .seg {
    display: flex;
    padding: 2px;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid var(--stroke);
  }
  .seg button {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 24px;
    padding: 0 9px;
    border: none;
    border-radius: 6px;
    background: none;
    color: var(--muted);
    font: inherit;
    font-size: 12px;
    cursor: pointer;
  }
  .seg button.active {
    background: rgba(255, 255, 255, 0.09);
    color: var(--text);
  }
  .n {
    font-size: 10.5px;
    padding: 0 5px;
    border-radius: 99px;
    line-height: 16px;
  }
  .n.warn {
    background: rgba(252, 211, 77, 0.15);
    color: #fcd34d;
  }
  .n.err {
    background: rgba(253, 164, 175, 0.15);
    color: #fda4af;
  }
  .spacer {
    flex: 1;
  }
  .icon.on {
    color: var(--accent);
  }
  .kill {
    color: #fda4af;
    border-color: rgba(253, 164, 175, 0.3);
    margin-left: 4px;
  }
  .kill:hover,
  .kill.confirm {
    background: rgba(239, 68, 68, 0.18);
    border-color: rgba(239, 68, 68, 0.55);
    color: #fecaca;
  }
  .primary {
    margin-left: 4px;
  }
  .log {
    flex: 1;
    overflow: auto;
    padding: 12px 16px;
    font-family: 'JetBrains Mono', 'Cascadia Code', Consolas, monospace;
    font-size: 12px;
    line-height: 1.6;
    user-select: text;
  }
  .line {
    white-space: pre-wrap;
    word-break: break-all;
    color: #c4c9d8;
  }
  .nowrap .line {
    white-space: pre;
    word-break: normal;
  }
  .line.launcher {
    color: var(--accent-2);
  }
  .line.warn {
    color: #fcd34d;
  }
  .line.err {
    color: #fda4af;
  }
  .empty {
    color: var(--faint);
  }
  .end {
    position: absolute;
    right: 18px;
    bottom: 16px;
    font-size: 12px;
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.5);
  }
</style>
