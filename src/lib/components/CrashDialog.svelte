<script lang="ts">
  // Shown when a game closes with an error: what went wrong, in plain words,
  // and a button that fixes it when Peak can.
  import { Wrench, FileText, Terminal, Power, Undo2, LoaderCircle, Search } from '@lucide/svelte';
  import Modal from './Modal.svelte';
  import ModVersionDialog from './ModVersionDialog.svelte';
  import { api } from '$lib/api';
  import { app, go, toast, errorText, refreshInstances, requestInstall, isRunning } from '$lib/state.svelte';
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
        return `Install ${fix.name}`;
      case 'memory':
        return `Give it ${+(fix.mb / 1024).toFixed(1)} GB`;
      case 'java':
        return `Use Java ${fix.major} automatically`;
      case 'changeVersion':
        return `Pick another version of ${fix.name}`;
      case 'disableMod':
        return `Turn off ${fix.name}`;
    }
  });

  async function done(message: string) {
    await refreshInstances();
    toast(`${message}. Press Play to try again.`, 'success');
    onclose();
  }

  async function modFile(fileName: string) {
    const items = await api.contentList(instanceId, 'mod');
    return items.find((i) => i.fileName === fileName || i.fileName === `${fileName}.disabled` || `${i.fileName}.disabled` === fileName) ?? null;
  }

  async function turnOff(fileName: string, name: string) {
    await api.contentSetEnabled(instanceId, 'mod', fileName, false);
    await done(`Turned off ${name}`);
  }

  async function apply() {
    if (!fix || !inst) return;
    busy = true;
    try {
      if (fix.action === 'installMod') {
        const pid = await api.crashFindMod(instanceId, fix.modId, fix.name);
        if (!pid) {
          toast(`Couldn't find ${fix.name} for this Minecraft version and loader. Search for it under Add mods.`, 'error');
          onclose();
          go('instance', instanceId, 'content');
          return;
        }
        if (await requestInstall(instanceId, 'mod', pid)) await done(`Installed ${fix.name}`);
      } else if (fix.action === 'memory') {
        await api.updateInstance({ ...$state.snapshot(inst), memoryMb: fix.mb });
        await done(`${inst.name} now has ${+(fix.mb / 1024).toFixed(1)} GB`);
      } else if (fix.action === 'java') {
        await api.updateInstance({ ...$state.snapshot(inst), javaPath: null, javaRuntime: null });
        await done(`${inst.name} now picks its Java automatically`);
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
        toast(`Going back to ${previous}. Press Play when it's done.`, 'success');
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
      await done(`Put back ${n} file${n === 1 ? '' : 's'} from before the update`);
    } catch (e) {
      toast(errorText(e), 'error');
    } finally {
      busy = false;
    }
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
  <Modal title={diagnosis.title} subtitle={inst ? `${inst.name} closed with an error` : 'The game closed with an error'} width={560} {onclose}>
    <p class="detail">{diagnosis.detail}</p>
    {#if open}
      <p class="hint">The game is still open behind its error window. Close that window, then use the button below.</p>
    {/if}

    {#if diagnosis.excerpt.length}
      <pre class="excerpt">{diagnosis.excerpt.join('\n')}</pre>
    {/if}

    {#if undo}
      <p class="hint">You updated {undo.count} file{undo.count === 1 ? '' : 's'} in this instance before this crash. Undoing that update often fixes it.</p>
    {/if}

    <div class="actions">
      {#if diagnosis.report}
        <button class="btn ghost" onclick={() => api.openCrashReport(diagnosis.report!)}><FileText size={14} /> Crash report</button>
      {/if}
      <button class="btn ghost" onclick={openConsole}><Terminal size={14} /> Console</button>
      {#if inst && inst.loader !== 'vanilla'}
        <button
          class="btn"
          class:primary={!fix && !undo && !previous}
          title="Turn mods off in batches until the one causing this is found"
          disabled={busy || open}
          onclick={findMod}
        >
          <Search size={14} /> Find the broken mod
        </button>
      {/if}
      <span class="spacer"></span>
      {#if fix && (fix.action === 'changeVersion' || fix.action === 'disableMod')}
        {#if fix.action === 'changeVersion'}
          <button class="btn" disabled={busy || open} onclick={() => turnOff(fix.fileName, fix.name)}><Power size={14} /> Turn it off</button>
        {/if}
      {/if}
      {#if undo}
        <button class="btn" class:primary={!fix} disabled={busy || open} onclick={undoUpdate}><Undo2 size={14} /> Undo the update</button>
      {:else if previous && !fix}
        <button class="btn primary" disabled={busy || open} onclick={packBack}><Undo2 size={14} /> Go back to {previous}</button>
      {/if}
      {#if fix}
        <button class="btn primary" disabled={busy || open} onclick={apply}>
          {#if busy}<LoaderCircle size={14} class="spin" />{:else if fix.action === 'installMod'}<Search size={14} />{:else}<Wrench size={14} />{/if}
          {fixLabel}
        </button>
      {:else if !undo && !previous}
        <button class="btn" class:primary={inst?.loader === 'vanilla'} onclick={onclose}>Close</button>
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
