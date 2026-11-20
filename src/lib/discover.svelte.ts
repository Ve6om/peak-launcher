// Joining servers from the Discover list. A modded server's pack may need to
// install first; Peak remembers the server and joins once the pack is ready,
// even if the player has moved to another page meanwhile.
import { api } from './api';
import { app, refreshInstances, toast, errorText, activeAccount, go, isBusy, isRunning } from './state.svelte';
import type { Instance, Listing } from './types';

/** A server-list id from a peak:// link, waiting for Discover to open it. */
export const serverLink = $state<{ id: string | null }>({ id: null });

/** Open Servers > Discover on the server a peak:// link points to. */
export function openServerLink(id: string) {
  serverLink.id = id;
  go('servers');
}

/** Instance id -> the server to join once it has finished installing. */
export const pendingJoins = $state<Record<string, { address: string; name: string }>>({});

let watching = false;

/** Start watching installs (once). */
export function watchPendingJoins() {
  if (watching) return;
  watching = true;
  $effect.root(() => {
    $effect(() => {
      for (const [id, target] of Object.entries(pendingJoins)) {
        const inst = app.instances.find((i) => i.id === id);
        if (!inst) {
          delete pendingJoins[id];
          continue;
        }
        if (inst.installed && !isBusy(id) && !isRunning(id)) {
          delete pendingJoins[id];
          join(inst, target.address, target.name);
        }
      }
    });
  });
}

async function join(inst: Instance, address: string, serverName: string) {
  app.logs[inst.id] = [];
  try {
    await api.discoverJoin(inst.id, address);
  } catch (e) {
    delete app.progress[inst.id];
    toast(`${serverName}: ${errorText(e)}`, 'error');
  }
}

/** Instances that can join a vanilla listing, best match first. */
export function vanillaChoices(l: Listing): Instance[] {
  const score = (i: Instance) => (i.gameVersion === l.gameVersion ? 2 : 0) + (i.loader === 'vanilla' && !i.pack ? 1 : 0);
  return [...app.instances].filter((i) => !i.pack).sort((a, b) => score(b) - score(a));
}

/** The installed instance for a modded listing's pack, if there is one. */
export function packInstance(l: Listing): Instance | null {
  if (!l.pack) return null;
  return app.instances.find((i) => i.pack?.projectId === l.pack!.projectId) ?? null;
}

/**
 * Join a listing. Modded: use the pack's instance, installing the pack first
 * if needed. Vanilla: use `instanceId`, or make a new instance for the
 * server's version when it's "new".
 */
export async function playListing(l: Listing, instanceId: string | 'new' | null) {
  if (!activeAccount()) {
    toast('Add an account to start playing', 'info');
    go('accounts');
    return;
  }
  try {
    if (l.pack) {
      const have = packInstance(l);
      if (have) {
        if (isRunning(have.id)) return toast(`${have.name} is already running.`, 'info');
        if (!have.installed || isBusy(have.id)) {
          pendingJoins[have.id] = { address: l.address, name: l.name };
          return toast(`${have.name} is still installing. Peak joins ${l.name} when it's ready.`, 'info');
        }
        return join(have, l.address, l.name);
      }
      const inst = await api.modpackInstall({ projectId: l.pack.projectId });
      await refreshInstances();
      pendingJoins[inst.id] = { address: l.address, name: l.name };
      toast(`Installing ${l.pack.name}. Peak joins ${l.name} as soon as it's ready.`, 'info');
      return;
    }
    let inst = instanceId && instanceId !== 'new' ? app.instances.find((i) => i.id === instanceId) : null;
    if (!inst) {
      inst = await api.createInstance(l.name, l.gameVersion, 'vanilla', null);
      await refreshInstances();
      toast(`Made a new ${l.gameVersion} instance for ${l.name}`, 'success');
    }
    if (isRunning(inst.id)) return toast(`${inst.name} is already running.`, 'info');
    await join(inst, l.address, l.name);
  } catch (e) {
    toast(errorText(e), 'error');
  }
}
