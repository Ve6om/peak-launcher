// Streamer mode: hides what viewers shouldn't see on stream or in a
// recording, like account names, server addresses and the folder paths that
// contain the computer's user name.
import { api } from './api';
import { app, toast, errorText } from './state.svelte';

export const streaming = () => !!app.settings?.streamerMode;

/** An account name as it should be shown right now. */
export function shownName(name: string) {
  return streaming() ? 'Player' : name;
}

/** A server address as it should be shown right now. */
export function shownAddress(address: string) {
  return streaming() ? 'Address hidden' : address;
}

// The user folder in a path: C:\Users\name, /home/name, /Users/name.
const HOME = /([A-Za-z]:[\\/](?:Users|Documents and Settings)[\\/]|\/home\/|\/Users\/|\/var\/home\/)[^\\/\s"':]+/gi;

/** A folder path with the user folder swapped for "~" while streaming. */
export function shownPath(path: string) {
  return streaming() ? hidePaths(path) : path;
}

function hidePaths(text: string) {
  return text.replace(HOME, '~');
}

const IPV4 = /\b(?:\d{1,3}\.){3}\d{1,3}(?::\d{1,5})?\b/g;
const TOKEN = /(--(?:accessToken|uuid|xuid|clientId)\s+)\S+/gi;
const SESSION = /((?:token|session(?:id)?|accesstoken)[=:]\s*)[\w.\-]{12,}/gi;

/** A game log line with names, addresses, paths and tokens hidden while streaming. */
export function shownLogLine(line: string) {
  if (!streaming()) return line;
  let out = hidePaths(line).replace(TOKEN, '$1<hidden>').replace(SESSION, '$1<hidden>').replace(IPV4, '<address>');
  for (const a of app.accounts) {
    if (a.username.length < 3) continue;
    out = out.replace(new RegExp(`\\b${a.username.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}\\b`, 'gi'), 'Player');
  }
  return out;
}

/** Turn streamer mode on or off straight away (it doesn't wait for Save). */
export async function setStreamerMode(on: boolean) {
  if (!app.settings || streaming() === on) return;
  try {
    app.settings = await api.saveSettings({ ...$state.snapshot(app.settings), streamerMode: on });
    toast(on ? 'Streamer mode is on. Names, addresses and folder paths are hidden.' : 'Streamer mode is off', 'info');
  } catch (e) {
    toast(errorText(e), 'error');
  }
}
