<script lang="ts">
  import { t } from '$lib/i18n.svelte';
  // Confirm a mod's required dependencies before installing them.
  import { Check, LoaderCircle, TriangleAlert, Link2 } from '@lucide/svelte';
  import Modal from './Modal.svelte';
  import ProjectIcon from './ProjectIcon.svelte';
  import { pending, installItems } from '$lib/state.svelte';

  const plan = $derived(pending.plan!);
  const root = $derived(plan.items.find((i) => i.isRoot) ?? plan.items[0]);
  const deps = $derived(plan.items.filter((i) => i !== root));
  let skipped = $state(new Set<string>());
  let busy = $state(false);

  function toggle(id: string) {
    const next = new Set(skipped);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    skipped = next;
  }

  function close(installed: boolean) {
    plan.resolve(installed);
    pending.plan = null;
  }

  async function confirm() {
    busy = true;
    const ok = await installItems(
      plan.instanceId,
      plan.items.filter((i) => !skipped.has(i.projectId))
    );
    busy = false;
    close(ok);
  }

  const size = (b: number) => (b > 1048576 ? t('{x} MB', { x: (b / 1048576).toFixed(1) }) : t('{n} KB', { n: Math.max(1, Math.round(b / 1024)) }));
</script>

<Modal
  title={t('Install {title}?', { title: root.title })}
  subtitle={deps.length > 1 ? t("{title} needs {length} other projects that aren't installed yet.", { title: root.title, length: deps.length }) : t("{title} needs 1 other project that isn't installed yet.", { title: root.title })}
  width={520}
  onclose={() => close(false)}
>
  <div class="list">
    <div class="row root">
      <ProjectIcon url={root.iconUrl} title={root.title} size={36} />
      <div class="text">
        <b>{root.title}</b>
        <span>{root.versionNumber} · {size(root.size)}{root.manual ? t(' · Download in browser') : ''}</span>
      </div>
      <span class="chip">{t('Requested')}</span>
    </div>

    <span class="label">{t('Required dependencies')}</span>
    {#each deps as d (d.projectId)}
      <button class="row dep" class:off={skipped.has(d.projectId)} onclick={() => toggle(d.projectId)}>
        <span class="box">{#if !skipped.has(d.projectId)}<Check size={12} strokeWidth={3} />{/if}</span>
        <ProjectIcon url={d.iconUrl} title={d.title} size={32} />
        <div class="text">
          <b>{d.title}</b>
          <span class="reason"><Link2 size={11} /> {t('Required by {requiredBy}', { requiredBy: d.requiredBy })}{d.manual ? ` · ${t('Download in browser')}` : ''}</span>
        </div>
        <span class="ver">{d.versionNumber}</span>
      </button>
    {/each}
  </div>

  {#if skipped.size}
    <p class="warn"><TriangleAlert size={14} /> {t('Mods usually crash the game without their required dependencies.')}</p>
  {/if}

  <footer>
    <button class="btn ghost" onclick={() => close(false)}>{t('Cancel')}</button>
    <button class="btn primary" disabled={busy} onclick={confirm}>
      {#if busy}<LoaderCircle size={16} class="spin" />{/if}
      {(plan.items.length - skipped.size === 1 ? t('Install {x} item', { x: plan.items.length - skipped.size }) : t('Install {x} items', { x: plan.items.length - skipped.size }))}
    </button>
  </footer>
</Modal>

<style>
  .list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-height: 50vh;
    overflow-y: auto;
  }
  .label {
    margin: 10px 0 2px 2px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 9px 12px;
    border-radius: 12px;
    text-align: left;
    border: 1px solid var(--stroke);
    background: rgba(255, 255, 255, 0.03);
    transition: opacity 0.2s, border-color 0.2s;
  }
  .row.root {
    border-color: color-mix(in srgb, var(--accent) 35%, transparent);
    background: color-mix(in srgb, var(--accent) 10%, transparent);
  }
  .dep:hover {
    border-color: var(--stroke-strong);
  }
  .dep.off {
    opacity: 0.5;
  }
  .box {
    width: 18px;
    height: 18px;
    flex-shrink: 0;
    display: grid;
    place-items: center;
    border-radius: 6px;
    border: 1.5px solid var(--stroke-strong);
    color: var(--on-accent);
    background: var(--gradient);
  }
  .dep.off .box {
    background: none;
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .text b {
    font-size: 13.5px;
    font-weight: 600;
  }
  .text span {
    font-size: 12px;
    color: var(--faint);
  }
  .reason {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: var(--accent-2) !important;
  }
  .ver {
    font-size: 11.5px;
    color: var(--faint);
    max-width: 130px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .warn {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 12px 0 0;
    font-size: 12.5px;
    color: #fde68a;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 18px;
  }
</style>
