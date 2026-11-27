// Joining servers from the Discover list. A modded server's pack may need to
// install first; Peak remembers the server and joins once the pack is ready,
// even if the player has moved to another page meanwhile.
import { api } from './api';
import { app, refreshInstances, toast, errorText, activeAccount, go, isBusy, isRunning } from './state.svelte';
import type { Instance, Listing } from './types';
import { playingOn } from './presence.svelte';
import { playingAt, invite, type JoinInvite } from './invite.svelte';
import { installPack } from './manualPack.svelte';

/** A server-list id from a peak:// link, waiting for Discover to open it. */
export const serverLink = $state<{ id: string | null }>({ id: null });

/** Open Servers > Discover on the server a peak:// link points to. */
export function openServerLink(id: string) {
  serverLink.id = id;
  go('servers');
}

/** A peak:// link: a server-list entry, or a friend's invite to a server. */
export type PeakLink = ({ kind: 'server'; id: string }) | ({ kind: 'play'; id: string }) | ({ kind: 'join' } & JoinInvite);

export function openLink(link: PeakLink) {
  if (link.kind === 'server') return openServerLink(link.id);
  if (link.kind === 'play') return playFromShortcut(link.id);
  const { kind: _, ...rest } = link;
  invite.current = rest;
}

/** A desktop shortcut started Peak (or was opened while it ran): play that instance. */
async function playFromShortcut(id: string) {
  const { app, play, go, toast, isRunning } = await import('./state.svelte');
  const inst = app.instances.find((i) => i.id === id);
  if (!inst) {
    toast("That shortcut's instance isn't in Peak any more.", 'error');
    return;
  }
  go('instance', inst.id);
  if (!isRunning(inst.id)) await play(inst);
}

/** Instance id -> the server to join once it has finished installing. */
export const pendingJoins = $state<Record<string, { address: string; name: string; pageUrl?: string | null }>>({});

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
          join(inst, target.address, target.name, target.pageUrl);
        }
      }
    });
  });
}

async function join(inst: Instance, address: string, serverName: string, pageUrl?: string | null) {
  app.logs[inst.id] = [];
  playingOn[inst.id] = serverName;
  playingAt[inst.id] = { address, name: serverName, pageUrl };
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

// ---- the same pack from the other site ----
// Many packs are on both CurseForge and Modrinth. When the name and version
// match, an instance from either site can join (e.g. Prominence II v4.1.9 is
// the same on both). Versions marked for one site, like "1.8.0 [CF]", are a
// different build, so they only ever match their own site (e.g. CobbleVerse).

const site = (projectId: string) => (projectId.startsWith('cf:') ? 'curseforge' : /^(ftb|technic):/.test(projectId) ? null : 'modrinth');

/** "Prominence™ II: Hasturian Era" and "Prominence II: Hasturian Era" are the same. */
export const sameName = (a: string, b: string) => {
  const n = (s: string) => s.toLowerCase().replace(/[^a-z0-9]/g, '');
  return !!n(a) && n(a) === n(b);
};

/**
 * The version numbers in a version name ("…Hasturian Era v4.1.9" -> "4.1.9";
 * "Pack 1.20.1 v3.0" -> "1.20.1,3.0"), or null if it's marked for one site.
 * All of them have to agree, so a Minecraft version in the name can't be
 * mistaken for the pack's.
 */
export function versionKey(v: string | null | undefined): string | null {
  if (!v) return null;
  if (/[[(]\s*(cf|curseforge|mr|modrinth)\s*[\])]|\b(curseforge|modrinth)\b|[-_ ]cf$/i.test(v)) return null;
  const nums = v.match(/\d+(?:\.\d+)+[a-z]?/gi);
  return nums ? nums.map((x) => x.toLowerCase()).join(',') : null;
}

/** Whether an instance from the other site has exactly the pack the listing wants. */
function sameFromOtherSite(l: Listing, i: Instance): boolean {
  const want = l.pack!;
  const from = site(want.projectId);
  const mine = i.pack?.projectId ? site(i.pack.projectId) : null;
  if (!from || !mine || from === mine || !sameName(want.name, i.pack!.name)) return false;
  const have = versionKey(i.pack!.versionNumber);
  // A server that names its version needs that exact one; one that doesn't
  // takes any build that isn't marked as being for one site only.
  return want.versionName ? !!have && have === versionKey(want.versionName) : !!have;
}

/**
 * The installed instance for a modded listing's pack, if there is one:
 * this site's pack at the right version, else the same pack and version from
 * the other site (`cross`), else this site's pack at another version.
 */
export function packMatch(l: Listing): { inst: Instance; cross: boolean } | null {
  if (!l.pack) return null;
  const own = app.instances.filter((i) => i.pack?.projectId === l.pack!.projectId);
  const want = l.pack.versionId;
  const exact = own.find((i) => !want || i.pack?.versionId === want);
  if (exact) return { inst: exact, cross: false };
  const other = app.instances.find((i) => sameFromOtherSite(l, i));
  if (other) return { inst: other, cross: true };
  return own[0] ? { inst: own[0], cross: false } : null;
}

/** The installed instance for a modded listing's pack, if there is one. */
export function packInstance(l: Listing): Instance | null {
  return packMatch(l)?.inst ?? null;
}

/** What pressing Play on a listing will do, so the page can ask first. */
export type PlayPlan =
  | { kind: 'join' }
  | { kind: 'install' }
  | { kind: 'switch'; inst: Instance; from: string; to: string };

export function planFor(l: Listing): PlayPlan {
  if (!l.pack) return { kind: 'join' };
  const match = packMatch(l);
  if (!match) return { kind: 'install' };
  // The same pack and version from the other site: nothing to change.
  if (match.cross) return { kind: 'join' };
  const have = match.inst;
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
      pendingJoins[inst.id] = { address: l.address, name: l.name, pageUrl: l.pageUrl };
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
    pendingJoins[inst.id] = { address: l.address, name: l.name, pageUrl: l.pageUrl };
    const { changePackVersion } = await import('./manualPack.svelte');
    // Null: CurseForge wants the file downloaded by hand; that dialog takes
    // over, and Peak still joins once the pack is ready.
    if (!(await changePackVersion(inst, l.pack.versionId))) return;
    await refreshInstances();
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
          pendingJoins[have.id] = { address: l.address, name: l.name, pageUrl: l.pageUrl };
          return toast(`${have.name} is still installing. Peak joins ${l.name} when it's ready.`, 'info');
        }
        return join(have, l.address, l.name, l.pageUrl);
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
    await join(inst, l.address, l.name, l.pageUrl);
  } catch (e) {
    toast(errorText(e), 'error');
  }
}
