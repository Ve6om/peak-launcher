// Discord status: works out what the player is doing from Peak's own state
// and hands it to the backend, which only talks to Discord when the card
// actually changes. Turned off with the switch in Settings.
import { api } from './api';
import { app } from './state.svelte';
import { instanceIcon } from './instanceImages';
import { loaderLabel, packVersion, type Instance } from './types';
import { playingAt, inviteUrl } from './invite.svelte';

const SITE = 'https://www.peaklauncher.com';
const launchedAt = Math.floor(Date.now() / 1000);

/** The server each running instance joined, by instance id (set when joining from Peak). */
export const playingOn = $state<Record<string, string>>({});

/** When each instance started running, for Discord's elapsed timer. */
const startedAt: Record<string, number> = {};

export interface PresenceCard {
  details: string;
  state?: string | null;
  start?: number | null;
  image?: string | null;
  imageText?: string | null;
  buttonLabel?: string | null;
  buttonUrl?: string | null;
}

function about(inst: Instance): string {
  if (inst.pack) {
    const v = packVersion(inst.pack);
    return `${inst.pack.name}${v ? ` ${v}` : ''}`;
  }
  return `${loaderLabel(inst.loader)} ${inst.gameVersion}`;
}

/** What to show right now. */
export function currentCard(): PresenceCard {
  const getPeak = { buttonLabel: 'Get Peak Launcher', buttonUrl: SITE };
  const running = app.instances.filter((i) => app.running.includes(i.id));
  if (running.length) {
    const inst = running[0];
    startedAt[inst.id] ??= Math.floor(Date.now() / 1000);
    const server = playingOn[inst.id];
    const icon = instanceIcon(inst);
    // Friends can join from the status: listed servers link to their page
    // (it has Play in Peak); others only if the player chose to share them.
    const at = playingAt[inst.id];
    const join = at?.pageUrl
      ? { buttonLabel: 'Join on Peak', buttonUrl: at.pageUrl }
      : at && app.settings?.discordJoinAnyServer
        ? { buttonLabel: 'Join on Peak', buttonUrl: inviteUrl(inst, at) }
        : getPeak;
    return {
      details: `Playing ${inst.name}`,
      state: server ? `${about(inst)} · on ${server}` : about(inst),
      start: startedAt[inst.id],
      image: icon?.startsWith('https://') ? icon : null,
      imageText: inst.pack?.name ?? inst.name,
      ...join
    };
  }
  for (const id of Object.keys(startedAt)) if (!app.running.includes(id)) delete startedAt[id];

  const installing = app.instances.find((i) => app.progress[i.id] && app.progress[i.id].stage !== 'Done');
  if (installing) {
    const icon = instanceIcon(installing);
    return {
      details: `Installing ${installing.name}`,
      state: about(installing),
      start: launchedAt,
      image: icon?.startsWith('https://') ? icon : null,
      imageText: installing.pack?.name ?? installing.name,
      ...getPeak
    };
  }

  const where: Record<string, string> = {
    home: 'In the launcher',
    library: 'Looking through their instances',
    servers: 'Looking for a server to play on',
    accounts: 'In the launcher',
    settings: 'In the launcher'
  };
  let details = where[app.view] ?? 'In the launcher';
  if (app.view === 'browse') details = app.browse.kind === 'modpack' ? 'Browsing modpacks' : 'Browsing mods';
  if (app.view === 'instance') {
    const inst = app.instances.find((i) => i.id === app.selectedId);
    if (inst) details = `Getting ${inst.name} ready`;
  }
  return { details, start: launchedAt, ...getPeak };
}

let started = false;

/** Keep Discord up to date (once; call after the app has loaded). */
export function startPresence() {
  if (started) return;
  started = true;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let pending = '';
  let sent = '';
  $effect.root(() => {
    $effect(() => {
      const on = app.settings?.discordPresence !== false;
      const card = on ? currentCard() : null;
      const key = card ? JSON.stringify(card) : 'off';
      // Install progress and other updates re-run this often; only a new
      // card restarts the short pause (so clicking through pages doesn't
      // send every step).
      if (key === pending) return;
      pending = key;
      clearTimeout(timer);
      timer = setTimeout(() => {
        if (key === sent) return;
        const wasOn = sent !== '' && sent !== 'off';
        sent = key;
        if (!card) {
          if (wasOn) api.presenceClear().catch(() => {});
          return;
        }
        api.presenceShow(card).catch(() => {});
      }, 1500);
    });
  });
}
