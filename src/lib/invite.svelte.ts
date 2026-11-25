// Join with a friend: invite links to the server you're on (with its
// modpack), and opening one: Peak installs the pack or picks an instance and
// joins, after the player says so.
import { app } from './state.svelte';
import type { Instance, Listing } from './types';

const SITE = 'https://www.peaklauncher.com';

/** The server each running instance joined from Peak: for invites and Discord's Join button. */
export const playingAt = $state<Record<string, { address: string; name: string; pageUrl?: string | null }>>({});

/** A peak://join invite, as Peak's backend checked it. */
export interface JoinInvite {
  address: string;
  name: string;
  gameVersion: string | null;
  pack: string | null;
  packName: string | null;
  version: string | null;
  versionName: string | null;
}

/** The invite open in the "join a friend" dialog. */
export const invite = $state<{ current: JoinInvite | null }>({ current: null });

/** An invite link (on the website, which opens Peak) to `server` with this instance's modpack. */
export function inviteUrl(inst: Instance, server: { address: string; name: string }): string {
  const q = new URLSearchParams({ s: server.address, n: server.name, mc: inst.gameVersion });
  if (inst.pack?.projectId) {
    q.set('p', inst.pack.projectId);
    q.set('pn', inst.pack.name);
    if (inst.pack.versionId) q.set('v', inst.pack.versionId);
    if (inst.pack.versionNumber) q.set('vn', inst.pack.versionNumber);
  }
  return `${SITE}/join?${q}`;
}

/** An invite as a server listing, so it joins the same way the server list does. */
export function inviteListing(i: JoinInvite): Listing {
  return {
    id: `invite:${i.address}`,
    name: i.name,
    address: i.address,
    tagline: '',
    description: '',
    tags: [],
    region: '',
    language: '',
    gameVersion: i.gameVersion ?? app.instances[0]?.gameVersion ?? '',
    loader: 'vanilla',
    pack: i.pack ? { projectId: i.pack, name: i.packName ?? '', iconUrl: null, versionId: i.version, versionName: i.versionName } : null,
    votes: 0,
    featured: false,
    verified: false,
    website: null,
    discord: null
  };
}
