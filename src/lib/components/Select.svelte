<script lang="ts" module>
  export interface SelectOption {
    value: string | null;
    label: string;
    /** Heading this option sits under (consecutive options with the same group share one). */
    group?: string;
    disabled?: boolean;
    /** Small grey text after the label. */
    hint?: string;
    /** A small picture (instance logo) before the label. */
    icon?: string | null;
    /** Shown instead of `icon` when there's no picture: that mod loader's logo. */
    loader?: string;
  }
</script>

<script lang="ts">
  // A dropdown in Peak's own style, used instead of the browser's <select>
  // (whose list is drawn by the system and ignores the app's look). Works
  // with the keyboard like a select: arrows, Enter, Escape, Home/End and
  // typing the start of an option.
  import { tick } from 'svelte';
  import { ChevronDown, Check } from '@lucide/svelte';
  import LoaderIcon from './LoaderIcon.svelte';

  let {
    value = $bindable(),
    options,
    placeholder = 'Choose…',
    disabled = false,
    small = false,
    /** No box of its own (sits inside another control). */
    bare = false,
    ariaLabel,
    title,
    class: klass = '',
    style = '',
    onchange
  }: {
    value: string | null | undefined;
    options: SelectOption[];
    placeholder?: string;
    disabled?: boolean;
    small?: boolean;
    bare?: boolean;
    ariaLabel?: string;
    title?: string;
    class?: string;
    /** Inline size tweaks (width, flex) from the page using it. */
    style?: string;
    onchange?: (value: string | null) => void;
  } = $props();

  const id = `sel-${Math.random().toString(36).slice(2, 9)}`;
  let open = $state(false);
  let active = $state(-1);
  let trigger: HTMLButtonElement | undefined = $state();
  let list: HTMLDivElement | undefined = $state();
  let pos = $state({ left: 0, top: 0, width: 0, maxHeight: 320, up: false });

  const current = $derived(options.find((o) => o.value === (value ?? null)) ?? options.find((o) => o.value === value));
  const enabled = (i: number) => i >= 0 && i < options.length && !options[i].disabled;

  function place() {
    if (!trigger) return;
    // Line up with the visible box: its own, or the one it sits in when bare.
    const box = (bare && trigger.parentElement) || trigger;
    const r = box.getBoundingClientRect();
    const below = window.innerHeight - r.bottom - 12;
    const above = r.top - 12;
    const up = below < 200 && above > below;
    const maxHeight = Math.max(120, Math.min(340, up ? above : below));
    const width = Math.max(r.width, 180);
    const left = Math.min(r.left, window.innerWidth - width - 8);
    pos = { left, top: up ? r.top - 6 : r.bottom + 6, width, maxHeight, up };
  }

  async function show() {
    if (disabled || !options.length) return;
    place();
    open = true;
    active = Math.max(0, options.findIndex((o) => o.value === (value ?? null)));
    await tick();
    list?.querySelector<HTMLElement>(`[data-i="${active}"]`)?.scrollIntoView({ block: 'nearest' });
  }

  function hide(focus = true) {
    open = false;
    if (focus) trigger?.focus();
  }

  function choose(i: number) {
    if (!enabled(i)) return;
    const v = options[i].value;
    hide();
    if (v !== value) {
      value = v;
      onchange?.(v);
    }
  }

  function move(step: number) {
    let i = active;
    for (let n = 0; n < options.length; n++) {
      i = (i + step + options.length) % options.length;
      if (enabled(i)) break;
    }
    active = i;
    list?.querySelector<HTMLElement>(`[data-i="${i}"]`)?.scrollIntoView({ block: 'nearest' });
  }

  let typed = '';
  let typedTimer: ReturnType<typeof setTimeout> | undefined;
  function typeAhead(key: string) {
    typed += key.toLowerCase();
    clearTimeout(typedTimer);
    typedTimer = setTimeout(() => (typed = ''), 700);
    const start = open ? active + 1 : 0;
    const order = [...options.keys()].map((k) => (k + start) % options.length);
    const hit = order.find((i) => enabled(i) && options[i].label.toLowerCase().startsWith(typed));
    if (hit === undefined) return;
    if (open) {
      active = hit;
      list?.querySelector<HTMLElement>(`[data-i="${hit}"]`)?.scrollIntoView({ block: 'nearest' });
    } else choose(hit);
  }

  function onkey(e: KeyboardEvent) {
    if (disabled) return;
    if (!open) {
      if (['ArrowDown', 'ArrowUp', 'Enter', ' '].includes(e.key)) {
        e.preventDefault();
        show();
      } else if (e.key.length === 1 && /\S/.test(e.key)) typeAhead(e.key);
      return;
    }
    if (e.key === 'ArrowDown') (e.preventDefault(), move(1));
    else if (e.key === 'ArrowUp') (e.preventDefault(), move(-1));
    else if (e.key === 'Home') (e.preventDefault(), (active = -1), move(1));
    else if (e.key === 'End') (e.preventDefault(), (active = options.length), move(-1));
    else if (e.key === 'Enter' || e.key === ' ') (e.preventDefault(), choose(active));
    else if (e.key === 'Escape') (e.preventDefault(), e.stopPropagation(), hide());
    else if (e.key === 'Tab') hide(false);
    else if (e.key.length === 1 && /\S/.test(e.key)) typeAhead(e.key);
  }

  function outside(e: PointerEvent) {
    const t = e.target as Node;
    if (open && !trigger?.contains(t) && !list?.contains(t)) hide(false);
  }

  // Attached to the window itself, so a scrolling panel or a dialog can't clip it.
  function toBody(node: HTMLElement) {
    document.body.appendChild(node);
    return { destroy: () => node.remove() };
  }
</script>

<svelte:window
  onpointerdown={outside}
  onresize={() => open && hide(false)}
  onscrollcapture={(e) => open && !list?.contains(e.target as Node) && hide(false)}
/>

<button
  bind:this={trigger}
  type="button"
  class="sel {klass}"
  {style}
  class:input={!bare}
  class:small
  class:bare
  class:open
  {disabled}
  {title}
  aria-label={ariaLabel}
  aria-haspopup="listbox"
  aria-expanded={open}
  aria-controls={open ? id : undefined}
  onclick={() => (open ? hide() : show())}
  onkeydown={onkey}
>
  {#if current?.icon}<img class="ico" src={current.icon} alt="" />{:else if current?.loader}<LoaderIcon loader={current.loader} size={18} />{/if}
  <span class="text" class:placeholder={!current}>{current?.label ?? placeholder}</span>
  <ChevronDown size={14} class="chev" />
</button>

{#if open}
  <div
    use:toBody
    bind:this={list}
    {id}
    class="sel-list glass"
    class:up={pos.up}
    role="listbox"
    tabindex="-1"
    aria-label={ariaLabel}
    style:left="{pos.left}px"
    style:top="{pos.top}px"
    style:min-width="{pos.width}px"
    style:max-height="{pos.maxHeight}px"
  >
    {#each options as o, i (i)}
      {#if o.group && o.group !== options[i - 1]?.group}
        <div class="group">{o.group}</div>
      {/if}
      <div
        class="opt"
        class:active={i === active}
        class:selected={o.value === (value ?? null)}
        class:disabled={o.disabled}
        role="option"
        tabindex="-1"
        aria-selected={o.value === (value ?? null)}
        aria-disabled={o.disabled}
        data-i={i}
        onpointermove={() => !o.disabled && (active = i)}
        onclick={() => choose(i)}
        onkeydown={() => {}}
      >
        {#if o.icon}<img class="ico" src={o.icon} alt="" />{:else if o.loader}<LoaderIcon loader={o.loader} size={18} />{/if}
        <span class="opt-text">{o.label}{#if o.hint}<em>{o.hint}</em>{/if}</span>
        {#if o.value === (value ?? null)}<Check size={14} class="tick" />{/if}
      </div>
    {/each}
  </div>
{/if}

<style>
  .sel {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    text-align: left;
    cursor: pointer;
    color: var(--text);
    font: inherit;
    font-size: 13.5px;
    min-width: 0;
  }
  .sel:disabled {
    cursor: default;
    opacity: 0.55;
  }
  .sel.small {
    height: 36px;
    padding: 0 10px;
    font-size: 13px;
  }
  .sel.bare {
    height: auto;
    padding: 0;
    border: 0;
    background: none;
    font-size: 12.5px;
  }
  .sel.open:not(.bare) {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 16%, transparent);
  }
  .text {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .placeholder {
    color: var(--faint);
  }
  .sel :global(.chev) {
    flex-shrink: 0;
    color: var(--muted);
    transition: transform 0.15s;
  }
  .sel.open :global(.chev) {
    transform: rotate(180deg);
  }
  .ico {
    width: 18px;
    height: 18px;
    border-radius: 4px;
    object-fit: cover;
    flex-shrink: 0;
  }

  .sel-list {
    position: fixed;
    z-index: 400;
    max-width: min(460px, calc(100vw - 16px));
    overflow-y: auto;
    padding: 5px;
    border-radius: 12px;
    background: rgba(18, 20, 30, 0.97);
    border: 1px solid var(--stroke);
    box-shadow: 0 16px 40px rgba(0, 0, 0, 0.5);
    animation: sel-in 0.12s ease-out;
  }
  .sel-list.up {
    transform: translateY(-100%);
    animation-name: sel-in-up;
  }
  @keyframes sel-in {
    from {
      opacity: 0;
      margin-top: -4px;
    }
  }
  @keyframes sel-in-up {
    from {
      opacity: 0;
      margin-top: 4px;
    }
  }
  .group {
    padding: 8px 10px 4px;
    font-size: 10.5px;
    font-weight: 650;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--faint);
  }
  .opt {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 7px 10px;
    border-radius: 8px;
    font-size: 13px;
    color: var(--text);
    cursor: pointer;
    white-space: nowrap;
  }
  .opt.active {
    background: rgba(255, 255, 255, 0.06);
  }
  .opt.selected {
    color: var(--accent);
    font-weight: 600;
  }
  .opt.disabled {
    opacity: 0.45;
    cursor: default;
  }
  .opt-text {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .opt-text em {
    margin-left: 6px;
    font-style: normal;
    font-weight: 400;
    color: var(--faint);
  }
  .opt :global(.tick) {
    flex-shrink: 0;
  }
</style>
