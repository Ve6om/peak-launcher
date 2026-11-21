// Modpack updates: checked quietly in the background, shown as "Latest" or
// "Outdated" on modpack instances, and only asked about when the player goes
// to play that pack.
import { api } from './api';
import { app } from './state.svelte';
import type { Instance, PackUpdate } from './types';

export const packUpdates = $state<{
  list: PackUpdate[];
  checked: boolean;
  /** The "update before playing?" question that's open, if any. */
  prompt: { inst: Instance; update: PackUpdate; resolve: (play: boolean) => void } | null;
}>({ list: [], checked: false, prompt: null });

export async function checkPackUpdates() {
  try {
    packUpdates.list = await api.modpackUpdates();
  } catch {
    packUpdates.list = [];
  } finally {
    packUpdates.checked = true;
  }
}

export function updateFor(inst: Instance): PackUpdate | null {
  return packUpdates.list.find((u) => u.instanceId === inst.id) ?? null;
}

/** "latest", "outdated", or null when unknown (not checked yet, from a file, or checks off). */
export function versionStatus(inst: Instance): 'latest' | 'outdated' | null {
  if (!inst.pack?.projectId || !inst.pack.versionId) return null;
  if (!packUpdates.checked || app.settings?.modpackUpdateCheck === false) return null;
  return updateFor(inst) ? 'outdated' : 'latest';
}

/**
 * Before playing a modpack with a newer version, ask once (unless the player
 * skipped that version or muted the pack). Resolves true to go ahead and play.
 */
export function askBeforePlay(inst: Instance): Promise<boolean> {
  const update = updateFor(inst);
  if (!update || update.skipped || update.muted || app.settings?.modpackUpdateCheck === false) return Promise.resolve(true);
  return new Promise((resolve) => {
    packUpdates.prompt = { inst, update, resolve };
  });
}
