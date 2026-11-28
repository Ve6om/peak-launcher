<script lang="ts">
  import { t } from '$lib/i18n.svelte';
  // Shown when a game closes with an error: what went wrong, in plain words,
  // and a button that fixes it when Peak can.
  import { Wrench, FileText, Terminal, Power, Undo2, LoaderCircle, Search, Share2 } from '@lucide/svelte';
  import Modal from './Modal.svelte';
  import ModVersionDialog from './ModVersionDialog.svelte';
  import { api } from '$lib/api';
  import { app, go, toast, errorText, refreshInstances, requestInstall, isRunning, shareLog } from '$lib/state.svelte';
  import { changePackVersion } from '$lib/manualPack.svelte';
  import type { CrashDiagnosis, InstalledItem } from '$lib/types';

  let { instanceId, diagnosis, onclose }: { instanceId: string; diagnosis: CrashDiagnosis; onclose: () => void } = $props();

  const inst = $derived(app.instances.find((i) => i.id === instanceId));
  const fix = $derived(diagnosis.fix);
  /** Still running (e.g. Fabric's error window is open): its mod files can't be changed yet. */
  const open = $derived(isRunning(instanceId));
  let busy = $state(false);
  let changing = $state<InstalledItem | null>(null);
  let undo = $state<{ count: number; when: string } | null>(null);

  // Offer to undo the last update too, when there's one: it's often the cause.
  $effect(() => {
    api.contentUndoInfo(instanceId).then((u) => (undo = u)).catch(() => {});
  });

  const fixLabel = $derived.by(() => {
    if (!fix) return '';
    switch (fix.action) {
      case 'installMod':
        return t('Install {name}', { name: fix.name });
      case 'memory':
        return t('Give it {x} GB', { x: +(fix.mb / 1024).toFixed(1) });
      case 'java':
        return t('Use Java {major} automatically', { major: fix.major });
      case 'changeVersion':
        return t('Pick another version of {name}', { name: fix.name });
      case 'disableMod':
        return t('Turn off {name}', { name: fix.name });
    }
  });

  async function done(message: string) {
    await refreshInstances();
    toast(t('{message}. Press Play to try again.', { message }), 'success');
    onclose();
  }

  async function modFile(fileName: string) {
    const items = await api.contentList(instanceId, 'mod');
    return items.find((i) => i.fileName === fileName || i.fileName === `${fileName}.disabled` || `${i.fileName}.disabled` === fileName) ?? null;
  }

  async function turnOff(fileName: string, name: string) {
    await api.contentSetEnabled(instanceId, 'mod', fileName, false);
    await done(t('Turned off {name}', { name }));
  }

  async function apply() {
    if (!fix || !inst) return;
    busy = true;
    try {
      if (fix.action === 'installMod') {
        const pid = await api.crashFindMod(instanceId, fix.modId, fix.name);
        if (!pid) {
          toast(t('Couldn\'t find {name} for this Minecraft version and loader. Search for it under Add mods.', { name: fix.name }), 'error');
          onclose();
          go('instance', instanceId, 'content');
          return;
        }
        if (await requestInstall(instanceId, 'mod', pid)) await done(t('Installed {name}', { name: fix.name }));
      } else if (fix.action === 'memory') {
        await api.updateInstance({ ...$state.snapshot(inst), memoryMb: fix.mb });
        await done(t('{name} now has {x} GB', { name: inst.name, x: +(fix.mb / 1024).toFixed(1) }));
      } else if (fix.action === 'java') {
        await api.updateInstance({ ...$state.snapshot(inst), javaPath: null, javaRuntime: null });
        await done(t('{name} now picks its Java automatically', { name: inst.name }));
      } else if (fix.action === 'disableMod') {
        await turnOff(fix.fileName, fix.name);
      } else if (fix.action === 'changeVersion') {
        const item = await modFile(fix.fileName);
        if (item?.projectId) changing = item;
        else await turnOff(fix.fileName, fix.name);
      }
    } catch (e) {
      toast(errorText(e), 'error');
    } finally {
      busy = false;
    }
  }

  const previous = $derived(inst?.pack?.previousVersionId ? (inst.pack.previousVersionNumber ?? 'the previous version') : null);

  async function packBack() {
    if (!inst?.pack?.previousVersionId) return;
    busy = true;
    try {
      const started = await changePackVersion(inst, inst.pack.previousVersionId);
      onclose();
      if (started) {
        await refreshInstances();
        go('instance', inst.id);
        toast(t('Going back to {previous}. Press Play when it\'s done.', { previous }), 'success');
      }
    } catch (e) {
      toast(errorText(e), 'error');
    } finally {
      busy = false;
    }
  }

  async function undoUpdate() {
    busy = true;
    try {
      const n = await api.contentUndo(instanceId);
      await done((n === 1 ? t('Put back {n} file from before the update', { n }) : t('Put back {n} files from before the update', { n })));
    } catch (e) {
      toast(errorText(e), 'error');
    } finally {
      busy = false;
    }
  }

  let confirmShare = $state(false);
  let sharing = $state(false);
  async function share() {
    if (!confirmShare) {
      confirmShare = true;
      setTimeout(() => (confirmShare = false), 4000);
      return;
    }
    confirmShare = false;
    sharing = true;
    await shareLog(instanceId);
    sharing = false;
  }

  function findMod() {
    onclose();
    app.modSearchFor = { instanceId, guess: null };
  }

  function openConsole() {
    onclose();
    go('instance', instanceId, 'console');
  }
</script>

{#if changing}
  {@const item = changing}
  <ModVersionDialog
    {instanceId}
    kind="mod"
    {item}
    onclose={() => (changing = null)}
    onpick={async (v) => {
      changing = null;
      if (await requestInstall(instanceId, 'mod', item.projectId!, v)) await done(`Switched ${item.title} to another version`);
    }}
  />
{:else}
  <Modal title={diagnosis.title} subtitle={inst ? t('{name} closed with an error', { name: inst.name }) : t('The game closed with an error')} width={560} {onclose}>
    <p class="detail">{diagnosis.detail}</p>
    {#if open}
      <p class="hint">{t('The game is still open behind its error window. Close that window, then use the button below.')}</p>
    {/if}

    {#if diagnosis.excerpt.length}
      <pre class="excerpt">{diagnosis.excerpt.join('\n')}</pre>
    {/if}

    {#if undo}
      <p class="hint">{(undo.count === 1 ? t('You updated {count} file in this instance before this crash. Undoing that update often fixes it.', { count: undo.count }) : t('You updated {count} files in this instance before this crash. Undoing that update often fixes it.', { count: undo.count }))}</p>
    {/if}

    <div class="actions">
      {#if diagnosis.report}
        <button class="btn ghost" onclick={() => api.openCrashReport(diagnosis.report!)}><FileText size={14} /> {t('Crash report')}</button>
      {/if}
      <button class="btn ghost" onclick={openConsole}><Terminal size={14} /> {t('Console')}</button>
      <button
        class="btn ghost"
        title={confirmShare ? t('Click again to upload. The link is public; Peak hides your user name, Minecraft name and sign-in token first.') : t('Upload the log and copy a link to send when asking for help')}
        disabled={sharing}
        onclick={share}
      >
        {#if sharing}<LoaderCircle size={14} class="spin" />{:else}<Share2 size={14} />{/if}
        {confirmShare ? t('Click again to upload') : t('Share log')}
      </button>
      {#if inst && inst.loader !== 'vanilla'}
        <button
          class="btn"
          class:primary={!fix && !undo && !previous}
          title={t('Turn mods off in batches until the one causing this is found')}
          disabled={busy || open}
          onclick={findMod}
        >
          <Search size={14} /> {t('Find the broken mod')}
        </button>
      {/if}
      <span class="spacer"></span>
      {#if fix && (fix.action === 'changeVersion' || fix.action === 'disableMod')}
        {#if fix.action === 'changeVersion'}
          <button class="btn" disabled={busy || open} onclick={() => turnOff(fix.fileName, fix.name)}><Power size={14} /> {t('Turn it off')}</button>
        {/if}
      {/if}
      {#if undo}
        <button class="btn" class:primary={!fix} disabled={busy || open} onclick={undoUpdate}><Undo2 size={14} /> {t('Undo the update')}</button>
      {:else if previous && !fix}
        <button class="btn primary" disabled={busy || open} onclick={packBack}><Undo2 size={14} /> {t('Go back to {previous}', { previous })}</button>
      {/if}
      {#if fix}
        <button class="btn primary" disabled={busy || open} onclick={apply}>
          {#if busy}<LoaderCircle size={14} class="spin" />{:else if fix.action === 'installMod'}<Search size={14} />{:else}<Wrench size={14} />{/if}
          {fixLabel}
        </button>
      {:else if !undo && !previous}
        <button class="btn" class:primary={inst?.loader === 'vanilla'} onclick={onclose}>{t('Close')}</button>
      {/if}
    </div>
  </Modal>
{/if}

<style>
  .detail {
    margin: 0;
    font-size: 13.5px;
    line-height: 1.5;
  }
  .excerpt {
    margin: 12px 0 0;
    padding: 10px 12px;
    max-height: 140px;
    overflow: auto;
    border-radius: 10px;
    background: rgba(0, 0, 0, 0.3);
    border: 1px solid var(--stroke);
    font-size: 11.5px;
    line-height: 1.5;
    white-space: pre-wrap;
    word-break: break-word;
    color: var(--muted);
  }
  .hint {
    margin: 12px 0 0;
    font-size: 12.5px;
    color: #fde68a;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin-top: 16px;
  }
  .spacer {
    flex: 1;
  }
</style>
