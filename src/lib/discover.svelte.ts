// Joining servers from the Discover list. A modded server's pack may need to
// install first; Peak remembers the server and joins once the pack is ready,
// even if the player has moved to another page meanwhile.
import { api } from './api';
import { app, refreshInstances, toast, errorText, activeAccount, go, isBusy, isRunning } from './state.svelte';
import type { Instance, Listing } from './types';
import { playingOn } from './presence.svelte';
import { installPack } from './manualPack.svelte';

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

/**
 * Stop waiting to join a server for this instance, e.g. when its install
 * failed or still needs files downloaded by hand.
 */
export function dropPendingJoin(instanceId: string, reason: 'blocked' | 'failed') {
  const target = pendingJoins[instanceId];
  if (!target) return;
  delete pendingJoins[instanceId];
  if (reason === 'blocked') {
    toast(`Some files need downloading by hand first. When they're done, press Play on ${target.name} to join.`, 'info');
  }
}

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
  playingOn[inst.id] = serverName;
  try {
    await api.discoverJoin(inst.id, address, serverName);
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

/** What pressing Play on a listing will do, so the page can ask first. */
export type PlayPlan =
  | { kind: 'join' }
  | { kind: 'install' }
  | { kind: 'switch'; inst: Instance; from: string; to: string };

export function planFor(l: Listing): PlayPlan {
  if (!l.pack) return { kind: 'join' };
  const have = packInstance(l);
  if (!have) return { kind: 'install' };
  const wanted = l.pack.versionId;
  if (wanted && have.pack?.versionId && have.pack.versionId !== wanted && have.installed && !isBusy(have.id)) {
    return { kind: 'switch', inst: have, from: have.pack.versionNumber || 'yours', to: l.pack.versionName || 'the server\u2019s version' };
  }
  return { kind: 'join' };
}

/** Install the server's modpack (its version, if it names one) and join once it's ready. */
export async function installAndJoin(l: Listing): Promise<Instance | null> {
  if (!l.pack) return null;
  if (!activeAccount()) {
    toast('Add an account to start playing', 'info');
    go('accounts');
    return null;
  }
  try {
    const joinWhenReady = (inst: Instance) => {
      pendingJoins[inst.id] = { address: l.address, name: l.name };
    };
    const inst = await installPack({ projectId: l.pack.projectId, versionId: l.pack.versionId ?? null }, joinWhenReady);
    // null: the pack has to be downloaded by hand; the dialog takes over and
    // sets up the join once it installs the file.
    if (!inst) return null;
    await refreshInstances();
    joinWhenReady(inst);
    go('instance', inst.id);
    toast(`Installing ${l.pack.name}. Peak joins ${l.name} as soon as it's ready.`, 'info');
    return inst;
  } catch (e) {
    toast(errorText(e), 'error');
    return null;
  }
}

/** Switch an instance to the pack version the server uses, then join. */
export async function switchAndJoin(l: Listing, inst: Instance) {
  if (!l.pack?.versionId) return;
  try {
    await api.modpackChangeVersion(inst.id, l.pack.versionId);
    await refreshInstances();
    pendingJoins[inst.id] = { address: l.address, name: l.name };
    go('instance', inst.id);
    toast(`Switching ${inst.name} to ${l.pack.versionName || 'the server\u2019s version'}. Peak joins ${l.name} when it's done.`, 'info');
  } catch (e) {
    toast(errorText(e), 'error');
  }
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
      await installAndJoin(l);
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
